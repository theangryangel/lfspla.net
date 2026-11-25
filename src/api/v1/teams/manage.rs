//! Every mutation locks the team row before checking roles or changing membership.

use super::*;
use crate::api::extractors::AuthenticatedPlayer;
use axum::http::StatusCode;
use sea_orm::{ActiveValue::Set, DatabaseTransaction, IntoActiveModel, TransactionTrait};

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

impl TeamDetailsRequest {
    fn validate(mut self) -> Result<Self, ApiError> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty()
            || self.name.chars().count() > 100
            || self.name.chars().any(char::is_control)
        {
            return Err(invalid(
                "Enter a team name of 1 to 100 characters without control characters",
            ));
        }
        self.tag = clean(self.tag, 32, "The team tag must be at most 32 characters")?;
        if self
            .tag
            .as_ref()
            .is_some_and(|tag| tag.chars().any(char::is_control))
        {
            return Err(invalid("The team tag cannot contain control characters"));
        }
        self.description = clean(
            self.description,
            5000,
            "The description must be at most 5,000 characters",
        )?;
        self.website_url = clean(self.website_url, 2048, "The website URL is too long")?;
        if let Some(website) = &self.website_url {
            let input = if website.as_str().contains("://") {
                website.clone()
            } else {
                format!("https://{website}")
            };
            let url = url::Url::parse(&input)
                .map_err(|_| invalid("Enter a valid HTTP or HTTPS website"))?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(invalid(
                    "Enter an HTTP or HTTPS website without login details",
                ));
            }
            self.website_url = Some(url.to_string());
        }
        let mut tags: Vec<team::Tag> = Vec::new();
        for tag in self.tags {
            if !tags.as_slice().contains(&tag) {
                tags.push(tag);
            }
        }
        self.tags = tags;
        Ok(self)
    }

    fn apply(self, active: &mut team::ActiveModel) {
        active.name = Set(self.name);
        active.tag = Set(self.tag);
        active.description = Set(self.description);
        active.website_url = Set(self.website_url);
        active.tags = Set(team::Tags(self.tags));
        active.accepts_applications = Set(self.accepts_applications);
        active.updated_at = Set(time::OffsetDateTime::now_utc());
    }
}

fn clean(
    value: Option<String>,
    max: usize,
    message: &'static str,
) -> Result<Option<String>, ApiError> {
    let value = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if value
        .as_ref()
        .is_some_and(|value| value.chars().count() > max)
    {
        return Err(invalid(message));
    }
    Ok(value)
}

pub(super) fn invalid(message: &'static str) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_team", message)
}

pub(super) fn conflict(message: &'static str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, "team_conflict", message)
}

pub(super) async fn lock_team(
    database: &DatabaseTransaction,
    id: i64,
) -> Result<team::Model, ApiError> {
    team::Entity::find_by_id(id)
        .lock_exclusive()
        .one(database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_not_found", "Team"))
}

pub(super) async fn require_owner(
    database: &impl ConnectionTrait,
    id: i64,
    player_id: i64,
) -> Result<(), ApiError> {
    let membership = team_member::Entity::find_by_id((id, player_id))
        .one(database)
        .await
        .map_err(ApiError::database)?;
    if membership.is_none_or(|membership| membership.role != team_member::Role::Owner) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "team_owner_required",
            "Only team owners can do this",
        ));
    }
    Ok(())
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
    let request = request.validate()?;
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    let mut active: team::ActiveModel = Default::default();
    request.apply(&mut active);
    active.status = Set(team::Status::Active);
    let team = active.insert(&tx).await.map_err(ApiError::database)?;
    team_member::ActiveModel {
        team_id: Set(team.id),
        player_id: Set(player.id),
        role: Set(team_member::Role::Owner),
    }
    .insert(&tx)
    .await
    .map_err(ApiError::database)?;
    tx.commit().await.map_err(ApiError::database)?;
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
    let request = request.validate()?;
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    let team = lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    if team.status == team::Status::Disbanded && request.accepts_applications {
        return Err(conflict("Reactivate the team before opening applications"));
    }
    let mut active = team.into_active_model();
    request.apply(&mut active);
    active.update(&tx).await.map_err(ApiError::database)?;
    tx.commit().await.map_err(ApiError::database)?;
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
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    let team = lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    let mut active = team.into_active_model();
    active.status = Set(request.status);
    active.updated_at = Set(time::OffsetDateTime::now_utc());
    if request.status == team::Status::Disbanded {
        active.accepts_applications = Set(false);
        super::invites::revoke_all(&tx, id).await?;
    }
    active.update(&tx).await.map_err(ApiError::database)?;
    tx.commit().await.map_err(ApiError::database)?;
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
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    lock_team(&tx, id).await?;
    if player.id != target {
        require_owner(&tx, id, player.id).await?;
    }
    let member = team_member::Entity::find_by_id((id, target))
        .one(&tx)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_member_not_found", "Team member"))?;
    if member.role == team_member::Role::Owner {
        require_another_owner(&tx, id).await?;
    }
    member.delete(&tx).await.map_err(ApiError::database)?;
    tx.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_another_owner(database: &DatabaseTransaction, id: i64) -> Result<(), ApiError> {
    let owners = team_member::Entity::find()
        .filter(team_member::Column::TeamId.eq(id))
        .filter(team_member::Column::Role.eq(team_member::Role::Owner))
        .count(database)
        .await
        .map_err(ApiError::database)?;
    if owners <= 1 {
        return Err(conflict(
            "A team must keep at least one owner. Make another member an owner first",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TeamMemberRoleRequest {
    role: team_member::Role,
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
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    let member = team_member::Entity::find_by_id((id, target))
        .one(&tx)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_member_not_found", "Team member"))?;
    if member.role != request.role {
        if member.role == team_member::Role::Owner {
            require_another_owner(&tx, id).await?;
            super::invites::revoke_all(&tx, id).await?;
        }
        let mut active = member.into_active_model();
        active.role = Set(request.role);
        active.update(&tx).await.map_err(ApiError::database)?;
    }
    tx.commit().await.map_err(ApiError::database)?;
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
    if player.id == request.player_id {
        return Err(invalid("Choose another team member"));
    }
    let tx = state.database.begin().await.map_err(ApiError::database)?;
    lock_team(&tx, id).await?;
    require_owner(&tx, id, player.id).await?;
    let target = team_member::Entity::find_by_id((id, request.player_id))
        .one(&tx)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_member_not_found", "Team member"))?;
    let mut target = target.into_active_model();
    target.role = Set(team_member::Role::Owner);
    target.update(&tx).await.map_err(ApiError::database)?;
    team_member::Entity::update_many()
        .col_expr(
            team_member::Column::Role,
            Expr::value(team_member::Role::Member),
        )
        .filter(team_member::Column::TeamId.eq(id))
        .filter(team_member::Column::PlayerId.eq(player.id))
        .exec(&tx)
        .await
        .map_err(ApiError::database)?;
    super::invites::revoke_all(&tx, id).await?;
    tx.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}
