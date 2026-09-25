//! Result codes produced by LFS replay validation.

/// Result values returned by LFS's `/hlvc` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HlvcResult {
    Unknown = 0,
    Ok = 1,
    NotFound = 2,
    Future = 3,
    Obsolete = 4,
    NotHotlap = 5,
    NotDemo = 6,
    NoTrack = 7,
    NoHlvc = 8,
    Corrupted = 9,
    OutOfSync = 10,
    NoMatch = 11,
    Fail = 12,
    End = 13,
    Custom = 14,
    PitStop = 15,
    FailGround = 16,
    FailWall = 17,
    FailObject = 18,
    FailWrongWay = 19,
    FailSpeeding = 20,
    FailOutOfBounds = 21,
    UnknownVehicle = 22,
}

impl HlvcResult {
    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Unknown => "unknown HLVC return value",
            Self::Ok => "OK",
            Self::NotFound => "replay file not found",
            Self::Future => "replay is from a later LFS version",
            Self::Obsolete => "replay is from an obsolete LFS version",
            Self::NotHotlap => "replay is not marked as a hotlap",
            Self::NotDemo => "LFS is not unlocked for this track",
            Self::NoTrack => "track could not be loaded",
            Self::NoHlvc => "replay header contains no HLVC best lap",
            Self::Corrupted => "replay is corrupted or invalid",
            Self::OutOfSync => "replay went out of sync",
            Self::NoMatch => "replayed lap time does not match the SPR header",
            Self::Fail => "HLVC violation",
            Self::End => "replay ended before the best lap was finished",
            Self::Custom => "valid replay uses objects or custom timing",
            Self::PitStop => "old hotlap includes a pit stop",
            Self::FailGround => "HLVC violation: ground",
            Self::FailWall => "HLVC violation: wall",
            Self::FailObject => "HLVC violation: object",
            Self::FailWrongWay => "HLVC violation: wrong way",
            Self::FailSpeeding => "HLVC violation: speeding",
            Self::FailOutOfBounds => "HLVC violation: out of bounds",
            Self::UnknownVehicle => "unknown vehicle",
        }
    }
}

impl TryFrom<i32> for HlvcResult {
    type Error = i32;

    fn try_from(code: i32) -> Result<Self, Self::Error> {
        Ok(match code {
            0 => Self::Unknown,
            1 => Self::Ok,
            2 => Self::NotFound,
            3 => Self::Future,
            4 => Self::Obsolete,
            5 => Self::NotHotlap,
            6 => Self::NotDemo,
            7 => Self::NoTrack,
            8 => Self::NoHlvc,
            9 => Self::Corrupted,
            10 => Self::OutOfSync,
            11 => Self::NoMatch,
            12 => Self::Fail,
            13 => Self::End,
            14 => Self::Custom,
            15 => Self::PitStop,
            16 => Self::FailGround,
            17 => Self::FailWall,
            18 => Self::FailObject,
            19 => Self::FailWrongWay,
            20 => Self::FailSpeeding,
            21 => Self::FailOutOfBounds,
            22 => Self::UnknownVehicle,
            code => return Err(code),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_documented_hlvc_results_are_mapped() {
        for code in 0..=22 {
            let result = HlvcResult::try_from(code).unwrap();
            assert_eq!(i32::from(result.code()), code);
        }
        assert!(HlvcResult::try_from(23).is_err());
        assert!(HlvcResult::try_from(-1).is_err());
    }
}
