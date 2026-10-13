//! Shared attachment-store construction and garbage collection reporting.

use std::sync::Arc;

use anyhow::Context;
use object_store::{ObjectStore, local::LocalFileSystem, prefix::PrefixStore};

use crate::settings::{StorageLocation, StorageSettings};

/// Builds the configured attachment object store.
pub fn build(settings: &StorageSettings) -> anyhow::Result<Arc<dyn ObjectStore>> {
    match &settings.object_store_root {
        StorageLocation::Local(root) => {
            std::fs::create_dir_all(root).with_context(|| {
                format!("failed to create object-store root {}", root.display())
            })?;
            let store = LocalFileSystem::new_with_prefix(root)
                .with_context(|| format!("failed to open object-store root {}", root.display()))?;
            Ok(Arc::new(store))
        }
        StorageLocation::Remote(url) => {
            let (store, prefix) = object_store::parse_url_opts(
                url,
                std::env::vars_os().filter_map(|(key, value)| {
                    Some((key.into_string().ok()?, value.into_string().ok()?))
                }),
            )
            .context("failed to build configured object store; enable its backend feature and supply its environment configuration")?;
            Ok(Arc::new(PrefixStore::new(store, prefix)))
        }
    }
}

/// A safe filename header for proxied attachment responses.
pub(crate) fn download_disposition(attachment: &crate::models::attachment::Model) -> String {
    let fallback = format!("{}.{}", attachment.public_id, attachment.extension);
    let filename = attachment
        .original_filename
        .as_deref()
        .and_then(|name| name.rsplit(['/', '\\']).next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && !matches!(*name, "." | ".."))
        .map(|name| {
            name.chars()
                .take(255)
                .map(|c| {
                    if c.is_control() || matches!(c, '\"' | '<' | '>' | ':' | '|' | '?' | '*') {
                        '_'
                    } else {
                        c
                    }
                })
                .collect::<String>()
        });
    let filename = filename.unwrap_or(fallback);
    let ascii: String = filename
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    // RFC 5987 preserves Unicode names while keeping the header itself ASCII.
    let encoded: String = filename
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let kind = if attachment.media_type.starts_with("image/") {
        "inline"
    } else {
        "attachment"
    };
    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

#[cfg(test)]
mod filename_tests {
    use super::*;

    #[test]
    fn download_names_preserve_originals_and_use_the_actual_extension_as_fallback() {
        let mut file = crate::models::attachment::Model {
            id: 1,
            public_id: sea_orm::prelude::Uuid::from_u128(1),
            object_key: "file.zip".to_owned(),
            original_filename: None,
            sha256: None,
            size_bytes: None,
            media_type: "application/zip".to_owned(),
            extension: "zip".to_owned(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        file.original_filename = Some("folder/driver lap.zip".to_owned());
        assert_eq!(
            download_disposition(&file),
            "attachment; filename=\"driver lap.zip\"; filename*=UTF-8''driver%20lap.zip"
        );
        for absent in [None, Some(""), Some("  "), Some(".."), Some("folder/")] {
            file.original_filename = absent.map(str::to_owned);
            assert!(
                download_disposition(&file).contains("00000000-0000-0000-0000-000000000001.zip")
            );
        }
        file.original_filename = Some("C:\\laps\\andré.zip".to_owned());
        assert_eq!(
            download_disposition(&file),
            "attachment; filename=\"andr_.zip\"; filename*=UTF-8''andr%C3%A9.zip"
        );
        file.original_filename = Some("../driver\"\r\n.zip".to_owned());
        let unsafe_name = download_disposition(&file);
        assert!(!unsafe_name.contains(['\r', '\n']));
        assert!(unsafe_name.contains("driver___.zip"));
        file.extension = "raf".to_owned();
        file.media_type = "application/octet-stream".to_owned();
        file.original_filename = Some("actual telemetry.raf".to_owned());
        assert!(download_disposition(&file).contains("actual telemetry.raf"));
        file.extension = "png".to_owned();
        file.media_type = "image/png".to_owned();
        file.original_filename = None;
        let header = download_disposition(&file);
        assert!(header.starts_with("inline;"));
        assert!(header.contains("00000000-0000-0000-0000-000000000001.png"));
    }
}
