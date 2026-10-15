//! Read local catalogue artwork and preserve unchanged attachment identities.

use crate::models::attachment::Model as Attachment;
use anyhow::{Context, bail};
use object_store::ObjectStore;
use sea_orm::DatabaseTransaction;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) struct Asset {
    bytes: Vec<u8>,
    extension: String,
    media_type: &'static str,
}

pub(super) fn read(source: &Path, file: &Path) -> anyhow::Result<Asset> {
    let path = source.parent().unwrap_or_else(|| Path::new(".")).join(file);
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        _ => bail!(
            "unsupported artwork format in {}: {}",
            source.display(),
            path.display()
        ),
    };
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "failed to read artwork {} referenced by {}",
            path.display(),
            source.display()
        )
    })?;
    anyhow::ensure!(
        !bytes.is_empty(),
        "empty artwork {} referenced by {}",
        path.display(),
        source.display()
    );
    Ok(Asset {
        bytes,
        extension,
        media_type,
    })
}

pub(super) enum AssetChange {
    Unchanged,
    WouldChange,
    Stored(i64),
}

/// Reuses the current attachment when file content has not changed.
pub(super) async fn store(
    transaction: &DatabaseTransaction,
    object_store: Option<&dyn ObjectStore>,
    asset: Asset,
    current: Option<i64>,
    file: &Path,
    dry_run: bool,
) -> anyhow::Result<AssetChange> {
    let digest = hex::encode(Sha256::digest(&asset.bytes));
    if let Some(id) = current {
        let attachment = Attachment::required(transaction, id).await?;
        if attachment.sha256.as_deref() == Some(&digest)
            && attachment.media_type == asset.media_type
        {
            return Ok(AssetChange::Unchanged);
        }
    }
    // A dry run reports a change but does not upload or create an attachment.
    if dry_run {
        return Ok(AssetChange::WouldChange);
    }
    let object_store =
        object_store.context("object storage is required to seed catalogue artwork")?;
    Ok(AssetChange::Stored(
        Attachment::store(
            transaction,
            object_store,
            asset.bytes,
            &asset.extension,
            asset.media_type,
            file.file_name().and_then(|name| name.to_str()),
        )
        .await?
        .id,
    ))
}
