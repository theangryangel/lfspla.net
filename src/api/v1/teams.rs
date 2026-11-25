//! Public team directory, rosters and authenticated team management.

mod invites;
mod manage;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::team::{self, TeamError, TeamFilter, TeamOrder, member},
};

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_teams))
        .routes(routes!(get_team))
        .routes(routes!(manage::create_team))
        .routes(routes!(manage::update_team))
        .routes(routes!(manage::set_team_status))
        .routes(routes!(manage::remove_team_member))
        .routes(routes!(manage::set_team_member_role))
        .routes(routes!(manage::transfer_team_ownership))
        .routes(routes!(invites::list_team_invites))
        .routes(routes!(invites::create_team_invite))
        .routes(routes!(invites::revoke_team_invite))
        .routes(routes!(invites::preview_team_invite))
        .routes(routes!(invites::accept_team_invite))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TeamQuery {
    /// Team name or tag; matched literally without regard to case.
    q: Option<String>,
    discipline: Option<team::Tag>,
    accepts_applications: Option<bool>,
    /// Only teams the caller belongs to, as an owner or member. Requires authentication.
    #[serde(default)]
    #[param(default = false)]
    mine: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamSummary {
    id: i64,
    name: String,
    #[schema(required)]
    tag: Option<String>,
    #[schema(required)]
    description: Option<String>,
    #[schema(required)]
    website_url: Option<String>,
    tags: Vec<team::Tag>,
    accepts_applications: bool,
    status: team::Status,
    member_count: i64,
}

impl TeamSummary {
    fn new(team: team::Model, member_count: i64) -> Self {
        Self {
            id: team.id,
            name: team.name,
            tag: team.tag,
            description: team.description,
            website_url: team.website_url,
            tags: team.tags.0,
            accepts_applications: team.accepts_applications,
            status: team.status,
            member_count,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamMemberResponse {
    player: PlayerSummary,
    role: member::Role,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct TeamResponse {
    team: TeamSummary,
    members: Vec<TeamMemberResponse>,
}

#[utoipa::path(
    get, path = "/api/v1/teams", tag = "teams",
    security((), ("cookie_session" = []), ("personal_access_token" = [])),
    params(TeamQuery, PaginationQuery),
    responses(
        (status = 200, description = "Teams in name order, with filtered totals", body = PaginatedResponse<TeamSummary>),
        (status = 400, description = "Invalid filters or pagination", body = ErrorResponse),
        (status = 401, description = "Authentication required for mine=true", body = ErrorResponse)
    )
)]
pub(crate) async fn list_teams(
    Query(query): Query<TeamQuery>,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Json<PaginatedResponse<TeamSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let mut teams = team::Entity::find();
    if query.mine {
        let player = viewer
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "authentication_required",
                    "Sign in to view your teams.",
                )
            })?
            .0;
        teams = teams.with_member(player.id);
    }
    if let Some(q) = query.q.as_deref() {
        teams = teams.matching(q);
    }
    if let Some(discipline) = query.discipline {
        teams = teams.with_tag(discipline);
    }
    if let Some(accepts) = query.accepts_applications {
        teams = teams.accepting_applications(accepts);
    }
    let total_items = teams
        .clone()
        .count(&state.database)
        .await
        .map_err(ApiError::database)?;
    let teams = teams
        .in_name_order()
        .offset(offset)
        .limit(pagination.per_page)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let counts = team::Model::member_counts(&state.database, teams.iter().map(|team| team.id))
        .await
        .map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items: teams
            .into_iter()
            .map(|team| {
                let count = counts.get(&team.id).copied().unwrap_or(0);
                TeamSummary::new(team, count)
            })
            .collect(),
        pagination: pagination.metadata(total_items),
    }))
}

#[utoipa::path(
    get, path = "/api/v1/teams/{id}", tag = "teams",
    params(("id" = i64, Path, description = "Team identity")),
    responses(
        (status = 200, description = "Public team details and roster, owners first", body = TeamResponse),
        (status = 404, description = "Team not found", body = ErrorResponse)
    )
)]
pub(crate) async fn get_team(
    Path(id): Path<i64>,
    State(state): State<ApiState>,
) -> Result<Json<TeamResponse>, ApiError> {
    let team = team::Entity::find_by_id(id)
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or(TeamError::NotFound)?;
    let members: Vec<_> = member::Model::roster(&state.database, id)
        .await
        .map_err(ApiError::database)?
        .into_iter()
        .map(|(role, player)| TeamMemberResponse {
            player: player.into(),
            role,
        })
        .collect();
    let member_count = i64::try_from(members.len())
        .map_err(|error| ApiError::internal(&error, "team roster count"))?;
    Ok(Json(TeamResponse {
        team: TeamSummary::new(team, member_count),
        members,
    }))
}

impl From<TeamError> for ApiError {
    fn from(error: TeamError) -> Self {
        match error {
            TeamError::NotFound => ApiError::not_found("team_not_found", "Team"),
            TeamError::MemberNotFound => {
                ApiError::not_found("team_member_not_found", "Team member")
            }
            TeamError::InviteNotFound => ApiError::not_found("team_invite_not_found", "Invitation"),
            TeamError::InviteUnavailable => {
                ApiError::not_found("team_invite_unavailable", "Active invitation")
            }
            TeamError::OwnerRequired => ApiError::new(
                StatusCode::FORBIDDEN,
                "team_owner_required",
                "Only team owners can do this",
            ),
            TeamError::LastOwner => ApiError::new(
                StatusCode::CONFLICT,
                "team_conflict",
                "A team must keep at least one owner. Make another member an owner first",
            ),
            TeamError::TransferToSelf => ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_team",
                "Choose another team member",
            ),
            TeamError::Disbanded => ApiError::new(
                StatusCode::CONFLICT,
                "team_conflict",
                "Reactivate the team first",
            ),
            TeamError::Validation(errors) => errors.into(),
            TeamError::Database(error) => ApiError::database(error),
        }
    }
}

#[cfg(test)]
mod management_tests;
#[cfg(test)]
mod tests;
