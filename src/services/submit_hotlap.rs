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
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement, TransactionTrait,
};
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
    #[error("the replay is not faster than the current personal best ({personal_best_ms} ms)")]
    NotFasterThanPersonalBest { personal_best_ms: i64 },
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

    // Only a confirmed PB can reject a submission. Pending laps may fail validation.
    // This is provisional: validation still compares against the PB at publication.
    if let Some(row) = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT hotlap.lap_time_ms FROM hotlap_personal_best pb
             JOIN hotlap ON hotlap.id = pb.hotlap_id
             WHERE pb.chart_id = $1 AND pb.player_id = $2",
            [chart.id.into(), player.id.into()],
        ))
        .await?
    {
        let personal_best_ms: i64 = row.try_get("", "lap_time_ms")?;
        let claimed_ms = crate::milliseconds::Milliseconds::try_from(lap_time)
            .map_err(|_| sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned()))?;
        if claimed_ms.as_millis() >= personal_best_ms {
            return Err(SubmitError::NotFasterThanPersonalBest { personal_best_ms });
        }
    }

    let digest = hex::encode(Sha256::digest(&bytes));
    let transaction = database.begin().await?;
    crate::db::lock_catalogue_read(&transaction).await?;
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

    fn replay(seconds: u8, username: &str) -> Bytes {
        let mut bytes = vec![0_u8; lfsplanet_spr::HEADER_LEN];
        bytes[..6].copy_from_slice(b"LFSSPR");
        bytes[8] = 3;
        bytes[15] = 1;
        bytes[16..20].copy_from_slice(b"0.7A");
        bytes[24..27].copy_from_slice(b"BL1");
        bytes[64..64 + username.len()].copy_from_slice(username.as_bytes());
        bytes[96..102].copy_from_slice(b"XF GTI");
        bytes[128] = 1;
        bytes[131] = 1;
        bytes[135] = 1;
        bytes[137] = seconds;
        bytes.into()
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn admission_requires_an_improvement_on_the_confirmed_pb(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!("validate_hotlap/fixtures.sql"))
            .execute(&pool)
            .await?;
        sqlx::raw_sql(
            "UPDATE hotlap SET state = 'valid', lap_time_ms = 30000,
                hlvc_result_code = 1, finished_at = now();
            UPDATE era SET open = true;
            INSERT INTO player (lfs_username, display_name) VALUES ('new-driver', 'New Driver');",
        )
        .execute(&pool)
        .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let era = crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        era.rebuild_personal_bests(&database).await?;
        let player = crate::models::player::Entity::find()
            .filter(crate::models::player::Column::LfsUsername.eq("worker-test"))
            .one(&database)
            .await?
            .unwrap();
        let store = object_store::memory::InMemory::new();
        let settings = HotlapSettings::default();

        for seconds in [30, 31] {
            let result = submit(
                &database,
                &store,
                &settings,
                &era,
                &player,
                "lap.spr",
                replay(seconds, "worker-test"),
            )
            .await;
            assert!(matches!(
                result,
                Err(SubmitError::NotFasterThanPersonalBest {
                    personal_best_ms: 30000
                })
            ));
        }
        // Rejections must happen before either a replay object or database row is stored.
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM hotlap")
                .fetch_one(&pool)
                .await?,
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
                .fetch_one(&pool)
                .await?,
            1
        );
        use futures::TryStreamExt;
        assert!(store.list(None).try_collect::<Vec<_>>().await?.is_empty());

        // Pending improvements do not become the admission threshold.
        for seconds in [28, 29] {
            let lap = submit(
                &database,
                &store,
                &settings,
                &era,
                &player,
                "lap.spr",
                replay(seconds, "worker-test"),
            )
            .await?;
            assert_eq!(lap.state, hotlap::HotlapState::Pending);
        }
        let new_player = crate::models::player::Entity::find()
            .filter(crate::models::player::Column::LfsUsername.eq("new-driver"))
            .one(&database)
            .await?
            .unwrap();
        submit(
            &database,
            &store,
            &settings,
            &era,
            &new_player,
            "lap.spr",
            replay(40, "new-driver"),
        )
        .await?;
        Ok(())
    }

    #[test]
    fn replay_username_must_match_unless_enforcement_is_disabled() {
        assert!(validate_replay_username(true, "example", "example").is_ok());
        assert!(validate_replay_username(true, "Example", "example").is_ok());
        assert!(validate_replay_username(true, "another", "example").is_err());
        assert!(validate_replay_username(false, "another", "example").is_ok());
    }
}
