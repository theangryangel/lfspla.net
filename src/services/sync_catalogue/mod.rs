//! Synchronise canonical tracks, vehicles, upstream mod metadata, and artwork.
mod images;
use crate::{lfs::api_client, settings::Settings};
use anyhow::Context;
use sea_orm::DatabaseConnection;
pub(crate) async fn sync(
    settings: &Settings,
    database: &DatabaseConnection,
    standard_vehicle_images_dir: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let client = api_client(&settings.lfs)?.context(
        "LFS OAuth credentials are required for catalogue synchronization; configure lfs.oauth",
    )?;

    crate::models::Track::sync(database)
        .await
        .context("canonical track synchronization failed")?;
    crate::models::Vehicle::sync_builtin(database)
        .await
        .context("built-in vehicle synchronization failed")?;
    let object_store = crate::storage::build(&settings.storage)?;
    let builtin_images = match standard_vehicle_images_dir {
        Some(directory) => images::seed_builtin_images(database, object_store.clone(), directory)
            .await
            .context("built-in vehicle image seeding failed")?,
        None => 0,
    };

    let previous_images = crate::models::Vehicle::image_cache_state(database)
        .await
        .context("Vehicle Mods image state loading failed")?;
    let remote = client
        .vehicle_mods()
        .await
        .context("Vehicle Mods catalogue refresh failed")?;
    crate::models::Vehicle::refresh(database, &remote)
        .await
        .context("Vehicle Mods catalogue persistence failed")?;
    let images = images::cache_images(database, object_store, client, &remote, &previous_images)
        .await
        .context("Vehicle Mods cover synchronization failed")?;
    let mods = remote.len();

    tracing::info!(mods, images, builtin_images, "catalogue synchronized");
    Ok(())
}
