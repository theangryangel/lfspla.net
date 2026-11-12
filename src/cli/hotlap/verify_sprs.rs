//! Scoped verification and optional removal of invalid stored replays.
use crate::{
    cli::Args,
    db,
    era_slug::EraSlug,
    models::{Player, attachment, era, hotlap},
    settings::Settings,
};
use anyhow::{Context, ensure};
use object_store::{ObjectStoreExt, path::Path};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::{collections::BTreeSet, io::Cursor};

#[derive(Clone, Debug, Eq, PartialEq, clap::Subcommand)]
pub(crate) enum Scope {
    /// Verify an era's replays, optionally restricted to one player.
    Era {
        era: EraSlug,
        #[arg(long)]
        player: Option<String>,
    },
    /// Verify one stored hotlap's replay.
    Hotlap {
        #[arg(value_parser = clap::value_parser!(i64).range(1..))]
        id: i64,
    },
    /// Verify one player's replays across eras.
    Player { username: String },
}

pub(super) async fn run(args: &Args, scope: &Scope, delete_invalid: bool) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let store = crate::storage::build(&settings.storage)?;
    let mut query = hotlap::Entity::find();
    let username = match scope {
        Scope::Era { era: slug, player } => {
            let era = era::Entity::find()
                .filter(era::Column::Slug.eq(slug.clone()))
                .one(&database)
                .await?
                .with_context(|| format!("era {slug} was not found"))?;
            query = query.filter(hotlap::Column::EraId.eq(era.id));
            player.as_deref()
        }
        Scope::Player { username } => Some(username.as_str()),
        Scope::Hotlap { id } => {
            query = query.filter(hotlap::Column::Id.eq(*id));
            None
        }
    };
    if let Some(username) = username {
        let player = Player::find_by_username(&database, username)
            .await?
            .with_context(|| format!("player {username} was not found"))?;
        query = query.filter(hotlap::Column::PlayerId.eq(player.id));
    }
    // Fix an upper bound so new uploads cannot indefinitely extend the audit.
    let upper = query
        .clone()
        .order_by_desc(hotlap::Column::Id)
        .one(&database)
        .await?;
    let Some(upper) = upper else {
        tracing::info!(?scope, "no hotlaps match the selected scope");
        return Ok(());
    };
    query = query.filter(hotlap::Column::Id.lte(upper.id));
    let (mut checked, mut invalid, mut missing, mut failures) = (0, 0, 0, 0);
    let (mut deleted, mut deletion_failures) = (0, 0);
    let mut affected_eras = BTreeSet::new();
    let mut after = 0;
    loop {
        let rows = query
            .clone()
            .filter(hotlap::Column::Id.gt(after))
            .order_by_asc(hotlap::Column::Id)
            .limit(100)
            .all(&database)
            .await?;
        if rows.is_empty() {
            break;
        }
        for lap in rows {
            after = lap.id;
            let Some(attachment_id) = lap.spr_attachment_id else {
                missing += 1;
                continue;
            };
            let result = async {
                let attachment = attachment::Model::required(&database, attachment_id).await?;
                let bytes = store
                    .get(&Path::from(attachment.object_key))
                    .await?
                    .bytes()
                    .await?;
                Ok::<_, anyhow::Error>(lfsplanet_spr::SprHeader::read(Cursor::new(bytes)))
            }
            .await;
            let (reason, rejected) = match result {
                Ok(Ok(_)) => {
                    checked += 1;
                    continue;
                }
                Ok(Err(error)) => {
                    checked += 1;
                    invalid += 1;
                    (error.to_string(), true)
                }
                Err(error) => {
                    failures += 1;
                    (format!("could not read SPR: {error:#}"), false)
                }
            };
            tracing::warn!(
                hotlap_id = lap.id,
                player_id = lap.player_id,
                era_id = lap.era_id,
                track = %lap.track,
                vehicle = %lap.vehicle,
                state = ?lap.state,
                %reason,
                "SPR verification failed"
            );
            if delete_invalid && rejected {
                match crate::models::Hotlap::delete_by_id(&database, lap.id).await {
                    Ok(Some((era_id, published))) => {
                        deleted += 1;
                        if published {
                            affected_eras.insert(era_id);
                        }
                        tracing::info!(hotlap_id = lap.id, era_id, "invalid hotlap deleted");
                    }
                    Ok(None) => tracing::info!(hotlap_id = lap.id, "hotlap already removed"),
                    Err(error) => {
                        deletion_failures += 1;
                        tracing::error!(hotlap_id = lap.id, %error, "failed to delete invalid hotlap");
                    }
                }
            }
        }
    }
    for era_id in affected_eras {
        let result = async {
            let era = era::Entity::find_by_id(era_id)
                .one(&database)
                .await?
                .context("hotlap era missing")?;
            era.rebuild_badges(&database).await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            deletion_failures += 1;
            tracing::error!(era_id, %error, "hotlaps deleted but badge rebuild failed; run hotlap rebadge");
        }
    }
    tracing::info!(
        ?scope,
        delete_invalid,
        deleted,
        deletion_failures,
        checked,
        invalid,
        without_spr = missing,
        read_failures = failures,
        "SPR verification complete"
    );
    ensure!(
        failures == 0 && deletion_failures == 0,
        "verification incomplete: {failures} SPR read failures; {deletion_failures} deletion or badge rebuild failures"
    );
    Ok(())
}
