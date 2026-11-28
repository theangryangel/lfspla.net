//! Synchronise upstream Vehicle Mod metadata and covers.
mod details;
mod images;
use anyhow::Context;
use lfsplanet_core::settings::StorageSettings;
use lfsplanet_lfs_net_api::LfsClient;
use sea_orm::DatabaseConnection;
pub async fn sync_mods(
    client: LfsClient,
    storage: &StorageSettings,
    database: &DatabaseConnection,
) -> anyhow::Result<()> {
    let object_store = lfsplanet_core::storage::build(storage)?;
    let previous_images = lfsplanet_core::models::Vehicle::image_cache_state(database)
        .await
        .context("Vehicle Mods image state loading failed")?;
    let remote = client
        .vehicle_mods()
        .await
        .context("Vehicle Mods catalogue refresh failed")?;
    lfsplanet_core::models::Vehicle::refresh(database, &remote)
        .await
        .context("Vehicle Mods catalogue persistence failed")?;
    let details = details::cache_details(database, &client, &remote).await?;
    let images = images::cache_images(database, object_store, client, &remote, &previous_images)
        .await
        .context("Vehicle Mods cover synchronization failed")?;
    let mods = remote.len();

    tracing::info!(mods, images, details, "Vehicle Mods synchronized");
    Ok(())
}
