//! Immediate hotlap validation for test environments.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::EntityTrait;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::{
        eras::{EraEntity, rebuild_published_badges},
        hotlaps::{self, HotlapFilter, lifecycle},
    },
};

use super::response::ManagedHotlapResponse;

#[utoipa::path(
    post,
    path = "/api/v1/hotlaps/{hotlap}/validate",
    tag = "hotlaps",
    security(
        ("cookie_session" = []),
        ("personal_access_token" = [])
    ),
    params(
        ("hotlap" = i64, Path, description = "Hotlap identity"),
        ("X-CSRF-Token" = String, Header, description = "Required with cookie-session authentication")
    ),
    responses(
        (status = 200, description = "Hotlap immediately marked valid", body = ManagedHotlapResponse),
        (status = 401, description = "Authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token invalid or test validation disabled", body = ErrorResponse),
        (status = 404, description = "Hotlap not found", body = ErrorResponse)
    )
)]
pub(crate) async fn validate_for_testing(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<Json<ManagedHotlapResponse>, ApiError> {
    if !state.hotlaps.allow_test_validation {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "test_validation_disabled",
            "Immediate test validation is disabled",
        ));
    }

    let hotlap = hotlaps::HotlapEntity::find_by_id(hotlap_id)
        .uploads()
        .owned_by(player.id)
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Hotlap"))?;
    if hotlap.vehicle.is_none() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "vehicle_unresolved",
            "The hotlap's vehicle has not been resolved",
        ));
    }
    let era = EraEntity::find_by_id(hotlap.era_id)
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| {
            ApiError::database(sea_orm::DbErr::Type(format!(
                "hotlap {hotlap_id} names era {} which no longer exists",
                hotlap.era_id
            )))
        })?;
    let eligible = match hotlap.vehicle.as_ref() {
        Some(vehicle) => era
            .admits_combination(&state.database, &hotlap.track.0, &vehicle.0)
            .await
            .map_err(ApiError::database)?,
        None => false,
    };
    if !eligible {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "combination_not_eligible",
            "The hotlap's combination is not eligible for this era",
        ));
    }
    let hotlap = lifecycle::validate_owned_for_testing(&state.database, hotlap_id, player.id)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Hotlap"))?;
    // Rebuild badges after publishing, as the background validator does.
    rebuild_published_badges(&state.database, hotlap.era_id).await;

    Ok(Json(ManagedHotlapResponse::new(hotlap, &era)))
}
