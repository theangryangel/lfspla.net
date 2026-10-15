//! Cache remote Vehicle Mod covers.
use crate::{
    models::attachment::Model as Attachment,
    models::vehicle::{Column as VehicleColumn, Entity as VehicleEntity},
};
use futures::stream::{self, StreamExt};
use lfsplanet_lfs_api::{LfsClient, RemoteVehicleMod};
use object_store::ObjectStore;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr,
};
use std::{collections::HashMap, sync::Arc};
const IMAGE_DOWNLOAD_CONCURRENCY: usize = 4;

/// Downloads covers for new Mods and when their LFS revision changes.
///
/// A failed refresh leaves a previously cached image intact. This keeps
/// catalogue synchronization resilient to a temporary CDN failure.
pub(crate) async fn cache_images(
    database: &DatabaseConnection,
    object_store: Arc<dyn ObjectStore>,
    client: LfsClient,
    remote: &[RemoteVehicleMod],
    previous: &HashMap<String, (Option<i16>, Option<i64>)>,
) -> Result<usize, sea_orm::DbErr> {
    let pending = remote
        .iter()
        .filter_map(|modification| {
            let source = modification.cover_thumb_url.as_ref()?;
            let cached = previous.get(&modification.id);
            let current = cached.is_some_and(|(version, object_key)| {
                *version == Some(i16::try_from(modification.version).unwrap_or_default())
                    && object_key.is_some()
            });
            (!current).then(|| {
                (
                    modification.id.clone(),
                    modification.version,
                    source.clone(),
                )
            })
        })
        .collect::<Vec<_>>();

    let cached = stream::iter(pending)
        .map(|(id, version, source)| {
            let database = database.clone();
            let object_store = Arc::clone(&object_store);
            let client = client.clone();
            async move {
                let version = i16::try_from(version)?;
                let cover = client
                    .vehicle_mod_cover(&source)
                    .await
                    .map_err(anyhow::Error::from)?;
                let transaction = database.begin().await?;
                crate::db::lock_catalogue_write(&transaction).await?;
                let extension = image_extension(&cover.content_type);
                let attachment = Attachment::store(
                    &transaction,
                    &*object_store,
                    cover.bytes.to_vec(),
                    extension,
                    &cover.content_type,
                    None,
                )
                .await?;
                let updated = VehicleEntity::update_many()
                    .filter(VehicleColumn::Id.eq(&id))
                    // A slower sync must not replace an image for a newer revision.
                    .filter(VehicleColumn::Version.eq(version))
                    .col_expr(VehicleColumn::ImageAttachmentId, Expr::value(attachment.id))
                    .col_expr(VehicleColumn::ImageVersion, Expr::value(version))
                    .col_expr(
                        VehicleColumn::ImageFetchedAt,
                        Expr::value(time::OffsetDateTime::now_utc()),
                    )
                    .exec(&transaction)
                    .await?;
                transaction.commit().await?;
                Ok::<bool, anyhow::Error>(updated.rows_affected == 1)
            }
        })
        .buffer_unordered(IMAGE_DOWNLOAD_CONCURRENCY)
        .fold(0usize, |cached, result| async move {
            match result {
                Ok(updated) => cached + usize::from(updated),
                Err(error) => {
                    tracing::warn!(?error, "Vehicle Mod cover download failed");
                    cached
                }
            }
        })
        .await;
    Ok(cached)
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}
