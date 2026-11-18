//! Team rosters and membership changes, serialized through the team row lock.

use super::*;
use crate::api::v1::PlayerSummary;
use crate::models::player;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct MemberResponse {
    pub(super) player: PlayerSummary,
    pub(super) role: Role,
}

/// Resolve the visible teams' rosters with two queries, not one per row.
pub(super) async fn rosters(
    db: &impl ConnectionTrait,
    team_ids: Vec<i64>,
) -> Result<std::collections::HashMap<i64, Vec<MemberResponse>>, ApiError> {
    let memberships = team_member::Entity::find()
        .filter(team_member::Column::TeamId.is_in(team_ids))
        .all(db)
        .await
        .map_err(ApiError::database)?;
    let player_ids: Vec<_> = memberships.iter().map(|member| member.player_id).collect();
    let players: std::collections::HashMap<_, _> = player::Entity::find()
        .filter(player::Column::Id.is_in(player_ids))
        .all(db)
        .await
        .map_err(ApiError::database)?
        .into_iter()
        .map(|player| (player.id, player))
        .collect();
    let mut rosters: std::collections::HashMap<i64, Vec<MemberResponse>> =
        std::collections::HashMap::new();
    for member in memberships {
        if let Some(player) = players.get(&member.player_id) {
            rosters
                .entry(member.team_id)
                .or_default()
                .push(MemberResponse {
                    player: player.clone().into(),
                    role: member.role,
                });
        }
    }
    for members in rosters.values_mut() {
        members.sort_by_cached_key(|member| member.player.lfs_username.to_ascii_lowercase());
    }
    Ok(rosters)
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ChangeRoleRequest {
    role: Role,
}

/// Caller holds the team lock, so simultaneous departures/demotions cannot
/// both observe another admin and leave an active team unmanaged.
async fn require_another_admin(
    db: &impl ConnectionTrait,
    team: &team::Model,
    member: &team_member::Model,
) -> Result<(), ApiError> {
    if team.is_active && member.role == Role::Admin {
        let remaining = team_member::Entity::find()
            .filter(team_member::Column::TeamId.eq(team.id))
            .filter(team_member::Column::Role.eq(Role::Admin))
            .filter(team_member::Column::PlayerId.ne(member.player_id))
            .count(db)
            .await
            .map_err(ApiError::database)?;
        if remaining == 0 {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "last_team_admin",
                "Appoint another admin before leaving or demoting the last admin, or disband the team",
            ));
        }
    }
    Ok(())
}

async fn required_member(
    db: &impl ConnectionTrait,
    team: i64,
    player: i64,
) -> Result<team_member::Model, ApiError> {
    team_member::Entity::find_by_id((team, player))
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("team_member_not_found", "Team member"))
}

#[utoipa::path(get, path = "/api/v1/teams/{team}/members", operation_id = "list_team_members", tag = "teams",
    params(("team" = i64, Path)), responses((status = 200, body = ListResponse<MemberResponse>), (status = 404, description = "Team not found")))]
pub(crate) async fn list(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
) -> Result<Json<ListResponse<MemberResponse>>, ApiError> {
    required(&state.database, id).await?;
    Ok(Json(ListResponse::from(
        rosters(&state.database, vec![id])
            .await?
            .remove(&id)
            .unwrap_or_default(),
    )))
}

#[utoipa::path(patch, path = "/api/v1/teams/{team}/members/{player}", operation_id = "update_team_member_role", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("player" = i64, Path), ("X-CSRF-Token" = String, Header)), request_body = ChangeRoleRequest,
    responses((status = 204, description = "Membership role updated"), (status = 403, description = "Admin and CSRF required"), (status = 409, description = "Team archived or last admin cannot be demoted")))]
pub(crate) async fn update(
    State(state): State<ApiState>,
    Path((id, target)): Path<(i64, i64)>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<ChangeRoleRequest>,
) -> Result<StatusCode, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = locked_team(&transaction, id).await?;
    require_manager(&transaction, &team, player.id).await?;
    require_active(&team)?;
    let current = required_member(&transaction, id, target).await?;
    if request.role != Role::Admin {
        require_another_admin(&transaction, &team, &current).await?;
    }
    let mut member: team_member::ActiveModel = current.into();
    member.role = Set(request.role);
    member
        .update(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/v1/teams/{team}/members/{player}", operation_id = "remove_team_member", tag = "teams",
    security(("cookie_session" = [])), params(("team" = i64, Path), ("player" = i64, Path), ("X-CSRF-Token" = String, Header)),
    responses((status = 204, description = "Left team or removed member"), (status = 403, description = "Insufficient access"), (status = 404, description = "Member not found"), (status = 409, description = "Last active admin cannot leave")))]
pub(crate) async fn remove(
    State(state): State<ApiState>,
    Path((id, target)): Path<(i64, i64)>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    let transaction = state.database.begin().await.map_err(ApiError::database)?;
    let team = locked_team(&transaction, id).await?;
    if target != player.id {
        require_active(&team)?;
        require_manager(&transaction, &team, player.id).await?;
    }
    let member = required_member(&transaction, id, target).await?;
    require_another_admin(&transaction, &team, &member).await?;
    member
        .delete(&transaction)
        .await
        .map_err(ApiError::database)?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
