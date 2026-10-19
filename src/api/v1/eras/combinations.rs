//! The actual pairs offered by an era, including pairs with no uploaded laps.
use super::response::Chart;
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, Ordering, PaginatedResponse, PaginationQuery,
        extractors as extract,
    },
    models::chart::{Column as ChartColumn, Entity as ChartEntity, Model as ChartModel},
};
use axum::{
    Json,
    extract::{Query, State},
};
use insim_core::{track::Track, vehicle::Vehicle};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, IsolationLevel,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, TransactionTrait,
    sea_query::Expr,
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
    /// Defaults to track. Participants counts drivers with a current valid personal best.
    #[serde(default)]
    #[param(inline)]
    column: CombinationColumn,
    /// Defaults to asc. Ties use track and vehicle codes.
    #[serde(default)]
    #[param(inline)]
    order: Ordering,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CombinationColumn {
    #[default]
    Track,
    Vehicle,
    Participants,
}

/// An offered chart and the number of drivers currently ranked on it.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Combination {
    #[serde(flatten)]
    chart: Chart,
    /// One current valid personal best per driver; repeated uploads do not increase this count.
    participants: i64,
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
        (status = 200, description = "Defined combinations with driver counts, sorted and paginated on the server", body = PaginatedResponse<Combination>),
        (status = 400, description = "Invalid pagination", body = ErrorResponse),
        (status = 404, description = "Era not found", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    Query(query): Query<CombinationQuery>,
    Query(pagination): Query<PaginationQuery>,
) -> Result<Json<PaginatedResponse<Combination>>, ApiError> {
    let offset = pagination.offset()?;
    let transaction = state
        .database
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(ApiError::database)?;
    let mut select = ChartEntity::find().filter(ChartColumn::EraId.eq(era.id));
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
    // The existing (chart_id, player_id) primary key makes each count an index-only
    // scan over personal bests, never replay history. PostgreSQL can defer these
    // counts until after LIMIT for catalogue sorts; popularity sorts count before
    // LIMIT so pagination remains correct.
    let order = match query.order {
        Ordering::Asc => sea_orm::Order::Asc,
        Ordering::Desc => sea_orm::Order::Desc,
    };
    let mut select = select.expr_as(
        Expr::cust("(SELECT COUNT(*) FROM hotlap_personal_best pb WHERE pb.chart_id = chart.id)"),
        "participants",
    );
    select = match query.column {
        CombinationColumn::Track => select.order_by(ChartColumn::TrackId, order),
        CombinationColumn::Vehicle => select.order_by(ChartColumn::VehicleId, order),
        CombinationColumn::Participants => select.order_by(Expr::cust("participants"), order),
    };
    let statement = select
        .order_by_asc(ChartColumn::TrackId)
        .order_by_asc(ChartColumn::VehicleId)
        .offset(offset)
        .limit(pagination.per_page)
        .build(sea_orm::DbBackend::Postgres);
    let rows = transaction
        .query_all_raw(statement)
        .await
        .map_err(ApiError::database)?;
    let pairs = rows
        .iter()
        .map(|row| ChartModel::from_query_result(row, ""))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::database)?;
    let charts = Chart::load(&transaction, &era, &pairs).await?;
    let items = charts
        .into_iter()
        .zip(&rows)
        .map(|(chart, row)| {
            Ok(Combination {
                chart,
                participants: row.try_get("", "participants")?,
            })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()
        .map_err(ApiError::database)?;
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

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn popularity_counts_drivers_before_pagination(pool: sqlx::PgPool) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!(
            "../../../services/validate_hotlap/fixtures.sql"
        ))
        .execute(&pool)
        .await?;
        sqlx::raw_sql(
            r"
INSERT INTO player (lfs_username, display_name) VALUES ('second-driver', 'Second Driver');
INSERT INTO vehicle (id, kind, name, normalized_name, license) VALUES
    ('XRG', 'standard', 'XR GT', 'xr gt', 'demo'),
    ('XRT', 'standard', 'XR GT Turbo', 'xr gt turbo', 's1'),
    ('FXO', 'standard', 'FXO Turbo', 'fxo turbo', 's1');
INSERT INTO era (slug, title, version_requirement, installation_id)
VALUES ('other-era', 'Other era', '<0.7A', '0.7');
INSERT INTO chart (era_id, track_id, vehicle_id)
SELECT id, 'BL1', vehicle FROM era
CROSS JOIN (VALUES ('XFG'), ('XRG'), ('XRT'), ('FXO')) AS vehicles(vehicle)
ON CONFLICT DO NOTHING;
INSERT INTO hotlap (
    player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
    split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, abs_enabled, player_flags, game_version, state
)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'Test car', 60000,
    0, 0, 0, 0, 'demo', 'demo:' || chart.era_id || ':' || laps.fingerprint, 'wheel', false, 0, '0.7A', 'valid'
FROM (VALUES
    ('worker-test', 'XFG', 'one'),
    ('worker-test', 'XRG', 'two'),
    ('worker-test', 'XRG', 'repeat'),
    ('second-driver', 'XRG', 'three')
) AS laps(username, vehicle, fingerprint)
JOIN player ON player.lfs_username = laps.username
JOIN chart ON chart.vehicle_id = laps.vehicle;
-- A second upload on XFG is invalid and must not count as another participant.
INSERT INTO hotlap (
    player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
    split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    spr_attachment_id, source, fingerprint, steering, abs_enabled, player_flags, game_version, state
)
SELECT player.id, hotlap.era_id, chart_id, track, vehicle, raw_vehicle_name, lap_time_ms,
    0, 0, 0, 0, spr_attachment_id, 'upload', repeat('b', 64), 'wheel', false, 0, '0.7A', 'invalid'
FROM hotlap CROSS JOIN player
WHERE hotlap.state = 'pending' AND player.lfs_username = 'second-driver';
",
        )
        .execute(&pool)
        .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let eras = crate::models::era::Entity::find().all(&database).await?;
        for era in &eras {
            era.rebuild_personal_bests(&database).await?;
        }
        let era = eras
            .into_iter()
            .find(|era| era.slug.to_string() == "2026-09-23")
            .unwrap();
        let state = ApiState {
            database,
            oauth: None,
            object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
            hotlaps: Default::default(),
            site_notice: None,
        };
        let load = |search: &str| {
            list(
                extract::Era(era.clone()),
                State(state.clone()),
                Query(serde_urlencoded::from_str::<CombinationQuery>(search).unwrap()),
                Query(serde_urlencoded::from_str::<PaginationQuery>(search).unwrap()),
            )
        };
        for (page, vehicle, participants) in
            [(1, "XRG", 2), (2, "XFG", 1), (3, "FXO", 0), (4, "XRT", 0)]
        {
            let Json(result) = load(&format!(
                "column=participants&order=desc&per_page=1&page={page}"
            ))
            .await?;
            assert_eq!(result.pagination.total_items, 4);
            assert_eq!(result.items.len(), 1);
            assert_eq!(
                serde_json::to_value(&result.items[0])?["vehicle"]["code"],
                vehicle
            );
            assert_eq!(result.items[0].participants, participants);
        }
        let Json(defaults) = load("").await?;
        assert_eq!(
            serde_json::to_value(&defaults.items[0])?["vehicle"]["code"],
            "FXO"
        );
        let Json(filtered) = load("vehicle=xrg&column=participants&order=asc").await?;
        assert_eq!(filtered.pagination.total_items, 1);
        assert_eq!(filtered.items[0].participants, 2);
        assert!(serde_urlencoded::from_str::<CombinationQuery>("column=invalid").is_err());
        assert!(serde_urlencoded::from_str::<CombinationQuery>("order=invalid").is_err());
        Ok(())
    }

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
            site_notice: None,
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
