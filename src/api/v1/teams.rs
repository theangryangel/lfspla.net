//! Shared team invite links. Reading a link never changes membership.

use crate::{
    api::{
        ApiError, ApiState, ListResponse, PaginatedResponse, PaginationQuery,
        extractors::BrowserAuthenticatedPlayer,
    },
    models::{
        team,
        team_member::{self, Role},
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::{
    ActiveValue::Set,
    QueryOrder, QuerySelect, TransactionTrait,
    entity::prelude::*,
    sea_query::{Expr, OnConflict, extension::postgres::PgExpr},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

mod members;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamResponse {
    id: i64,
    name: String,
    description: Option<String>,
    is_active: bool,
    website_url: Option<String>,
    discord_url: Option<String>,
    invite_enabled: bool,
}

impl From<team::Model> for TeamResponse {
    fn from(team: team::Model) -> Self {
        Self {
            id: team.id,
            name: team.name,
            description: team.description,
            is_active: team.is_active,
            website_url: team.website_url,
            discord_url: team.discord_url,
            invite_enabled: team.invite_token_hash.is_some(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct MembershipResponse {
    role: Option<Role>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamDirectoryResponse {
    #[serde(flatten)]
    team: TeamResponse,
    members: Vec<members::MemberResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct InviteLinkResponse {
    /// Returned once. Regenerating invalidates the previous link.
    token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct CreateTeamRequest {
    name: String,
    description: Option<String>,
    website_url: Option<String>,
    discord_url: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct UpdateTeamRequest {
    name: String,
    description: Option<String>,
    website_url: Option<String>,
    discord_url: Option<String>,
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TeamListQuery {
    q: Option<String>,
    status: Option<TeamStatus>,
    #[serde(default)]
    include_archived: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TeamStatus {
    Active,
    Archived,
    All,
}

impl CreateTeamRequest {
    fn validate(mut self) -> Result<Self, ApiError> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty()
            || self.name.chars().count() > 100
            || self.name.chars().any(char::is_control)
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_team",
                "Use a team name of 1–100 characters without control characters",
            ));
        }
        self.description = self
            .description
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if self
            .description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 5000)
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_team",
                "Team descriptions must be at most 5,000 characters",
            ));
        }
        for field in [&mut self.website_url, &mut self.discord_url] {
            *field = field
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            if let Some(value) = field {
                let valid = value.len() <= 2048
                    && url::Url::parse(value).is_ok_and(|url| {
                        matches!(url.scheme(), "http" | "https")
                            && url.host_str().is_some()
                            && url.username().is_empty()
                            && url.password().is_none()
                    });
                if !valid {
                    return Err(ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_team",
                        "Team links must be HTTP or HTTPS URLs without embedded credentials, at most 2,048 characters",
                    ));
                }
            }
        }
        Ok(self)
    }
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(create))
        .routes(routes!(get))
        .routes(routes!(update))
        .routes(routes!(archive))
        .routes(routes!(restore))
        .routes(routes!(membership))
        .routes(routes!(members::list))
        .routes(routes!(members::update))
        .routes(routes!(members::remove))
        .routes(routes!(regenerate))
        .routes(routes!(disable))
        .routes(routes!(preview))
        .routes(routes!(join))
}

#[utoipa::path(get, path = "/api/v1/teams", operation_id = "list_teams", tag = "teams",
    params(TeamListQuery, PaginationQuery),
    responses((status = 200, body = PaginatedResponse<TeamDirectoryResponse>), (status = 400, description = "Invalid filter or pagination")))]
pub(crate) async fn list(
    State(state): State<ApiState>,
    Query(query): Query<TeamListQuery>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<TeamDirectoryResponse>>, ApiError> {
    let offset = pagination.offset()?;
    let mut teams = team::Entity::find();
    match query.status.unwrap_or(if query.include_archived {
        TeamStatus::All
    } else {
        TeamStatus::Active
    }) {
        TeamStatus::Active => teams = teams.filter(team::Column::IsActive.eq(true)),
        TeamStatus::Archived => teams = teams.filter(team::Column::IsActive.eq(false)),
        TeamStatus::All => {}
    }
    if let Some(q) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        if q.chars().count() > 100 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_team_search",
                "Team searches must be at most 100 characters",
            ));
        }
        teams = teams.filter(
            Expr::col(team::Column::Name).ilike(crate::db::escaped_like::EscapedLike::contains(q)),
        );
    }
    let total_items = teams
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let teams = teams
        .order_by_asc(team::Column::Name)
        .order_by_asc(team::Column::Id)
        .offset(offset)
        .limit(pagination.per_page)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let mut rosters =
        members::rosters(&state.database, teams.iter().map(|team| team.id).collect()).await?;
    Ok(Json(PaginatedResponse {
        items: teams
            .into_iter()
            .map(|team| TeamDirectoryResponse {
                members: rosters.remove(&team.id).unwrap_or_default(),
                team: team.into(),
            })
            .collect(),
        pagination: pagination.metadata(total_items),
    }))
}

#[utoipa::path(post, path = "/api/v1/teams", operation_id = "create_team", tag = "teams",
    security(("cookie_session" = [])), params(("X-CSRF-Token" = String, Header)), request_body = CreateTeamRequest,
    responses((status = 201, body = TeamResponse), (status = 400, description = "Invalid team details"), (status = 401, description = "Sign in required"), (status = 403, description = "CSRF required")))]
pub(crate) async fn create(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<CreateTeamRequest>,
) -> Result<(StatusCode, Json<TeamResponse>), ApiError> {
    let request = request.validate()?;
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let now = time::OffsetDateTime::now_utc();
    let team = team::ActiveModel {
        name: Set(request.name),
        description: Set(request.description),
        website_url: Set(request.website_url),
        discord_url: Set(request.discord_url),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&transaction)
    .await
    .map_err(ApiError::database)?;
    team_member::ActiveModel {
        team_id: Set(team.id),
        player_id: Set(player.id),
        role: Set(Role::Admin),
        joined_at: Set(now),
    }
    .insert(&transaction)
    .await
    .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok((StatusCode::CREATED, Json(team.into())))
}

async fn required(db: &impl ConnectionTrait, id: i64) -> Result<team::Model, ApiError> {
    team::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_not_found", "Team"))
}

fn require_active(team: &team::Model) -> Result<(), ApiError> {
    if team.is_active {
        return Ok(());
    }
    Err(ApiError::new(
        StatusCode::CONFLICT,
        "team_archived",
        "This team is disbanded. An admin must restore it before making changes",
    ))
}

async fn locked_team(db: &impl ConnectionTrait, id: i64) -> Result<team::Model, ApiError> {
    team::Entity::find_by_id(id)
        .lock_exclusive()
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_not_found", "Team"))
}

#[utoipa::path(patch, path = "/api/v1/teams/{team}", operation_id = "update_team", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("X-CSRF-Token" = String, Header)), request_body = UpdateTeamRequest,
    responses((status = 200, body = TeamResponse), (status = 400, description = "Invalid team details"), (status = 403, description = "Admin and CSRF required"), (status = 409, description = "Team is archived")))]
pub(crate) async fn update(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<UpdateTeamRequest>,
) -> Result<Json<TeamResponse>, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = locked_team(&transaction, id).await?;
    require_manager(&transaction, &team, player.id).await?;
    require_active(&team)?;
    let request_details = CreateTeamRequest {
        name: request.name,
        description: request.description,
        website_url: request.website_url,
        discord_url: request.discord_url,
    }
    .validate()?;
    let mut active: team::ActiveModel = team.into();
    active.name = Set(request_details.name);
    active.description = Set(request_details.description);
    active.website_url = Set(request_details.website_url);
    active.discord_url = Set(request_details.discord_url);
    active.updated_at = Set(time::OffsetDateTime::now_utc());
    let updated = active
        .update(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(updated.into()))
}

async fn set_active(
    state: ApiState,
    id: i64,
    player: i64,
    is_active: bool,
) -> Result<Json<TeamResponse>, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = locked_team(&transaction, id).await?;
    require_manager(&transaction, &team, player).await?;
    let mut active: team::ActiveModel = team.into();
    active.is_active = Set(is_active);
    if !is_active {
        active.invite_token_hash = Set(None);
    }
    active.updated_at = Set(time::OffsetDateTime::now_utc());
    let updated = active
        .update(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(updated.into()))
}

#[utoipa::path(post, path = "/api/v1/teams/{team}/archive", operation_id = "archive_team", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 200, body = TeamResponse), (status = 403, description = "Admin and CSRF required")))]
pub(crate) async fn archive(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<TeamResponse>, ApiError> {
    set_active(state, id, player.id, false).await
}

#[utoipa::path(post, path = "/api/v1/teams/{team}/restore", operation_id = "restore_team", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 200, body = TeamResponse), (status = 403, description = "Admin and CSRF required")))]
pub(crate) async fn restore(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<TeamResponse>, ApiError> {
    set_active(state, id, player.id, true).await
}

async fn role(db: &impl ConnectionTrait, team: i64, player: i64) -> Result<Option<Role>, ApiError> {
    Ok(team_member::Entity::find_by_id((team, player))
        .one(db)
        .await
        .map_err(ApiError::database)?
        .map(|member| member.role))
}

async fn require_manager(
    db: &impl ConnectionTrait,
    team: &team::Model,
    player: i64,
) -> Result<(), ApiError> {
    if role(db, team.id, player).await? == Some(Role::Admin) {
        return Ok(());
    }
    Err(ApiError::new(
        StatusCode::FORBIDDEN,
        "team_admin_required",
        "Only admins can manage the team",
    ))
}

fn token_hash(token: &str) -> Result<Vec<u8>, ApiError> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(invalid_link());
    }
    Ok(Sha256::digest(token.as_bytes()).to_vec())
}

fn invalid_link() -> ApiError {
    ApiError::not_found("team_invite_not_found", "Team invite link")
}

async fn invited_team(db: &impl ConnectionTrait, token: &str) -> Result<team::Model, ApiError> {
    team::Entity::find()
        .filter(team::Column::InviteTokenHash.eq(token_hash(token)?))
        .filter(team::Column::IsActive.eq(true))
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(invalid_link)
}

#[utoipa::path(get, path = "/api/v1/teams/{team}", operation_id = "get_team", tag = "teams",
    params(("team" = i64, Path)), responses((status = 200, body = TeamResponse), (status = 404, description = "Team not found")))]
pub(crate) async fn get(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
) -> Result<Json<TeamResponse>, ApiError> {
    Ok(Json(required(&state.database, id).await?.into()))
}

#[utoipa::path(get, path = "/api/v1/teams/{team}/membership", operation_id = "get_team_membership", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path)),
    responses((status = 200, body = MembershipResponse), (status = 401, description = "Sign in required")))]
pub(crate) async fn membership(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<MembershipResponse>, ApiError> {
    required(&state.database, id).await?;
    Ok(Json(MembershipResponse {
        role: role(&state.database, id, player.id).await?,
    }))
}

#[utoipa::path(post, path = "/api/v1/teams/{team}/invite-link", operation_id = "regenerate_team_invite_link", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 200, body = InviteLinkResponse), (status = 403, description = "Admin and CSRF required")))]
pub(crate) async fn regenerate(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<InviteLinkResponse>, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = team::Entity::find_by_id(id)
        .lock_exclusive()
        .one(&transaction)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_not_found", "Team"))?;
    require_manager(&transaction, &team, player.id).await?;
    require_active(&team)?;
    let token = hex::encode(rand::random::<[u8; 32]>());
    let mut active: team::ActiveModel = team.into();
    active.invite_token_hash = Set(Some(token_hash(&token)?));
    active.updated_at = Set(time::OffsetDateTime::now_utc());
    active
        .update(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(InviteLinkResponse { token }))
}

#[utoipa::path(delete, path = "/api/v1/teams/{team}/invite-link", operation_id = "disable_team_invite_link", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 204, description = "Invites disabled"), (status = 403, description = "Admin and CSRF required")))]
pub(crate) async fn disable(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = team::Entity::find_by_id(id)
        .lock_exclusive()
        .one(&transaction)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_not_found", "Team"))?;
    require_manager(&transaction, &team, player.id).await?;
    let mut active: team::ActiveModel = team.into();
    active.invite_token_hash = Set(None);
    active.updated_at = Set(time::OffsetDateTime::now_utc());
    active
        .update(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/team-invites/{token}", operation_id = "preview_team_invite", tag = "teams",
    params(("token" = String, Path)), responses((status = 200, body = TeamResponse), (status = 404, description = "Invalid or disabled link")))]
pub(crate) async fn preview(
    State(state): State<ApiState>,
    Path(token): Path<String>,
) -> Result<Json<TeamResponse>, ApiError> {
    Ok(Json(invited_team(&state.database, &token).await?.into()))
}

#[utoipa::path(post, path = "/api/v1/team-invites/{token}", operation_id = "join_team", tag = "teams",
    security(("cookie_session" = [])), params(("token" = String, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 204, description = "Joined, or already a member"), (status = 404, description = "Invalid or disabled link"), (status = 401, description = "Sign in required"), (status = 403, description = "CSRF required")))]
pub(crate) async fn join(
    State(state): State<ApiState>,
    Path(token): Path<String>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    // Lock the team before checking its current hash so disable/rotation and joining
    // are serialized. A link revoked while this request waits cannot admit a member.
    let candidate = invited_team(&transaction, &token).await?;
    let team = team::Entity::find_by_id(candidate.id)
        .lock_exclusive()
        .one(&transaction)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(invalid_link)?;
    if !team.is_active || team.invite_token_hash.as_deref() != Some(token_hash(&token)?.as_slice())
    {
        return Err(invalid_link());
    }
    team_member::Entity::insert(team_member::ActiveModel {
        team_id: Set(team.id),
        player_id: Set(player.id),
        role: Set(Role::Member),
        joined_at: Set(time::OffsetDateTime::now_utc()),
    })
    .on_conflict(
        OnConflict::columns([team_member::Column::TeamId, team_member::Column::PlayerId])
            .do_nothing()
            .to_owned(),
    )
    .try_insert()
    .exec(&transaction)
    .await
    .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
