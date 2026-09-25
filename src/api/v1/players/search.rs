//! Public player search.

use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{EntityTrait, QueryOrder, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    api::{ApiError, ApiState, ListResponse, v1::PlayerSummary},
    models::players::{PlayerColumn, PlayerEntity, PlayerFilter},
};

const SEARCH_LIMIT: u64 = 25;

/// Search text for the public driver finder.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PlayerSearchQuery {
    /// Username or display name search text.
    q: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/players",
    tag = "players",
    params(PlayerSearchQuery),
    responses((status = 200, description = "Matching players", body = ListResponse<PlayerSummary>))
)]
pub(crate) async fn search(
    Query(query): Query<PlayerSearchQuery>,
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<PlayerSummary>>, ApiError> {
    let matches = PlayerEntity::find()
        .matching(&query.q)
        .order_by_asc(PlayerColumn::LfsUsername)
        .limit(SEARCH_LIMIT)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(ListResponse::from(
        matches
            .into_iter()
            .map(PlayerSummary::from)
            .collect::<Vec<_>>(),
    )))
}
