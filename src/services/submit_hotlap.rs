//! Replay parsing, vehicle resolution, object storage, and submission admission.
use crate::{
    models::attachment::Model as Attachment,
    models::{
        Era, Hotlap, Player,
        hotlap::{self, InsertError, NewHotlap},
    },
    settings::HotlapSettings,
};
use axum::body::Bytes;
use insim_core::game_version::GameVersion;
use object_store::ObjectStore;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use sha2::{Digest, Sha256};
use std::{io::Cursor, time::Duration};
#[derive(Debug, thiserror::Error)]
pub(crate) enum SubmitError {
    #[error("the SPR header could not be parsed")]
    InvalidSpr,
    #[error("the replay belongs to another era")]
    WrongEra {
        version: GameVersion,
        accepting: Option<Box<Era>>,
    },
    #[error("the track is not supported")]
    UnsupportedTrack,
    #[error("the combination is not supported")]
    UnsupportedCombination,
    #[error("the replay belongs to another account")]
    ReplayUsernameMismatch,
    #[error("the replay has no lap time")]
    MissingLapTime,
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Attachment(#[from] anyhow::Error),
    #[error(transparent)]
    Admission(#[from] InsertError),
}

/// The adapter has checked filename and body size before calling this workflow.
pub(crate) async fn submit(
    database: &DatabaseConnection,
    object_store: &dyn ObjectStore,
    settings: &HotlapSettings,
    era: &Era,
    player: &Player,
    filename: &str,
    bytes: Bytes,
) -> Result<Hotlap, SubmitError> {
    let header = lfsplanet_spr::SprHeader::read(Cursor::new(bytes.as_ref())).map_err(|error| {
        tracing::debug!(?error, "uploaded SPR header could not be parsed");
        SubmitError::InvalidSpr
    })?;

    if !era.accepts_replay_version(&header.lfs_version) {
        return Err(SubmitError::WrongEra {
            version: header.lfs_version.clone(),
            accepting: Era::accepting_version(database, &header.lfs_version)
                .await?
                .map(Box::new),
        });
    }
    if crate::models::chart::Entity::find()
        .filter(crate::models::chart::Column::EraId.eq(era.id))
        .filter(crate::models::chart::Column::TrackId.eq(header.track.to_string()))
        .one(database)
        .await?
        .is_none()
    {
        return Err(SubmitError::UnsupportedTrack);
    }
    validate_replay_username(
        settings.enforce_replay_username,
        &header.user_name,
        &player.lfs_username,
    )?;

    let lap_time = replay_lap_time(&header)?;
    let raw_vehicle_name = header.car_name.as_str().to_owned();
    let (vehicle_name, mod_version) = header.car_name.into_parts();
    let vehicle = if let Some(vehicle) =
        crate::models::Vehicle::standard_by_name(database, &raw_vehicle_name).await?
    {
        Some(vehicle)
    } else {
        crate::models::Vehicle::resolve(database, vehicle_name, mod_version).await?
    };
    let vehicle = vehicle.ok_or(SubmitError::UnsupportedCombination)?;
    let chart = crate::models::Chart::find_combination(
        database,
        era.id,
        &header.track.to_string(),
        &vehicle.to_string(),
    )
    .await?
    .ok_or(SubmitError::UnsupportedCombination)?;
    if !chart.is_available(database).await? {
        return Err(SubmitError::UnsupportedCombination);
    }

    let digest = hex::encode(Sha256::digest(&bytes));
    let transaction = database.begin().await?;
    crate::db::lock_admission(&transaction).await?;
    let attachment = Attachment::store(
        &transaction,
        object_store,
        bytes.to_vec(),
        "spr",
        "application/octet-stream",
        Some(filename),
    )
    .await?;

    let hotlap = crate::models::Hotlap::insert(
        &transaction,
        NewHotlap {
            player_id: player.id,
            era_id: era.id,
            track: header.track,
            vehicle,
            raw_vehicle_name: &raw_vehicle_name,
            mod_version,
            lap_time,
            split_times: header.split_times,
            fingerprint: &digest,
            spr_attachment_id: attachment.id,
            abs_enabled: hotlap::resolve_abs(header.abs_enabled, &header.lfs_version),
            player_flags: header.player_flags.bits(),
            game_version: &header.lfs_version,
        },
        settings.max_queued_per_player.get(),
    )
    .await;
    let hotlap = hotlap?;
    transaction.commit().await?;
    Ok(hotlap)
}

fn replay_lap_time(header: &lfsplanet_spr::SprHeader) -> Result<Duration, SubmitError> {
    header
        .split_count
        .checked_sub(1)
        .and_then(|index| header.split_times.get(index as usize))
        .copied()
        .ok_or(SubmitError::MissingLapTime)
}

fn validate_replay_username(
    enforce: bool,
    replay_username: &str,
    authenticated_username: &str,
) -> Result<(), SubmitError> {
    if enforce && !replay_username.eq_ignore_ascii_case(authenticated_username) {
        return Err(SubmitError::ReplayUsernameMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_username_must_match_unless_enforcement_is_disabled() {
        assert!(validate_replay_username(true, "example", "example").is_ok());
        assert!(validate_replay_username(true, "Example", "example").is_ok());
        assert!(validate_replay_username(true, "another", "example").is_err());
        assert!(validate_replay_username(false, "another", "example").is_ok());
    }
}
