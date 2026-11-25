//! Owner and member actions on a team. The workflows live in `models::team`.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use utoipa::ToSchema;

use super::TeamSummary;
use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::team::{self, TeamDetails, member},
};

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TeamDetailsRequest {
    name: String,
    tag: Option<String>,
    description: Option<String>,
    website_url: Option<String>,
    tags: Vec<team::Tag>,
    accepts_applications: bool,
}

impl From<TeamDetailsRequest> for TeamDetails {
    fn from(request: TeamDetailsRequest) -> Self {
        Self {
            name: request.name,
            tag: request.tag,
            description: request.description,
            website_url: request.website_url,
            tags: request.tags,
            accepts_applications: request.accepts_applications,
        }
    }
}

#[utoipa::path(post, path = "/api/v1/teams", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = TeamDetailsRequest,
    responses((status = 201, body = TeamSummary, description = "Team created with its creator as owner"),
        (status = 400, body = ErrorResponse, description = "Invalid team details"),
        (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Invalid CSRF token")))]
pub(crate) async fn create_team(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<TeamDetailsRequest>,
) -> Result<(StatusCode, Json<TeamSummary>), ApiError> {
    let team = team::Model::create(&state.database, player.id, request.into()).await?;
    tracing::info!(player_id = player.id, team_id = team.id, "team created");
    Ok((StatusCode::CREATED, Json(TeamSummary::new(team, 1))))
}

#[utoipa::path(put, path = "/api/v1/teams/{id}", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = TeamDetailsRequest,
    responses((status = 204, description = "Team updated"), (status = 400, body = ErrorResponse, description = "Invalid details"),
        (status = 401, body = ErrorResponse, description = "Sign in required"), (status = 403, body = ErrorResponse, description = "Owner required"),
        (status = 404, body = ErrorResponse, description = "Team not found"), (status = 409, body = ErrorResponse, description = "Disbanded team cannot accept applications")))]
pub(crate) async fn update_team(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<TeamDetailsRequest>,
) -> Result<StatusCode, ApiError> {
    team::Model::update(&state.database, id, player.id, request.into()).await?;
    tracing::info!(player_id = player.id, team_id = id, "team updated");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TeamStatusRequest {
    status: team::Status,
}

#[utoipa::path(put, path = "/api/v1/teams/{id}/status", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = TeamStatusRequest,
    responses((status = 204, description = "Team status updated"), (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Owner required"), (status = 404, body = ErrorResponse, description = "Team not found")))]
pub(crate) async fn set_team_status(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<TeamStatusRequest>,
) -> Result<StatusCode, ApiError> {
    team::Model::set_status(&state.database, id, player.id, request.status).await?;
    tracing::info!(player_id = player.id, team_id = id, status = ?request.status, "team status changed");
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/teams/{id}/members/{player_id}", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("player_id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    responses((status = 204, description = "Member removed or left"), (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Only owners can remove other members"),
        (status = 404, body = ErrorResponse, description = "Membership not found"), (status = 409, body = ErrorResponse, description = "Last owner cannot leave")))]
pub(crate) async fn remove_team_member(
    Path((id, target)): Path<(i64, i64)>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    member::Model::remove(&state.database, id, player.id, target).await?;
    tracing::info!(
        player_id = player.id,
        team_id = id,
        member_id = target,
        "team member removed"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TeamMemberRoleRequest {
    role: member::Role,
}

#[utoipa::path(put, path = "/api/v1/teams/{id}/members/{player_id}/role", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("player_id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = TeamMemberRoleRequest,
    responses((status = 204, description = "Member role updated; teams may have multiple owners"),
        (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Owner required"),
        (status = 404, body = ErrorResponse, description = "Team or membership not found"),
        (status = 409, body = ErrorResponse, description = "Last owner cannot be demoted")))]
pub(crate) async fn set_team_member_role(
    Path((id, target)): Path<(i64, i64)>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<TeamMemberRoleRequest>,
) -> Result<StatusCode, ApiError> {
    member::Model::set_role(&state.database, id, player.id, target, request.role).await?;
    tracing::info!(player_id = player.id, team_id = id, member_id = target, role = ?request.role, "team member role changed");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransferTeamOwnershipRequest {
    player_id: i64,
}

#[utoipa::path(post, path = "/api/v1/teams/{id}/transfer-ownership", tag = "teams",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("id" = i64, Path), ("X-CSRF-Token" = Option<String>, Header, description = "Required for browser sessions")),
    request_body = TransferTeamOwnershipRequest,
    responses((status = 204, description = "Ownership transferred; sender becomes a member and invitations are revoked"),
        (status = 400, body = ErrorResponse, description = "Choose another member"), (status = 401, body = ErrorResponse, description = "Sign in required"),
        (status = 403, body = ErrorResponse, description = "Owner required"), (status = 404, body = ErrorResponse, description = "Member not found")))]
pub(crate) async fn transfer_team_ownership(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Json(request): Json<TransferTeamOwnershipRequest>,
) -> Result<StatusCode, ApiError> {
    team::Model::transfer_ownership(&state.database, id, player.id, request.player_id).await?;
    tracing::info!(
        player_id = player.id,
        team_id = id,
        new_owner_id = request.player_id,
        "team ownership transferred"
    );
    Ok(StatusCode::NO_CONTENT)
}
