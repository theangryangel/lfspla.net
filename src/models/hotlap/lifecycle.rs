//! Persistence for uploaded hotlaps throughout validation.

use super::{HotlapFilter, HotlapState, SOURCE_UPLOAD, SteeringInput};
use crate::milliseconds::Milliseconds;
use crate::{
    game_version::GameVersionCode,
    models::{
        Era, Hotlap,
        era::Entity as EraEntity,
        hotlap::{ActiveModel as HotlapMutation, Column as HotlapColumn, Entity as HotlapEntity},
        player::Entity as PlayerEntity,
    },
    player_flags::PlayerFlagsBits,
};
use crate::{track_id::TrackId, vehicle_id::VehicleId};
use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};
use lfsplanet_spr::PlayerFlags;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, ModelTrait,
    PaginatorTrait, QueryFilter, QuerySelect, Statement, TransactionTrait, TryInsertResult, Value,
};
use std::time::Duration;

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

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestValidationError {
    #[error("The hotlap's combination is not eligible for this era")]
    UnsupportedCombination,
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl Hotlap {
    /// Makes a newly valid hotlap its chart's personal best when it beats the
    /// current row. The full comparator is deliberately shared with rebuilds.
    pub(crate) async fn consider_personal_best<C>(&self, database: &C) -> Result<bool, DbErr>
    where
        C: ConnectionTrait,
    {
        let hotlap_id = self.id;
        // The caller holds the catalogue admission lock and uses a transaction.
        // Lock before the upsert, so the following rerank sees prior chart writes.
        let chart = crate::models::chart::Entity::find_by_id(self.chart_id)
            .one(database)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("hotlap chart missing".into()))?;
        let era_id = chart.era_id;
        chart.lock(database).await?;
        let changed = database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id)
SELECT chart_id, player_id, id
FROM hotlap
WHERE id = $1
  AND state = 'valid'
ON CONFLICT (chart_id, player_id) DO UPDATE
SET hotlap_id = EXCLUDED.hotlap_id
WHERE (
    SELECT ROW(lap_time_ms, created_at, id)
    FROM hotlap
    WHERE id = EXCLUDED.hotlap_id
) < (
    SELECT ROW(lap_time_ms, created_at, id)
    FROM hotlap
    WHERE id = hotlap_personal_best.hotlap_id
)
",
                [Value::from(hotlap_id)],
            ))
            .await?
            .rows_affected();
        if changed > 0 {
            chart.rerank_personal_bests(database).await?;
        }
        // Even a slower published lap changes the "exactly one hotlap" badge.
        crate::models::badge::request_badge_refresh(database, era_id).await?;
        let position: Option<i64> = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT position FROM hotlap_personal_best WHERE hotlap_id = $1",
                [Value::from(hotlap_id)],
            ))
            .await?
            .map(|row| row.try_get("", "position"))
            .transpose()?;
        Ok(position == Some(1))
    }

    /// Rebuilds a player's one chart after its present best was removed.
    /// The caller holds the chart lock from before removal through commit.
    pub(crate) async fn refresh_personal_best<C>(&self, database: &C) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        let era_id = self.era_id;
        let player_id = self.player_id;
        let chart = crate::models::chart::Entity::find_by_id(self.chart_id)
            .one(database)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("hotlap chart missing".into()))?;
        let values = [Value::from(self.chart_id), Value::from(player_id)];
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
DELETE FROM hotlap_personal_best
WHERE chart_id = $1 AND player_id = $2
",
                values.clone(),
            ))
            .await?;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id)
SELECT chart_id, player_id, id
FROM hotlap
WHERE chart_id = $1 AND player_id = $2
  AND state = 'valid'
ORDER BY lap_time_ms, created_at, id
LIMIT 1
",
                values,
            ))
            .await?;
        chart.rerank_personal_bests(database).await?;
        crate::models::badge::request_badge_refresh(database, era_id).await?;
        Ok(())
    }

    pub async fn insert(
        database: &DatabaseConnection,
        new: NewHotlap<'_>,
        max_outstanding: u64,
    ) -> Result<Hotlap, InsertError> {
        let transaction = database.begin().await?;
        crate::db::lock_admission(&transaction).await?;
        let chart = crate::models::Chart::find_combination(
            &transaction,
            new.era_id,
            &new.track.to_string(),
            &new.vehicle.to_string(),
        )
        .await?
        .ok_or(InsertError::UnsupportedCombination)?;
        let active = HotlapMutation {
            chart_id: Set(chart.id),
            id: NotSet,
            player_id: Set(new.player_id),
            era_id: Set(new.era_id),
            track: Set(TrackId(new.track)),
            vehicle: Set(VehicleId(new.vehicle)),
            raw_vehicle_name: Set(new.raw_vehicle_name.to_owned()),
            mod_version: Set(new
                .mod_version
                .map(i16::try_from)
                .transpose()
                .map_err(|_| sea_orm::DbErr::Type("mod version exceeds SMALLINT".to_owned()))?),
            lap_time_ms: Set(Milliseconds::try_from(new.lap_time).map_err(|_| {
                sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned())
            })?),
            split_1_ms: Set(Milliseconds::try_from(new.split_times[0]).map_err(|_| {
                sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned())
            })?),
            split_2_ms: Set(Milliseconds::try_from(new.split_times[1]).map_err(|_| {
                sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned())
            })?),
            split_3_ms: Set(Milliseconds::try_from(new.split_times[2]).map_err(|_| {
                sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned())
            })?),
            split_4_ms: Set(Milliseconds::try_from(new.split_times[3]).map_err(|_| {
                sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned())
            })?),
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
        let admitted = match era_for(&transaction, new.era_id).await? {
            Some(era) => {
                if !era.open {
                    return Err(InsertError::EraClosed);
                }
                if !era.accepts_replay_version(new.game_version) {
                    return Err(InsertError::WrongEra);
                }
                chart.is_available(&transaction).await?
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
        let outstanding = Self::count_outstanding(&transaction, new.player_id).await?;
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
        crate::db::lock_admission(&transaction).await?;
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
        if published {
            let chart = crate::models::chart::Entity::find_by_id(hotlap.chart_id)
                .one(&transaction)
                .await?
                .ok_or_else(|| DbErr::RecordNotFound("hotlap chart missing".into()))?;
            chart.lock(&transaction).await?;
        }
        HotlapEntity::delete_by_id(hotlap_id)
            .exec(&transaction)
            .await?;
        if published {
            // The foreign key removes the old projection row. Select the next best
            // lap before committing so no reader can observe a missing PB.
            hotlap.refresh_personal_best(&transaction).await?;
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
    ) -> Result<Option<(Hotlap, Era)>, TestValidationError> {
        let transaction = database.begin().await?;
        crate::db::lock_admission(&transaction).await?;
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

        // The chart FK guarantees catalogue membership and matching codes.
        // Vehicle availability remains a mutable admission policy.
        let available = model
            .find_related(crate::models::vehicle::Entity)
            .filter(crate::models::vehicle::Column::Available.eq(true))
            .one(&transaction)
            .await?
            .is_some();
        if !available {
            transaction.rollback().await?;
            return Err(TestValidationError::UnsupportedCombination);
        }
        let era = model
            .find_related(EraEntity)
            .one(&transaction)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("hotlap era missing".into()))?;

        let mut active: HotlapMutation = model.into();
        active.state = Set(HotlapState::Valid);
        active.hlvc_result_code = Set(Some(1));
        active.error_detail = Set(None);
        active.next_attempt_at = Set(None);
        active.finished_at = Set(Some(time::OffsetDateTime::now_utc()));
        let model = active.update(&transaction).await?;
        let world_record = model.consider_personal_best(&transaction).await?;
        crate::models::WebhookNotification::enqueue_hotlap(&transaction, &model, world_record)
            .await?;
        transaction.commit().await?;
        Ok(Some((model, era)))
    }
}

/// Loads the era a stored hotlap names, for an eligibility check.
async fn era_for<C>(database: &C, era_id: i64) -> Result<Option<Era>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    EraEntity::find_by_id(era_id).one(database).await
}
