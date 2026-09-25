//! Persistence for uploaded hotlaps throughout validation.

use std::time::Duration;

use super::{
    HotlapColumn, HotlapEntity, HotlapFilter, HotlapModel, HotlapMutation, HotlapState,
    SOURCE_UPLOAD, SteeringInput, personal_bests,
};
use crate::models::{
    duration_millis,
    eras::{EraEntity, EraModel},
    players::PlayerEntity,
};
use crate::models::{
    player_flags::PlayerFlagsBits, track::TrackId, vehicle::VehicleId, version::GameVersionCode,
};
use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};
use lfsplanet_spr::PlayerFlags;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QuerySelect,
    TransactionTrait, TryInsertResult,
};

/// Structurally parsed values accepted from an SPR upload.
pub struct NewHotlap<'a> {
    pub player_id: i64,
    pub era_id: i64,
    pub track: Track,
    pub vehicle: Vehicle,
    pub raw_vehicle_name: &'a str,
    pub mod_version: Option<u16>,
    pub lap_time: Duration,
    pub split_times: [Duration; 4],
    pub original_filename: &'a str,
    pub fingerprint: &'a str,
    pub spr_object_key: &'a str,
    /// `None` when the replay's version could not report ABS.
    pub abs_enabled: Option<bool>,
    /// Raw 16-bit player flags for steering, driving aids, and driver side.
    pub player_flags: u16,
    pub game_version: &'a GameVersion,
}

#[derive(Debug, thiserror::Error)]
pub enum InsertError {
    #[error("player already has {outstanding} hotlaps awaiting processing (limit {limit})")]
    QueueFull { outstanding: u64, limit: u64 },
    #[error("replay has already been uploaded")]
    Duplicate,
    #[error("the track and vehicle combination is not eligible for this era")]
    UnsupportedCombination,
    #[error("the era is closed to new submissions")]
    EraClosed,
    #[error("the era no longer accepts the replay version")]
    WrongEra,
    #[error("the player is no longer permitted to upload")]
    UploadsBanned,
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub async fn insert(
    database: &DatabaseConnection,
    new: NewHotlap<'_>,
    max_outstanding: u64,
) -> Result<HotlapModel, InsertError> {
    let active = HotlapMutation {
        id: NotSet,
        player_id: Set(new.player_id),
        era_id: Set(new.era_id),
        track: Set(TrackId(new.track)),
        vehicle: Set(Some(VehicleId(new.vehicle))),
        raw_vehicle_name: Set(new.raw_vehicle_name.to_owned()),
        mod_version: Set(new
            .mod_version
            .map(i16::try_from)
            .transpose()
            .map_err(|_| sea_orm::DbErr::Type("mod version exceeds SMALLINT".to_owned()))?),
        lap_time_ms: Set(duration_millis(new.lap_time)?),
        split_1_ms: Set(duration_millis(new.split_times[0])?),
        split_2_ms: Set(duration_millis(new.split_times[1])?),
        split_3_ms: Set(duration_millis(new.split_times[2])?),
        split_4_ms: Set(duration_millis(new.split_times[3])?),
        original_filename: Set(Some(new.original_filename.to_owned())),
        spr_object_key: Set(Some(new.spr_object_key.to_owned())),
        source: Set(SOURCE_UPLOAD.to_owned()),
        fingerprint: Set(new.fingerprint.to_owned()),
        // Materialised from the same bits the row stores, purely so the chart
        // query can filter on it without decoding flags in SQL.
        steering: Set(SteeringInput::from_player_flags(
            PlayerFlags::from_bits_retain(new.player_flags),
        )),
        abs_enabled: Set(new.abs_enabled),
        player_flags: Set(PlayerFlagsBits(PlayerFlags::from_bits_retain(
            new.player_flags,
        ))),
        created_at: NotSet,
        game_version: Set(GameVersionCode(new.game_version.clone())),
        state: Set(HotlapState::Pending),
        hlvc_result_code: Set(None),
        error_detail: Set(None),
        attempt_count: Set(0),
        next_attempt_at: Set(None),
        started_at: Set(None),
        finished_at: Set(None),
    };

    // Locking the player serializes admission for that player. The repeated
    // in-transaction count is the authoritative queue-limit check.
    let transaction = database.begin().await?;
    crate::models::eras::lock_admission(&transaction).await?;
    let admitted = match era_for(&transaction, new.era_id).await? {
        Some(era) => {
            if !era.open {
                return Err(InsertError::EraClosed);
            }
            if !era.accepts_replay_version(new.game_version) {
                return Err(InsertError::WrongEra);
            }
            era.admits_combination(&transaction, &new.track, &new.vehicle)
                .await?
        }
        None => false,
    };
    if !admitted {
        return Err(InsertError::UnsupportedCombination);
    }
    let player = PlayerEntity::find_by_id(new.player_id)
        .lock_exclusive()
        .one(&transaction)
        .await?
        .ok_or_else(|| {
            sea_orm::DbErr::RecordNotFound(format!(
                "player {} disappeared while admitting a hotlap",
                new.player_id
            ))
        })?;
    if player.deny_auth || player.deny_uploads {
        return Err(InsertError::UploadsBanned);
    }
    let outstanding = count_outstanding(&transaction, new.player_id).await?;
    if outstanding >= max_outstanding {
        transaction.rollback().await?;
        return Err(InsertError::QueueFull {
            outstanding,
            limit: max_outstanding,
        });
    }

    let result = HotlapEntity::insert(active)
        .on_conflict_do_nothing_on([HotlapColumn::Source, HotlapColumn::Fingerprint])
        .exec_with_returning(&transaction)
        .await?;
    transaction.commit().await?;

    match result {
        TryInsertResult::Inserted(model) => Ok(model),
        TryInsertResult::Conflicted => Err(InsertError::Duplicate),
        TryInsertResult::Empty => Err(InsertError::Database(sea_orm::DbErr::Type(
            "the hotlap insert produced no rows".to_owned(),
        ))),
    }
}

pub async fn count_outstanding<C>(database: &C, player_id: i64) -> Result<u64, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    HotlapEntity::find()
        .uploads()
        .owned_by(player_id)
        .outstanding()
        .count(database)
        .await
}

pub async fn delete_owned(
    database: &DatabaseConnection,
    hotlap_id: i64,
    player_id: i64,
) -> Result<Option<(String, i64, bool)>, sea_orm::DbErr> {
    let transaction = database.begin().await?;
    crate::models::eras::lock_admission(&transaction).await?;
    let Some(hotlap) = HotlapEntity::find_by_id(hotlap_id)
        .uploads()
        .owned_by(player_id)
        .lock_exclusive()
        .one(&transaction)
        .await?
    else {
        transaction.rollback().await?;
        return Ok(None);
    };
    let published = hotlap.state == HotlapState::Valid;
    let era_id = hotlap.era_id;
    let player_id = hotlap.player_id;
    let track = hotlap.track.to_string();
    let vehicle = hotlap.vehicle.as_ref().map(ToString::to_string);
    if published {
        personal_bests::lock_chart(
            &transaction,
            era_id,
            &track,
            vehicle
                .as_deref()
                .ok_or_else(|| sea_orm::DbErr::Type("a valid hotlap has no vehicle".into()))?,
        )
        .await?;
    }
    HotlapEntity::delete_by_id(hotlap_id)
        .exec(&transaction)
        .await?;
    if published {
        // The foreign key removes the old projection row. Select the next best
        // lap before committing so no reader can observe a missing PB.
        personal_bests::refresh_key(
            &transaction,
            era_id,
            player_id,
            &track,
            vehicle
                .as_deref()
                .ok_or_else(|| sea_orm::DbErr::Type("a valid hotlap has no vehicle".to_owned()))?,
        )
        .await?;
    }
    transaction.commit().await?;
    let object_key = hotlap.spr_object_key.ok_or_else(|| {
        sea_orm::DbErr::Type(format!("uploaded hotlap {hotlap_id} has no replay object"))
    })?;
    Ok(Some((object_key, era_id, published)))
}

pub async fn validate_owned_for_testing(
    database: &DatabaseConnection,
    hotlap_id: i64,
    player_id: i64,
) -> Result<Option<HotlapModel>, sea_orm::DbErr> {
    let transaction = database.begin().await?;
    crate::models::eras::lock_admission(&transaction).await?;
    let Some(model) = HotlapEntity::find_by_id(hotlap_id)
        .uploads()
        .owned_by(player_id)
        .lock_exclusive()
        .one(&transaction)
        .await?
    else {
        transaction.rollback().await?;
        return Ok(None);
    };

    let VehicleId(vehicle) = model.vehicle.ok_or_else(|| {
        sea_orm::DbErr::Type("an unresolved vehicle cannot be validated".to_owned())
    })?;
    let admitted = match era_for(&transaction, model.era_id).await? {
        Some(era) => {
            era.admits_combination(&transaction, &model.track.0, &vehicle)
                .await?
        }
        None => false,
    };
    if !admitted {
        transaction.rollback().await?;
        return Err(sea_orm::DbErr::Type(format!(
            "combination with vehicle {vehicle} is not eligible for era {}",
            model.era_id
        )));
    }

    let mut active: HotlapMutation = model.into();
    active.state = Set(HotlapState::Valid);
    active.hlvc_result_code = Set(Some(1));
    active.error_detail = Set(None);
    active.next_attempt_at = Set(None);
    active.finished_at = Set(Some(time::OffsetDateTime::now_utc()));
    let model = active.update(&transaction).await?;
    let world_record = personal_bests::consider(&transaction, model.id).await?;
    crate::models::webhooks::enqueue_hotlap(&transaction, &model, world_record).await?;
    transaction.commit().await?;
    Ok(Some(model))
}

/// Loads the era a stored hotlap names, for an eligibility check.
async fn era_for<C>(database: &C, era_id: i64) -> Result<Option<EraModel>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    EraEntity::find_by_id(era_id).one(database).await
}
