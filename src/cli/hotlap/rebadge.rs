//! Manual player badge rebuilds.

use super::RebadgeScope;
use crate::{cli::Args, db, models::era, settings::Settings};
use anyhow::{Context, bail};
use sea_orm::{EntityTrait, QueryOrder};
use std::collections::HashSet;

/// Rebuilds every badge for the selected eras.
///
/// Use this for repairs after era policy changes or badge drift.
pub(super) async fn run(args: &Args, scope: &RebadgeScope) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let eras = era::Entity::find()
        .order_by_asc(era::Column::Id)
        .all(&database)
        .await
        .context("failed to load eras for badge rebuilding")?;

    let selected = if scope.all {
        eras.iter().collect::<Vec<_>>()
    } else if scope.open {
        eras.iter().filter(|era| era.open).collect::<Vec<_>>()
    } else {
        let mut seen = HashSet::with_capacity(scope.eras.len());
        let mut selected = Vec::with_capacity(scope.eras.len());
        for era_id in &scope.eras {
            if !seen.insert(era_id) {
                bail!("era {era_id} was selected more than once");
            }
            selected.push(
                eras.iter()
                    .find(|era| era.slug == *era_id)
                    .with_context(|| format!("era {era_id} was not found"))?,
            );
        }
        selected
    };

    for era in selected {
        let players = era
            .rebuild_badges(&database)
            .await
            .with_context(|| format!("failed to rebuild badges for era {}", era.slug))?;
        tracing::info!(era_id = era.id, players, "player badges rebuilt");
    }
    Ok(())
}
