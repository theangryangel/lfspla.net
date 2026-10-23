//! Apply complete league snapshots, preserving local identity and moderation.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde::Serialize;
use time::OffsetDateTime;

use super::Report;
use crate::models::{event, league, season};

#[derive(Debug, Serialize)]
pub(crate) struct Applied {
    pub local_id: i64,
    pub external_id: String,
    pub change: event::source::Change,
}

#[derive(Debug, Serialize)]
pub(crate) struct SyncReport {
    pub dry_run: bool,
    pub leagues: Vec<Applied>,
    pub seasons: Vec<Applied>,
    pub events: Vec<Applied>,
    pub stale_events: Vec<i64>,
    pub failures: Vec<String>,
    pub images_updated: usize,
    pub warnings: Vec<String>,
}

/// Hold a dedicated connection for the session lock. Dropping it closes the
/// session, including on cancellation, so pooled connections never retain locks.
pub(super) async fn lock(database: &DatabaseConnection) -> Result<sqlx::PgConnection> {
    let mut connection = database
        .get_postgres_connection_pool()
        .acquire()
        .await?
        .detach();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(0x4c46_5350_4556_i64)
        .fetch_one(&mut connection)
        .await?;
    ensure!(acquired, "another event sync is already running");
    Ok(connection)
}

pub(super) async fn apply(
    database: &DatabaseConnection,
    snapshot: Report,
    dry_run: bool,
    now: OffsetDateTime,
) -> Result<SyncReport> {
    let mut report = SyncReport {
        dry_run,
        leagues: Vec::new(),
        seasons: Vec::new(),
        events: Vec::new(),
        stale_events: Vec::new(),
        failures: snapshot.failures.clone(),
        images_updated: 0,
        warnings: Vec::new(),
    };
    for source_league in &snapshot.leagues {
        let result = async {
            let transaction = database.begin().await?;
            let key = source_league.external_id.to_string();
            let (league, change) = league::Model::import(
                &transaction,
                &key,
                &source_league.name,
                &source_league.source_url,
                now,
            )
            .await?;
            let league_change = Applied {
                local_id: league.id,
                external_id: key,
                change,
            };
            let mut season_ids = BTreeMap::new();
            let mut seasons = Vec::new();
            for source in snapshot
                .seasons
                .iter()
                .filter(|season| season.league_id == source_league.external_id)
            {
                let key = format!("{}/{}", source.league_id, source.external_id);
                let (row, change) = season::Model::import(
                    &transaction,
                    &key,
                    league.id,
                    &source.name,
                    &source.source_url,
                    now,
                )
                .await?;
                ensure!(
                    season_ids.insert(source.external_id, row.id).is_none(),
                    "duplicate season in source snapshot"
                );
                seasons.push(Applied {
                    local_id: row.id,
                    external_id: key,
                    change,
                });
            }
            let mut events = Vec::new();
            let mut seen_events = std::collections::BTreeSet::new();
            for source in snapshot
                .events
                .iter()
                .filter(|event| event.league_id == source_league.external_id)
            {
                let season_id = *season_ids
                    .get(&source.season_id)
                    .context("event season missing from league snapshot")?;
                let key = format!(
                    "{}/{}/{}",
                    source.league_id, source.season_id, source.external_id
                );
                ensure!(
                    seen_events.insert(key.clone()),
                    "duplicate event in source snapshot"
                );
                let input = event::Import {
                    league_id: league.id,
                    season_id: Some(season_id),
                    title: source.title.clone(),
                    description: source.description.clone(),
                    starts_at: source.starts_at,
                    details_url: Some(source.source_url.clone()),
                    signup_url: source.signup_url.clone(),
                    track_label: source.track_label.clone(),
                    cars_label: source.cars_label.clone(),
                };
                let (row, change) =
                    event::Model::import(&transaction, &key, input, &source.source_url, now)
                        .await?;
                events.push(Applied {
                    local_id: row.id,
                    external_id: key,
                    change,
                });
            }
            let stale = event::Model::stale(&transaction, league.id, now).await?;
            if dry_run {
                transaction.rollback().await?;
            } else {
                transaction.commit().await?;
            }
            Ok::<_, anyhow::Error>((league_change, seasons, events, stale))
        }
        .await;
        match result {
            Ok((league, seasons, events, stale)) => {
                report.leagues.push(league);
                report.seasons.extend(seasons);
                report.events.extend(events);
                report.stale_events.extend(stale);
            }
            Err(error) => report.failures.push(format!(
                "league {} persistence: {error:#}",
                source_league.external_id
            )),
        }
    }
    Ok(report)
}
