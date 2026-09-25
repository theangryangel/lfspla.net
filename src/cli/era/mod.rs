//! Operator CLI for database-backed era policy.

mod args;
mod definition;
#[cfg(test)]
mod tests;

pub(crate) use args::EraCommand;
pub(crate) use definition::EraDefinition;
pub(crate) use definition::list as list_definitions;
#[cfg(test)]
pub(crate) use definition::test_definitions;

use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
};

use crate::{
    cli::Args,
    models::{
        eras::{self, EraColumn, EraEntity, EraModel, EraMutation},
        hotlaps::HotlapRankable,
        rankings::{
            RankingChartEntity, RankingChartMutation, RankingColumn, RankingEntity, RankingMutation,
        },
    },
    settings::Settings,
    startup,
};
use anyhow::{Context, bail, ensure};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
    Statement, TransactionTrait,
};
use validator::{Validate, ValidationError};

/// The path that selects standard input instead of an era YAML file.
const STDIN_PATH: &str = "-";

/// Runs one era catalogue operator command.
pub(crate) async fn run(args: &Args, action: &EraCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = startup::connect(&settings.database, 2).await?;

    match action {
        EraCommand::List => list(&database).await,
        EraCommand::Export { id } => export(&database, id).await,
        EraCommand::Apply { files, yes } => apply(&database, files, *yes).await,
        EraCommand::Delete { id, yes } => delete(&database, id, *yes).await,
        EraCommand::Open { id } => set_open(&database, id, true).await,
        EraCommand::Close { id } => set_open(&database, id, false).await,
    }
}

async fn list(database: &DatabaseConnection) -> anyhow::Result<()> {
    let eras = EraEntity::find()
        .order_by_asc(EraColumn::Slug)
        .all(database)
        .await?;
    if eras.is_empty() {
        println!("No eras configured.");
        return Ok(());
    }
    for era in eras {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            era.slug,
            if era.open { "open" } else { "closed" },
            era.version_requirement,
            era.installation_id,
            era.title
        );
    }
    Ok(())
}

async fn export(database: &DatabaseConnection, id: &str) -> anyhow::Result<()> {
    let era = definition::find(database, id)
        .await?
        .with_context(|| format!("era {id:?} was not found"))?;
    let yaml = serde_saphyr::to_string(&era).context("failed to serialize era as YAML")?;
    print!("{yaml}");
    Ok(())
}

async fn apply(
    database: &DatabaseConnection,
    paths: &[std::path::PathBuf],
    yes: bool,
) -> anyhow::Result<()> {
    let desired = read_desired(paths)?;

    let transaction = database.begin().await?;
    lock(&transaction).await?;

    let existing_models = EraEntity::find().all(&transaction).await?;
    let existing_definitions = definition::list(&transaction).await?;
    let mut changed = Vec::new();
    for desired in &desired {
        let existing_model = existing_models
            .iter()
            .find(|model| model.slug == desired.id)
            .cloned();
        let existing = existing_definitions.iter().find(|era| era.id == desired.id);

        describe_change(existing, desired);
        if let Some(existing_model) = &existing_model {
            describe_affected_hotlaps(&transaction, existing_model.id, desired).await?;
        }
        if existing == Some(desired) {
            continue;
        }
        changed.push((existing_model, desired));
    }
    if !yes && changed.iter().any(|(existing, _)| existing.is_some()) {
        transaction.rollback().await?;
        bail!("changing an existing era requires --yes");
    }
    for (existing, desired) in &changed {
        persist(&transaction, existing.clone(), desired).await?;
    }

    eras::validate_version_requirements(&transaction)
        .await
        .context("the proposed era would make the catalogue invalid")?;
    // Eligibility is defined by the ranking-chart catalogue, so update the PB
    // projection in the same transaction as a changed era definition.
    let mut changed_eras = Vec::with_capacity(changed.len());
    for (_, desired) in &changed {
        let era = EraEntity::find()
            .filter(EraColumn::Slug.eq(&desired.id))
            .one(&transaction)
            .await?
            .expect("persisted era must exist");
        era.rebuild_personal_bests(&transaction).await?;
        changed_eras.push(era);
    }
    transaction.commit().await?;
    if changed.is_empty() {
        println!("No changes.");
    } else {
        for era in &changed_eras {
            era.rebuild_badges(database).await.with_context(|| {
                format!(
                    "era {} was applied, but badge rebuilding failed; run hotlap rebadge --all",
                    era.slug
                )
            })?;
        }
        for (_, era) in changed {
            println!("Era {} applied.", era.id);
        }
    }
    Ok(())
}

/// Reads one era YAML file, or standard input when the path is `-`.
fn read_era_yaml(path: &Path) -> anyhow::Result<String> {
    if path == Path::new(STDIN_PATH) {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .context("failed to read era YAML from standard input")?;
        Ok(input)
    } else {
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read era YAML from {}", path.display()))
    }
}

fn read_desired(paths: &[std::path::PathBuf]) -> anyhow::Result<Vec<EraDefinition>> {
    let mut desired = Vec::with_capacity(paths.len());
    let mut ids = HashSet::with_capacity(paths.len());
    let stdin_count = paths
        .iter()
        .filter(|path| path.as_path() == Path::new(STDIN_PATH))
        .count();
    ensure!(stdin_count <= 1, "stdin may only be selected once");
    ensure!(
        stdin_count == 0 || paths.len() == 1,
        "stdin cannot be combined with era YAML paths"
    );
    for path in paths {
        let input = read_era_yaml(path)?;
        let mut era: EraDefinition = serde_saphyr::from_str(&input)
            .with_context(|| format!("failed to parse era definition from {}", path.display()))?;
        normalise_file(&mut era);
        era.validate()
            .with_context(|| format!("invalid era definition in {}", path.display()))?;
        ensure!(
            ids.insert(era.id.clone()),
            "duplicate era {} in input",
            era.id
        );
        desired.push(era);
    }
    Ok(desired)
}

async fn delete(database: &DatabaseConnection, id: &str, yes: bool) -> anyhow::Result<()> {
    if !yes {
        bail!("deleting an era requires --yes");
    }
    let transaction = database.begin().await?;
    lock(&transaction).await?;
    let result = EraEntity::delete_many()
        .filter(EraColumn::Slug.eq(id))
        .exec(&transaction)
        .await?;
    ensure!(result.rows_affected == 1, "era {id:?} was not found");
    eras::validate_version_requirements(&transaction)
        .await
        .context("deleting the era would make the catalogue invalid")?;
    transaction
        .commit()
        .await
        .context("failed to delete era; stored hotlaps may still reference it")?;
    println!("Era {id} deleted.");
    Ok(())
}

async fn set_open(database: &DatabaseConnection, id: &str, open: bool) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    lock(&transaction).await?;
    let model = EraEntity::find()
        .filter(EraColumn::Slug.eq(id))
        .one(&transaction)
        .await?
        .with_context(|| format!("era {id:?} was not found"))?;
    if model.open == open {
        transaction.rollback().await?;
        println!(
            "Era {id} is already {}.",
            if open { "open" } else { "closed" }
        );
        return Ok(());
    }
    let mut active = model.into_active_model();
    active.open = Set(open);
    active.update(&transaction).await?;
    transaction.commit().await?;
    println!("Era {id} {}.", if open { "opened" } else { "closed" });
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn persist(
    transaction: &DatabaseTransaction,
    existing: Option<EraModel>,
    desired: &EraDefinition,
) -> anyhow::Result<()> {
    let version_requirement = desired.version_requirement.clone();
    let era = if let Some(existing) = existing {
        let mut active = existing.into_active_model();
        active.title = Set(desired.title.trim().to_owned());
        active.installation_id = Set(desired.installation_id.clone());
        active.version_requirement = Set(version_requirement);
        active.open = Set(desired.open);
        active.update(transaction).await?
    } else {
        EraMutation {
            id: NotSet,
            slug: Set(desired.id.clone()),
            title: Set(desired.title.trim().to_owned()),
            installation_id: Set(desired.installation_id.clone()),
            version_requirement: Set(version_requirement),
            open: Set(desired.open),
        }
        .insert(transaction)
        .await?
    };

    let existing_rankings = RankingEntity::find()
        .filter(RankingColumn::EraId.eq(era.id))
        .all(transaction)
        .await?
        .into_iter()
        .map(|ranking| (ranking.slug.clone(), ranking))
        .collect::<HashMap<_, _>>();
    for (position, definition) in desired.rankings.iter().enumerate() {
        let position = i32::try_from(position).context("too many rankings for one era")?;
        let badge = definition
            .badge
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .context("failed to serialize ranking badge")?;
        let ranking = if let Some(existing) = existing_rankings.get(definition.id.as_str()) {
            let mut active = existing.clone().into_active_model();
            active.position = Set(position);
            active.title = Set(definition.title.trim().to_owned());
            active.description = Set(definition.description.trim().to_owned());
            active.benchmark_percent = Set(definition.rules.benchmark_percent);
            active.nation_max_points = Set(definition.rules.nation_max_points);
            active.nation_driver_limit = Set(definition.rules.nation_driver_limit);
            active.badge = Set(badge);
            active.update(transaction).await?
        } else {
            RankingMutation {
                id: NotSet,
                era_id: Set(era.id),
                slug: Set(definition.id.to_string()),
                position: Set(position),
                title: Set(definition.title.trim().to_owned()),
                description: Set(definition.description.trim().to_owned()),
                benchmark_percent: Set(definition.rules.benchmark_percent),
                nation_max_points: Set(definition.rules.nation_max_points),
                nation_driver_limit: Set(definition.rules.nation_driver_limit),
                badge: Set(badge),
            }
            .insert(transaction)
            .await?
        };

        RankingChartEntity::delete_many()
            .filter(crate::models::rankings::RankingChartColumn::RankingId.eq(ranking.id))
            .exec(transaction)
            .await?;

        let charts = definition
            .combinations
            .pairs()
            .enumerate()
            .map(|(chart_position, chart)| {
                Ok(RankingChartMutation {
                    era_id: Set(era.id),
                    ranking_id: Set(ranking.id),
                    position: Set(
                        i32::try_from(chart_position).context("too many charts for one ranking")?
                    ),
                    track_id: Set(chart.track.to_string()),
                    vehicle_id: Set(chart.vehicle.to_string()),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        RankingChartEntity::insert_many(charts)
            .exec(transaction)
            .await?;
    }
    let desired_slugs = desired
        .rankings
        .iter()
        .map(|ranking| ranking.id.to_string())
        .collect::<Vec<_>>();
    RankingEntity::delete_many()
        .filter(RankingColumn::EraId.eq(era.id))
        .filter(RankingColumn::Slug.is_not_in(desired_slugs))
        .exec(transaction)
        .await?;
    Ok(())
}

fn combination_keys(era: &EraDefinition) -> HashSet<(String, String)> {
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

fn validate_ranking_charts(definition: &definition::RankingDefinition) -> anyhow::Result<()> {
    let combinations = &definition.combinations;
    ensure!(
        combinations.pairs().next().is_some(),
        "ranking {} must contain combinations",
        definition.id
    );
    let mut chart_keys = HashSet::new();
    for chart in combinations.pairs() {
        ensure!(
            chart.track.is_hotlap_rankable(),
            "ranking {} uses ineligible track {}",
            definition.id,
            chart.track
        );
        ensure!(
            chart.vehicle.is_hotlap_rankable(),
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

fn validate_installation_id(id: &str) -> Result<(), ValidationError> {
    if crate::lfs::valid_installation_id(id) {
        return Ok(());
    }
    Err(validation_error(
        "invalid_installation_id",
        "installation must be a path-safe identifier containing lowercase ASCII letters, digits, '.', '_' or '-'",
    ))
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

fn validate_chart_matrix(ranking: &definition::RankingDefinition) -> anyhow::Result<()> {
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
            track.is_hotlap_rankable(),
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
            vehicle.is_hotlap_rankable(),
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

fn normalise_file(file: &mut EraDefinition) {
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

fn describe_change(existing: Option<&EraDefinition>, desired: &EraDefinition) {
    let Some(existing) = existing else {
        println!(
            "Create era {}: {} combinations, {} rankings.",
            desired.id,
            combination_keys(desired).len(),
            desired.rankings.len()
        );
        return;
    };
    if existing == desired {
        return;
    }
    println!("Update era {}:", desired.id);
    if existing.title != desired.title {
        println!("  title: {:?} -> {:?}", existing.title, desired.title);
    }
    if existing.version_requirement != desired.version_requirement {
        println!(
            "  version: {:?} -> {:?}",
            existing.version_requirement, desired.version_requirement
        );
    }
    if existing.installation_id != desired.installation_id {
        println!(
            "  installation: {:?} -> {:?}",
            existing.installation_id, desired.installation_id
        );
    }
    if existing.open != desired.open {
        println!("  open: {} -> {}", existing.open, desired.open);
    }
    let before = combination_keys(existing);
    let after = combination_keys(desired);
    let mut added: Vec<_> = after.difference(&before).collect();
    let mut removed: Vec<_> = before.difference(&after).collect();
    added.sort();
    removed.sort();
    for (track, vehicle) in added {
        println!("  combination added: {track}/{vehicle}");
    }
    for (track, vehicle) in removed {
        println!("  combination removed: {track}/{vehicle}");
    }

    let existing_ids = existing
        .rankings
        .iter()
        .map(|ranking| ranking.id.as_str())
        .collect::<Vec<_>>();
    let desired_ids = desired
        .rankings
        .iter()
        .map(|ranking| ranking.id.as_str())
        .collect::<Vec<_>>();
    if existing_ids != desired_ids {
        println!("  ranking order: {existing_ids:?} -> {desired_ids:?}");
    }
    for ranking in &desired.rankings {
        match existing
            .rankings
            .iter()
            .find(|candidate| candidate.id == ranking.id)
        {
            None => println!("  ranking added: {}", ranking.id),
            Some(previous) if previous != ranking => {
                println!("  ranking changed: {}", ranking.id);
            }
            Some(_) => {}
        }
    }
    for ranking in &existing.rankings {
        if !desired
            .rankings
            .iter()
            .any(|candidate| candidate.id == ranking.id)
        {
            println!("  ranking removed: {}", ranking.id);
        }
    }
}

async fn lock(transaction: &DatabaseTransaction) -> anyhow::Result<()> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [eras::CATALOGUE_LOCK.into()],
        ))
        .await?;
    Ok(())
}

/// Reports hotlaps outside the proposed combinations. Leaves them unchanged.
async fn describe_affected_hotlaps<C: ConnectionTrait>(
    database: &C,
    era_id: i64,
    desired: &EraDefinition,
) -> anyhow::Result<()> {
    let pairs: Vec<_> = combination_keys(desired)
        .into_iter()
        .map(|(track, vehicle)| serde_json::json!({"track": track, "vehicle": vehicle}))
        .collect();
    let rows = database
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT track, vehicle, state, COUNT(*)::BIGINT AS laps
         FROM hotlap
         WHERE era_id = $1 AND vehicle IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM jsonb_to_recordset($2::jsonb) AS pair(track TEXT, vehicle TEXT)
             WHERE pair.track = hotlap.track AND pair.vehicle = hotlap.vehicle)
         GROUP BY track, vehicle, state ORDER BY track, vehicle, state",
            [era_id.into(), serde_json::json!(pairs).into()],
        ))
        .await?;
    for row in rows {
        let track: String = row.try_get("", "track")?;
        let vehicle: String = row.try_get("", "vehicle")?;
        let state: String = row.try_get("", "state")?;
        let laps: i64 = row.try_get("", "laps")?;
        println!(
            "  outside defined combinations: {track}/{vehicle}: {laps} {state} laps (history retained)"
        );
    }
    Ok(())
}
