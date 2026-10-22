//! Fetch and synchronise upstream event advertisements.

mod images;
mod lfs_net;
mod persist;
#[cfg(test)]
mod tests;

pub(crate) use persist::SyncReport;

use serde::Serialize;
use time::OffsetDateTime;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct League {
    pub external_id: u64,
    pub name: String,
    pub source_url: String,
    pub logo_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Season {
    pub external_id: u64,
    pub league_id: u64,
    pub name: String,
    pub source_url: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Event {
    pub external_id: u64,
    pub league_id: u64,
    pub season_id: u64,
    pub title: String,
    #[serde(with = "time::serde::rfc3339")]
    pub starts_at: OffsetDateTime,
    pub source_url: String,
    pub description: Option<String>,
    pub signup_url: Option<String>,
    pub track_label: Option<String>,
    pub cars_label: Option<String>,
}

/// A source preview, not a comparison with persisted records.
#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Report {
    pub leagues: Vec<League>,
    pub seasons: Vec<Season>,
    pub events: Vec<Event>,
    pub failures: Vec<String>,
}

/// Read public LFS pages. No OAuth credentials or database are required.
pub(crate) async fn preview(league: Option<u64>) -> anyhow::Result<Report> {
    let now = OffsetDateTime::now_utc();
    let mut report = lfs_net::snapshot(league, now, &[]).await?;
    report.events.retain(|event| event.starts_at >= now);
    Ok(report)
}

/// Serialize syncs before fetching, then apply each league in a short transaction.
pub(crate) async fn sync(
    database: &sea_orm::DatabaseConnection,
    league: Option<u64>,
    dry_run: bool,
    storage: &crate::settings::StorageSettings,
) -> anyhow::Result<SyncReport> {
    let _lock = persist::lock(database).await?;
    let now = OffsetDateTime::now_utc();
    let known = crate::models::league::Model::known_with_future_events(database, now).await?;
    let snapshot = lfs_net::snapshot(league, now, &known).await?;
    let logos = snapshot.leagues.clone();
    let mut report = persist::apply(database, snapshot, dry_run, now).await?;
    let store = if dry_run {
        None
    } else {
        match crate::storage::build(storage) {
            Ok(store) => Some(store),
            Err(error) => {
                report
                    .warnings
                    .push(format!("league logo storage: {error:#}"));
                return Ok(report);
            }
        }
    };
    images::sync(database, store.as_deref(), &logos, &mut report).await;
    Ok(report)
}
