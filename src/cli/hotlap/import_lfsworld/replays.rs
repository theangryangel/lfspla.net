//! Optional filesystem replay lookup and incremental attachment of imported laps.

use super::{ImportData, ImportedHotlap, persist::SOURCE};
use crate::{milliseconds::Milliseconds, models::attachment::Model as Attachment};
use anyhow::{Context, ensure};
use lfsplanet_spr::SprHeader;
use object_store::ObjectStore;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct ReplayIndex {
    paths: HashMap<String, HashSet<PathBuf>>,
}

impl ReplayIndex {
    pub(super) fn load(root: &Path) -> anyhow::Result<Self> {
        ensure!(
            root.is_dir(),
            "SPR directory {} is not a directory",
            root.display()
        );
        let mut index = Self::default();
        let mut pending = vec![root.to_path_buf()];
        let mut files = 0;
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory)
                .with_context(|| format!("failed to scan SPR directory {}", directory.display()))?
            {
                let entry = entry?;
                let kind = entry.file_type()?;
                // Do not follow symlinks: this also prevents recursive directory cycles.
                if kind.is_dir() {
                    pending.push(entry.path());
                } else if kind.is_file() {
                    let path = entry.path();
                    for name in filename_keys(&entry.file_name()) {
                        if name.ends_with(".spr") {
                            index.paths.entry(name).or_default().insert(path.clone());
                        }
                    }
                    files += 1;
                }
            }
        }
        tracing::info!(files, directory = %root.display(), "indexed local replay files");
        Ok(index)
    }

    fn find(&self, filename: &str) -> anyhow::Result<Option<&Path>> {
        let Some(paths) = self.paths.get(&filename.to_lowercase()) else {
            return Ok(None);
        };
        ensure!(paths.len() == 1, "multiple local files match {filename:?}");
        Ok(paths.iter().next().map(PathBuf::as_path))
    }

    /// Each reference commits separately so interrupted runs can safely resume.
    pub(super) async fn attach(
        &self,
        database: &DatabaseConnection,
        store: &dyn ObjectStore,
        data: &ImportData,
    ) -> anyhow::Result<()> {
        let fingerprints = data
            .hotlaps
            .iter()
            .map(|lap| lap.fingerprint.clone())
            .collect::<Vec<_>>();
        let missing: HashMap<String, i64> = sea_orm::sqlx::query_as::<_, (String, i64)>(
            "SELECT fingerprint, id FROM hotlap WHERE source = $1 AND fingerprint = ANY($2) AND spr_attachment_id IS NULL",
        )
        .bind(SOURCE)
        .bind(&fingerprints)
        .fetch_all(database.get_postgres_connection_pool())
        .await?
        .into_iter()
        .collect();
        let mut attached = 0_u64;
        let mut not_found = 0_u64;
        let mut skipped = 0_u64;
        for lap in &data.hotlaps {
            let Some(id) = missing.get(&lap.fingerprint) else {
                continue;
            };
            let bytes = match self.read(lap) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    not_found += 1;
                    continue;
                }
                Err(error) => {
                    skipped += 1;
                    tracing::warn!(filename = %lap.spr_filename, %error, "skipping local SPR attachment");
                    continue;
                }
            };
            let transaction = database.begin().await?;
            let attachment = Attachment::store(
                &transaction,
                store,
                bytes,
                "spr",
                "application/octet-stream",
                Some(&lap.spr_filename),
            )
            .await
            .with_context(|| format!("failed to store SPR {:?}", lap.spr_filename))?;
            let updated = transaction.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE hotlap SET spr_attachment_id = $1 WHERE id = $2 AND spr_attachment_id IS NULL",
                [attachment.id.into(), (*id).into()],
            )).await?.rows_affected();
            if updated == 0 {
                // A concurrent importer may have supplied the attachment already.
                transaction.rollback().await?;
            } else {
                transaction.commit().await?;
                attached += 1;
                if attached.is_multiple_of(1_000) {
                    tracing::info!(attached, "attaching imported SPR files");
                }
            }
        }
        tracing::info!(
            attached,
            not_found,
            skipped,
            already_attached = data.hotlaps.len() - missing.len(),
            "imported SPR attachment pass complete"
        );
        Ok(())
    }

    fn read(&self, lap: &ImportedHotlap) -> anyhow::Result<Option<Vec<u8>>> {
        let Some(path) = self.find(&lap.spr_filename)? else {
            return Ok(None);
        };
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let header = SprHeader::read(Cursor::new(&bytes)).context("invalid SPR header")?;
        validate_header(&header, lap)?;
        Ok(Some(bytes))
    }
}

fn filename_keys(name: &OsStr) -> Vec<String> {
    if let Some(name) = name.to_str() {
        return vec![name.to_lowercase()];
    }
    // On Unix, archived Windows filenames can contain bytes that are not UTF-8.
    // Retain the actual PathBuf for opening the file; only decode the lookup key.
    let raw = name.as_encoded_bytes();
    let (windows, _, _) = encoding_rs::WINDOWS_1252.decode(raw);
    let latin1: String = raw.iter().map(|byte| char::from(*byte)).collect();
    vec![windows.to_lowercase(), latin1.to_lowercase()]
}

fn validate_header(header: &SprHeader, lap: &ImportedHotlap) -> anyhow::Result<()> {
    ensure!(
        header.user_name.eq_ignore_ascii_case(&lap.lfs_username),
        "SPR username does not match CSV"
    );
    ensure!(
        header.track.to_string() == lap.track,
        "SPR track does not match CSV"
    );
    let vehicle = crate::models::vehicle::STANDARD_VEHICLES
        .iter()
        .find(|(code, name)| {
            code.to_string()
                .eq_ignore_ascii_case(header.car_name.as_str())
                || name.eq_ignore_ascii_case(header.car_name.as_str())
        });
    ensure!(
        vehicle.is_some_and(|(code, _)| code.to_string() == lap.vehicle),
        "SPR vehicle does not match CSV"
    );
    let time = header
        .split_count
        .checked_sub(1)
        .and_then(|index| header.split_times.get(usize::from(index)))
        .context("SPR has no lap time")?;
    ensure!(
        Milliseconds::try_from(*time)? == lap.lap_time_ms,
        "SPR lap time does not match CSV"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::cli::hotlap::import_lfsworld) fn replay_bytes(
        username: &str,
        track: &str,
        vehicle: &str,
    ) -> Vec<u8> {
        let mut bytes = vec![0; lfsplanet_spr::HEADER_LEN];
        bytes[..6].copy_from_slice(b"LFSSPR");
        bytes[16..20].copy_from_slice(b"0.5P");
        bytes[24..24 + track.len()].copy_from_slice(track.as_bytes());
        bytes[64..64 + username.len()].copy_from_slice(username.as_bytes());
        bytes[96..96 + vehicle.len()].copy_from_slice(vehicle.as_bytes());
        bytes[128] = 1; // configuration
        bytes[135] = 1; // split_count
        bytes[136] = 1; // split_1: one minute (minutes, seconds, hundredths, thousandths)
        bytes
    }

    fn lap() -> ImportedHotlap {
        ImportedHotlap {
            fingerprint: "2006-01-01:1".into(),
            spr_filename: "driver_BL1_XFG_100000.spr".into(),
            lfs_username: "driver".into(),
            era_slug: "2006-01-01".parse().unwrap(),
            era_id: 1,
            track: "BL1".into(),
            vehicle: "XFG".into(),
            lap_time_ms: Milliseconds::from_millis(60_000),
            split_times_ms: [Milliseconds::ZERO; 4],
            steering: crate::models::hotlap::SteeringInput::Wheel,
            abs_enabled: None,
            player_flags: 0,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            game_version: "0.5P".into(),
        }
    }

    #[test]
    fn matching_headers_are_required_before_attaching() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let lap = lap();
        let path = root.path().join(&lap.spr_filename);
        let bytes = replay_bytes("DRIVER", "BL1", "XF GTI");
        fs::write(&path, &bytes)?;
        let index = ReplayIndex::load(root.path())?;
        assert_eq!(index.read(&lap)?, Some(bytes.clone()));
        let mut header = SprHeader::read(Cursor::new(&bytes))?;
        header.user_name = "someone else".into();
        assert!(validate_header(&header, &lap).is_err());
        header.user_name = "driver".into();
        header.track = "BL2".parse()?;
        assert!(validate_header(&header, &lap).is_err());
        header.track = "BL1".parse()?;
        header.car_name = "XR GT".into();
        assert!(validate_header(&header, &lap).is_err());
        header.car_name = "XFG".into();
        assert!(validate_header(&header, &lap).is_ok());
        header.split_times[0] = std::time::Duration::from_secs(61);
        assert!(validate_header(&header, &lap).is_err());
        header.split_count = 0;
        assert!(validate_header(&header, &lap).is_err());
        fs::write(&path, b"not an SPR")?;
        assert!(index.read(&lap).is_err());
        fs::remove_file(&path)?;
        assert!(index.read(&lap).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires locally extracted tmp/sprs and tmp/s2hotlaps.csv"]
    fn audit_local_archive_headers() -> anyhow::Result<()> {
        let index = ReplayIndex::load(Path::new("tmp/sprs"))?;
        let mut reader = csv::Reader::from_path("tmp/s2hotlaps.csv")?;
        let mut matched = 0;
        let mut missing = 0;
        let mut rejected = 0;
        for row in reader.deserialize::<super::super::CsvHotlap>() {
            let row = row?;
            let filename = lfsplanet_spr::ConventionalFilename::try_from(row.spr.as_str())?;
            let mut lap = lap();
            lap.spr_filename = row.spr;
            lap.lfs_username = row.user;
            lap.track = filename.track.to_string();
            lap.vehicle = filename.vehicle.to_string();
            lap.lap_time_ms = row.laptime;
            let result = index.find(&lap.spr_filename).and_then(|path| {
                path.map(|path| {
                    let header = SprHeader::from_path(path)?;
                    validate_header(&header, &lap)
                })
                .transpose()
            });
            match result {
                Ok(Some(())) => matched += 1,
                Ok(None) => missing += 1,
                Err(error) => {
                    rejected += 1;
                    eprintln!("{}: {error:#}", lap.spr_filename);
                }
            }
        }
        eprintln!("matched={matched}, missing={missing}, rejected={rejected}");
        ensure!(matched > 0, "no matching replays");
        Ok(())
    }

    #[test]
    fn recursive_lookup_is_case_insensitive_and_rejects_ambiguity() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let nested = root.path().join("track/car");
        fs::create_dir_all(&nested)?;
        let path = nested.join("Driver_BL1_XFG_100000.SPR");
        fs::write(&path, [])?;
        let index = ReplayIndex::load(root.path())?;
        assert_eq!(
            index.find("driver_bl1_xfg_100000.spr")?,
            Some(path.as_path())
        );
        assert_eq!(index.find("missing.spr")?, None);
        fs::write(root.path().join("driver_bl1_xfg_100000.spr"), [])?;
        assert!(
            ReplayIndex::load(root.path())?
                .find("driver_bl1_xfg_100000.spr")
                .is_err()
        );
        assert!(ReplayIndex::load(&root.path().join("absent")).is_err());
        Ok(())
    }

    #[test]
    fn utf8_names_are_preserved() {
        assert_eq!(
            filename_keys(OsStr::new("Jürgen_BL1_XFG_100000.spr")),
            ["jürgen_bl1_xfg_100000.spr"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn legacy_names_open_using_the_original_path() -> anyhow::Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let root = tempfile::tempdir()?;
        let name = OsStr::from_bytes(b"J\xfcrgen_BL1_XFG_100000.spr");
        let path = root.path().join(name);
        fs::write(&path, b"original bytes")?;
        let index = ReplayIndex::load(root.path())?;
        let found = index.find("Jürgen_BL1_XFG_100000.spr")?.unwrap();
        assert_eq!(fs::read(found)?, b"original bytes");
        let keys = filename_keys(OsStr::from_bytes(b"driver\x80.spr"));
        assert!(keys.contains(&"driver€.spr".to_owned()));
        assert!(keys.contains(&"driver\u{80}.spr".to_owned()));
        Ok(())
    }
}
