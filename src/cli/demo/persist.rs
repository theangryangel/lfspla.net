use super::{DemoArgs, generate};
use crate::models::Era;
use anyhow::{Context, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng, seq::SliceRandom};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    time::{Duration, Instant},
};
use time::OffsetDateTime;
const BATCH_SIZE: usize = 500;
const COUNTRIES: [&str; 12] = [
    "GB", "DE", "FR", "FI", "SE", "NL", "PL", "US", "BR", "AU", "JP", "CA",
];

pub(super) async fn populate(
    database: &DatabaseConnection,
    options: &DemoArgs,
) -> anyhow::Result<usize> {
    ensure!(options.yes, "generating demo data requires --yes");
    let transaction = database.begin().await?;
    progress(
        "Waiting for the catalogue lock",
        crate::db::locks::lock_rebuild(&transaction),
    )
    .await?;
    let era = progress(
        "Creating the demo era and track/vehicle combinations",
        create_era(&transaction),
    )
    .await?;
    let seed = options.seed.unwrap_or_else(rand::random);
    eprintln!("Demo seed: {seed} (repeat with --seed {seed})");
    let mut rng = StdRng::seed_from_u64(seed);
    let mut charts = era.combinations().all(&transaction).await?;
    ensure!(!charts.is_empty(), "the demo catalogue has no combinations");
    charts.shuffle(&mut rng);
    charts.truncate((charts.len() * usize::from(options.coverage)).div_ceil(100));
    tracing::info!(
        "Populating {} combinations with {} players ({}% coverage, standard tracks and vehicles only).",
        charts.len(),
        options.players,
        options.coverage
    );
    let baselines: Vec<i64> = charts
        .iter()
        .map(|_| rng.random_range(40_000..200_000))
        .collect();
    let now = OffsetDateTime::now_utc();
    let profiles = generate::profiles(&mut rng, options.players);
    let mut player_ids = Vec::with_capacity(profiles.len());
    tracing::info!("Creating demo players...");
    for chunk in profiles.chunks(BATCH_SIZE) {
        let players: Vec<_> = chunk
            .iter()
            .map(|profile| {
                json!({
                    "username": profile.username,
                    "name": profile.name,
                    "country": COUNTRIES[rng.random_range(0..COUNTRIES.len())],
                })
            })
            .collect();
        let rows = transaction.query_all_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO player (lfs_username, display_name, country_code, deny_auth, deny_uploads, created_at)
             SELECT username, name, country, TRUE, TRUE, to_timestamp($2::bigint)
             FROM jsonb_to_recordset($1) AS p(username TEXT, name TEXT, country TEXT)
             ON CONFLICT (LOWER(lfs_username)) DO UPDATE
             SET lfs_username = player.lfs_username
             RETURNING id, lfs_username",
            [json!(players).into(), (now.unix_timestamp() - 31_536_000).into()])).await
            .context("could not insert demo players")?
            .into_iter().map(|row| Ok((row.try_get::<String>("", "lfs_username")?.to_ascii_lowercase(), row.try_get::<i64>("", "id")?)))
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;
        let ids = rows.into_iter().collect::<HashMap<_, _>>();
        ensure!(ids.len() == chunk.len(), "demo usernames must be unique");
        player_ids.extend(
            chunk
                .iter()
                .map(|profile| ids[&profile.username.to_ascii_lowercase()]),
        );
        tracing::info!(
            "Player IDs resolved: {}/{}",
            player_ids.len(),
            profiles.len()
        );
    }

    let mut total = 0;
    let mut pending = Vec::with_capacity(BATCH_SIZE);
    let started = Instant::now();
    let mut last_report = started;
    tracing::info!("Generating and inserting hotlaps...");
    for (index, profile) in profiles.iter().enumerate() {
        for chart_index in generate::entries(&mut rng, profile, index, profiles.len(), charts.len())
        {
            let chart = &charts[chart_index];
            let (lap, date) = generate::lap(&mut rng, profile, baselines[chart_index], now);
            pending.push(json!({
                    "player": player_ids[index], "era": era.id,
                    "track": chart.track_id, "vehicle": chart.vehicle_id,
                    "lap": lap, "date": date.unix_timestamp(), "version": "0.0A",
                    "fingerprint": format!("demo:{}:{}:{}:{}", era.id, player_ids[index], chart.track_id, chart.vehicle_id),
                }));
            if pending.len() == BATCH_SIZE {
                total += insert_hotlaps(&transaction, &pending).await?;
                pending.clear();
                if last_report.elapsed() >= Duration::from_secs(2) {
                    tracing::info!(
                        "Hotlaps inserted: {total}; processing player {}/{}; elapsed {:.0}s",
                        index + 1,
                        profiles.len(),
                        started.elapsed().as_secs_f64()
                    );
                    last_report = Instant::now();
                }
            }
        }
    }
    if !pending.is_empty() {
        total += insert_hotlaps(&transaction, &pending).await?;
        pending.clear();
    }
    // These uncommitted inserts are invisible to autovacuum.
    tracing::info!(
        "Inserted {total} hotlaps for {} players in {:.1}s.",
        profiles.len(),
        started.elapsed().as_secs_f64()
    );
    progress(
        "Updating database statistics",
        transaction.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "ANALYZE player, hotlap, ranking_chart".to_owned(),
        )),
    )
    .await?;
    progress(
        "Rebuilding personal bests and chart positions",
        era.rebuild_personal_bests(&transaction),
    )
    .await?;
    progress(
        "Updating ranking statistics",
        transaction.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "ANALYZE hotlap_personal_best".to_owned(),
        )),
    )
    .await?;
    tracing::info!(
        era = era.slug,
        combinations = charts.len(),
        hotlaps = total,
        "demo era generated"
    );
    progress("Committing demo data", transaction.commit()).await?;
    progress(
        "Rebuilding badges (demo data is committed)",
        era.rebuild_badges(database),
    )
    .await
    .context(
        "demo data was committed, but badge rebuilding failed; run maintenance badges-refresh",
    )?;
    Ok(total)
}

async fn progress<T, E>(label: &str, task: impl Future<Output = Result<T, E>>) -> Result<T, E> {
    let started = Instant::now();
    tracing::info!("{label}...");
    let mut interval = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(5),
        Duration::from_secs(5),
    );
    tokio::pin!(task);
    loop {
        tokio::select! {
            result = &mut task => {
                let status = if result.is_ok() { "finished" } else { "failed" };
                tracing::info!("{label}: {status} after {:.1}s.", started.elapsed().as_secs_f64());
                return result;
            }
            _ = interval.tick() => {
                tracing::info!("{label}: still running ({:.0}s elapsed)...", started.elapsed().as_secs_f64());
            }
        }
    }
}

async fn create_era(database: &impl ConnectionTrait) -> anyhow::Result<Era> {
    crate::models::Track::sync(database).await?;
    crate::models::Vehicle::sync_builtin(database).await?;
    database
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "INSERT INTO era (slug, title, version_requirement, installation_id, open)
             VALUES ('demo', 'Demo', '=0.0A', 'demo', FALSE)
             ON CONFLICT (slug) DO UPDATE SET
                title = EXCLUDED.title,
                version_requirement = EXCLUDED.version_requirement,
                installation_id = EXCLUDED.installation_id,
                open = EXCLUDED.open"
                .to_owned(),
        ))
        .await?;
    crate::models::Era::validate_version_requirements(database).await?;
    let era = crate::models::Era::find_by_slug(database, "demo")
        .await?
        .context("demo era was not found")?;
    let ranking = database.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO ranking (era_id, slug, position, title, description,
            benchmark_percent, nation_max_points, nation_driver_limit, badge)
         VALUES ($1, 'all', 0, 'Demo ranking', 'Synthetic laps across standard tracks and vehicles.',
            103, 10, 3, '{\"label\":\"Demo\",\"qualification\":{\"type\":\"ranked\",\"limit\":3}}'::jsonb)
         ON CONFLICT (era_id, slug) DO UPDATE SET
            position = EXCLUDED.position,
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            benchmark_percent = EXCLUDED.benchmark_percent,
            nation_max_points = EXCLUDED.nation_max_points,
            nation_driver_limit = EXCLUDED.nation_driver_limit,
            badge = EXCLUDED.badge
         RETURNING id",
        [era.id.into()])).await?.context("demo ranking upsert returned no ID")?;
    let ranking_id = ranking.try_get::<i64>("", "id")?;
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM ranking_chart WHERE era_id = $1 AND ranking_id = $2",
            [era.id.into(), ranking_id.into()],
        ))
        .await?;
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO ranking_chart (era_id, ranking_id, position, track_id, vehicle_id)
         SELECT ranking.era_id, ranking.id,
            (row_number() OVER (ORDER BY track.id, vehicle.id) - 1)::integer, track.id, vehicle.id
         FROM ranking CROSS JOIN track CROSS JOIN vehicle
         WHERE ranking.era_id = $1 AND NOT track.open_configuration AND vehicle.available
           AND vehicle.kind = 'standard'",
            [era.id.into()],
        ))
        .await?;
    Ok(era)
}

async fn insert_hotlaps(database: &impl ConnectionTrait, rows: &[Value]) -> anyhow::Result<usize> {
    let result = database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO hotlap (
            player_id, era_id, track, vehicle, raw_vehicle_name,
            lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
            source, fingerprint, steering, player_flags, created_at, game_version, state
         ) SELECT player, era, track, vehicle, vehicle,
            lap, lap / 3, lap * 2 / 3, lap, 0,
            'demo', fingerprint, 'wheel', 0, to_timestamp(date), version, 'valid'
         FROM jsonb_to_recordset($1) AS h(
            player BIGINT, era BIGINT, track TEXT, vehicle TEXT, lap BIGINT,
            fingerprint TEXT, date BIGINT, version TEXT
         ) ON CONFLICT (source, fingerprint) DO NOTHING",
            [json!(rows).into()],
        ))
        .await?;
    Ok(usize::try_from(result.rows_affected())?)
}
