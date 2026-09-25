use std::{fs, io::Read, num::NonZeroU8, path::Path, str::FromStr, time::Duration};

use bytes::Bytes;

use crate::{Error, PlayerFlags, RaceLaps, SprVehicle};
use insim_core::{
    Decode, DecodeContext, DecodeError, DecodeErrorKind, game_version::GameVersion, track::Track,
    wind::Wind,
};

/// The number of bytes in an SPR file header, including its magic signature.
pub const HEADER_LEN: usize = 192;

const MAGIC: &[u8; 6] = b"LFSSPR";
const ENCODED_HEADER_LEN: usize = HEADER_LEN - MAGIC.len();

/// The two file-version bytes stored at offsets 6 and 7.
///
/// The SPR specification says readers should not use these bytes to reject a
/// file, so they are retained as metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersion {
    pub high: u8,
    pub low: u8,
}

/// The replay's skill level as encoded by LFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SkillLevel {
    Level0 = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
}

impl Decode for SkillLevel {
    const PRIMITIVE: bool = true;

    fn decode(ctx: &mut DecodeContext) -> Result<Self, DecodeError> {
        match u8::decode(ctx)? {
            0 => Ok(Self::Level0),
            1 => Ok(Self::Level1),
            2 => Ok(Self::Level2),
            3 => Ok(Self::Level3),
            4 => Ok(Self::Level4),
            found => Err(no_variant(found)),
        }
    }
}

/// Whether and how hotlap validation was enabled for the replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HotlapMode {
    Off = 0,
    Enabled = 1,
    Custom = 2,
    Invalid = 3,
}

impl Decode for HotlapMode {
    const PRIMITIVE: bool = true;

    fn decode(ctx: &mut DecodeContext) -> Result<Self, DecodeError> {
        match u8::decode(ctx)? {
            0 => Ok(Self::Off),
            1 => Ok(Self::Enabled),
            2 => Ok(Self::Custom),
            3 => Ok(Self::Invalid),
            found => Err(no_variant(found)),
        }
    }
}

/// Strongly typed metadata from the 192-byte SPR header.
#[derive(Debug, Clone)]
pub struct SprHeader {
    pub file_version: FileVersion,
    pub spr_version: u8,
    pub qualifying_time: Duration,
    pub race_laps: RaceLaps,
    pub skill: SkillLevel,
    pub wind: Wind,
    pub hotlap_mode: HotlapMode,
    pub lfs_version: GameVersion,
    pub track: Track,
    pub added_mass: u8,
    pub intake_restriction: u8,
    pub abs_enabled: bool,
    pub track_name: String,
    pub user_name: String,
    pub car_name: SprVehicle,
    pub configuration: NonZeroU8,
    pub reversed: bool,
    pub weather: u8,
    pub driver_count: u8,
    /// Retains every bit LFS wrote, including any this build cannot name.
    pub player_flags: PlayerFlags,
    pub hlvc_best_lap: u8,
    /// Number of meaningful entries in [`split_times`](Self::split_times).
    pub split_count: u8,
    /// Four on-disk MSHT values. Entries at and above `split_count` are padding.
    pub split_times: [Duration; 4],
    pub flags: i32,
    /// `None` represents the specified zero/unknown replay length.
    pub replay_length: Option<Duration>,
    /// Local driver name, retaining LFS colour control sequences such as `^1`.
    pub local_driver_name: String,
}

impl SprHeader {
    /// Reads and parses the fixed SPR header without reading the replay stream.
    pub fn read<R: Read>(mut reader: R) -> Result<Self, Error> {
        let mut magic = [0_u8; 6];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(Error::InvalidMagic { found: magic });
        }

        // Reading the complete fixed header up front also guarantees that the
        // primitive insim_core decoders cannot encounter a short buffer.
        let mut encoded_header = [0_u8; ENCODED_HEADER_LEN];
        reader.read_exact(&mut encoded_header)?;
        let mut bytes = Bytes::copy_from_slice(&encoded_header);
        Ok(Self::decode(&mut DecodeContext::new(&mut bytes))?)
    }

    /// Opens an SPR file and parses only its fixed header.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::read(fs::File::open(path)?)
    }
}

impl Decode for SprHeader {
    fn decode(ctx: &mut DecodeContext) -> Result<Self, DecodeError> {
        let file_version = FileVersion {
            high: ctx.decode("file_version_high")?,
            low: ctx.decode("file_version_low")?,
        };
        let spr_version = ctx.decode("spr_version")?;
        ctx.pad("reserved", 2)?;

        let qualifying_minutes: u8 = ctx.decode("qualifying_minutes")?;
        let qualifying_time = Duration::from_secs(u64::from(qualifying_minutes) * 60);
        let race_laps = ctx.decode("race_laps")?;
        let skill = ctx.decode("skill")?;
        let wind = ctx.decode("wind")?;
        let hotlap_mode = ctx.decode("hotlap_mode")?;

        let lfs_version_text = decode_ascii::<8>(ctx, "lfs_version", true)?;
        let lfs_version = GameVersion::from_str(&lfs_version_text).map_err(|error| {
            DecodeErrorKind::GameVersionParseError(error).context("lfs_version")
        })?;

        let track_code = decode_ascii::<4>(ctx, "track", false)?;
        let track = Track::from_str(&track_code).map_err(|_| {
            DecodeErrorKind::BadMagic {
                found: Box::new(track_code),
            }
            .context("track")
        })?;

        let added_mass = ctx.decode("added_mass")?;
        let intake_restriction = ctx.decode("intake_restriction")?;
        let abs_enabled = decode_bool(ctx, "abs_enabled")?;
        decode_zero(ctx, "zero")?;

        let track_name = decode_codepage::<32>(ctx, "track_name", true)?;
        let user_name = decode_codepage::<32>(ctx, "user_name", true)?;
        let car_name = SprVehicle::from(decode_codepage::<32>(ctx, "car_name", true)?);

        let configuration_value: u8 = ctx.decode("configuration")?;
        let configuration = NonZeroU8::new(configuration_value).ok_or_else(|| {
            DecodeErrorKind::OutOfRange {
                min: 1,
                max: u8::MAX as usize,
                found: 0,
            }
            .context("configuration")
        })?;
        let reversed = decode_bool(ctx, "reversed")?;
        let weather = ctx.decode("weather")?;
        let driver_count = ctx.decode("driver_count")?;
        // Decoded as raw bits rather than through `PlayerFlags::decode`, which
        // truncates to the flags this build happens to know about. The archived
        // corpus shows bits 1 and 2 set on two thirds of 0.5P-0.5X replays with
        // no meaning in the current protocol model, so truncating here would
        // erase them before anything had a chance to record them.
        let player_flags = PlayerFlags::from_bits_retain(ctx.decode::<u16>("player_flags")?);
        let hlvc_best_lap = ctx.decode("hlvc_best_lap")?;
        let split_count = ctx.decode("split_count")?;
        if split_count > 4 {
            return Err(DecodeErrorKind::OutOfRange {
                min: 0,
                max: 4,
                found: split_count as usize,
            }
            .context("split_count"));
        }

        let split_times = [
            decode_msht(ctx, "split_1")?,
            decode_msht(ctx, "split_2")?,
            decode_msht(ctx, "split_3")?,
            decode_msht(ctx, "split_4")?,
        ];
        let flags = ctx.decode("flags")?;
        let replay_length_centiseconds: u32 = ctx.decode("replay_length")?;
        let replay_length = (replay_length_centiseconds != 0)
            .then(|| Duration::from_millis(u64::from(replay_length_centiseconds) * 10));
        let local_driver_name = decode_codepage::<32>(ctx, "local_driver_name", false)?;

        Ok(Self {
            file_version,
            spr_version,
            qualifying_time,
            race_laps,
            skill,
            wind,
            hotlap_mode,
            lfs_version,
            track,
            added_mass,
            intake_restriction,
            abs_enabled,
            track_name,
            user_name,
            car_name,
            configuration,
            reversed,
            weather,
            driver_count,
            player_flags,
            hlvc_best_lap,
            split_count,
            split_times,
            flags,
            replay_length,
            local_driver_name,
        })
    }
}

fn decode_bool(ctx: &mut DecodeContext, name: &'static str) -> Result<bool, DecodeError> {
    match ctx.decode::<u8>(name)? {
        0 => Ok(false),
        1 => Ok(true),
        found => Err(no_variant(found).context(name)),
    }
}

fn decode_zero(ctx: &mut DecodeContext, name: &'static str) -> Result<(), DecodeError> {
    let found = ctx.decode::<u8>(name)?;
    if found == 0 {
        Ok(())
    } else {
        Err(DecodeErrorKind::BadMagic {
            found: Box::new(found),
        }
        .context(name))
    }
}

fn decode_msht(ctx: &mut DecodeContext, name: &'static str) -> Result<Duration, DecodeError> {
    let [minutes, seconds, hundredths, thousandths] = ctx.decode::<[u8; 4]>(name)?;
    if seconds > 59 {
        return Err(out_of_range(0, 59, seconds).context(name));
    }
    if hundredths > 99 {
        return Err(out_of_range(0, 99, hundredths).context(name));
    }
    if thousandths > 9 {
        return Err(out_of_range(0, 9, thousandths).context(name));
    }

    let milliseconds = u64::from(minutes) * 60_000
        + u64::from(seconds) * 1_000
        + u64::from(hundredths) * 10
        + u64::from(thousandths);
    Ok(Duration::from_millis(milliseconds))
}

fn decode_ascii<const N: usize>(
    ctx: &mut DecodeContext,
    name: &'static str,
    require_nul: bool,
) -> Result<String, DecodeError> {
    let raw = ctx.decode::<[u8; N]>(name)?;
    let end = raw.iter().position(|byte| *byte == 0);
    if require_nul && end.is_none() {
        return Err(DecodeErrorKind::ExpectedNull.context(name));
    }
    let text = &raw[..end.unwrap_or(N)];
    if !text.is_ascii() {
        return Err(DecodeErrorKind::BadMagic {
            found: Box::new(text.to_vec()),
        }
        .context(name));
    }
    Ok(String::from_utf8(text.to_vec()).expect("ASCII is valid UTF-8"))
}

fn decode_codepage<const N: usize>(
    ctx: &mut DecodeContext,
    name: &'static str,
    require_nul: bool,
) -> Result<String, DecodeError> {
    let raw = ctx.decode::<[u8; N]>(name)?;
    let end = raw.iter().position(|byte| *byte == 0);
    if require_nul && end.is_none() {
        return Err(DecodeErrorKind::ExpectedNull.context(name));
    }
    Ok(insim_core::string::codepages::to_lossy_string(&raw[..end.unwrap_or(N)]).into_owned())
}

fn no_variant(found: u8) -> DecodeError {
    DecodeErrorKind::NoVariantMatch {
        found: u64::from(found),
    }
    .into()
}

fn out_of_range(min: u8, max: u8, found: u8) -> DecodeError {
    DecodeErrorKind::OutOfRange {
        min: min as usize,
        max: max as usize,
        found: found as usize,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use insim::insim::{PlayerFlags, RaceLaps};
    use insim_core::{track::Track, wind::Wind};

    use super::*;

    #[test]
    fn parses_typed_header_without_reading_replay_data() {
        let mut input = valid_spr();
        input.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let mut input = Cursor::new(input);

        let header = SprHeader::read(&mut input).expect("valid SPR header should parse");

        assert_eq!(header.file_version, FileVersion { high: 1, low: 2 });
        assert_eq!(header.spr_version, 3);
        assert_eq!(header.qualifying_time, Duration::from_secs(30 * 60));
        assert!(matches!(header.race_laps, RaceLaps::Laps(100)));
        assert_eq!(header.skill, SkillLevel::Level4);
        assert!(matches!(header.wind, Wind::Strong));
        assert_eq!(header.hotlap_mode, HotlapMode::Enabled);
        assert_eq!(header.lfs_version.to_string(), "0.6B");
        assert_eq!(header.track, Track::Bl2r);
        assert!(header.abs_enabled);
        assert_eq!(header.track_name, "Blackwood GP Reverse");
        assert_eq!(header.user_name, "racer");
        assert_eq!(header.car_name.as_str(), "XR GT");
        assert_eq!(header.configuration.get(), 2);
        assert!(header.reversed);
        assert!(header.player_flags.contains(PlayerFlags::LEFTSIDE));
        assert!(header.player_flags.contains(PlayerFlags::AUTOGEARS));
        assert_eq!(header.split_count, 2);
        assert_eq!(header.split_times[0], Duration::from_millis(62_345));
        assert_eq!(header.split_times[1], Duration::from_millis(59_999));
        assert_eq!(header.flags, -7);
        assert_eq!(header.replay_length, Some(Duration::from_millis(123_450)));
        assert_eq!(header.local_driver_name, "^1Racer");
        assert_eq!(input.position(), HEADER_LEN as u64);
    }

    #[test]
    fn rejects_a_non_spr_file_before_reading_a_header() {
        let error = SprHeader::read(Cursor::new(b"LFSMPR".to_vec())).unwrap_err();
        assert!(matches!(error, Error::InvalidMagic { .. }));
    }

    #[test]
    fn rejects_a_truncated_header() {
        let error = SprHeader::read(Cursor::new(b"LFSSPR".to_vec())).unwrap_err();
        assert!(matches!(
            error,
            Error::Io {
                kind: std::io::ErrorKind::UnexpectedEof,
                ..
            }
        ));
    }

    #[test]
    fn rejects_non_boolean_header_values() {
        let mut input = valid_spr();
        input[129] = 2;
        let error = SprHeader::read(Cursor::new(input)).unwrap_err();
        assert!(matches!(error, Error::Decode(_)));
        assert!(error.to_string().contains("reversed"));
    }

    #[test]
    fn rejects_invalid_msht_components() {
        let mut input = valid_spr();
        input[137] = 60;
        let error = SprHeader::read(Cursor::new(input)).unwrap_err();
        assert!(matches!(error, Error::Decode(_)));
        assert!(error.to_string().contains("split_1"));
    }

    #[test]
    fn retains_player_flag_bits_the_protocol_model_cannot_name() {
        let mut input = valid_spr();
        // Bits 1 and 2 have no name in the current protocol model, yet the
        // archived LFSWorld corpus sets them on two thirds of 0.5P-0.5X rows.
        input[132..134].copy_from_slice(&(1_u16 | 2 | 4 | 8 | 512).to_le_bytes());

        let header = SprHeader::read(Cursor::new(input)).expect("valid SPR header should parse");

        assert_eq!(header.player_flags.bits(), 1 | 2 | 4 | 8 | 512);
        assert!(header.player_flags.contains(PlayerFlags::LEFTSIDE));
        assert!(header.player_flags.contains(PlayerFlags::AUTOGEARS));
        assert!(header.player_flags.contains(PlayerFlags::AUTOCLUTCH));
    }

    /// Checks the header offsets used by `frontend2/src/lib/spr.ts`.
    #[test]
    fn the_fields_a_client_reads_stay_at_their_documented_offsets() {
        let bytes = valid_spr();
        assert_eq!(&bytes[16..20], b"0.6B", "lfs_version starts at 16");
        assert_eq!(&bytes[24..28], b"BL2R", "track starts at 24");
        assert_eq!(&bytes[64..69], b"racer", "user_name starts at 64");

        let header = SprHeader::read(Cursor::new(bytes)).expect("valid SPR header should parse");

        assert_eq!(header.lfs_version.to_string(), "0.6B");
        assert_eq!(header.track, Track::Bl2r);
        assert_eq!(header.user_name, "racer");
    }

    fn valid_spr() -> Vec<u8> {
        let mut bytes = vec![0_u8; HEADER_LEN];
        bytes[..6].copy_from_slice(MAGIC);
        bytes[6] = 1;
        bytes[7] = 2;
        bytes[8] = 3;
        bytes[11] = 30;
        bytes[12] = 100;
        bytes[13] = 4;
        bytes[14] = 2;
        bytes[15] = 1;
        put_text(&mut bytes[16..24], b"0.6B");
        bytes[24..28].copy_from_slice(b"BL2R");
        bytes[28] = 20;
        bytes[29] = 10;
        bytes[30] = 1;
        put_text(&mut bytes[32..64], b"Blackwood GP Reverse");
        put_text(&mut bytes[64..96], b"racer");
        put_text(&mut bytes[96..128], b"XR GT");
        bytes[128] = 2;
        bytes[129] = 1;
        bytes[130] = 2;
        bytes[131] = 4;
        bytes[132..134].copy_from_slice(&(1_u16 | 8 | 512).to_le_bytes());
        bytes[134] = 3;
        bytes[135] = 2;
        bytes[136..140].copy_from_slice(&[1, 2, 34, 5]);
        bytes[140..144].copy_from_slice(&[0, 59, 99, 9]);
        bytes[152..156].copy_from_slice(&(-7_i32).to_le_bytes());
        bytes[156..160].copy_from_slice(&12_345_u32.to_le_bytes());
        put_text(&mut bytes[160..192], b"^1Racer");
        bytes
    }

    fn put_text(destination: &mut [u8], text: &[u8]) {
        destination[..text.len()].copy_from_slice(text);
    }
}
