//! Serialized era definitions used by catalogue and batch CLI commands.

use std::{collections::HashMap, fmt};

use anyhow::Context;
use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};
use itertools::Itertools;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationErrors};

use crate::models::{
    eras::{EraColumn, EraEntity, EraModel},
    hotlaps::HotlapRankable,
    rankings::{
        RankingBadgeDefinition, RankingColumn, RankingEntity, RankingRow, RankingRules, with_charts,
    },
};
use lfsplanet_game_version_req::GameVersionReq;

/// One complete era catalogue document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[validate(schema(
    function = "super::validate_era_definition",
    skip_on_field_errors = false
))]
#[serde(deny_unknown_fields)]
pub(crate) struct EraDefinition {
    #[validate(custom(function = "crate::models::eras::validate_era_slug"))]
    pub(crate) id: String,
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub(crate) title: String,
    #[serde(rename = "installation")]
    #[validate(custom(function = "super::validate_installation_id"))]
    pub(crate) installation_id: String,
    #[serde(default)]
    pub(crate) open: bool,
    #[serde(rename = "version")]
    #[validate(custom(function = "super::validate_version_requirement"))]
    pub(crate) version_requirement: GameVersionReq,
    #[validate(length(min = 1), nested)]
    pub(crate) rankings: Vec<RankingDefinition>,
}

/// One ranking declared by an era definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct RankingDefinition {
    pub(crate) id: RankingId,
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub(crate) title: String,
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub(crate) description: String,
    #[validate(nested)]
    pub(crate) rules: RankingRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub(crate) badge: Option<RankingBadgeDefinition>,
    #[validate(nested)]
    pub(crate) combinations: CombinationDefinitions,
}

/// One chart contribution declared by a ranking definition.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct CombinationDefinition {
    pub(crate) track: Track,
    pub(crate) vehicle: Vehicle,
}

/// One explicit chart list or a Cartesian product used to author one.
#[derive(Clone, Debug)]
pub(crate) enum CombinationDefinitions {
    Explicit(Vec<CombinationDefinition>),
    Matrix(MatrixDefinition),
}

/// The axes and optional excluded submatrices of a chart matrix.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct MatrixDefinition {
    #[validate(length(min = 1))]
    pub(crate) tracks: Vec<Track>,
    #[validate(length(min = 1))]
    pub(crate) vehicles: Vec<Vehicle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(nested)]
    pub(crate) exclude: Vec<MatrixExclusion>,
}

/// One Cartesian submatrix omitted from a chart matrix.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct MatrixExclusion {
    #[validate(length(min = 1))]
    pub(crate) tracks: Vec<Track>,
    #[validate(length(min = 1))]
    pub(crate) vehicles: Vec<Vehicle>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
enum GeneratedCombinationDefinitions<T = MatrixDefinition> {
    Matrix(T),
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum CombinationDefinitionsRepresentation {
    Explicit(Vec<CombinationDefinition>),
    Generated(GeneratedCombinationDefinitions),
}

impl CombinationDefinitions {
    fn from_matrix(definition: MatrixDefinition) -> Self {
        Self::Matrix(definition)
    }

    pub(crate) fn matrix_definition(&self) -> Option<&MatrixDefinition> {
        match self {
            Self::Explicit(_) => None,
            Self::Matrix(definition) => Some(definition),
        }
    }

    /// Generates concrete pairs only while a caller needs them.
    pub(crate) fn pairs(&self) -> impl Iterator<Item = CombinationDefinition> + '_ {
        match self {
            Self::Explicit(combinations) => itertools::Either::Left(combinations.iter().cloned()),
            Self::Matrix(definition) => itertools::Either::Right(
                definition
                    .tracks
                    .iter()
                    .copied()
                    .cartesian_product(definition.vehicles.iter().copied())
                    .filter(|(track, vehicle)| {
                        !definition.exclude.iter().any(|exclusion| {
                            exclusion.tracks.contains(track) && exclusion.vehicles.contains(vehicle)
                        })
                    })
                    .map(|(track, vehicle)| CombinationDefinition { track, vehicle }),
            ),
        }
    }
}

impl From<Vec<CombinationDefinition>> for CombinationDefinitions {
    fn from(combinations: Vec<CombinationDefinition>) -> Self {
        Self::Explicit(combinations)
    }
}

impl PartialEq for CombinationDefinitions {
    fn eq(&self, other: &Self) -> bool {
        self.pairs().eq(other.pairs())
    }
}

impl Eq for CombinationDefinitions {}

impl Validate for CombinationDefinitions {
    fn validate(&self) -> Result<(), ValidationErrors> {
        match self {
            Self::Explicit(combinations) => ValidationErrors::merge_all(
                Ok(()),
                "combinations",
                combinations.iter().map(Validate::validate).collect(),
            ),
            Self::Matrix(definition) => {
                let result = ValidationErrors::merge(Ok(()), "definition", definition.validate());
                ValidationErrors::merge_all(
                    result,
                    "combinations",
                    self.pairs().map(|pair| pair.validate()).collect(),
                )
            }
        }
    }
}

impl<'de> Deserialize<'de> for CombinationDefinitions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match CombinationDefinitionsRepresentation::deserialize(deserializer)? {
            CombinationDefinitionsRepresentation::Explicit(combinations) => {
                Ok(Self::Explicit(combinations))
            }
            CombinationDefinitionsRepresentation::Generated(
                GeneratedCombinationDefinitions::Matrix(definition),
            ) => Ok(Self::from_matrix(definition)),
        }
    }
}

impl Serialize for CombinationDefinitions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Explicit(combinations) => combinations.serialize(serializer),
            Self::Matrix(definition) => {
                GeneratedCombinationDefinitions::Matrix(definition).serialize(serializer)
            }
        }
    }
}

/// Stable, URL-safe identity of a concrete ranking within an era.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(try_from = "String")]
pub(crate) struct RankingId(String);

impl RankingId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RankingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for RankingId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl TryFrom<String> for RankingId {
    type Error = RankingIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        }) {
            Ok(Self(value))
        } else {
            Err(RankingIdError(value))
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "ranking id must contain lowercase ASCII letters or digits separated by single hyphens: {0}"
)]
pub(crate) struct RankingIdError(String);

impl EraDefinition {
    pub(crate) fn accepts_replay_version(&self, version: &GameVersion) -> bool {
        self.version_requirement.matches(version)
    }

    pub(crate) fn allows_combination(&self, track: Track, vehicle: Vehicle) -> bool {
        track.is_hotlap_rankable()
            && vehicle.is_hotlap_rankable()
            && self.rankings.iter().any(|ranking| {
                ranking
                    .combinations
                    .pairs()
                    .any(|pair| pair.track == track && pair.vehicle == vehicle)
            })
    }
}

/// Decodes catalogue data written by older builds without weakening the
/// authoring schema used for YAML definitions.
fn decode_stored_badge(
    mut value: serde_json::Value,
) -> Result<RankingBadgeDefinition, serde_json::Error> {
    if let Some(badge) = value.as_object_mut() {
        badge.remove("icon");
    }
    serde_json::from_value(value)
}

fn definition_from_rows(
    model: EraModel,
    rankings: Vec<(RankingRow, Vec<crate::models::rankings::RankingChartModel>)>,
) -> anyhow::Result<EraDefinition> {
    let rankings = rankings
        .into_iter()
        .map(|(ranking, combinations)| {
            let ranking_id = ranking.slug.clone();
            let rules = ranking.rules();
            let badge = ranking
                .badge
                .map(decode_stored_badge)
                .transpose()
                .with_context(|| {
                    format!("ranking {} has invalid badge configuration", ranking.slug)
                })?;
            Ok(RankingDefinition {
                id: RankingId::try_from(ranking.slug)?,
                title: ranking.title,
                description: ranking.description,
                rules,
                badge,
                combinations: combinations
                    .into_iter()
                    .map(|chart| {
                        let track = chart.track_id.parse().with_context(|| {
                            format!(
                                "ranking {} has invalid track {}",
                                ranking_id, chart.track_id
                            )
                        })?;
                        let vehicle = chart
                            .vehicle_id
                            .parse::<Vehicle>()
                            .expect("insim_core vehicle parsing is infallible");
                        Ok(CombinationDefinition { track, vehicle })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(EraDefinition {
        id: model.slug,
        title: model.title,
        installation_id: model.installation_id,
        open: model.open,
        version_requirement: model.version_requirement,
        rankings,
    })
}

/// Loads every complete era definition through a connection or transaction.
pub(crate) async fn list<C>(database: &C) -> anyhow::Result<Vec<EraDefinition>>
where
    C: ConnectionTrait,
{
    let eras = EraEntity::find()
        .order_by_asc(EraColumn::Slug)
        .all(database)
        .await
        .context("failed to load eras")?;
    let mut rankings = with_charts(database, RankingEntity::find())
        .await
        .context("failed to load rankings and charts")?;
    rankings.sort_by_key(|(ranking, _)| (ranking.era_id, ranking.position));

    let mut rankings_by_era: HashMap<i64, Vec<_>> = HashMap::new();
    for (ranking, combinations) in rankings {
        rankings_by_era
            .entry(ranking.era_id)
            .or_default()
            .push((ranking, combinations));
    }

    eras.into_iter()
        .map(|era| {
            let id = era.id;
            definition_from_rows(era, rankings_by_era.remove(&id).unwrap_or_default())
        })
        .collect()
}

/// Loads one complete era definition by identifier.
pub(crate) async fn find<C>(database: &C, id: &str) -> anyhow::Result<Option<EraDefinition>>
where
    C: ConnectionTrait,
{
    let Some(era) = crate::models::eras::find_by_slug(id)
        .one(database)
        .await
        .context("failed to load era")?
    else {
        return Ok(None);
    };
    let mut rankings = with_charts(
        database,
        RankingEntity::find().filter(RankingColumn::EraId.eq(era.id)),
    )
    .await
    .context("failed to load rankings and charts")?;
    rankings.sort_by_key(|(ranking, _)| ranking.position);

    definition_from_rows(era, rankings).map(Some)
}

#[cfg(test)]
pub(crate) fn test_definitions() -> Vec<EraDefinition> {
    [
        ("2005-06-24", "2005 physics", ">=0.5P,<0.5T", true),
        ("2006-04-21", "2006 physics", ">=0.5T,<0.5Y", false),
        ("2007-12-21", "2007 physics", ">=0.5Y,<0.8", true),
    ]
    .into_iter()
    .map(|(id, title, version_requirement, open)| EraDefinition {
        id: id.to_owned(),
        title: title.to_owned(),
        installation_id: "0.7".to_owned(),
        open,
        version_requirement: version_requirement.parse().expect("valid test requirement"),
        rankings: vec![RankingDefinition {
            id: RankingId::try_from("test".to_owned()).expect("valid test ranking id"),
            title: "Test ranking".to_owned(),
            description: "Test ranking description".to_owned(),
            rules: RankingRules {
                benchmark_percent: 103,
                nation_max_points: 10,
                nation_driver_limit: 3,
            },
            badge: None,
            combinations: vec![CombinationDefinition {
                track: "FE1".parse().expect("valid test track"),
                vehicle: "XFG".parse().expect("valid test vehicle"),
            }]
            .into(),
        }],
    })
    .collect()
}
