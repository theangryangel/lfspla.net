//! LFSWorld v1 hotlap CSV importer.

mod persist;

use std::{
    collections::{BTreeMap, HashMap, HashSet, btree_map::Entry},
    fs::File,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, bail, ensure};
use celes::Country;
use insim_core::game_version::GameVersion;
use lfsplanet_spr::{ConventionalFilename, PlayerFlags};
use sea_orm::EntityTrait;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::{
    cli::era::{self as era_definitions, EraDefinition},
    models::hotlaps::{self, SteeringInput},
    settings::Settings,
    startup,
};

use crate::cli::Args;
use persist::persist;

const CURRENT_HEADERS: [&str; 20] = [
    "itemnr",
    "user",
    "id",
    "track",
    "car",
    "split1",
    "split2",
    "split3",
    "split4",
    "laptime",
    "bestlapnr",
    "spr",
    "flags",
    "steering",
    "abs",
    "date",
    "country",
    "version",
    "raf_result",
    "raf_result2",
];

const P_Q_HEADERS: [&str; 15] = [
    "itemnr", "user", "id", "track", "car", "split1", "split2", "split3", "split4", "laptime",
    "spr", "flags", "steering", "date", "country",
];

const R_X_HEADERS: [&str; 17] = [
    "itemnr",
    "user",
    "id",
    "track",
    "car",
    "split1",
    "split2",
    "split3",
    "split4",
    "laptime",
    "bestlapnr",
    "spr",
    "flags",
    "steering",
    "date",
    "country",
    "version",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExportFormat {
    Pq,
    Rx,
    Current,
}

impl ExportFormat {
    fn default_version(self) -> Option<&'static str> {
        match self {
            Self::Pq => Some("0.5P"),
            Self::Rx => Some("0.5T"),
            Self::Current => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvHotlap {
    itemnr: i64,
    user: String,
    id: i64,
    #[serde(rename = "track")]
    _legacy_track: String,
    #[serde(rename = "car")]
    _legacy_vehicle: String,
    split1: i64,
    split2: i64,
    split3: i64,
    split4: i64,
    laptime: i64,
    #[serde(default, rename = "bestlapnr")]
    _best_lap_number: Option<i64>,
    spr: String,
    flags: i32,
    steering: String,
    #[serde(default)]
    abs: Option<i32>,
    date: i64,
    country: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "raf_result")]
    _legacy_raf_result: Option<i32>,
    #[serde(default, rename = "raf_result2")]
    _legacy_raf_result2: Option<i32>,
}

#[derive(Debug)]
struct ImportedHotlap {
    fingerprint: String,
    lfs_username: String,
    era_slug: String,
    era_id: i64,
    track: String,
    vehicle: String,
    lap_time_ms: i64,
    split_times_ms: [i64; 4],
    original_filename: String,
    /// Materialised for the chart query's controller filter; every other
    /// control setting is derived from `player_flags` when the row is read.
    steering: SteeringInput,
    abs_enabled: Option<bool>,
    player_flags: u16,
    created_at: OffsetDateTime,
    game_version: String,
}

#[derive(Debug)]
struct ImportedPlayer {
    lfsworld_id: i64,
    country_code: Option<String>,
    created_at: OffsetDateTime,
}

#[derive(Debug)]
struct ImportData {
    players: BTreeMap<String, ImportedPlayer>,
    hotlaps: Vec<ImportedHotlap>,
}

/// Imports an LFSWorld v1 hotlap CSV export.
pub(super) async fn run(args: &Args, hotlaps_csv: &[PathBuf]) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = startup::connect(&settings.database, 2).await?;
    let eras = era_definitions::list_definitions(&database).await?;
    let mut data = load(hotlaps_csv, &eras)?;
    let persisted_eras = crate::models::eras::EraEntity::find()
        .all(&database)
        .await?;
    let era_ids = persisted_eras
        .iter()
        .map(|era| (era.slug.as_str(), era.id))
        .collect::<HashMap<_, _>>();
    for hotlap in &mut data.hotlaps {
        let slug = hotlap.era_slug.as_str();
        hotlap.era_id = *era_ids
            .get(slug)
            .with_context(|| format!("era {slug:?} is not installed"))?;
    }

    tracing::info!(
        files = hotlaps_csv.len(),
        players = data.players.len(),
        hotlaps = data.hotlaps.len(),
        "hotlap CSV passed preflight validation"
    );

    let inserted = persist(&database, &data).await?;
    tracing::info!(
        players = data.players.len(),
        hotlaps_read = data.hotlaps.len(),
        hotlaps_inserted = inserted,
        hotlaps_skipped = u64::try_from(data.hotlaps.len())
            .unwrap_or(u64::MAX)
            .saturating_sub(inserted),
        "LFSWorld v1 hotlap import complete"
    );

    // Imported laps are published without passing through validation, so
    // nothing else rebuilds the badges they change. Unchanged rows change no
    // standings, so an idempotent re-import rebuilds nothing.
    if inserted > 0 {
        let touched = data
            .hotlaps
            .iter()
            .map(|hotlap| hotlap.era_id)
            .collect::<HashSet<_>>();
        for era in persisted_eras
            .iter()
            .filter(|era| touched.contains(&era.id))
        {
            era.rebuild_badges(&database).await?;
        }
    }
    Ok(())
}

fn load<P: AsRef<Path>>(paths: &[P], eras: &[EraDefinition]) -> anyhow::Result<ImportData> {
    let mut players = BTreeMap::new();
    let mut user_by_lfsworld_id = HashMap::new();
    let mut fingerprints = HashSet::new();
    let mut hotlaps = Vec::new();

    for path in paths {
        let path = path.as_ref();
        let file = File::open(path)
            .with_context(|| format!("failed to open hotlap CSV {}", path.display()))?;
        let mut reader = csv::Reader::from_reader(file);
        let headers = reader
            .headers()
            .with_context(|| format!("failed to read CSV headers from {}", path.display()))?;
        let format = if headers.iter().eq(P_Q_HEADERS) {
            ExportFormat::Pq
        } else if headers.iter().eq(R_X_HEADERS) {
            ExportFormat::Rx
        } else if headers.iter().eq(CURRENT_HEADERS) {
            ExportFormat::Current
        } else {
            bail!(
                "unexpected hotlap CSV headers in {}: found {}",
                path.display(),
                headers.iter().collect::<Vec<_>>().join(",")
            );
        };

        for (index, result) in reader.deserialize::<CsvHotlap>().enumerate() {
            let line = index + 2;
            let row = result
                .with_context(|| format!("failed to parse CSV row {}:{}", path.display(), line))?;
            let mut hotlap = parse_row(&row, format, eras)
                .with_context(|| format!("invalid CSV row {}:{}", path.display(), line))?;
            ensure!(
                fingerprints.insert(hotlap.fingerprint.clone()),
                "duplicate itemnr {} at {}:{}",
                row.itemnr,
                path.display(),
                line
            );
            match user_by_lfsworld_id.entry(row.id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(row.user.clone());
                }
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let existing = entry.get();
                    ensure!(
                        existing.eq_ignore_ascii_case(&row.user),
                        "LFSWorld player id {} refers to both {existing:?} and {:?}",
                        row.id,
                        row.user
                    );
                    // Early exports did not consistently preserve username case.
                    hotlap.lfs_username.clone_from(existing);
                }
            }

            let country_code = country_code(&row.country)
                .with_context(|| {
                    format!(
                        "unknown country {:?} at {}:{}",
                        row.country,
                        path.display(),
                        line
                    )
                })?
                .map(|country| country.alpha2.to_owned());
            match players.entry(hotlap.lfs_username.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(ImportedPlayer {
                        lfsworld_id: row.id,
                        country_code,
                        created_at: hotlap.created_at,
                    });
                }
                Entry::Occupied(mut entry) => {
                    let player = entry.get_mut();
                    ensure!(
                        player.lfsworld_id == row.id,
                        "LFSWorld username {:?} has multiple player ids",
                        hotlap.lfs_username
                    );
                    ensure!(
                        player.country_code == country_code,
                        "LFSWorld username {:?} has multiple countries",
                        hotlap.lfs_username
                    );
                    player.created_at = player.created_at.min(hotlap.created_at);
                }
            }
            hotlaps.push(hotlap);
        }
    }
    ensure!(!hotlaps.is_empty(), "the hotlap CSV contains no records");

    Ok(ImportData { players, hotlaps })
}

#[allow(clippy::too_many_lines)]
fn parse_row(
    row: &CsvHotlap,
    format: ExportFormat,
    eras: &[EraDefinition],
) -> anyhow::Result<ImportedHotlap> {
    ensure!(row.itemnr > 0, "itemnr must be positive");
    ensure!(row.id > 0, "player id must be positive");
    ensure!(!row.user.is_empty(), "username must not be empty");
    ensure!(row.laptime > 0, "lap time must be positive");
    let split_times_ms = [row.split1, row.split2, row.split3, row.split4];
    ensure!(
        split_times_ms.iter().all(|split| *split >= 0),
        "split times must not be negative"
    );
    ensure!(
        split_times_ms.into_iter().max() == Some(row.laptime),
        "lap time does not equal the last cumulative split"
    );

    let normalized_filename;
    let filename_text = if format == ExportFormat::Current {
        row.spr.as_str()
    } else {
        // A few archived names have a truncated or otherwise mistyped time.
        // The CSV lap time is authoritative; retain and validate the filename's
        // username/track/vehicle while reconstructing only its time suffix.
        let (prefix, extension) = row
            .spr
            .rsplit_once('_')
            .context("archived SPR filename has no lap-time separator")?;
        ensure!(
            // The uploader and conventional filename parser both accept any casing, so
            // this guard must not be the one thing that rejects `.SPR`.
            extension.to_ascii_lowercase().ends_with(".spr"),
            "archived SPR filename has no .spr extension"
        );
        let minutes = row.laptime / 60_000;
        let seconds = row.laptime % 60_000 / 1_000;
        let milliseconds = row.laptime % 1_000;
        normalized_filename = format!("{prefix}_{minutes}{seconds:02}{milliseconds:03}.spr");
        &normalized_filename
    };
    let filename = ConventionalFilename::try_from(filename_text)
        .with_context(|| format!("invalid SPR filename {:?}", row.spr))?;
    let filename_time =
        i64::try_from(filename.lap_time.as_millis()).context("filename lap time exceeds BIGINT")?;
    ensure!(
        filename_time == row.laptime,
        "filename lap time {filename_time} does not match CSV lap time {}",
        row.laptime
    );

    let version_text = row
        .version
        .as_deref()
        .unwrap_or_default()
        .trim_end_matches('\0');
    let version_text = if version_text.is_empty() {
        format
            .default_version()
            .context("game version is missing from a current-era export")?
    } else {
        version_text
    };
    let game_version = version_text
        .parse::<GameVersion>()
        .with_context(|| format!("invalid game version {version_text:?}"))?;
    let era = eras
        .iter()
        .find(|era| era.accepts_replay_version(&game_version))
        .with_context(|| format!("game version {version_text} is not covered by an era"))?;
    ensure!(
        era.allows_combination(filename.track, filename.vehicle),
        "combination {}/{} is not eligible for era {}",
        filename.track,
        filename.vehicle,
        era.id
    );
    let csv_steering = match row.steering.as_str() {
        "w" => SteeringInput::Wheel,
        "m" => SteeringInput::Mouse,
        "kn" => SteeringInput::Keyboard,
        "ks" => SteeringInput::KeyboardStabilised,
        value => bail!("unknown steering value {value:?}"),
    };
    let flag_bits = u16::try_from(row.flags).context("player flags exceed 16 bits")?;
    let player_flags = if format == ExportFormat::Current {
        PlayerFlags::from_bits(flag_bits).context("unknown player flag bits")?
    } else {
        // P-X used older player-flag layouts. Preserve the bits understood by
        // the current protocol model and ignore retired bits; the export's
        // dedicated steering column is still checked below.
        PlayerFlags::from_bits_retain(flag_bits)
    };
    let steering = SteeringInput::from_player_flags(player_flags);
    ensure!(
        steering == csv_steering,
        "steering value disagrees with player flags"
    );
    // The P-Q and R-X exports have no `abs` column at all, so an absent value is
    // unknown rather than off. A present zero still has to clear the version
    // gate, because LFS reported zero before it could report ABS.
    let abs_enabled = match row.abs {
        None => None,
        Some(0) => hotlaps::resolve_abs(false, &game_version),
        Some(1) => Some(true),
        Some(value) => bail!("unknown ABS value {value}"),
    };
    let created_at = OffsetDateTime::from_unix_timestamp(row.date)
        .with_context(|| format!("invalid Unix timestamp {}", row.date))?;

    Ok(ImportedHotlap {
        // LFSWorld restarted item numbers when the incompatible hotlap table
        // was archived, so the era is part of the durable import identity.
        fingerprint: format!("{}:{}", era.id, row.itemnr),
        lfs_username: row.user.clone(),
        era_slug: era.id.clone(),
        era_id: 0,
        track: filename.track.to_string(),
        vehicle: filename.vehicle.to_string(),
        lap_time_ms: row.laptime,
        split_times_ms,
        original_filename: row.spr.clone(),
        steering,
        abs_enabled,
        player_flags: flag_bits,
        created_at,
        game_version: game_version.to_string(),
    })
}

fn country_code(value: &str) -> anyhow::Result<Option<Country>> {
    if value.is_empty() || value == "Other" {
        return Ok(None);
    }

    // `celes` accepts ISO codes, canonical names, and aliases after spaces and
    // punctuation have been stripped. This covers mappings such as Scotland
    // to GB; the few unsupported historical names fall through below.
    let normalized = value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    if let Ok(country) = Country::from_str(&normalized) {
        return Ok(Some(country));
    }

    // Historical LFSWorld exports contain a few obsolete spellings and
    // geopolitical names which are normalised to iso3166.
    // This isn't a political statement.
    let alpha2 = match value {
        "Cocos (Keeling) Islands" => "CC",
        "France, metro" => "FR",
        "Iran, Islamic Republic of" => "IR",
        "Kazakstan" => "KZ",
        "Korea" | "Korea, Republic of" => "KR",
        "Korea, Democratic People's Republic of" => "KP",
        "Moldova, Republic of" => "MD",
        "Serbia and Montenegro" | "Yugoslavia" => "RS",
        "Slovak Republic" => "SK",
        "Taiwan Region" => "TW",
        "Wales" => "GB",
        _ => bail!("unrecognised LFSWorld country name"),
    };
    Ok(Some(
        Country::from_alpha2(alpha2).map_err(anyhow::Error::msg)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_country_names_and_deliberate_non_iso_values() {
        assert_eq!(country_code("Germany").unwrap(), Some(Country::germany()));
        assert_eq!(
            country_code("England").unwrap(),
            Some(Country::the_united_kingdom_of_great_britain_and_northern_ireland())
        );
        assert_eq!(
            country_code("Scotland").unwrap(),
            Some(Country::the_united_kingdom_of_great_britain_and_northern_ireland())
        );
        assert_eq!(
            country_code("Czech Republic").unwrap(),
            Some(Country::czechia())
        );
        assert_eq!(country_code("Kosovo").unwrap(), Some(Country::kosovo()));
        assert_eq!(country_code("Other").unwrap(), None);
        assert!(country_code("Definitely not a country").is_err());
    }
}
