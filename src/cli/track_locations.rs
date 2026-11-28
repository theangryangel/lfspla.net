//! Operator CLI for authored track locations.

use crate::{cli::Args, settings::Settings};
use anyhow::ensure;
use clap::Subcommand;
use lfsplanet_core::{
    db,
    models::track::{self, location},
};
use lfsplanet_hotlaps::services::catalogue::locations;
use sea_orm::{EntityTrait, QueryOrder};

/// Track location operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum TrackLocationsCommand {
    /// List locations with their track counts.
    List,
    /// Delete a location that no tracks reference.
    Delete {
        /// Location code, as declared in its YAML definition.
        id: String,
        /// Confirm deletion.
        #[arg(long)]
        yes: bool,
    },
}

pub(crate) async fn run(args: &Args, action: &TrackLocationsCommand) -> anyhow::Result<()> {
    if let TrackLocationsCommand::Delete { yes, .. } = action {
        ensure!(*yes, "deleting a track location requires --yes");
    }
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    match action {
        TrackLocationsCommand::List => {
            let rows = location::Entity::find()
                .order_by_asc(location::Column::Sequence)
                .order_by_asc(location::Column::Id)
                .find_with_related(track::Entity)
                .all(&database)
                .await?;
            if rows.is_empty() {
                println!("No track locations configured.");
            }
            for (location, tracks) in rows {
                println!("{}\t{}\t{}", location.id, tracks.len(), location.name);
            }
        }
        TrackLocationsCommand::Delete { id, .. } => {
            locations::delete(&database, id).await?;
            println!("Track location {id} deleted.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Command;
    use clap::Parser;

    #[tokio::test]
    async fn deletion_requires_confirmation_before_connecting() {
        let args = Args::try_parse_from([
            "lfsplanet",
            "--config",
            "/missing/planet.yaml",
            "track-locations",
            "delete",
            "OTHER",
        ])
        .unwrap();
        let Command::TrackLocations { action } = &args.command else {
            unreachable!()
        };
        let error = run(&args, action).await.unwrap_err();
        assert!(error.to_string().contains("requires --yes"));
    }
}
