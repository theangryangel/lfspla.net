//! Semver-style requirements over Live for Speed game versions.

use std::{fmt, str::FromStr};

use insim_core::game_version::{GameVersion, GameVersionParseError};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

#[cfg(feature = "sea-orm")]
mod sea_orm;

/// A conjunction of comparisons against an LFS game version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersionReq {
    comparators: Vec<GameVersionComparator>,
}

impl GameVersionReq {
    /// Returns whether the version satisfies every comparator in this requirement.
    #[must_use]
    pub fn matches(&self, version: &GameVersion) -> bool {
        self.comparators
            .iter()
            .all(|comparator| comparator.matches(version))
    }

    /// Returns whether at least one ordered game version can satisfy the requirement.
    #[must_use]
    pub fn is_satisfiable(&self) -> bool {
        let (lower, upper) = self.bounds();
        match (lower, upper) {
            (Some(lower), Some(upper)) => {
                lower.version < upper.version
                    || (lower.version == upper.version && lower.inclusive && upper.inclusive)
            }
            _ => true,
        }
    }

    /// Returns whether a game version could satisfy both requirements.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        let mut comparators = self.comparators.clone();
        comparators.extend(other.comparators.iter().cloned());
        Self { comparators }.is_satisfiable()
    }

    fn bounds(&self) -> (Option<VersionBound>, Option<VersionBound>) {
        let mut lower = None;
        let mut upper = None;
        for comparator in &self.comparators {
            match comparator.operator {
                ComparatorOperator::Equal => {
                    update_lower(&mut lower, comparator.version.clone(), true);
                    update_upper(&mut upper, comparator.version.clone(), true);
                }
                ComparatorOperator::Greater => {
                    update_lower(&mut lower, comparator.version.clone(), false);
                }
                ComparatorOperator::GreaterOrEqual => {
                    update_lower(&mut lower, comparator.version.clone(), true);
                }
                ComparatorOperator::Less => {
                    update_upper(&mut upper, comparator.version.clone(), false);
                }
                ComparatorOperator::LessOrEqual => {
                    update_upper(&mut upper, comparator.version.clone(), true);
                }
            }
        }
        (lower, upper)
    }
}

#[derive(Debug)]
struct VersionBound {
    version: GameVersion,
    inclusive: bool,
}

fn update_lower(bound: &mut Option<VersionBound>, version: GameVersion, inclusive: bool) {
    let replace = bound.as_ref().is_none_or(|current| {
        version > current.version || (version == current.version && current.inclusive && !inclusive)
    });
    if replace {
        *bound = Some(VersionBound { version, inclusive });
    }
}

fn update_upper(bound: &mut Option<VersionBound>, version: GameVersion, inclusive: bool) {
    let replace = bound.as_ref().is_none_or(|current| {
        version < current.version || (version == current.version && current.inclusive && !inclusive)
    });
    if replace {
        *bound = Some(VersionBound { version, inclusive });
    }
}

impl FromStr for GameVersionReq {
    type Err = GameVersionReqParseError;

    fn from_str(requirement: &str) -> Result<Self, Self::Err> {
        if requirement.trim().is_empty() {
            return Err(GameVersionReqParseError::Empty);
        }
        let comparators = requirement
            .split(',')
            .enumerate()
            .map(|(index, comparator)| parse_comparator(index, comparator))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { comparators })
    }
}

impl fmt::Display for GameVersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, comparator) in self.comparators.iter().enumerate() {
            if index > 0 {
                formatter.write_str(",")?;
            }
            comparator.fmt(formatter)?;
        }
        Ok(())
    }
}

impl Serialize for GameVersionReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for GameVersionReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// One comparison within a game-version requirement.
#[derive(Debug, Clone)]
pub struct GameVersionComparator {
    pub operator: ComparatorOperator,
    pub version: GameVersion,
    display_version: String,
}

impl PartialEq for GameVersionComparator {
    fn eq(&self, other: &Self) -> bool {
        self.operator == other.operator && self.version == other.version
    }
}
impl Eq for GameVersionComparator {}

impl GameVersionComparator {
    /// Returns whether the game version satisfies this comparison.
    #[must_use]
    pub fn matches(&self, version: &GameVersion) -> bool {
        match self.operator {
            ComparatorOperator::Equal => version == &self.version,
            ComparatorOperator::Greater => version > &self.version,
            ComparatorOperator::GreaterOrEqual => version >= &self.version,
            ComparatorOperator::Less => version < &self.version,
            ComparatorOperator::LessOrEqual => version <= &self.version,
        }
    }
}
impl fmt::Display for GameVersionComparator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.operator, self.display_version)
    }
}

/// Comparison operation supported by [`GameVersionReq`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparatorOperator {
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}
impl fmt::Display for ComparatorOperator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Equal => "=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
        })
    }
}

/// A malformed game-version requirement.
#[derive(Debug, thiserror::Error)]
pub enum GameVersionReqParseError {
    #[error("game-version requirement must not be empty")]
    Empty,
    #[error("comparator {index} must not be empty")]
    EmptyComparator { index: usize },
    #[error("comparator `{comparator}` must include a game version")]
    MissingVersion { comparator: String },
    #[error("invalid game version `{version}` in comparator `{comparator}`")]
    InvalidVersion {
        comparator: String,
        version: String,
        #[source]
        source: GameVersionParseError,
    },
}

fn parse_comparator(
    index: usize,
    comparator: &str,
) -> Result<GameVersionComparator, GameVersionReqParseError> {
    let comparator = comparator.trim();
    if comparator.is_empty() {
        return Err(GameVersionReqParseError::EmptyComparator { index });
    }
    let (operator, version) = if let Some(version) = comparator.strip_prefix(">=") {
        (ComparatorOperator::GreaterOrEqual, version)
    } else if let Some(version) = comparator.strip_prefix("<=") {
        (ComparatorOperator::LessOrEqual, version)
    } else if let Some(version) = comparator.strip_prefix('>') {
        (ComparatorOperator::Greater, version)
    } else if let Some(version) = comparator.strip_prefix('<') {
        (ComparatorOperator::Less, version)
    } else if let Some(version) = comparator.strip_prefix('=') {
        (ComparatorOperator::Equal, version)
    } else {
        (ComparatorOperator::Equal, comparator)
    };
    let version = version.trim();
    if version.is_empty() {
        return Err(GameVersionReqParseError::MissingVersion {
            comparator: comparator.to_owned(),
        });
    }
    let parsed: GameVersion =
        version
            .parse()
            .map_err(|source| GameVersionReqParseError::InvalidVersion {
                comparator: comparator.to_owned(),
                version: version.to_owned(),
                source,
            })?;
    let display_version = if version.chars().any(char::is_alphabetic) {
        parsed.to_string()
    } else {
        parsed.major.to_string()
    };
    Ok(GameVersionComparator {
        operator,
        version: parsed,
        display_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn version(value: &str) -> GameVersion {
        value.parse().unwrap()
    }

    #[test]
    fn matches_a_speculative_range() {
        let requirement = ">=0.5Y,<0.8".parse::<GameVersionReq>().unwrap();
        assert!(!requirement.matches(&version("0.5X39")));
        assert!(requirement.matches(&version("0.5Y")));
        assert!(requirement.matches(&version("0.7D41")));
        assert!(!requirement.matches(&version("0.8A")));
    }
    #[test]
    fn a_bare_version_is_an_exact_requirement() {
        let requirement = "0.7D41".parse::<GameVersionReq>().unwrap();
        assert!(requirement.matches(&version("0.7D41")));
        assert!(!requirement.matches(&version("0.7D40")));
        assert!(!requirement.matches(&version("0.7D42")));
        assert_eq!(requirement.to_string(), "=0.7D41");
    }
    #[test]
    fn supports_every_comparison_operation() {
        let cases = [
            (">0.7D40", true),
            (">0.7D41", false),
            (">=0.7D41", true),
            ("<0.7D42", true),
            ("<0.7D41", false),
            ("<=0.7D41", true),
            ("=0.7D41", true),
        ];
        for (requirement, expected) in cases {
            assert_eq!(
                requirement
                    .parse::<GameVersionReq>()
                    .unwrap()
                    .matches(&version("0.7D41")),
                expected,
                "requirement: {requirement}"
            );
        }
    }
    #[test]
    fn trims_whitespace_and_has_a_canonical_display() {
        let requirement = " >= 0.5y , < 0.8 ".parse::<GameVersionReq>().unwrap();
        assert_eq!(requirement.to_string(), ">=0.5Y,<0.8");
        assert_eq!(requirement.comparators.len(), 2);
    }
    #[test]
    fn rejects_empty_or_invalid_comparators() {
        assert!(matches!(
            "  ".parse::<GameVersionReq>(),
            Err(GameVersionReqParseError::Empty)
        ));
        assert!(matches!(
            ">=0.5Y,,<0.8".parse::<GameVersionReq>(),
            Err(GameVersionReqParseError::EmptyComparator { index: 1 })
        ));
        assert!(matches!(
            ">=".parse::<GameVersionReq>(),
            Err(GameVersionReqParseError::MissingVersion { .. })
        ));
        assert!(matches!(
            ">=not-a-version".parse::<GameVersionReq>(),
            Err(GameVersionReqParseError::InvalidVersion { .. })
        ));
    }
    #[test]
    fn serializes_as_the_requirement_string() {
        let requirement = ">=0.5Y,<0.8".parse::<GameVersionReq>().unwrap();
        let json = serde_json::to_string(&requirement).unwrap();
        assert_eq!(json, r#"">=0.5Y,<0.8""#);
        assert_eq!(
            serde_json::from_str::<GameVersionReq>(&json).unwrap(),
            requirement
        );
    }
    #[test]
    fn detects_impossible_and_overlapping_requirements() {
        let old = ">=0.5Y,<0.8".parse::<GameVersionReq>().unwrap();
        let new = ">=0.8".parse::<GameVersionReq>().unwrap();
        let overlapping = ">=0.7D".parse::<GameVersionReq>().unwrap();
        let impossible = ">0.8,<=0.8".parse::<GameVersionReq>().unwrap();
        assert!(!old.intersects(&new));
        assert!(old.intersects(&overlapping));
        assert!(!impossible.is_satisfiable());
    }
}
