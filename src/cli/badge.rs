//! Operator-managed lifetime badge grants.
use crate::{
    cli::Args,
    db,
    models::{Player, badge},
    settings::Settings,
};
use anyhow::Context;
use clap::Subcommand;
use sea_orm::TransactionTrait;

#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum BadgeCommand {
    /// Grant a lifetime badge, or update its variant and public description.
    Grant {
        badge: String,
        lfs_username: String,
        #[arg(long, default_value = "standard")]
        variant: String,
        /// Public award text; defaults to the catalogue description.
        #[arg(long)]
        description: Option<String>,
    },
    /// Revoke a manual lifetime grant; automatic awards are unaffected.
    Revoke { badge: String, lfs_username: String },
    /// List a player's manual grants.
    List { lfs_username: String },
}

pub(crate) async fn run(args: &Args, action: &BadgeCommand) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let username = match action {
        BadgeCommand::Grant { lfs_username, .. }
        | BadgeCommand::Revoke { lfs_username, .. }
        | BadgeCommand::List { lfs_username } => lfs_username,
    };
    let transaction = database.begin().await?;
    db::lock_catalogue_write(&transaction).await?;
    let player = Player::find_by_username(&transaction, username)
        .await?
        .with_context(|| format!("player {username:?} was not found"))?;
    match action {
        BadgeCommand::Grant {
            badge,
            variant,
            description,
            ..
        } => {
            crate::services::catalogue::badges::validate_slug(badge)?;
            crate::services::catalogue::badges::validate_slug(variant)?;
            if let Some(description) = description {
                crate::models::community_card::text(description, "award description", 1000)?;
            }
            badge::grant_manual(
                &transaction,
                player.id,
                badge,
                variant,
                description.as_deref(),
            )
            .await
            .context("cannot grant badge; seed its catalogue definition and variant first")?;
            transaction.commit().await?;
            println!(
                "Badge {badge}/{variant} granted to {}.",
                player.lfs_username
            );
        }
        BadgeCommand::Revoke { badge, .. } => {
            let removed = badge::revoke_manual(&transaction, player.id, badge).await?;
            transaction.commit().await?;
            println!(
                "Removed {removed} manual grant(s) of {badge} from {}.",
                player.lfs_username
            );
        }
        BadgeCommand::List { .. } => {
            for award in badge::Model::list_manual(&transaction, player.id).await? {
                println!(
                    "{}\t{}\t{}",
                    award.badge_id,
                    award.variant,
                    award.detail.unwrap_or_default()
                );
            }
            transaction.commit().await?;
        }
    }
    Ok(())
}
