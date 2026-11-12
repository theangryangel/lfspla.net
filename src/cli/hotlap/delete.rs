//! Operator removal of a hotlap using the shared ranking repair lifecycle.
use crate::{
    cli::Args,
    db,
    models::{Era, Hotlap},
    settings::Settings,
};
use anyhow::Context;

pub(super) async fn run(args: &Args, id: i64) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 2).await?;
    let (era_id, published) = Hotlap::delete_by_id(&database, id)
        .await?
        .with_context(|| format!("hotlap {id} was not found"))?;
    if published {
        use sea_orm::EntityTrait;
        let era = crate::models::era::Entity::find_by_id(era_id)
            .one(&database)
            .await?
            .context("hotlap era missing")?;
        Era::rebuild_badges(&era, &database)
            .await
            .context("hotlap deleted, but badge rebuilding failed; run hotlap rebadge")?;
    }
    tracing::info!(
        hotlap_id = id,
        era_id,
        published,
        "hotlap deleted; unreferenced attachments will be cleared at next storage-gc"
    );
    Ok(())
}
