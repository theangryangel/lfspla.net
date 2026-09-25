//! Era capability endpoints.

mod charts;
mod combinations;
mod rankings;
mod tracks;
mod upload;
mod vehicles;
mod world_records;

use axum::{Json, extract::State};
use sea_orm::{EntityTrait, QueryOrder};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{ApiError, ApiState, ErrorResponse, ListResponse, extractors as extract},
    models::{eras, rankings as ranking_store, rankings::RankingFilter},
};

use rankings::RankingSummary;

/// An era and the rankings it offers.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct EraSummary {
    id: String,
    title: String,
    open: bool,
    version_requirement: String,
    rankings: Vec<RankingSummary>,
}

/// Builds era routes.
pub(super) fn router(max_spr_upload_bytes: usize) -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(detail))
        .merge(charts::router())
        .merge(combinations::router())
        .merge(rankings::router())
        .merge(tracks::router())
        .merge(upload::router(max_spr_upload_bytes))
        .merge(vehicles::router())
        .routes(routes!(world_records::list))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras",
    operation_id = "list_eras",
    tag = "eras",
    responses((status = 200, description = "Eras and the rankings they offer", body = ListResponse<EraSummary>))
)]
pub(crate) async fn list(
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<EraSummary>>, ApiError> {
    // One statement: each era arrives already paired with the rankings it
    // offers. sea-orm prepends the era key ordering so it can group the rows,
    // which is the same chronological order the era list has always used, and
    // the ranking position survives as the final sort key.
    let eras = eras::EraEntity::find()
        .find_with_related(ranking_store::RankingEntity)
        .order_by_asc(ranking_store::RankingColumn::Position)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(ListResponse::from(
        eras.into_iter()
            .map(|(era, rankings)| era_summary(era, rankings))
            .collect::<Vec<_>>(),
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/eras/{era}",
    operation_id = "get_era",
    tag = "eras",
    params(("era" = String, Path, description = "Era slug")),
    responses(
        (status = 200, description = "One era and the rankings it offers", body = EraSummary),
        (status = 404, description = "Era was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn detail(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
) -> Result<Json<EraSummary>, ApiError> {
    let rankings = ranking_store::RankingEntity::find()
        .in_era(era.id)
        .order_by_asc(ranking_store::RankingColumn::Position)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;

    Ok(Json(era_summary(era, rankings)))
}

/// Builds the era navigation payload shared by the list and detail endpoints.
fn era_summary(era: eras::EraModel, rankings: Vec<ranking_store::RankingRow>) -> EraSummary {
    EraSummary {
        id: era.slug,
        title: era.title,
        open: era.open,
        version_requirement: era.version_requirement.to_string(),
        rankings: rankings.into_iter().map(Into::into).collect(),
    }
}
