//! Pure parsers for the public, server-rendered LFS league pages.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use scraper::{ElementRef, Html, Selector};
use time::{OffsetDateTime, PrimitiveDateTime, macros::format_description};

use super::super::{Event, League, Season};

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static CSS selector")
}

fn text(element: ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn required_text(element: ElementRef<'_>) -> Result<String> {
    let value = text(element);
    ensure!(
        !value.is_empty(),
        "missing text in {}",
        element.value().name()
    );
    Ok(value)
}

/// Accept only same-origin public league paths. Never follow arbitrary source links.
pub(super) fn source_url(path: &str) -> Result<url::Url> {
    let base = url::Url::parse("https://www.lfs.net/")?;
    let url = base.join(path)?;
    ensure!(
        url.origin() == base.origin() && url.username().is_empty() && url.password().is_none(),
        "unexpected source origin: {path}"
    );
    ensure!(url.query().is_none(), "unexpected source query");
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    let valid = match parts.as_slice() {
        ["leagues"] | ["leagues", "calendar"] => true,
        ["leagues", league] => league
            .split('-')
            .next()
            .is_some_and(|id| id.parse::<u64>().is_ok_and(|n| n > 0)),
        ["leagues", league, "season", season] => positive(league) && positive(season),
        ["leagues", league, "season", season, "round", round] => {
            positive(league) && positive(season) && positive(round)
        }
        _ => false,
    };
    ensure!(valid, "unexpected source path: {path}");
    Ok(url)
}

fn positive(value: &str) -> bool {
    value.parse::<u64>().is_ok_and(|n| n > 0)
}

fn ids(path: &str) -> Result<Vec<u64>> {
    let url = source_url(path)?;
    url.path()
        .trim_matches('/')
        .split('/')
        .skip(1)
        .step_by(2)
        .map(|part| {
            part.split('-')
                .next()
                .context("missing ID")?
                .parse()
                .context("invalid source ID")
        })
        .collect()
}

pub(super) fn leagues(html: &str) -> Result<Vec<League>> {
    let document = Html::parse_document(html);
    ensure!(
        document
            .select(&selector("#leaguesTableBody"))
            .next()
            .is_some(),
        "missing league directory table"
    );
    let mut leagues = BTreeMap::new();
    for link in document.select(&selector("#leaguesTableBody tr td:first-child a[href]")) {
        let path = link.attr("href").context("missing league href")?;
        let keys = ids(path)?;
        ensure!(keys.len() == 1, "expected a league link");
        let league = League {
            external_id: keys[0],
            name: required_text(link)?,
            logo_url: None,
            source_url: source_url(path)?.to_string(),
        };
        ensure!(
            leagues.insert(keys[0], league).is_none(),
            "duplicate league ID"
        );
    }
    ensure!(!leagues.is_empty(), "unexpectedly empty league directory");
    Ok(leagues.into_values().collect())
}

pub(super) fn league(html: &str, league_id: u64) -> Result<(League, Vec<Season>)> {
    let document = Html::parse_document(html);
    let heading = document
        .select(&selector("#BodyDiv h1"))
        .next()
        .context("missing league heading")?;
    let league = League {
        external_id: league_id,
        name: required_text(heading)?,
        logo_url: document
            .select(&selector("#BodyDiv img.leagueLogo[src]"))
            .next()
            .and_then(|image| image.attr("src"))
            .filter(|src| !src.is_empty())
            .map(str::to_owned),
        source_url: format!("https://www.lfs.net/leagues/{league_id}"),
    };
    let mut found = false;
    let mut seasons = BTreeMap::new();
    for article in document.select(&selector("#BodyDiv article")) {
        let Some(heading) = article.select(&selector("h1")).next() else {
            continue;
        };
        let heading = text(heading);
        if heading != "Current Season" && heading != "Past, Present and Upcoming Seasons" {
            continue;
        }
        found = true;
        ensure!(
            article.select(&selector("table")).next().is_some(),
            "missing season table"
        );
        for row in article.select(&selector("tr")) {
            let cells: Vec<_> = row.select(&selector("td")).collect();
            if cells.is_empty() {
                continue;
            }
            ensure!(cells.len() >= 5, "invalid season row");
            if heading == "Past, Present and Upcoming Seasons" {
                ensure!(cells.len() == 6, "missing active season column");
                match text(cells[5]).as_str() {
                    "Yes" => (),
                    "No" => continue,
                    _ => bail!("unknown season active flag"),
                }
            }
            let link = cells[0]
                .select(&selector("a[href]"))
                .next()
                .context("missing season link")?;
            let path = link.attr("href").context("missing season href")?;
            let keys = ids(path)?;
            ensure!(
                keys.len() == 2 && keys[0] == league_id,
                "season belongs to a different league"
            );
            seasons.insert(
                keys[1],
                Season {
                    external_id: keys[1],
                    league_id,
                    name: required_text(link)?,
                    source_url: source_url(path)?.to_string(),
                },
            );
        }
    }
    ensure!(found, "missing season sections");
    Ok((league, seasons.into_values().collect()))
}

fn date(value: &str) -> Result<OffsetDateTime> {
    Ok(PrimitiveDateTime::parse(
        value,
        format_description!("[year]-[month]-[day] [hour]:[minute]Z"),
    )?
    .assume_utc())
}

fn field<'a>(document: &'a Html, label: &str) -> Option<ElementRef<'a>> {
    document
        .select(&selector("#BodyDiv .left.bold.w_150"))
        .find(|node| text(*node) == label)?
        .next_siblings()
        .filter_map(ElementRef::wrap)
        .next()
}

pub(super) fn season(html: &str, season: &Season) -> Result<Vec<Event>> {
    let document = Html::parse_document(html);
    ensure!(
        field(&document, "Season").is_some_and(|node| text(node) == season.name),
        "season identity mismatch"
    );
    let parent = field(&document, "League").context("missing league identity")?;
    let parent = parent
        .select(&selector("a[href]"))
        .next()
        .context("missing league link")?;
    ensure!(
        ids(parent.attr("href").context("missing league href")?)? == [season.league_id],
        "league identity mismatch"
    );
    let description = field(&document, "Description")
        .map(text)
        .filter(|value| !value.is_empty());
    let signup_url = field(&document, "Signup")
        .and_then(|node| node.select(&selector("a[href]")).next())
        .map(|link| public_link(link.attr("href").context("missing signup href")?))
        .transpose()?;
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();
    for heading in document.select(&selector("#BodyDiv h2[id^=round]")) {
        let link = heading
            .select(&selector("a[href]"))
            .next()
            .context("missing round link")?;
        let path = link.attr("href").context("missing round href")?;
        let keys = ids(path)?;
        ensure!(
            keys.len() == 3 && keys[0] == season.league_id && keys[1] == season.external_id,
            "round parent mismatch"
        );
        ensure!(seen.insert(keys[2]), "duplicate round ID");
        let mut table = None;
        for sibling in heading.next_siblings().filter_map(ElementRef::wrap) {
            if sibling.value().name() == "h2" {
                break;
            }
            if sibling.value().name() == "table" {
                table = Some(sibling);
                break;
            }
        }
        let table = table.context("missing round schedule table")?;
        let headers: Vec<_> = table.select(&selector("th")).map(text).collect();
        ensure!(
            headers
                == [
                    "Nr.",
                    "Type",
                    "Duration",
                    "Track",
                    "Cars",
                    "Seats",
                    "Start Time"
                ],
            "unexpected schedule columns"
        );
        let mut times = Vec::new();
        let mut tracks = BTreeSet::new();
        let mut cars = BTreeSet::new();
        for row in table.select(&selector("tr")) {
            let cells: Vec<_> = row.select(&selector("td")).collect();
            if cells.is_empty() {
                continue;
            }
            ensure!(cells.len() == 7, "invalid schedule row");
            let time = cells[6]
                .select(&selector("time[datetime]"))
                .next()
                .context("missing session date")?;
            times.push(date(time.attr("datetime").context("missing datetime")?)?);
            tracks.insert(text(cells[3]));
            cars.insert(text(cells[4]));
        }
        let starts_at = times.into_iter().min().context("empty round schedule")?;
        events.push(Event {
            external_id: keys[2],
            league_id: season.league_id,
            season_id: season.external_id,
            title: required_text(link)?,
            starts_at,
            source_url: source_url(path)?.to_string(),
            description: description.clone(),
            signup_url: signup_url.clone(),
            track_label: consistent_label(tracks),
            cars_label: consistent_label(cars),
        });
    }
    ensure!(!events.is_empty(), "missing round schedules");
    Ok(events)
}

fn consistent_label(values: BTreeSet<String>) -> Option<String> {
    if values.len() != 1 {
        return None;
    }
    values
        .into_iter()
        .next()
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("unspecified"))
}

fn public_link(value: &str) -> Result<String> {
    let url = url::Url::parse("https://www.lfs.net/")?.join(value)?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "invalid public link"
    );
    Ok(url.to_string())
}

pub(super) fn calendar(html: &str) -> Result<BTreeMap<(u64, u64, u64), OffsetDateTime>> {
    let document = Html::parse_document(html);
    ensure!(
        document
            .select(&selector("table.calendarBig"))
            .next()
            .is_some(),
        "missing calendar table"
    );
    let mut events = BTreeMap::new();
    for aside in document.select(&selector("table.calendarBig aside")) {
        let onclick = aside
            .attr("onclick")
            .context("missing calendar event destination")?;
        let path = onclick
            .strip_prefix("location.href='")
            .and_then(|value| value.strip_suffix('\''))
            .context("unexpected calendar destination")?;
        let keys = ids(path)?;
        ensure!(keys.len() == 3, "invalid calendar round link");
        let time = aside
            .select(&selector("time[datetime]"))
            .next()
            .context("missing advertised start")?;
        ensure!(
            events
                .insert(
                    (keys[0], keys[1], keys[2]),
                    date(time.attr("datetime").context("missing datetime")?)?
                )
                .is_none(),
            "duplicate calendar event"
        );
    }
    Ok(events)
}
