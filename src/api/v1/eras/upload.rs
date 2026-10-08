//! SPR uploads to a chosen era.
//!
//! The replay version must match the era. Version ranges cannot overlap,
//! so a replay can match at most one era.

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, extractors as extract, extractors::AuthenticatedPlayer,
        v1::hotlaps::response::Hotlap,
    },
    models::{Era, hotlap::InsertError},
};
use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use insim_core::game_version::GameVersion;
use serde_json::json;
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::{Validate, ValidationError};
// The request limit includes multipart boundaries and headers; the file itself
// is checked separately against the configured file limit while it is extracted.
const MULTIPART_OVERHEAD_BYTES: usize = 64 * 1024;

/// Builds the era-scoped upload route.
///
/// The body limit is scoped to this one route: every other era endpoint is a
/// small GET, for which axum's own default is the right limit.
pub(super) fn router(max_spr_upload_bytes: usize) -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(upload))
        .layer(DefaultBodyLimit::max(
            max_spr_upload_bytes.saturating_add(MULTIPART_OVERHEAD_BYTES),
        ))
}

#[derive(Debug, Validate)]
struct UploadedSpr {
    #[validate(
        length(
            max = 255,
            message = "The uploaded filename must be no longer than 255 characters"
        ),
        custom(
            function = "crate::validate::validate_non_blank",
            message = "The uploaded filename must not be blank"
        ),
        non_control_character(
            message = "The uploaded filename must not contain control characters"
        ),
        custom(
            function = "validate_spr_filename_shape",
            message = "The uploaded filename must be a safe SPR filename"
        )
    )]
    filename: String,
    bytes: Bytes,
}

#[derive(utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "describes the multipart body extracted by axum")]
struct UploadHotlapForm {
    /// Non-empty SPR file with a safe `.spr` filename.
    #[schema(value_type = String, format = Binary)]
    spr: Vec<u8>,
}

#[utoipa::path(
    post,
    path = "/api/v1/eras/{era}/hotlaps",
    operation_id = "upload_hotlap",
    tag = "hotlaps",
    security(
        ("cookie_session" = []),
        ("personal_access_token" = [])
    ),
    params(
        ("era" = String, Path, description = "Era the replay is claimed to belong to"),
        ("X-CSRF-Token" = String, Header, description = "Required with cookie-session authentication")
    ),
    request_body(
        content = inline(UploadHotlapForm),
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 202, description = "Hotlap created and awaiting delayed validation", body = Hotlap,
            headers(("Location" = String, description = "The created hotlap resource"))),
        (status = 400, description = "Invalid multipart upload", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token invalid, uploads are banned, or replay belongs to another player", body = ErrorResponse),
        (status = 404, description = "Era was not found", body = ErrorResponse),
        (status = 409, description = "Era is closed or replay has already been submitted", body = ErrorResponse),
        (status = 429, description = "Player already has the maximum number of hotlaps awaiting processing", body = ErrorResponse),
        (status = 413, description = "Upload exceeds the size limit", body = ErrorResponse),
        (status = 422, description = "SPR metadata is invalid, or the replay does not belong to this era", body = ErrorResponse)
    )
)]
#[allow(
    clippy::too_many_lines,
    reason = "a single validation pipeline whose order is the contract"
)]
pub(crate) async fn upload(
    extract::Era(era): extract::Era,
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    if player.deny_uploads {
        return Err(ApiError::uploads_banned());
    }
    // Refused before the body is read: an archived era cannot take a replay
    // however good it is, so there is no reason to transfer one.
    if !era.open {
        return Err(era_closed(&era));
    }

    // This inexpensive pre-check avoids reading and parsing a multipart body
    // for the common full-queue case. `insert` repeats the check while holding
    // a player-row lock, which is the authoritative concurrency-safe check.
    let outstanding = crate::models::Hotlap::count_outstanding(&state.database, player.id)
        .await
        .map_err(ApiError::database)?;
    let max_queued = state.hotlaps.max_queued_per_player.get();
    if outstanding >= max_queued {
        return Err(ApiError::hotlap_queue_full(outstanding, max_queued));
    }

    let upload = spr_field(multipart, state.hotlaps.max_spr_upload_bytes()).await?;
    upload.validate()?;
    let hotlap = crate::services::submit_hotlap::submit(
        &state.database,
        state.object_store.as_ref(),
        &state.hotlaps,
        &era,
        &player,
        &upload.filename,
        upload.bytes,
    )
    .await
    .map_err(|error| submission_error(error, &era))?;

    let location = format!("/api/v1/hotlaps/{}", hotlap.id);
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(Hotlap::new(&hotlap, &era, player.into()).with_submission(&hotlap)),
    ))
}

fn era_closed(era: &Era) -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "era_closed",
        format!("{} is closed to new submissions", era.title),
    )
}

/// Reports a version mismatch. Includes the matching era, if found,
/// so the client can retry there.
fn wrong_era(era: &Era, version: &GameVersion, accepting: Option<Box<Era>>) -> ApiError {
    match accepting {
        Some(other) => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "wrong_era",
            format!(
                "This replay was recorded with LFS {version}, which {} does not accept ({}). Upload it to {} instead.",
                era.title, era.version_requirement, other.title
            ),
        )
        .with_details(json!({
            "era_id": other.slug,
            "era_title": other.title,
            "game_version": version.to_string(),
        })),
        None => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_game_version",
            format!(
                "This replay was recorded with LFS {version}, which no era accepts ({} accepts {}).",
                era.title, era.version_requirement
            ),
        ),
    }
}

fn submission_error(error: crate::services::submit_hotlap::SubmitError, era: &Era) -> ApiError {
    use crate::services::submit_hotlap::SubmitError;
    match error {
        SubmitError::InvalidSpr => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_spr",
            "The uploaded file does not have a valid SPR header",
        ),
        SubmitError::WrongEra { version, accepting } => wrong_era(era, &version, accepting),
        SubmitError::UnsupportedTrack => ApiError::unsupported_track(),
        SubmitError::UnsupportedCombination => ApiError::unsupported_combination(),
        SubmitError::ReplayUsernameMismatch => ApiError::new(
            StatusCode::FORBIDDEN,
            "replay_username_mismatch",
            "The replay belongs to a different LFS account",
        ),
        SubmitError::MissingLapTime => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing_lap_time",
            "The SPR header does not contain a lap time",
        ),
        SubmitError::Database(error) => ApiError::database(error),
        SubmitError::ObjectStore(error) => ApiError::object_store(error),
        SubmitError::Admission(error) => match error {
            InsertError::QueueFull { outstanding, limit } => {
                ApiError::hotlap_queue_full(outstanding, limit)
            }
            InsertError::Duplicate => ApiError::new(
                StatusCode::CONFLICT,
                "duplicate_replay",
                "This replay has already been submitted",
            ),
            InsertError::Database(error) => ApiError::database(error),
            InsertError::UnsupportedCombination => ApiError::unsupported_combination(),
            InsertError::EraClosed => era_closed(era),
            InsertError::WrongEra => ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "wrong_era",
                "The era no longer accepts this replay version. Refresh the era catalogue and try again.",
            ),
            InsertError::UploadsBanned => ApiError::uploads_banned(),
        },
    }
}

async fn spr_field(
    mut multipart: Multipart,
    max_spr_upload_bytes: usize,
) -> Result<UploadedSpr, ApiError> {
    let mut spr = None;
    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::debug!(?error, "invalid multipart upload");
        ApiError::invalid_upload("The upload must contain one SPR file field")
    })? {
        if field.name() != Some("spr") || spr.is_some() {
            return Err(ApiError::invalid_upload(
                "The upload must contain one SPR file field",
            ));
        }
        let filename = field
            .file_name()
            .map(str::to_owned)
            .ok_or_else(|| ApiError::invalid_upload("The SPR field must include a filename"))?;
        let bytes = field.bytes().await.map_err(|error| {
            tracing::debug!(?error, "could not read multipart SPR field");
            ApiError::invalid_upload("The SPR upload could not be read")
        })?;
        if bytes.len() > max_spr_upload_bytes {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "spr_upload_too_large",
                "The SPR file exceeds the upload size limit",
            ));
        }
        spr = Some(UploadedSpr { filename, bytes });
    }

    spr.filter(|upload| !upload.bytes.is_empty())
        .ok_or_else(|| ApiError::invalid_upload("The SPR upload must not be empty"))
}

fn validate_spr_filename_shape(filename: &str) -> Result<(), ValidationError> {
    if filename.contains(['/', '\\']) || !filename.to_ascii_lowercase().ends_with(".spr") {
        return Err(ValidationError::new("invalid_spr_filename"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uploaded_spr(filename: impl Into<String>) -> UploadedSpr {
        UploadedSpr {
            filename: filename.into(),
            bytes: Bytes::from_static(b"spr"),
        }
    }

    #[test]
    fn accepts_safe_spr_filenames() {
        assert!(uploaded_spr("piranhot.spr").validate().is_ok());
        assert!(
            uploaded_spr("driver_track_vehicle_time.SPR")
                .validate()
                .is_ok()
        );
        assert!(uploaded_spr("not-a-replay.txt").validate().is_err());
        assert!(uploaded_spr("../piranhot.spr").validate().is_err());
        assert!(uploaded_spr("folder\\piranhot.spr").validate().is_err());
        assert!(uploaded_spr("piranhot\n.spr").validate().is_err());
        assert!(
            uploaded_spr(format!("{}.spr", "a".repeat(252)))
                .validate()
                .is_err()
        );
        assert!(
            uploaded_spr(format!("{}.spr", "a".repeat(251)))
                .validate()
                .is_ok()
        );
        assert!(
            uploaded_spr(format!("{}.spr", "é".repeat(251)))
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn full_hotlap_queue_returns_a_client_actionable_rate_limit() {
        let error = ApiError::hotlap_queue_full(10, 10);
        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.code, "hotlap_queue_full");
        assert_eq!(
            error.message,
            "You already have 10 replays awaiting processing (limit 10). Wait for one to finish or remove one before uploading another."
        );
    }

    #[test]
    fn upload_ban_returns_a_stable_forbidden_error() {
        let error = ApiError::uploads_banned();
        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "uploads_banned");
        assert_eq!(error.message, "You are not permitted to upload replays");
    }
}
