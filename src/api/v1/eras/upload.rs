//! SPR uploads to a chosen era.
//!
//! The replay version must match the era. Version ranges cannot overlap,
//! so a replay can match at most one era.

use std::{io::Cursor, time::Duration};

use axum::{
    Json,
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use insim_core::game_version::GameVersion;
use object_store::PutPayload;
use sea_orm::{EntityTrait, QueryOrder};
use serde_json::json;
use sha2::{Digest, Sha256};
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::{Validate, ValidationError};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse,
        extractors::{self as extract, AuthenticatedPlayer},
        v1::hotlaps::response::ManagedHotlapResponse,
    },
    models::{
        eras::{self, EraModel},
        hotlaps::{
            self, HotlapEntity,
            lifecycle::{self, InsertError, NewHotlap},
        },
        vehicles,
    },
    storage::Storage,
};

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
    responses(
        (status = 202, description = "Hotlap created and awaiting delayed validation", body = ManagedHotlapResponse,
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
    let outstanding = lifecycle::count_outstanding(&state.database, player.id)
        .await
        .map_err(ApiError::database)?;
    let max_queued = state.hotlaps.max_queued_per_player.get();
    if outstanding >= max_queued {
        return Err(ApiError::hotlap_queue_full(outstanding, max_queued));
    }

    let upload = spr_field(multipart, state.hotlaps.max_spr_upload_bytes()).await?;
    upload.validate()?;
    let header =
        lfsplanet_spr::SprHeader::read(Cursor::new(upload.bytes.as_ref())).map_err(|error| {
            tracing::debug!(?error, "uploaded SPR header could not be parsed");
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_spr",
                "The uploaded file does not have a valid SPR header",
            )
        })?;

    if !era.accepts_replay_version(&header.lfs_version) {
        return Err(wrong_era(&state, &era, &header.lfs_version).await);
    }
    if !era
        .admits_track(&state.database, &header.track)
        .await
        .map_err(ApiError::database)?
    {
        return Err(ApiError::unsupported_track());
    }
    validate_replay_username(
        state.hotlaps.enforce_replay_username,
        &header.user_name,
        &player.lfs_username,
    )?;

    let lap_time = replay_lap_time(&header)?;
    let raw_vehicle_name = header.car_name.as_str().to_owned();
    let (vehicle_name, mod_version) = header.car_name.into_parts();
    let vehicle = if let Some(vehicle) =
        vehicles::standard_by_name(&state.database, &raw_vehicle_name)
            .await
            .map_err(ApiError::database)?
    {
        Some(vehicle)
    } else {
        vehicles::mods::resolve(&state.database, vehicle_name, mod_version)
            .await
            .map_err(ApiError::database)?
    };
    let vehicle = vehicle.ok_or_else(ApiError::unsupported_combination)?;
    if !era
        .admits_combination(&state.database, &header.track, &vehicle)
        .await
        .map_err(ApiError::database)?
    {
        return Err(ApiError::unsupported_combination());
    }

    let digest = hex::encode(Sha256::digest(&upload.bytes));
    let object_key = HotlapEntity::store(
        state.object_store.as_ref(),
        &replay_object_suffix(&digest),
        PutPayload::from(upload.bytes),
    )
    .await
    .map_err(ApiError::object_store)?;

    let hotlap = lifecycle::insert(
        &state.database,
        NewHotlap {
            player_id: player.id,
            era_id: era.id,
            track: header.track,
            vehicle,
            raw_vehicle_name: &raw_vehicle_name,
            mod_version,
            lap_time,
            split_times: header.split_times,
            original_filename: &upload.filename,
            fingerprint: &digest,
            spr_object_key: &object_key,
            abs_enabled: hotlaps::resolve_abs(header.abs_enabled, &header.lfs_version),
            player_flags: header.player_flags.bits(),
            game_version: &header.lfs_version,
        },
        max_queued,
    )
    .await;
    // Clean up definite refusals only. A database error can mean COMMIT
    // succeeded but its response was lost; GC must resolve that uncertainty.
    if hotlap.is_err()
        && !matches!(&hotlap, Err(InsertError::Database(_)))
        && let Err(error) = state
            .object_store
            .delete(&object_store::path::Path::from(object_key.as_str()))
            .await
    {
        tracing::warn!(?error, %object_key, "failed to clean up refused replay upload");
    }
    let hotlap = hotlap.map_err(|error| match error {
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
        InsertError::EraClosed => era_closed(&era),
        InsertError::WrongEra => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "wrong_era",
            "The era no longer accepts this replay version. Refresh the era catalogue and try again.",
        ),
        InsertError::UploadsBanned => ApiError::uploads_banned(),
    })?;

    let location = format!("/api/v1/hotlaps/{}", hotlap.id);
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(ManagedHotlapResponse::new(hotlap, &era)),
    ))
}

fn replay_object_suffix(digest: &str) -> String {
    // A deletion of an earlier submission must never delete a re-upload's bytes.
    // The fingerprint still deduplicates submissions in PostgreSQL.
    let identity = hex::encode(rand::random::<[u8; 32]>());
    format!("{}/{digest}-{identity}.spr", &digest[..2])
}

fn era_closed(era: &EraModel) -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "era_closed",
        format!("{} is closed to new submissions", era.title),
    )
}

/// Reports a version mismatch. Includes the matching era, if found,
/// so the client can retry there.
async fn wrong_era(state: &ApiState, era: &EraModel, version: &GameVersion) -> ApiError {
    let catalogue = eras::EraEntity::find()
        .order_by_asc(eras::EraColumn::Id)
        .all(&state.database)
        .await;
    let accepting = match catalogue {
        Ok(catalogue) => catalogue
            .into_iter()
            .find(|candidate| candidate.accepts_replay_version(version)),
        Err(error) => return ApiError::database(error),
    };
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

fn replay_lap_time(header: &lfsplanet_spr::SprHeader) -> Result<Duration, ApiError> {
    header
        .split_count
        .checked_sub(1)
        .and_then(|index| header.split_times.get(index as usize))
        .copied()
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "missing_lap_time",
                "The SPR header does not contain a lap time",
            )
        })
}

fn validate_replay_username(
    enforce: bool,
    replay_username: &str,
    authenticated_username: &str,
) -> Result<(), ApiError> {
    if enforce && !replay_username.eq_ignore_ascii_case(authenticated_username) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "replay_username_mismatch",
            "The replay belongs to a different LFS account",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deleting_an_earlier_upload_preserves_identical_reuploaded_bytes() {
        use object_store::{ObjectStore, memory::InMemory, path::Path};

        let store = InMemory::new();
        let bytes = Bytes::from_static(b"same replay");
        let digest = hex::encode(Sha256::digest(&bytes));
        let first =
            HotlapEntity::store(&store, &replay_object_suffix(&digest), bytes.clone().into())
                .await
                .unwrap();
        let second =
            HotlapEntity::store(&store, &replay_object_suffix(&digest), bytes.clone().into())
                .await
                .unwrap();
        store.delete(&Path::from(first)).await.unwrap();
        assert_eq!(
            store
                .get(&Path::from(second))
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap(),
            bytes
        );
    }

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
    fn replay_username_must_match_unless_enforcement_is_disabled() {
        assert!(validate_replay_username(true, "example", "example").is_ok());
        assert!(validate_replay_username(true, "Example", "example").is_ok());
        assert!(validate_replay_username(true, "another", "example").is_err());
        assert!(validate_replay_username(false, "another", "example").is_ok());
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
