//! Operator output and arguments for local catalogue seeding.

use crate::{
    db,
    models::era::definition::{EraDefinition, combination_keys},
    services::catalogue,
    settings::Settings,
};
use anyhow::bail;
use clap::Args;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Args)]
pub(crate) struct SeedArgs {
    /// Tagged catalogue YAML files; shell glob expansion is supported.
    #[arg(value_name = "FILE", required = true)]
    files: Vec<PathBuf>,
    /// Report changes and validate proposed rows, then roll back.
    #[arg(long)]
    dry_run: bool,
    /// Confirm replacement of existing era definitions.
    #[arg(long)]
    yes: bool,
}

pub(crate) async fn run(args: &super::Args, options: &SeedArgs) -> anyhow::Result<()> {
    let documents = catalogue::load::files(&options.files)?;
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let store = if options.dry_run {
        None
    } else {
        Some(crate::storage::build(&settings.storage)?)
    };
    let applied = catalogue::seed(
        &database,
        &documents,
        store.as_deref(),
        options.dry_run,
        |changes| review_changes(changes, options.yes, options.dry_run),
    )
    .await?;
    report_applied(
        &applied.eras,
        options.dry_run,
        !applied.locations.is_empty()
            || !applied.tracks.is_empty()
            || !applied.vehicles.is_empty()
            || !applied.community_cards.is_empty()
            || !applied.badges.is_empty(),
    );
    for id in &applied.locations {
        println!(
            "Location {id} {}.",
            if options.dry_run {
                "would change"
            } else {
                "applied"
            }
        );
    }
    for id in &applied.tracks {
        println!(
            "Track {id} {}.",
            if options.dry_run {
                "would change"
            } else {
                "applied"
            }
        );
    }
    for id in &applied.vehicles {
        println!(
            "Vehicle {id} {}.",
            if options.dry_run {
                "would change"
            } else {
                "applied"
            }
        );
    }
    for id in &applied.badges {
        println!(
            "Badge {id} {}.",
            if options.dry_run {
                "would change"
            } else {
                "applied"
            }
        );
    }
    for id in &applied.community_cards {
        println!(
            "Community card {id} {}.",
            if options.dry_run {
                "would change"
            } else {
                "applied"
            }
        );
    }
    Ok(())
}

fn review_changes(
    changes: &[catalogue::eras::EraChange],
    yes: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
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
        && !dry_run
        && changes.iter().any(|change| {
            change.existing.is_some() && change.existing.as_ref() != Some(&change.desired)
        })
    {
        bail!("changing an existing era requires --yes");
    }
    Ok(())
}

fn report_applied(applied: &[crate::era_slug::EraSlug], dry_run: bool, artwork_changed: bool) {
    if dry_run {
        println!("Dry run complete; no catalogue changes committed.");
    } else if applied.is_empty() && !artwork_changed {
        println!("No changes.");
    }
    for id in applied {
        println!("Era {id} applied.");
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
