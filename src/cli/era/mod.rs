//! Operator CLI for database-backed era policy.

use std::path::PathBuf;

use crate::era_slug::EraSlug;
use clap::Subcommand;

/// Era policy operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum EraCommand {
    /// List configured eras.
    List,
    /// Write one era definition as YAML to stdout.
    Export {
        /// Stable era identifier.
        id: EraSlug,
    },
    /// Create or replace one or more eras from YAML; use - to read stdin.
    Apply {
        /// Era YAML paths, or - for stdin.
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,
        /// Confirm replacement of an existing era.
        #[arg(long)]
        yes: bool,
    },
    /// Delete an era with no stored references.
    Delete {
        /// Stable era identifier.
        id: EraSlug,
        /// Confirm deletion.
        #[arg(long)]
        yes: bool,
    },
    /// Open an era for hotlap uploads.
    Open {
        /// Stable era identifier.
        id: EraSlug,
    },
    /// Close an era to new hotlap uploads.
    Close {
        /// Stable era identifier.
        id: EraSlug,
    },
}

use crate::{
    models::era::definition::{self, combination_keys, normalise_file},
    services::apply_eras,
};
#[cfg(test)]
mod tests;

use definition::EraDefinition;

use crate::{
    cli::Args,
    db,
    models::era::{Column as EraColumn, Entity as EraEntity},
    settings::Settings,
};
use anyhow::{Context, bail, ensure};
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};
use std::{collections::HashSet, io::Read, path::Path};
use validator::Validate;

/// The path that selects standard input instead of an era YAML file.
const STDIN_PATH: &str = "-";

/// Runs one era catalogue operator command.
pub(crate) async fn run(args: &Args, action: &EraCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;

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

async fn export(database: &DatabaseConnection, id: &EraSlug) -> anyhow::Result<()> {
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

    let applied = apply_eras::apply(database, &desired, |changes| {
        for change in changes {
            describe_change(change.existing.as_ref(), &change.desired);
            for affected in &change.affected {
                println!(
                    "  outside defined combinations: {}/{}: {} {} laps (will be deleted)",
                    affected.track, affected.vehicle, affected.laps, affected.state
                );
            }
        }
        if !yes
            && changes.iter().any(|change| {
                change.existing.is_some() && change.existing.as_ref() != Some(&change.desired)
            })
        {
            bail!("changing an existing era requires --yes");
        }
        Ok(())
    })
    .await?;
    if applied.is_empty() {
        println!("No changes.");
    }
    for id in applied {
        println!("Era {id} applied.");
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

async fn delete(database: &DatabaseConnection, id: &EraSlug, yes: bool) -> anyhow::Result<()> {
    if !yes {
        bail!("deleting an era requires --yes");
    }
    apply_eras::delete(database, id).await?;
    println!("Era {id} deleted.");
    Ok(())
}

async fn set_open(database: &DatabaseConnection, id: &EraSlug, open: bool) -> anyhow::Result<()> {
    if !apply_eras::set_open(database, id, open).await? {
        println!(
            "Era {id} is already {}.",
            if open { "open" } else { "closed" }
        );
        return Ok(());
    }
    println!("Era {id} {}.", if open { "opened" } else { "closed" });
    Ok(())
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
