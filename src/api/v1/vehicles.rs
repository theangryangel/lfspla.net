//! Public vehicle image delivery.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use object_store::{ObjectStore, path::Path as ObjectPath};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{ApiError, ApiState, ErrorResponse},
    models::vehicles::{VehicleColumn, VehicleEntity},
};

/// The relative URL for a vehicle's cached image.
pub(crate) fn image_url(vehicle_id: &str) -> String {
    format!("/api/v1/vehicles/{vehicle_id}/image")
}

/// Builds public vehicle routes.
pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(image))
}

/// Serves a cached image for an available catalogue vehicle.
#[utoipa::path(
    get,
    path = "/api/v1/vehicles/{vehicle}/image",
    tag = "vehicles",
    params(("vehicle" = String, Path, description = "Canonical vehicle code")),
    responses(
        (status = 200, description = "Cached vehicle image"),
        (status = 404, description = "Vehicle or cached image was not found", body = ErrorResponse)
    )
)]
pub(crate) async fn image(
    Path(vehicle_id): Path<String>,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let vehicle = VehicleEntity::find_by_id(vehicle_id)
        .filter(VehicleColumn::Available.eq(true))
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("vehicle_not_found", "Vehicle"))?;
    let (object_key, content_type) = match (vehicle.image_object_key, vehicle.image_content_type) {
        (Some(object_key), Some(content_type)) => (object_key, content_type),
        _ => {
            return Err(ApiError::not_found(
                "vehicle_image_not_found",
                "Vehicle image",
            ));
        }
    };
    let image = state
        .object_store
        .get(&ObjectPath::from(object_key.as_str()))
        .await
        .map_err(|error| match error {
            object_store::Error::NotFound { .. } => {
                ApiError::not_found("vehicle_image_not_found", "Vehicle image")
            }
            error => ApiError::object_store_read(error),
        })?;
    let etag = object_key.rsplit('/').next().unwrap_or(&object_key);
    let etag = HeaderValue::from_str(&format!("\"{etag}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("\"image\""));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, image.meta.size)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .header(header::ETAG, etag)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from_stream(image.into_stream()))
        .map_err(|error| ApiError::internal(&error, "vehicle image response"))
}

#[cfg(test)]
mod tests {
    use super::image_url;

    #[test]
    fn creates_a_relative_image_url() {
        assert_eq!(image_url("41A2A0"), "/api/v1/vehicles/41A2A0/image");
    }
}
