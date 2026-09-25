//! Reading one identified hotlap.

use axum::{
    Json,
    extract::{Path, State},
};
use sea_orm::EntityTrait;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer, v1::PlayerSummary},
    models::{
        eras::{EraEntity, EraModel},
        hotlaps::{self, DriverSide, HotlapFilter, HotlapModel, HotlapState, SteeringInput},
        players,
    },
};

/// Hotlap details. Owners can see all states and processing fields.
/// Other callers see only valid laps, with processing fields set to null.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct HotlapResponse {
    id: i64,
    era_id: String,
    player: PlayerSummary,
    track: String,
    /// Absent until the hotlap's vehicle has been resolved.
    #[schema(required)]
    vehicle: Option<String>,
    lap_time_ms: i64,
    split_1_ms: i64,
    split_2_ms: i64,
    split_3_ms: i64,
    split_4_ms: i64,
    steering: SteeringInput,
    brake_help_enabled: bool,
    automatic_gears: bool,
    #[schema(required)]
    manual_shifter: Option<bool>,
    axis_clutch: bool,
    automatic_clutch: bool,
    driver_side: DriverSide,
    #[schema(required)]
    abs_enabled: Option<bool>,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
    game_version: String,
    #[schema(required)]
    replay_url: Option<String>,
    /// Owner only: lifecycle state, null for everyone else.
    #[schema(required)]
    state: Option<HotlapState>,
    /// Owner only: vehicle name exactly as the replay recorded it.
    #[schema(required)]
    raw_vehicle_name: Option<String>,
    /// Owner only: raw HLVC exit code from the validation run.
    #[schema(required)]
    hlvc_result_code: Option<i16>,
    /// Owner only: why validation failed, when it did.
    #[schema(required)]
    error_detail: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/hotlaps/{hotlap}",
    operation_id = "get_hotlap",
    tag = "hotlaps",
    security(
        (),
        ("cookie_session" = []),
        ("personal_access_token" = [])
    ),
    params(("hotlap" = i64, Path, description = "Hotlap identity")),
    responses(
        (status = 200, description = "One hotlap, with processing fields when the caller owns it", body = HotlapResponse),
        (status = 401, description = "Supplied credentials are invalid", body = ErrorResponse),
        (status = 404, description = "Hotlap not found", body = ErrorResponse)
    )
)]
pub(crate) async fn detail(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Json<HotlapResponse>, ApiError> {
    let viewer = viewer.map(|AuthenticatedPlayer(viewer)| viewer);
    let viewer_id = viewer.map(|viewer| viewer.id);

    // The owner lookup runs first because it is the wider one: it matches the
    // caller's own hotlaps in any state, where the public lookup only ever
    // matches validated ones.
    // The lap and its owner arrive together: the hotlap entity declares the
    // relation, so this is one join rather than a second round trip.
    let viewers_hotlap = match viewer_id {
        Some(player_id) => hotlaps::HotlapEntity::find_by_id(hotlap_id)
            .uploads()
            .owned_by(player_id)
            .find_also_related(players::PlayerEntity)
            .find_also_related(EraEntity)
            .one(&state.database)
            .await
            .map_err(ApiError::database)?,
        None => None,
    };
    let owner = viewers_hotlap.is_some();
    let (hotlap, player, era) = match viewers_hotlap {
        Some(found) => found,
        None => hotlaps::HotlapEntity::find_by_id(hotlap_id)
            .valid()
            .find_also_related(players::PlayerEntity)
            .find_also_related(EraEntity)
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(hotlap_not_found)?,
    };
    // `Restrict` on the relation means a lap cannot outlive its player, so an
    // absent owner here is a broken row rather than an ordinary 404.
    let player = player.ok_or_else(|| {
        ApiError::database(sea_orm::DbErr::Type(format!(
            "hotlap {hotlap_id} has no player"
        )))
    })?;

    let era = era.ok_or_else(|| {
        ApiError::database(sea_orm::DbErr::Type(format!(
            "hotlap {hotlap_id} has no era"
        )))
    })?;

    Ok(Json(response(hotlap, player, &era, owner)))
}

fn hotlap_not_found() -> ApiError {
    ApiError::not_found("hotlap_not_found", "Hotlap")
}

fn response(
    hotlap: HotlapModel,
    player: players::PlayerModel,
    era: &EraModel,
    owner: bool,
) -> HotlapResponse {
    let hotlap_id = hotlap.id;
    let controls = hotlap.controls();
    HotlapResponse {
        id: hotlap_id,
        era_id: era.slug.clone(),
        player: player.into(),
        track: hotlap.track.to_string(),
        vehicle: hotlap.vehicle.map(|vehicle| vehicle.to_string()),
        lap_time_ms: hotlap.lap_time_ms,
        split_1_ms: hotlap.split_1_ms,
        split_2_ms: hotlap.split_2_ms,
        split_3_ms: hotlap.split_3_ms,
        split_4_ms: hotlap.split_4_ms,
        steering: controls.steering,
        brake_help_enabled: controls.brake_help_enabled,
        automatic_gears: controls.automatic_gears,
        manual_shifter: controls.manual_shifter,
        axis_clutch: controls.axis_clutch,
        automatic_clutch: controls.automatic_clutch,
        driver_side: controls.driver_side,
        abs_enabled: hotlap.abs_enabled,
        created_at: hotlap.created_at,
        game_version: hotlap.game_version.to_string(),
        replay_url: hotlap
            .spr_object_key
            .map(|_| super::replay::download_url(hotlap_id)),
        state: owner.then_some(hotlap.state),
        raw_vehicle_name: owner.then_some(hotlap.raw_vehicle_name),
        hlvc_result_code: owner.then_some(hotlap.hlvc_result_code).flatten(),
        error_detail: owner.then_some(hotlap.error_detail).flatten(),
    }
}
