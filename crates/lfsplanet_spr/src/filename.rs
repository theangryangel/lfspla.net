//! Parsing for the conventional metadata-bearing SPR filename.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
    time::Duration,
};

use insim_core::{track::Track, vehicle::Vehicle};
use regex::Regex;

static CONVENTIONAL_FILENAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        // The lap time is minutes, then exactly two seconds digits and three
        // millisecond digits, so the regex splits the run rather than leaving
        // the caller to slice it back apart by byte offset.
        r"(?i)^(?P<username>.+)_(?P<track>[A-Z0-9]+)_(?P<vehicle>[A-Z0-9]+)_(?P<minutes>[0-9]+)(?P<seconds>[0-9]{2})(?P<millis>[0-9]{3})\.spr$",
    )
    .expect("the conventional SPR filename regex is valid")
});

/// Metadata encoded by LFS's conventional hotlap replay filename.
///
/// SPR files do not have to use this filename format. Parse a filename as this
/// type only when the metadata-bearing LFS convention is expected. Conversions
/// from [`Path`] and [`PathBuf`] inspect only the final filename component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalFilename {
    pub username: String,
    pub track: Track,
    pub vehicle: Vehicle,
    pub lap_time: Duration,
}

/// A malformed or unsupported conventional hotlap replay filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConventionalFilenameError {
    #[error("filename does not follow the LFS hotlap naming convention")]
    Format,
    #[error("filename contains an unknown track")]
    Track,
    #[error("filename contains an unknown vehicle")]
    Vehicle,
    #[error("filename contains an invalid lap time")]
    LapTime,
}

impl FromStr for ConventionalFilename {
    type Err = ConventionalFilenameError;

    fn from_str(filename: &str) -> Result<Self, Self::Err> {
        Self::try_from(filename)
    }
}

impl TryFrom<&str> for ConventionalFilename {
    type Error = ConventionalFilenameError;

    /// Interprets `<username>_<track>_<vehicle>_<lap-time>.spr` from the right.
    fn try_from(filename: &str) -> Result<Self, Self::Error> {
        let captures = CONVENTIONAL_FILENAME
            .captures(filename)
            .ok_or(ConventionalFilenameError::Format)?;
        let track = captures["track"]
            .parse::<Track>()
            .map_err(|_| ConventionalFilenameError::Track)?;
        let vehicle = captures["vehicle"]
            .parse::<Vehicle>()
            .expect("insim_core vehicle parsing is infallible");
        if vehicle == Vehicle::Unknown {
            return Err(ConventionalFilenameError::Vehicle);
        }
        let lap_time = lap_time(
            &captures["minutes"],
            &captures["seconds"],
            &captures["millis"],
        )?;

        Ok(Self {
            username: captures["username"].to_owned(),
            track,
            vehicle,
            lap_time,
        })
    }
}

impl TryFrom<&Path> for ConventionalFilename {
    type Error = ConventionalFilenameError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        path.file_name()
            .and_then(|filename| filename.to_str())
            .ok_or(ConventionalFilenameError::Format)?
            .try_into()
    }
}

impl TryFrom<PathBuf> for ConventionalFilename {
    type Error = ConventionalFilenameError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(path.as_path())
    }
}

/// Combines the three captured digit groups into a lap time.
fn lap_time(
    minutes: &str,
    seconds: &str,
    millis: &str,
) -> Result<Duration, ConventionalFilenameError> {
    let parse = |value: &str| {
        value
            .parse::<u64>()
            .map_err(|_| ConventionalFilenameError::LapTime)
    };
    let (minutes, seconds, millis) = (parse(minutes)?, parse(seconds)?, parse(millis)?);
    if seconds > 59 {
        return Err(ConventionalFilenameError::LapTime);
    }
    minutes
        .checked_mul(60_000)
        .and_then(|total| total.checked_add(seconds * 1_000))
        .and_then(|total| total.checked_add(millis))
        .map(Duration::from_millis)
        .ok_or(ConventionalFilenameError::LapTime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_conventional_filename_from_the_right() {
        let parsed = ConventionalFilename::try_from("ventspils_619_SO4R_RB4_158200.spr").unwrap();
        assert_eq!(parsed.username, "ventspils_619");
        assert_eq!(parsed.track, Track::So4r);
        assert_eq!(parsed.vehicle, Vehicle::Rb4);
        assert_eq!(parsed.lap_time, Duration::from_millis(118_200));
    }

    #[test]
    fn can_be_parsed_with_from_str() {
        let parsed = "driver_SO4R_RB4_158200.spr"
            .parse::<ConventionalFilename>()
            .unwrap();

        assert_eq!(parsed.username, "driver");
    }

    #[test]
    fn can_be_parsed_from_a_path() {
        let path = PathBuf::from("/replays/ventspils_619_SO4R_RB4_158200.spr");
        let parsed = ConventionalFilename::try_from(path.as_path()).unwrap();
        let parsed_from_owned_path = ConventionalFilename::try_from(path).unwrap();

        assert_eq!(parsed.username, "ventspils_619");
        assert_eq!(parsed, parsed_from_owned_path);
    }

    #[test]
    fn rejects_invalid_conventional_filename_metadata() {
        assert_eq!(
            ConventionalFilename::try_from("driver_NOPE_RB4_158200.spr"),
            Err(ConventionalFilenameError::Track)
        );
        assert_eq!(
            ConventionalFilename::try_from("driver_SO4R_NOPE_158200.spr"),
            Err(ConventionalFilenameError::Vehicle)
        );
        assert_eq!(
            ConventionalFilename::try_from("driver_SO4R_RB4_160000.spr"),
            Err(ConventionalFilenameError::LapTime)
        );
    }

    #[test]
    fn arbitrary_spr_names_are_not_conventional_filenames() {
        assert_eq!(
            ConventionalFilename::try_from("piranhot.spr"),
            Err(ConventionalFilenameError::Format)
        );
    }
}
