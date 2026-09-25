//! Removing an uploaded hotlap, which only its owner may do.

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use object_store::path::Path as ObjectPath;

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::{eras::rebuild_published_badges, hotlaps::lifecycle},
};

#[utoipa::path(
    delete,
    path = "/api/v1/hotlaps/{hotlap}",
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
        (status = 204, description = "Hotlap removed"),
        (status = 401, description = "Authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse),
        (status = 404, description = "Hotlap not found", body = ErrorResponse)
    )
)]
pub(crate) async fn remove(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    let (object_key, era_id, published) =
        lifecycle::delete_owned(&state.database, hotlap_id, player.id)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Hotlap"))?;

    if published {
        rebuild_published_badges(&state.database, era_id).await;
    }

    if let Err(error) = state
        .object_store
        .delete(&ObjectPath::from(object_key.as_str()))
        .await
    {
        tracing::error!(?error, %object_key, hotlap_id, "failed to remove replay object");
    }

    Ok(StatusCode::NO_CONTENT)
}
