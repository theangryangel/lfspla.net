//! Replay file downloads for published hotlaps and their owners' submissions.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use object_store::{ObjectStore, path::Path as ObjectPath};
use sea_orm::EntityTrait;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{ApiError, ApiState, ErrorResponse, extractors::AuthenticatedPlayer},
    models::hotlaps::{self, HotlapState},
};

/// Path prefix for replay downloads.
const PATH_PREFIX: &str = "/api/v1/hotlaps";

/// The relative URL for a hotlap's replay.
pub(crate) fn download_url(hotlap_id: i64) -> String {
    format!("{PATH_PREFIX}/{hotlap_id}/replay")
}

/// Builds the replay download route.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(download))
}

/// Serves a published replay publicly, or any stored replay to its owner.
///
/// `id` is the **hotlap** identity, not the object-store key: replays are
/// stored under private, unique object keys.
#[utoipa::path(
    get,
    path = "/api/v1/hotlaps/{hotlap}/replay",
    tag = "replays",
    params(("hotlap" = i64, Path, description = "Hotlap identity")),
    responses(
        (status = 200, description = "Original SPR replay", content_type = "application/octet-stream"),
        (status = 404, description = "Hotlap or stored replay not found", body = ErrorResponse)
    )
)]
pub(crate) async fn download(
    Path(hotlap_id): Path<i64>,
    State(state): State<ApiState>,
    viewer: Option<AuthenticatedPlayer>,
) -> Result<Response, ApiError> {
    let viewer = viewer.map(|AuthenticatedPlayer(viewer)| viewer);
    let hotlap = hotlaps::HotlapEntity::find_by_id(hotlap_id)
        .one(&state.database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Hotlap"))?;
    if hotlap.state != HotlapState::Valid
        && viewer
            .as_ref()
            .is_none_or(|player| player.id != hotlap.player_id)
    {
        return Err(ApiError::not_found("hotlap_not_found", "Hotlap"));
    }
    let object_key = hotlap
        .spr_object_key
        .as_deref()
        .ok_or_else(ApiError::replay_not_available)?;
    let replay = state
        .object_store
        .get(&ObjectPath::from(object_key))
        .await
        .map_err(|error| match error {
            object_store::Error::NotFound { .. } => ApiError::replay_not_available(),
            error => ApiError::object_store_read(error),
        })?;
    let content_length = replay.meta.size;
    let disposition = download_disposition(hotlap.original_filename.as_deref(), hotlap_id);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CONTENT_LENGTH, content_length)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from_stream(replay.into_stream()))
        .map_err(|error| ApiError::internal(&error, "replay download response"))
}

/// Builds a download header with a safe filename. Falls back to a generic name
/// if the header is invalid.
fn download_disposition(original_filename: Option<&str>, hotlap_id: i64) -> HeaderValue {
    let filename = safe_download_filename(original_filename, hotlap_id);
    HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap_or_else(|_| {
        HeaderValue::from_str(&format!("attachment; filename=\"hotlap-{hotlap_id}.spr\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
    })
}

fn safe_download_filename(original_filename: Option<&str>, hotlap_id: i64) -> String {
    let filename = original_filename
        .and_then(|value| value.rsplit(['/', '\\']).next())
        .filter(|value| !value.is_empty())
        .unwrap_or("hotlap.spr");
    let filename = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();

    if filename.eq_ignore_ascii_case(".spr") || !filename.to_ascii_lowercase().ends_with(".spr") {
        format!("hotlap-{hotlap_id}.spr")
    } else {
        filename
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_safe_spr_download_filenames() {
        assert_eq!(download_url(42), "/api/v1/hotlaps/42/replay");
        assert_eq!(
            safe_download_filename(Some("driver SO4R RB4 158200.spr"), 42),
            "driver_SO4R_RB4_158200.spr"
        );
        assert_eq!(
            safe_download_filename(Some("../unsafe\"name.spr"), 42),
            "unsafe_name.spr"
        );
        assert_eq!(safe_download_filename(None, 42), "hotlap.spr");
        assert_eq!(
            safe_download_filename(Some("not-an-spr.txt"), 42),
            "hotlap-42.spr"
        );
    }
}
