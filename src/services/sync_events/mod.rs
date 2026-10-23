//! Fetch and synchronise upstream event advertisements.

mod images;
mod persist;
#[cfg(test)]
mod tests;

pub(crate) use persist::SyncReport;

#[cfg(test)]
use lfsplanet_lfs_leagues::{Event, Season};
pub(crate) use lfsplanet_lfs_leagues::{League, Report};
use time::OffsetDateTime;

/// Read public LFS pages. No OAuth credentials or database are required.
pub(crate) async fn preview(league: Option<u64>) -> anyhow::Result<Report> {
    let now = OffsetDateTime::now_utc();
    let mut report = lfsplanet_lfs_leagues::snapshot(league, now, &[]).await?;
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
    let snapshot = lfsplanet_lfs_leagues::snapshot(league, now, &known).await?;
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
