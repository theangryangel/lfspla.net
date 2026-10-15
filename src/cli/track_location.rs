//! Operator CLI for authored track locations.

use crate::{
    cli::Args,
    db,
    models::track::{self, location},
    services::catalogue::locations,
    settings::Settings,
};
use anyhow::ensure;
use clap::Subcommand;
use sea_orm::{EntityTrait, QueryOrder};

/// Track location operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum TrackLocationCommand {
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

pub(crate) async fn run(args: &Args, action: &TrackLocationCommand) -> anyhow::Result<()> {
    if let TrackLocationCommand::Delete { yes, .. } = action {
        ensure!(*yes, "deleting a track location requires --yes");
    }
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    match action {
        TrackLocationCommand::List => {
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
        TrackLocationCommand::Delete { id, .. } => {
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

    #[test]
    fn track_location_commands_parse() {
        assert!(Args::try_parse_from(["lfsplanet", "track-location", "list"]).is_ok());
        let args =
            Args::try_parse_from(["lfsplanet", "track-location", "delete", "OTHER", "--yes"])
                .unwrap();
        assert_eq!(
            args.command,
            Command::TrackLocation {
                action: TrackLocationCommand::Delete {
                    id: "OTHER".into(),
                    yes: true
                },
            }
        );
    }

    #[tokio::test]
    async fn deletion_requires_confirmation_before_connecting() {
        let args = Args::try_parse_from([
            "lfsplanet",
            "--config",
            "/missing/planet.yaml",
            "track-location",
            "delete",
            "OTHER",
        ])
        .unwrap();
        let Command::TrackLocation { action } = &args.command else {
            unreachable!()
        };
        let error = run(&args, action).await.unwrap_err();
        assert!(error.to_string().contains("requires --yes"));
    }
}
