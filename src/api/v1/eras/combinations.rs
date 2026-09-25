//! The actual pairs offered by an era, including pairs with no uploaded laps.
use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{
    AccessMode, ColumnTrait, EntityTrait, IsolationLevel, PaginatorTrait, QueryFilter, QuerySelect,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use super::{tracks::TrackSummary, vehicles::VehicleSummary};
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors as extract,
    },
    models::{
        hotlaps::HotlapRankable,
        rankings::{RankingChartColumn, RankingChartModel},
        tracks::{TrackColumn, TrackEntity},
        vehicles::{VehicleColumn, VehicleEntity},
    },
};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CombinationQuery {
    /// Exact canonical track code.
    track: Option<String>,
    /// Exact canonical vehicle code.
    vehicle: Option<String>,
}

/// A track and vehicle pair offered by an era, identified by their codes.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CombinationSummary {
    track: TrackSummary,
    vehicle: VehicleSummary,
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(validate))
}

#[utoipa::path(
    get, path = "/api/v1/eras/{era}/combinations",
    operation_id = "list_era_combinations", tag = "eras",
    params(("era" = String, Path, description = "Era slug"), CombinationQuery, PaginationQuery),
    responses(
        (status = 200, description = "Defined combinations, ordered by track and vehicle code", body = PaginatedResponse<CombinationSummary>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 404, description = "Era not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<CombinationQuery>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<CombinationSummary>>, ApiError> {
    let offset = pagination.offset()?;
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    let mut select = era.combinations();
    if let Some(track) = query.track {
        select = select.filter(RankingChartColumn::TrackId.eq(track.to_ascii_uppercase()));
    }
    if let Some(vehicle) = query.vehicle {
        select = select.filter(RankingChartColumn::VehicleId.eq(vehicle.to_ascii_uppercase()));
    }
    let total = select
        .clone()
        .count(&transaction)
        .await
        .map_err(ApiError::database)?;
    let pairs = select
        .offset(offset)
        .limit(pagination.per_page)
        .all(&transaction)
        .await
        .map_err(ApiError::database)?;
    let items = hydrate(&transaction, pairs).await?;
    transaction.commit().await.map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items,
        pagination: pagination.metadata(total),
    }))
}

/// The track and vehicle a caller wants checked against one era.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CombinationCheckQuery {
    /// Canonical track configuration code, e.g. `BL1`.
    track: String,
    /// Canonical vehicle code, e.g. `XFG`.
    vehicle: String,
}

/// Why an era does not offer a pair.
#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CombinationRejection {
    /// The code names no canonical LFS track configuration.
    UnknownTrack,
    /// The configuration is an open one, which no hotlap can be ranked on.
    OpenConfiguration,
    /// The code names no canonical LFS vehicle.
    UnknownVehicle,
    /// Both codes are canonical, but no ranking in the era selects the pair.
    NotOffered,
}

/// Whether an era offers a pair, and the pair itself when it does.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CombinationCheck {
    valid: bool,
    /// The offered combination; absent whenever `valid` is false.
    #[schema(required)]
    combination: Option<CombinationSummary>,
    /// Why the pair was rejected; absent whenever `valid` is true.
    #[schema(required)]
    reason: Option<CombinationRejection>,
}

#[utoipa::path(
    get, path = "/api/v1/eras/{era}/combinations/validate",
    operation_id = "validate_era_combination", tag = "eras",
    params(("era" = String, Path, description = "Era slug"), CombinationCheckQuery),
    responses(
        (status = 200, description = "Whether the era offers the pair", body = CombinationCheck),
        (status = 400, description = "A code is missing", body = ErrorResponse),
        (status = 404, description = "Era not found", body = ErrorResponse)
    )
)]
pub(crate) async fn validate(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<CombinationCheckQuery>,
) -> Result<Json<CombinationCheck>, ApiError> {
    // Invalid codes return a rejection reason so clients can check form input.
    // Missing parameters return 400.
    let (track, vehicle) = match rankable_pair(&query.track, &query.vehicle) {
        Ok(pair) => pair,
        Err(reason) => return Ok(Json(rejected(reason))),
    };

    let pair = era
        .combinations()
        .filter(RankingChartColumn::TrackId.eq(track.to_string()))
        .filter(RankingChartColumn::VehicleId.eq(vehicle.to_string()))
        .one(&state.database)
        .await
        .map_err(ApiError::database)?;
    let Some(pair) = pair else {
        return Ok(Json(rejected(CombinationRejection::NotOffered)));
    };
    // `hydrate` returns an error if track or vehicle metadata is missing.
    let combination = hydrate(&state.database, vec![pair])
        .await?
        .pop()
        .ok_or_else(|| {
            ApiError::database(sea_orm::DbErr::RecordNotFound(
                "hydrated combination missing".into(),
            ))
        })?;
    Ok(Json(CombinationCheck {
        valid: true,
        combination: Some(combination),
        reason: None,
    }))
}

/// The canonical pair two codes name, or why they name no rankable pair.
fn rankable_pair(track: &str, vehicle: &str) -> Result<(Track, Vehicle), CombinationRejection> {
    let track = match track.to_ascii_uppercase().parse::<Track>() {
        Ok(track) if track.is_hotlap_rankable() => track,
        Ok(_) => return Err(CombinationRejection::OpenConfiguration),
        Err(_) => return Err(CombinationRejection::UnknownTrack),
    };
    // Vehicle parsing is infallible - anything unrecognised becomes `Unknown` -
    // so rankability is what separates a real code from a typo here.
    let vehicle = vehicle
        .to_ascii_uppercase()
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible")
        .ensure_hotlap_rankable()
        .map_err(|_| CombinationRejection::UnknownVehicle)?;
    Ok((track, vehicle))
}

fn rejected(reason: CombinationRejection) -> CombinationCheck {
    CombinationCheck {
        valid: false,
        combination: None,
        reason: Some(reason),
    }
}

/// Loads track and vehicle metadata in two queries for the whole page.
async fn hydrate(
    database: &impl sea_orm::ConnectionTrait,
    pairs: Vec<RankingChartModel>,
) -> Result<Vec<CombinationSummary>, ApiError> {
    let (tracks, vehicles) = tokio::try_join!(
        TrackEntity::find()
            .filter(TrackColumn::Id.is_in(pairs.iter().map(|p| p.track_id.clone())))
            .all(database),
        VehicleEntity::find()
            .filter(VehicleColumn::Id.is_in(pairs.iter().map(|p| p.vehicle_id.clone())))
            .all(database),
    )
    .map_err(ApiError::database)?;
    let tracks: HashMap<_, _> = tracks.into_iter().map(|t| (t.id.clone(), t)).collect();
    let vehicles: HashMap<_, _> = vehicles.into_iter().map(|v| (v.id.clone(), v)).collect();
    pairs
        .into_iter()
        .map(|p| {
            let track = tracks.get(&p.track_id).ok_or_else(|| {
                ApiError::database(sea_orm::DbErr::RecordNotFound(
                    "combination track missing".into(),
                ))
            })?;
            let vehicle = vehicles.get(&p.vehicle_id).ok_or_else(|| {
                ApiError::database(sea_orm::DbErr::RecordNotFound(
                    "combination vehicle missing".into(),
                ))
            })?;
            Ok(CombinationSummary {
                track: track.clone().into(),
                vehicle: vehicle.clone().into(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rankable_pair_accepts_either_case() {
        let (track, vehicle) = rankable_pair("bl1", "xfg").expect("BL1/XFG is rankable");
        assert_eq!(track.to_string(), "BL1");
        assert_eq!(vehicle.to_string(), "XFG");
    }

    #[test]
    fn unrankable_codes_are_named_rather_than_rejected_as_bad_requests() {
        assert_eq!(
            rankable_pair("NOPE", "XFG").unwrap_err(),
            CombinationRejection::UnknownTrack
        );
        // An open configuration has no lap to time, so no era can offer it.
        assert_eq!(
            rankable_pair("BL3X", "XFG").unwrap_err(),
            CombinationRejection::OpenConfiguration
        );
        assert_eq!(
            rankable_pair("BL1", "NOPE").unwrap_err(),
            CombinationRejection::UnknownVehicle
        );
    }
}
