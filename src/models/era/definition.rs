//! Serialized era definitions used by catalogue and batch CLI commands.

use crate::era_slug::EraSlug;
use crate::models::{
    Era, Ranking,
    era::{Column as EraColumn, Entity as EraEntity},
    ranking::{
        Column as RankingColumn, Entity as RankingEntity, RankingBadgeDefinition, RankingRules,
    },
};
use crate::{installation_id::InstallationId, ranking_slug::RankingSlug};
use anyhow::{Context, ensure};
use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};
use itertools::Itertools;
use lfsplanet_game_version_req::GameVersionReq;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use validator::{Validate, ValidationError, ValidationErrors};

/// One complete era catalogue document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[validate(schema(
    function = "crate::models::era::definition::validate_era_definition",
    skip_on_field_errors = false
))]
#[serde(deny_unknown_fields)]
pub(crate) struct EraDefinition {
    pub(crate) id: EraSlug,
    #[validate(custom(function = "crate::validate::validate_non_blank"))]
    pub(crate) title: String,
    #[serde(rename = "installation")]
    pub(crate) installation_id: InstallationId,
    #[serde(default)]
    pub(crate) open: bool,
    #[serde(rename = "version")]
    #[validate(custom(function = "crate::models::era::definition::validate_version_requirement"))]
    pub(crate) version_requirement: GameVersionReq,
    #[validate(length(min = 1), nested)]
    pub(crate) rankings: Vec<RankingDefinition>,
}

/// One ranking declared by an era definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct RankingDefinition {
    pub(crate) id: RankingSlug,
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

impl EraDefinition {
    pub(crate) fn accepts_replay_version(&self, version: &GameVersion) -> bool {
        self.version_requirement.matches(version)
    }

    pub(crate) fn allows_combination(&self, track: Track, vehicle: Vehicle) -> bool {
        !track.is_open()
            && vehicle != Vehicle::Unknown
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
    model: Era,
    rankings: Vec<crate::models::ranking::RankingWithCharts>,
) -> anyhow::Result<EraDefinition> {
    let rankings = rankings
        .into_iter()
        .map(|loaded| {
            let ranking = loaded.definition;
            let combinations = loaded.charts;
            let rules = ranking.rules();
            let badge = ranking
                .badge
                .map(decode_stored_badge)
                .transpose()
                .with_context(|| {
                    format!("ranking {} has invalid badge configuration", ranking.slug)
                })?;
            Ok(RankingDefinition {
                id: ranking.slug,
                title: ranking.title,
                description: ranking.description,
                rules,
                badge,
                combinations: combinations
                    .into_iter()
                    .map(|chart| CombinationDefinition {
                        track: chart.track_id.0,
                        vehicle: chart.vehicle_id.0,
                    })
                    .collect::<Vec<_>>()
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
    let mut rankings = Ranking::with_charts(database, RankingEntity::find())
        .await
        .context("failed to load rankings and charts")?;
    rankings.sort_by_key(|loaded| (loaded.definition.era_id, loaded.definition.position));

    let mut rankings_by_era: HashMap<i64, Vec<_>> = HashMap::new();
    for loaded in rankings {
        rankings_by_era
            .entry(loaded.definition.era_id)
            .or_default()
            .push(loaded);
    }

    eras.into_iter()
        .map(|era| {
            let id = era.id;
            definition_from_rows(era, rankings_by_era.remove(&id).unwrap_or_default())
        })
        .collect()
}

/// Loads one complete era definition by identifier.
pub(crate) async fn find<C>(database: &C, id: &EraSlug) -> anyhow::Result<Option<EraDefinition>>
where
    C: ConnectionTrait,
{
    let Some(era) = crate::models::Era::find_by_slug(database, id)
        .await
        .context("failed to load era")?
    else {
        return Ok(None);
    };
    let mut rankings = Ranking::with_charts(
        database,
        RankingEntity::find().filter(RankingColumn::EraId.eq(era.id)),
    )
    .await
    .context("failed to load rankings and charts")?;
    rankings.sort_by_key(|loaded| loaded.definition.position);

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
        id: id.parse().expect("valid test era slug"),
        title: title.to_owned(),
        installation_id: "0.7".parse().unwrap(),
        open,
        version_requirement: version_requirement.parse().expect("valid test requirement"),
        rankings: vec![RankingDefinition {
            id: RankingSlug::try_from("test".to_owned()).expect("valid test ranking slug"),
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

pub(crate) fn combination_keys(era: &EraDefinition) -> HashSet<(String, String)> {
    era.rankings
        .iter()
        .flat_map(|ranking| ranking.combinations.pairs())
        .map(|pair| (pair.track.to_string(), pair.vehicle.to_string()))
        .collect()
}

fn validate_era_definition(file: &EraDefinition) -> Result<(), ValidationError> {
    let result = (|| -> anyhow::Result<()> {
        validate_rankings(file)?;
        Ok(())
    })();

    result.map_err(|error| validation_error("invalid_era_definition", error.to_string()))
}

fn validate_rankings(file: &EraDefinition) -> anyhow::Result<()> {
    let mut ranking_ids = HashSet::new();
    for definition in &file.rankings {
        ensure!(
            ranking_ids.insert(&definition.id),
            "duplicate ranking {}",
            definition.id
        );
        validate_chart_matrix(definition)?;
        validate_ranking_charts(definition)?;
    }
    Ok(())
}

fn validate_ranking_charts(definition: &RankingDefinition) -> anyhow::Result<()> {
    let combinations = &definition.combinations;
    ensure!(
        combinations.pairs().next().is_some(),
        "ranking {} must contain combinations",
        definition.id
    );
    let mut chart_keys = HashSet::new();
    for chart in combinations.pairs() {
        ensure!(
            !chart.track.is_open(),
            "ranking {} uses ineligible track {}",
            definition.id,
            chart.track
        );
        ensure!(
            chart.vehicle != Vehicle::Unknown,
            "ranking {} uses ineligible vehicle {}",
            definition.id,
            chart.vehicle
        );
        ensure!(
            chart_keys.insert((chart.track, chart.vehicle)),
            "ranking {} contains duplicate chart {}/{}",
            definition.id,
            chart.track,
            chart.vehicle
        );
    }
    Ok(())
}

fn validate_version_requirement(
    requirement: &lfsplanet_game_version_req::GameVersionReq,
) -> Result<(), ValidationError> {
    if requirement.is_satisfiable() {
        return Ok(());
    }
    Err(ValidationError::new("unsatisfiable_version_requirement"))
}

fn validation_error(code: &'static str, message: impl Into<String>) -> ValidationError {
    let mut error = ValidationError::new(code);
    error.message = Some(message.into().into());
    error
}

fn validate_chart_matrix(ranking: &RankingDefinition) -> anyhow::Result<()> {
    let Some(matrix) = ranking.combinations.matrix_definition() else {
        return Ok(());
    };
    let mut matrix_tracks = HashSet::new();
    for track in &matrix.tracks {
        ensure!(
            matrix_tracks.insert(track),
            "ranking {} matrix contains duplicate track {}",
            ranking.id,
            track
        );
        ensure!(
            !track.is_open(),
            "ranking {} matrix uses ineligible track {}",
            ranking.id,
            track
        );
    }
    let mut matrix_vehicles = HashSet::new();
    for vehicle in &matrix.vehicles {
        ensure!(
            matrix_vehicles.insert(vehicle),
            "ranking {} matrix contains duplicate vehicle {}",
            ranking.id,
            vehicle
        );
        ensure!(
            *vehicle != Vehicle::Unknown,
            "ranking {} matrix uses ineligible vehicle {}",
            ranking.id,
            vehicle
        );
    }
    for exclusion in &matrix.exclude {
        for track in &exclusion.tracks {
            ensure!(
                matrix_tracks.contains(track),
                "ranking {} matrix excludes track {} outside its track axis",
                ranking.id,
                track
            );
        }
        for vehicle in &exclusion.vehicles {
            ensure!(
                matrix_vehicles.contains(vehicle),
                "ranking {} matrix excludes vehicle {} outside its vehicle axis",
                ranking.id,
                vehicle
            );
        }
    }
    Ok(())
}

pub(crate) fn normalise_file(file: &mut EraDefinition) {
    file.title = file.title.trim().to_owned();
    for ranking in &mut file.rankings {
        ranking.title = ranking.title.trim().to_owned();
        ranking.description = ranking.description.trim().to_owned();
        if let Some(badge) = &mut ranking.badge {
            badge.label = badge.label.trim().to_owned();
            if let Some(completion) = &mut badge.completion {
                completion.label = completion.label.trim().to_owned();
            }
        }
    }
}
