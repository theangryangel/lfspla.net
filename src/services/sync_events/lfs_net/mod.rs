//! LFS league/season discovery and source-to-advertisement mapping.

pub(super) mod fetch;
mod parse;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

use super::{Event, Report};

pub(super) async fn snapshot(
    only_league: Option<u64>,
    now: OffsetDateTime,
    known_leagues: &[u64],
) -> Result<Report> {
    let mut fetch = fetch::Fetcher::new()?;
    let mut report = Report::default();
    let calendar = parse::calendar(
        &fetch
            .get("/leagues/calendar")
            .await
            .context("fetch calendar")?,
    )
    .context("parse calendar")?;
    let mut league_ids = if let Some(id) = only_league {
        BTreeSet::from([id])
    } else {
        parse::leagues(
            &fetch
                .get("/leagues")
                .await
                .context("fetch league directory")?,
        )?
        .into_iter()
        .map(|league| league.external_id)
        .collect()
    };
    if only_league.is_none() {
        league_ids.extend(known_leagues.iter().copied());
        league_ids.extend(calendar.keys().map(|key| key.0));
    }
    for id in league_ids {
        let result = async {
            let html = fetch.get(&format!("/leagues/{id}")).await?;
            let (league, seasons) = parse::league(&html, id)?;
            let mut events = Vec::new();
            for season in &seasons {
                let html = fetch.get(&season.source_url).await?;
                events.extend(
                    parse::season(&html, season)
                        .with_context(|| format!("season {}", season.external_id))?,
                );
            }
            reconcile(&mut events, &calendar, id, now)?;
            Ok::<_, anyhow::Error>((league, seasons, events))
        }
        .await;
        match result {
            Ok((league, seasons, events)) => {
                report.leagues.push(league);
                report.seasons.extend(seasons);
                report.events.extend(events);
            }
            Err(error) => report.failures.push(format!("league {id}: {error:#}")),
        }
    }
    report
        .events
        .sort_by_key(|event| (event.starts_at, event.league_id, event.external_id));
    Ok(report)
}

/// Calendar history can outlive an active season; only upcoming entries must resolve.
fn reconcile(
    events: &mut Vec<Event>,
    calendar: &BTreeMap<(u64, u64, u64), OffsetDateTime>,
    league_id: u64,
    now: OffsetDateTime,
) -> Result<()> {
    for event in &mut *events {
        if let Some(start) = calendar.get(&(event.league_id, event.season_id, event.external_id)) {
            event.starts_at = *start;
        }
    }
    for (key, start) in calendar {
        if key.0 != league_id || *start < now {
            continue;
        }
        anyhow::ensure!(
            events
                .iter()
                .any(|event| (event.league_id, event.season_id, event.external_id) == *key),
            "calendar round {} is absent from active season schedules",
            key.2
        );
    }
    Ok(())
}
