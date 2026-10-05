//! Signed elapsed time stored and serialized as whole milliseconds.

use std::{fmt, num::TryFromIntError, time::Duration};

use serde::{Deserialize, Serialize};

/// A signed millisecond count, including negative time gaps and handicaps.
///
/// SQL and serde use integer milliseconds; display uses `1:31.234` formatting.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Deserialize,
    Serialize,
    sea_orm::DeriveValueType,
    sqlx::Type,
    utoipa::ToSchema,
)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Milliseconds(i64);

impl Milliseconds {
    pub const ZERO: Self = Self(0);

    pub const fn from_millis(value: i64) -> Self {
        Self(value)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

impl From<i64> for Milliseconds {
    fn from(value: i64) -> Self {
        Self::from_millis(value)
    }
}

impl From<Milliseconds> for i64 {
    fn from(value: Milliseconds) -> Self {
        value.as_millis()
    }
}

impl TryFrom<Duration> for Milliseconds {
    type Error = TryFromIntError;

    /// Truncates sub-millisecond precision, checking that the count fits BIGINT.
    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        i64::try_from(value.as_millis()).map(Self)
    }
}

impl TryFrom<Milliseconds> for Duration {
    type Error = TryFromIntError;

    /// Negative durations cannot be represented by std::time::Duration.
    fn try_from(value: Milliseconds) -> Result<Self, Self::Error> {
        u64::try_from(value.0).map(Self::from_millis)
    }
}

impl fmt::Display for Milliseconds {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let magnitude = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(
            formatter,
            "{sign}{}:{:02}.{:03}",
            magnitude / 60_000,
            magnitude / 1_000 % 60,
            magnitude % 1_000,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::{Value, ValueType};

    #[test]
    fn formats_signed_times_without_wrapping_minutes_or_overflowing() {
        for (value, expected) in [
            (0, "0:00.000"),
            (91_234, "1:31.234"),
            (-2_150, "-0:02.150"),
            (3_600_001, "60:00.001"),
            (i64::MIN, "-153722867280912:55.808"),
        ] {
            assert_eq!(Milliseconds::from_millis(value).to_string(), expected);
        }
    }

    #[test]
    fn storage_and_json_remain_signed_integer_milliseconds() {
        for value in [i64::MIN, -100, 0, 91_234, i64::MAX] {
            let duration = Milliseconds::from_millis(value);
            assert_eq!(Value::from(duration), Value::from(value));
            assert_eq!(
                <Milliseconds as ValueType>::try_from(Value::from(value)).unwrap(),
                duration,
            );
            let json = serde_json::to_string(&duration).unwrap();
            assert_eq!(json, value.to_string());
            assert_eq!(
                serde_json::from_str::<Milliseconds>(&json).unwrap(),
                duration
            );
        }
        assert!(serde_json::from_str::<Milliseconds>("1.5").is_err());
        let schema =
            serde_json::to_value(<Milliseconds as utoipa::PartialSchema>::schema()).unwrap();
        assert_eq!(schema["type"], "integer");
        assert_eq!(schema["format"], "int64");
    }

    #[test]
    fn csv_timings_deserialize_as_integer_milliseconds() {
        #[derive(Deserialize)]
        struct Timing {
            lap_time_ms: Milliseconds,
        }

        let mut reader = csv::Reader::from_reader("lap_time_ms\n91234\n-2150\n".as_bytes());
        let timings = reader
            .deserialize::<Timing>()
            .map(|record| record.unwrap().lap_time_ms.as_millis())
            .collect::<Vec<_>>();
        assert_eq!(timings, [91_234, -2_150]);
    }

    #[test]
    fn duration_conversions_check_range_and_sign_and_truncate_fractional_milliseconds() {
        let duration = Duration::from_nanos(91_234_999_999);
        let milliseconds = <Milliseconds as TryFrom<Duration>>::try_from(duration).unwrap();
        assert_eq!(milliseconds.as_millis(), 91_234);
        assert_eq!(
            Duration::try_from(milliseconds).unwrap(),
            Duration::from_millis(91_234)
        );
        assert!(<Milliseconds as TryFrom<Duration>>::try_from(Duration::MAX).is_err());
        assert!(Duration::try_from(Milliseconds::from_millis(-1)).is_err());
        let maximum = Milliseconds::from_millis(i64::MAX);
        assert_eq!(
            <Milliseconds as TryFrom<Duration>>::try_from(Duration::try_from(maximum).unwrap())
                .unwrap(),
            maximum
        );
    }
}
