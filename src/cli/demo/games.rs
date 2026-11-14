//! Historical mini-game fixtures, generated alongside the demo hotlaps.
use super::generate::Profile;
use crate::models::{
    attachment, event,
    event::game::{GameConfig, GameSource, VehicleRule},
};
use anyhow::{Context, ensure};
use object_store::ObjectStore;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter,
    Statement,
};
use serde_json::json;
use time::{Duration, OffsetDateTime};

pub(super) async fn populate(
    database: &DatabaseTransaction,
    store: &dyn ObjectStore,
    profiles: &[Profile],
    players: &[i64],
    seed: u64,
    now: OffsetDateTime,
) -> anyhow::Result<()> {
    // Keep this stream separate so adding game fixtures never changes hotlap RNG.
    let mut rng = StdRng::seed_from_u64(seed ^ 0x4741_4d45_4445_4d4f);
    let league = database.query_one_raw(Statement::from_string(DbBackend::Postgres,
        "INSERT INTO league (name,description) SELECT 'Demo mini-games','Synthetic competitions for local development.' WHERE NOT EXISTS (SELECT 1 FROM league WHERE name='Demo mini-games' AND source='local') RETURNING id")).await?;
    let league_id = match league {
        Some(row) => row.try_get::<i64>("", "id")?,
        None => database.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT id FROM league WHERE name='Demo mini-games' AND source='local' ORDER BY id LIMIT 1")).await?.context("demo league missing")?.try_get("", "id")?,
    };
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO season (league_id,name) SELECT $1,'Demo history' WHERE NOT EXISTS (SELECT 1 FROM season WHERE league_id=$1 AND name='Demo history' AND source='local')", [league_id.into()])).await?;
    let season_id: i64 = database.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT id FROM season WHERE league_id=$1 AND name='Demo history' AND source='local' ORDER BY id LIMIT 1", [league_id.into()])).await?.context("demo season missing")?.try_get("", "id")?;
    // A small valid LYT preview, not a claim to be a playable competition layout.
    let layout_bytes = [
        b'L', b'F', b'S', b'L', b'Y', b'T', 0, 0, 1, 0, 0, 9, 0, 0, 0, 0, 0, 0, 20, 0,
    ];
    insim_lyt::Lyt::read(layout_bytes.as_slice())?;
    let mut layout_id = None;
    let slots = this_week_slots(database, now).await?;
    let mut inserted_events = 0;
    let mut inserted_results = 0;
    for (index, (id, title, kind, source)) in [
        (
            "demo-bomb",
            "Demo Bomb Run",
            "bomb",
            GameSource::Bomb {
                checkpoint_timeout_secs: 30,
                checkpoint_penalty_ms: 250,
                collision_max_penalty_ms: 500,
            },
        ),
        (
            "demo-metronome",
            "Demo Metronome",
            "metronome",
            GameSource::Metronome { target_ms: 60000 },
        ),
        (
            "demo-shortcut",
            "Demo Shortcut",
            "shortcut",
            GameSource::Shortcut,
        ),
        (
            "demo-land-speed-record",
            "Demo Land Speed Record",
            "land_speed_record",
            GameSource::LandSpeedRecord,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (start, end) = slots[index];
        let existing = event::Entity::find()
            .filter(event::Column::CatalogueId.eq(id))
            .one(database)
            .await?;
        let event = match existing {
            Some(event) => {
                ensure!(
                    event.league_id == league_id
                        && event.source == source.kind()
                        && event.ends_at.is_some_and(|end| end < now),
                    "reserved demo event {id} is not a completed demo competition"
                );
                reschedule(database, &event, start, end).await?;
                event::Entity::find_by_id(event.id)
                    .one(database)
                    .await?
                    .context("demo event missing")?
            }
            None => {
                let layout = match layout_id {
                    Some(id) => id,
                    None => {
                        let id = attachment::Model::store(
                            database,
                            store,
                            layout_bytes.to_vec(),
                            "lyt",
                            "application/octet-stream",
                            Some("demo-preview.lyt"),
                        )
                        .await?
                        .id;
                        layout_id = Some(id);
                        id
                    }
                };
                let config = GameConfig {
                    source,
                    track: "BL1".into(),
                    vehicles: vec![VehicleRule {
                        vehicle: "XFG".into(),
                        added_mass_kg: 0,
                        intake_restriction: 0,
                    }],
                };
                let row = database.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                    "INSERT INTO event (league_id,season_id,catalogue_id,title,description,starts_at,ends_at,source,game_config,layout_attachment_id,track_label,cars_label) VALUES ($1,$2,$3,$4,'Synthetic historical results for development. The layout is a minimal preview, not a playable event layout.',$5,$6,$7,$8,$9,'Blackwood GP','XFG') RETURNING id",
                    [league_id.into(),season_id.into(),id.into(),title.into(),start.into(),end.into(),kind.into(),serde_json::to_value(config)?.into(),layout.into()])).await?.context("demo event insert returned no row")?;
                let event_id = row.try_get::<i64>("", "id")?;
                inserted_events += 1;
                event::Entity::find_by_id(event_id)
                    .one(database)
                    .await?
                    .context("demo event missing")?
            }
        };
        let end = event.ends_at.context("demo event requires end time")?;
        database.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO game_event_execution (event_id,started_at,stopped_at) VALUES ($1,$2,$3) ON CONFLICT (event_id) DO UPDATE SET started_at=excluded.started_at,stopped_at=excluded.stopped_at", [event.id.into(),event.starts_at.into(),end.into()])).await?;
        for (profile_chunk, player_chunk) in profiles.chunks(500).zip(players.chunks(500)) {
            let mut rows = Vec::with_capacity(player_chunk.len() * 3);
            for (profile, player) in profile_chunk.iter().zip(player_chunk) {
                for attempt in 0_i32..3 {
                    let checkpoints = rng.random_range(5..25) + (1500 - profile.pace) / 50;
                    let deviation = rng.random_range(-1500..=1500);
                    let speed_kph = if kind == "land_speed_record" {
                        190.0
                            + f64::from(i32::try_from(1500 - profile.pace)?) / 20.0
                            + rng.random_range(0.0..15.0)
                    } else {
                        0.0
                    };
                    rows.push(json!({
                        "player":player,"checkpoints":checkpoints,
                        "survival":checkpoints * 12000 + rng.random_range(1000..10000),
                        "time":35000 * profile.pace/1000 + rng.random_range(0..5000),
                        "delta":deviation,"speed_kph":speed_kph,
                        "recorded":(event.starts_at + (end-event.starts_at) * (attempt * 3 + 1) / 10_i32).format(&time::format_description::well_known::Rfc3339)?,
                    }));
                }
            }
            let sql = match kind {
                "bomb" => {
                    "INSERT INTO bomb_run (event_id,player_id,vehicle,checkpoint_count,survival_ms,recorded_at) SELECT $1,r.player,'XFG',r.checkpoints,r.survival,r.recorded FROM jsonb_to_recordset($2) AS r(player BIGINT,checkpoints BIGINT,survival BIGINT,recorded TIMESTAMPTZ) WHERE NOT EXISTS (SELECT 1 FROM bomb_run old WHERE old.event_id=$1 AND old.player_id=r.player)"
                }
                "land_speed_record" => {
                    "INSERT INTO land_speed_record_run (event_id,player_id,vehicle,speed_kph,recorded_at) SELECT $1,r.player,'XFG',r.speed_kph,r.recorded FROM jsonb_to_recordset($2) AS r(player BIGINT,speed_kph DOUBLE PRECISION,recorded TIMESTAMPTZ) WHERE NOT EXISTS (SELECT 1 FROM land_speed_record_run old WHERE old.event_id=$1 AND old.player_id=r.player)"
                }
                "metronome" => {
                    "INSERT INTO metronome_result (event_id,player_id,delta_ms,recorded_at) SELECT $1,r.player,r.delta,r.recorded FROM jsonb_to_recordset($2) AS r(player BIGINT,delta BIGINT,recorded TIMESTAMPTZ) WHERE NOT EXISTS (SELECT 1 FROM metronome_result old WHERE old.event_id=$1 AND old.player_id=r.player)"
                }
                _ => {
                    "INSERT INTO shortcut_time (event_id,player_id,vehicle,time_ms,recorded_at) SELECT $1,r.player,'XFG',r.time,r.recorded FROM jsonb_to_recordset($2) AS r(player BIGINT,time BIGINT,recorded TIMESTAMPTZ) WHERE NOT EXISTS (SELECT 1 FROM shortcut_time old WHERE old.event_id=$1 AND old.player_id=r.player)"
                }
            };
            inserted_results += database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    [event.id.into(), json!(rows).into()],
                ))
                .await?
                .rows_affected();
        }
        eprintln!(
            "Completed {title}: /events/{} ({} to {})",
            event.id,
            event.starts_at.date(),
            end.date()
        );
    }
    tracing::info!(
        inserted_events,
        inserted_results,
        "historical demo mini-games generated"
    );
    Ok(())
}

/// Use a free part of the elapsed UTC calendar week, even on Monday.
async fn this_week_slots(
    database: &DatabaseTransaction,
    now: OffsetDateTime,
) -> anyhow::Result<[(OffsetDateTime, OffsetDateTime); 4]> {
    let week_start = now.date().midnight().assume_utc()
        - Duration::days(i64::from(now.weekday().number_days_from_monday()));
    let cutoff = now.replace_nanosecond(0)? - Duration::seconds(1);
    let occupied = database.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT starts_at,ends_at FROM event WHERE game_config IS NOT NULL AND NOT hidden AND catalogue_id NOT IN ('demo-bomb','demo-metronome','demo-shortcut','demo-land-speed-record') AND starts_at < $2 AND ends_at > $1 ORDER BY starts_at", [week_start.into(),cutoff.into()])).await?;
    let mut cursor = week_start;
    let mut best = (week_start, week_start);
    for row in occupied {
        let start: OffsetDateTime = row.try_get("", "starts_at")?;
        let end: OffsetDateTime = row.try_get("", "ends_at")?;
        let gap_end = start.min(cutoff);
        if gap_end - cursor > best.1 - best.0 {
            best = (cursor, gap_end);
        }
        cursor = cursor.max(end);
    }
    if cutoff - cursor > best.1 - best.0 {
        best = (cursor, cutoff);
    }
    ensure!(
        best.1 - best.0 >= Duration::seconds(8),
        "not enough free completed time in this calendar week for demo events"
    );
    let duration = ((best.1 - best.0) / 8_i32).min(Duration::hours(1));
    Ok(std::array::from_fn(|index| {
        let start = best.0 + duration * i32::try_from(index * 2).unwrap();
        (start, start + duration)
    }))
}

/// Move only our synthetic fixtures, preserving scores and relative attempt times.
async fn reschedule(
    database: &DatabaseTransaction,
    event: &event::Model,
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> anyhow::Result<()> {
    let old_end = event.ends_at.context("demo event requires an end")?;
    if event.starts_at == start && old_end == end {
        return Ok(());
    }
    for sql in [
        "UPDATE bomb_run SET recorded_at=$2::timestamptz+(recorded_at-$3::timestamptz)*(extract(epoch FROM ($4::timestamptz-$2::timestamptz))/extract(epoch FROM ($5::timestamptz-$3::timestamptz)))::double precision WHERE event_id=$1",
        "UPDATE metronome_result SET recorded_at=$2::timestamptz+(recorded_at-$3::timestamptz)*(extract(epoch FROM ($4::timestamptz-$2::timestamptz))/extract(epoch FROM ($5::timestamptz-$3::timestamptz)))::double precision WHERE event_id=$1",
        "UPDATE shortcut_time SET recorded_at=$2::timestamptz+(recorded_at-$3::timestamptz)*(extract(epoch FROM ($4::timestamptz-$2::timestamptz))/extract(epoch FROM ($5::timestamptz-$3::timestamptz)))::double precision WHERE event_id=$1",
        "UPDATE land_speed_record_run SET recorded_at=$2::timestamptz+(recorded_at-$3::timestamptz)*(extract(epoch FROM ($4::timestamptz-$2::timestamptz))/extract(epoch FROM ($5::timestamptz-$3::timestamptz)))::double precision WHERE event_id=$1",
    ] {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                [
                    event.id.into(),
                    start.into(),
                    event.starts_at.into(),
                    end.into(),
                    old_end.into(),
                ],
            ))
            .await?;
    }
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE event SET starts_at=$2,ends_at=$3,updated_at=now() WHERE id=$1",
            [event.id.into(), start.into(), end.into()],
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::SqlxPostgresConnector;
    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn completed_slots_fit_even_early_on_monday(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        use sea_orm::TransactionTrait;
        let tx = database.begin().await?;
        let now = time::macros::datetime!(2030-01-07 0:30 UTC);
        let slots = this_week_slots(&tx, now).await?;
        for (start, end) in slots {
            assert_eq!(start.date(), now.date());
            assert!(start < end && end < now);
        }
        assert!(slots.windows(2).all(|pair| pair[0].1 <= pair[1].0));
        Ok(())
    }
}
