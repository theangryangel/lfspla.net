//! Invitation secrets are returned once. The workflows live in `models::team::invite`.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::TeamSummary;
use crate::api::{
    ApiError, ApiState, ErrorResponse, ListResponse, extractors::AuthenticatedPlayer,
};
use lfsplanet_core::models::team::invite::{self, CreateTeamInvite};

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamInviteResponse {
    id: i64,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    expires_at: Option<time::OffsetDateTime>,
    #[schema(required)]
    max_uses: Option<i64>,
    uses: i64,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    revoked_at: Option<time::OffsetDateTime>,
}

impl From<invite::Model> for TeamInviteResponse {
    fn from(record: invite::Model) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            expires_at: record.expires_at,
            max_uses: record.max_uses,
            uses: record.uses,
            revoked_at: record.revoked_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateTeamInviteRequest {
    /// Null or omitted means no expiry; otherwise 1 to 365 days.
    expires_in_days: Option<i64>,
    /// Null or omitted means unlimited; otherwise 1 to 10,000 joins.
    max_uses: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CreatedTeamInviteResponse {
    /// Invitation secret, shown once. Build the share URL with /teams/join/{token}.
    token: String,
    invite: TeamInviteResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamInvitePreview {
    team: TeamSummary,
}

#[utoipa::path(get, path = "/api/v1/teams/{id}/invites", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])), params(("id" = i64, Path)),
    responses((status = 200, body = ListResponse<TeamInviteResponse>, description = "Owner-visible invitation metadata, never secrets"),
        (status = 401, body = ErrorResponse, description = "Sign in required"), (status = 403, body = ErrorResponse, description = "Owner required")))]
pub(crate) async fn list_team_invites(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<Json<ListResponse<TeamInviteResponse>>, ApiError> {
    let records = invite::Model::list(&state.database, id, player.id).await?;
    Ok(Json(ListResponse::from(
        records.into_iter().map(Into::into).collect::<Vec<_>>(),
    )))
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/invites", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = CreateTeamInviteRequest,
    responses((status = 201, body = CreatedTeamInviteResponse, description = "Invitation created; secret shown once"),
        (status = 400, body = ErrorResponse, description = "Invalid limits"), (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Owner required"), (status = 404, body = ErrorResponse, description = "Team not found"),
        (status = 409, body = ErrorResponse, description = "Team is disbanded")))]
pub(crate) async fn create_team_invite(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<CreateTeamInviteRequest>,
) -> Result<(StatusCode, Json<CreatedTeamInviteResponse>), ApiError> {
    let issued = invite::Model::issue(
        &state.database,
        id,
        player.id,
        CreateTeamInvite {
            expires_in_days: request.expires_in_days,
            max_uses: request.max_uses,
        },
    )
    .await?;
    tracing::info!(
        player_id = player.id,
        team_id = id,
        invite_id = issued.record.id,
        "team invitation created"
    );
    Ok((
        StatusCode::CREATED,
        Json(CreatedTeamInviteResponse {
            token: issued.token,
            invite: issued.record.into(),
        }),
    ))
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/invites/{invite_id}", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("invite_id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    responses((status = 204, description = "Invitation revoked"), (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Owner required"), (status = 404, body = ErrorResponse, description = "Invitation not found")))]
pub(crate) async fn revoke_team_invite(
    Path((id, invite_id)): Path<(i64, i64)>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    invite::Model::revoke(&state.database, id, player.id, invite_id).await?;
    tracing::info!(
        player_id = player.id,
        team_id = id,
        invite_id,
        "team invitation revoked"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/team-invites/{token}", tag = "teams", params(("token" = String, Path)),
    responses((status = 200, body = TeamInvitePreview, description = "Invitation preview; does not join or consume a use"),
        (status = 404, body = ErrorResponse, description = "Invitation expired, revoked, exhausted or disbanded")))]
pub(crate) async fn preview_team_invite(
    Path(token): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<TeamInvitePreview>, ApiError> {
    let team = invite::Model::preview(&state.database, &token).await?;
    let count = team
        .member_count(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(TeamInvitePreview {
        team: TeamSummary::new(team, count),
    }))
}

#[utoipa::path(post, path = "/api/v1/team-invites/{token}/accept", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("token" = String, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    responses((status = 200, body = TeamSummary, description = "Joined as a member; existing membership is unchanged"),
        (status = 401, body = ErrorResponse, description = "Sign in required"), (status = 403, body = ErrorResponse, description = "Invalid CSRF token"),
        (status = 404, body = ErrorResponse, description = "Invitation unavailable")))]
pub(crate) async fn accept_team_invite(
    Path(token): Path<String>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<Json<TeamSummary>, ApiError> {
    let team = invite::Model::accept(&state.database, &token, player.id).await?;
    tracing::info!(
        player_id = player.id,
        team_id = team.id,
        "team invitation accepted"
    );
    let count = team
        .member_count(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(TeamSummary::new(team, count)))
}
