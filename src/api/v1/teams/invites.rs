//! Invitation secrets are returned once. Team locks serialize redemption with revocation.

use super::manage::{conflict, invalid, lock_team, require_owner};
use super::*;
use crate::{
    api::{ListResponse, extractors::AuthenticatedPlayer},
    models::team_invite,
};
use axum::http::StatusCode;
use sea_orm::{ActiveValue::Set, DatabaseTransaction, IntoActiveModel, TransactionTrait};
use sha2::{Digest, Sha256};

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

impl From<team_invite::Model> for TeamInviteResponse {
    fn from(record: team_invite::Model) -> Self {
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

pub(super) async fn revoke_all(database: &DatabaseTransaction, id: i64) -> Result<(), ApiError> {
    team_invite::Entity::update_many()
        .col_expr(
            team_invite::Column::RevokedAt,
            Expr::value(time::OffsetDateTime::now_utc()),
        )
        .filter(team_invite::Column::TeamId.eq(id))
        .filter(team_invite::Column::RevokedAt.is_null())
        .exec(database)
        .await
        .map_err(ApiError::database)?;
    Ok(())
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
    require_owner(&state.database, id, player.id).await?;
    let records = team_invite::Entity::find()
        .filter(team_invite::Column::TeamId.eq(id))
        .order_by_desc(team_invite::Column::CreatedAt)
        .order_by_desc(team_invite::Column::Id)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(
        records
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>()
            .into(),
    ))
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
    if request
        .expires_in_days
        .is_some_and(|days| !(1..=365).contains(&days))
        || request
            .max_uses
            .is_some_and(|uses| !(1..=10_000).contains(&uses))
    {
        return Err(invalid(
            "Choose an expiry of 1 to 365 days and a usage limit of 1 to 10,000, or leave them unlimited",
        ));
    }
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    let team = lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    if team.status != team::Status::Active {
        return Err(conflict("Reactivate the team before inviting members"));
    }
    let token = hex::encode(rand::random::<[u8; 32]>());
    let now = time::OffsetDateTime::now_utc();
    let record = team_invite::ActiveModel {
        team_id: Set(id),
        created_by: Set(player.id),
        token_hash: Set(hash(&token)),
        expires_at: Set(request
            .expires_in_days
            .map(|days| now + time::Duration::days(days))),
        max_uses: Set(request.max_uses),
        ..Default::default()
    }
    .insert(&tx)
    .await
    .map_err(ApiError::database)?;
    tx.commit().await.map_err(ApiError::database)?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedTeamInviteResponse {
            token,
            invite: record.into(),
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
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    let result = team_invite::Entity::update_many()
        .col_expr(
            team_invite::Column::RevokedAt,
            Expr::value(time::OffsetDateTime::now_utc()),
        )
        .filter(team_invite::Column::Id.eq(invite_id))
        .filter(team_invite::Column::TeamId.eq(id))
        .exec(&tx)
        .await
        .map_err(ApiError::database)?;
    if result.rows_affected == 0 {
        return Err(ApiError::not_found("team_invite_not_found", "Invitation"));
    }
    tx.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn unavailable() -> ApiError {
    ApiError::not_found("team_invite_unavailable", "Active invitation")
}

async fn find_invite(
    database: &impl ConnectionTrait,
    token: &str,
) -> Result<team_invite::Model, ApiError> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(unavailable());
    }
    team_invite::Entity::find()
        .filter(team_invite::Column::TokenHash.eq(hash(token)))
        .one(database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(unavailable)
}

#[utoipa::path(get, path = "/api/v1/team-invites/{token}", tag = "teams", params(("token" = String, Path)),
    responses((status = 200, body = TeamInvitePreview, description = "Invitation preview; does not join or consume a use"),
        (status = 404, body = ErrorResponse, description = "Invitation expired, revoked, exhausted or disbanded")))]
pub(crate) async fn preview_team_invite(
    Path(token): Path<String>,
    State(state): State<ApiState>,
) -> Result<Json<TeamInvitePreview>, ApiError> {
    let invite = find_invite(&state.database, &token).await?;
    if !invite.usable_at(time::OffsetDateTime::now_utc()) {
        return Err(unavailable());
    }
    let team = team::Entity::find_by_id(invite.team_id)
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(unavailable)?;
    if team.status != team::Status::Active {
        return Err(unavailable());
    }
    let count = team_member::Entity::find()
        .filter(team_member::Column::TeamId.eq(team.id))
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let count =
        i64::try_from(count).map_err(|error| ApiError::internal(&error, "team roster count"))?;
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
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    let invite = find_invite(&tx, &token).await?;
    let team = lock_team(&tx, invite.team_id).await?;
    // Re-read after locking the team: revocation or another redemption may have won the lock.
    let invite = find_invite(&tx, &token).await?;
    if team.status != team::Status::Active || !invite.usable_at(time::OffsetDateTime::now_utc()) {
        return Err(unavailable());
    }
    let member = team_member::Entity::find_by_id((team.id, player.id))
        .one(&tx)
        .await
        .map_err(ApiError::database)?;
    if member.is_none() {
        team_member::ActiveModel {
            team_id: Set(team.id),
            player_id: Set(player.id),
            role: Set(team_member::Role::Member),
        }
        .insert(&tx)
        .await
        .map_err(ApiError::database)?;
        let mut active = invite.into_active_model();
        active.uses = Set(active.uses.unwrap() + 1);
        active.update(&tx).await.map_err(ApiError::database)?;
    }
    let count = team_member::Entity::find()
        .filter(team_member::Column::TeamId.eq(team.id))
        .count(&tx)
        .await
        .map_err(ApiError::database)?;
    let count =
        i64::try_from(count).map_err(|error| ApiError::internal(&error, "team roster count"))?;
    tx.commit().await.map_err(ApiError::database)?;
    Ok(Json(TeamSummary::new(team, count)))
}
