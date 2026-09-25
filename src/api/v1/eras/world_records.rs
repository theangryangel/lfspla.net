//! Era-wide holders of current individual-chart world records.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
};
use sea_orm::{
    AccessMode, ColumnTrait, EntityTrait, IsolationLevel, QueryFilter, TransactionTrait,
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors as extract, v1::PlayerSummary,
    },
    models::{
        eras::rank_world_record_holders,
        players::{PlayerColumn, PlayerEntity},
    },
};

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct WorldRecordHolderResponse {
    /// Competition rank by record count; equal counts share a position.
    position: u64,
    player: PlayerSummary,
    /// Number of eligible charts on which this driver currently ranks first.
    world_records: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}/world-records",
    operation_id = "list_era_world_record_holders",
    tag = "ranking",
    params(("era" = String, Path, description = "Era slug"), PaginationQuery),
    responses(
        (status = 200, description = "Current world record holders, most records first", body = PaginatedResponse<WorldRecordHolderResponse>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 404, description = "Era not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    extract::Era(era): extract::Era,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<ApiState>,
) -> Result<Json<PaginatedResponse<WorldRecordHolderResponse>>, ApiError> {
    let offset = pagination.offset()?;
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    // Reuse the chart podium rules and the shared ranker so this table, the
    // Gold badge, and the world record leader badge cannot disagree about who
    // owns a record. Each chart contributes exactly one first place.
    let counts = era
        .list_podium_counts(&transaction)
        .await
        .map_err(ApiError::database)?;
    let standings = rank_world_record_holders(&counts);
    let total = standings.len() as u64;
    let page: Vec<_> = standings
        .into_iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(pagination.per_page).expect("validated per_page is at most 100"))
        .collect();
    let mut players: HashMap<_, _> = PlayerEntity::find()
        .filter(PlayerColumn::Id.is_in(page.iter().map(|holder| holder.player_id)))
        .all(&transaction)
        .await
        .map_err(ApiError::database)?
        .into_iter()
        .map(|player| (player.id, player))
        .collect();
    let items = page
        .into_iter()
        .map(|holder| {
            let player = players.remove(&holder.player_id).ok_or_else(|| {
                ApiError::database(sea_orm::DbErr::Type(format!(
                    "missing WR holder {}",
                    holder.player_id
                )))
            })?;
            Ok(WorldRecordHolderResponse {
                position: holder.position,
                player: player.into(),
                world_records: holder.world_records,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items,
        pagination: pagination.metadata(total),
    }))
}
