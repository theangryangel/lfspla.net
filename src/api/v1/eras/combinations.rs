//! The actual pairs offered by an era, including pairs with no uploaded laps.
use super::response::Chart;
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors as extract,
    },
    models::chart::Column as ChartColumn,
};
use axum::{
    Json,
    extract::{Query, State},
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{
    AccessMode, ColumnTrait, IsolationLevel, PaginatorTrait, QueryFilter, QuerySelect,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CombinationQuery {
    /// Exact canonical track code.
    track: Option<String>,
    /// Exact canonical vehicle code.
    vehicle: Option<String>,
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
        (status = 200, description = "Defined combinations, ordered by track and vehicle code", body = PaginatedResponse<Chart>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 404, description = "Era not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<CombinationQuery>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<Chart>>, ApiError> {
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
        select = select.filter(ChartColumn::TrackId.eq(track.to_ascii_uppercase()));
    }
    if let Some(vehicle) = query.vehicle {
        select = select.filter(ChartColumn::VehicleId.eq(vehicle.to_ascii_uppercase()));
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
    let items = Chart::load(&transaction, &era, &pairs).await?;
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

/// Whether an era offers a pair, and the resolved chart when it does.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CombinationCheck {
    valid: bool,
    /// The resolved chart; null whenever `valid` is false.
    #[schema(required)]
    chart: Option<Chart>,
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
    let pair = crate::models::Chart::find_combination(
        &state.database,
        era.id,
        &query.track.to_ascii_uppercase(),
        &query.vehicle.to_ascii_uppercase(),
    )
    .await
    .map_err(ApiError::database)?;
    let Some(pair) = pair else {
        // Parse only a miss to retain the public rejection reasons.
        let reason = match rankable_pair(&query.track, &query.vehicle) {
            Ok(_) => CombinationRejection::NotOffered,
            Err(reason) => reason,
        };
        return Ok(Json(rejected(reason)));
    };
    // The shared loader rejects missing catalogue metadata.
    let chart = Chart::load(&state.database, &era, &[pair])
        .await?
        .pop()
        .ok_or_else(|| {
            ApiError::database(sea_orm::DbErr::RecordNotFound(
                "hydrated chart missing".into(),
            ))
        })?;
    Ok(Json(CombinationCheck {
        valid: true,
        chart: Some(chart),
        reason: None,
    }))
}

/// The canonical pair two codes name, or why they name no rankable pair.
fn rankable_pair(track: &str, vehicle: &str) -> Result<(Track, Vehicle), CombinationRejection> {
    let track = match track.to_ascii_uppercase().parse::<Track>() {
        Ok(track) if !track.is_open() => track,
        Ok(_) => return Err(CombinationRejection::OpenConfiguration),
        Err(_) => return Err(CombinationRejection::UnknownTrack),
    };
    // Vehicle parsing is infallible - anything unrecognised becomes `Unknown` -
    // reject that sentinel before looking up the combination.
    let vehicle = vehicle
        .to_ascii_uppercase()
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible");
    if vehicle == Vehicle::Unknown {
        return Err(CombinationRejection::UnknownVehicle);
    }
    Ok((track, vehicle))
}

fn rejected(reason: CombinationRejection) -> CombinationCheck {
    CombinationCheck {
        valid: false,
        chart: None,
        reason: Some(reason),
    }
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
    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn chart_lookup_keeps_combination_rejection_reasons(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!(
            "../../../services/validate_hotlap/fixtures.sql"
        ))
        .execute(&pool)
        .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let era = crate::models::Era::find_by_slug(&database, &"2026-09-23".parse().unwrap())
            .await?
            .unwrap();
        let state = ApiState {
            database,
            oauth: None,
            object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
        };
        for (track, vehicle, reason) in [
            ("bl1", "xfg", None),
            ("NOPE", "XFG", Some(CombinationRejection::UnknownTrack)),
            ("BL3X", "XFG", Some(CombinationRejection::OpenConfiguration)),
            ("BL1", "NOPE", Some(CombinationRejection::UnknownVehicle)),
            ("BL1", "XRG", Some(CombinationRejection::NotOffered)),
        ] {
            let Json(check) = validate(
                extract::Era(era.clone()),
                State(state.clone()),
                Query(CombinationCheckQuery {
                    track: track.into(),
                    vehicle: vehicle.into(),
                }),
            )
            .await?;
            assert_eq!(check.valid, reason.is_none());
            assert_eq!(check.reason, reason);
            assert_eq!(check.chart.is_some(), reason.is_none());
        }
        Ok(())
    }
}
