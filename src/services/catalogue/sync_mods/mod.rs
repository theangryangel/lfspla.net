//! Synchronise upstream Vehicle Mod metadata and covers.
mod images;
use crate::settings::Settings;
use anyhow::Context;
use sea_orm::DatabaseConnection;
pub(crate) async fn sync_mods(
    settings: &Settings,
    database: &DatabaseConnection,
) -> anyhow::Result<()> {
    let client = settings.lfs.to_api_client()?.context(
        "LFS OAuth credentials are required for Vehicle Mod synchronization; configure lfs.oauth",
    )?;

    let object_store = crate::storage::build(&settings.storage)?;
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

    tracing::info!(mods, images, "Vehicle Mods synchronized");
    Ok(())
}
