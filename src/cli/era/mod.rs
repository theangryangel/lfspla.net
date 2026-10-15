//! Operator CLI for database-backed era policy.

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

use crate::{models::era::definition, services::catalogue::eras};

use crate::{
    cli::Args,
    db,
    models::era::{Column as EraColumn, Entity as EraEntity},
    settings::Settings,
};
use anyhow::{Context, bail};
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

/// Runs one era catalogue operator command.
pub(crate) async fn run(args: &Args, action: &EraCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;

    match action {
        EraCommand::List => list(&database).await,
        EraCommand::Export { id } => export(&database, id).await,
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
    let yaml = serde_saphyr::to_string(&crate::services::catalogue::CatalogueDefinition::Era(era))
        .context("failed to serialize era as YAML")?;
    print!("{yaml}");
    Ok(())
}

async fn delete(database: &DatabaseConnection, id: &EraSlug, yes: bool) -> anyhow::Result<()> {
    if !yes {
        bail!("deleting an era requires --yes");
    }
    eras::delete(database, id).await?;
    println!("Era {id} deleted.");
    Ok(())
}

async fn set_open(database: &DatabaseConnection, id: &EraSlug, open: bool) -> anyhow::Result<()> {
    if !eras::set_open(database, id, open).await? {
        println!(
            "Era {id} is already {}.",
            if open { "open" } else { "closed" }
        );
        return Ok(());
    }
    println!("Era {id} {}.", if open { "opened" } else { "closed" });
    Ok(())
}
