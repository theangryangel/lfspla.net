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

/// S3 client retained separately from the generic, prefix-wrapped object store.
/// Other backends continue to stream through the API.
pub(crate) struct DownloadSigner {
    #[cfg(feature = "aws")]
    client: Arc<object_store::aws::AmazonS3>,
    #[cfg(feature = "aws")]
    prefix: object_store::path::Path,
}

impl DownloadSigner {
    pub(crate) async fn sign(
        &self,
        attachment: &crate::models::attachment::Model,
        disposition: &str,
    ) -> anyhow::Result<url::Url> {
        #[cfg(feature = "aws")]
        {
            use object_store::signer::{Method, SignedUrlOptions, Signer};
            let relative = object_store::path::Path::from(attachment.object_key.as_str());
            let path = self.prefix.parts().chain(relative.parts()).collect();
            let mut options = SignedUrlOptions::default().with_query([
                ("response-content-type", attachment.media_type.clone()),
                ("response-cache-control", "private, no-store".to_owned()),
            ]);
            options = options.with_query([("response-content-disposition", disposition)]);
            Ok(self
                .client
                .signed_url_opts(
                    Method::GET,
                    &path,
                    std::time::Duration::from_secs(300),
                    &options,
                )
                .await?)
        }
        #[cfg(not(feature = "aws"))]
        {
            let _ = (attachment, disposition);
            anyhow::bail!("S3 download signing requires the aws feature")
        }
    }
}

/// A safe filename header shared by local responses and signed S3 overrides.
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

pub(crate) struct WebStorage {
    pub(crate) object_store: Arc<dyn ObjectStore>,
    pub(crate) download_signer: Option<Arc<DownloadSigner>>,
}

/// Builds the web store and, for S3, a signer using the same client and prefix.
pub(crate) fn build_with_downloads(settings: &StorageSettings) -> anyhow::Result<WebStorage> {
    #[cfg(feature = "aws")]
    if let StorageLocation::Remote(url) = &settings.object_store_root {
        use object_store::{ObjectStoreScheme, aws::AmazonS3Builder};
        let (scheme, prefix) = ObjectStoreScheme::parse(url)?;
        if scheme == ObjectStoreScheme::AmazonS3 {
            let builder = std::env::vars_os()
                .filter_map(|(key, value)| {
                    Some((key.into_string().ok()?, value.into_string().ok()?))
                })
                .fold(
                    AmazonS3Builder::new().with_url(url.to_string()),
                    |builder, (key, value)| match key.to_ascii_lowercase().parse() {
                        Ok(key) => builder.with_config(key, value),
                        Err(_) => builder,
                    },
                );
            let client = Arc::new(
                builder
                    .build()
                    .context("failed to build S3 attachment store")?,
            );
            let prefix = object_store::path::Path::parse(prefix)?;
            let store = Arc::new(PrefixStore::new(client.clone(), prefix.clone()));
            return Ok(WebStorage {
                object_store: store,
                download_signer: Some(Arc::new(DownloadSigner { client, prefix })),
            });
        }
    }
    Ok(WebStorage {
        object_store: build(settings)?,
        download_signer: None,
    })
}

#[cfg(all(test, feature = "aws"))]
mod tests {
    use super::*;

    pub(crate) fn signer(prefix: &str) -> Arc<DownloadSigner> {
        Arc::new(DownloadSigner {
            client: Arc::new(
                object_store::aws::AmazonS3Builder::new()
                    .with_bucket_name("lfsplanet")
                    .with_endpoint("https://uk-lon-1.katapultobjects.com:443")
                    .with_region("us-east-1")
                    .with_access_key_id("test-access-key")
                    .with_secret_access_key("test-secret-key")
                    .build()
                    .unwrap(),
            ),
            prefix: object_store::path::Path::from(prefix),
        })
    }

    #[tokio::test]
    async fn signs_prefixed_get_urls_with_download_response_headers() {
        let mut attachment = crate::models::attachment::Model {
            id: 1,
            public_id: sea_orm::prelude::Uuid::from_u128(1),
            object_key: "ab/file name.spr".to_owned(),
            original_filename: Some("driver lap.spr".to_owned()),
            sha256: None,
            size_bytes: None,
            media_type: "application/octet-stream".to_owned(),
            extension: "spr".to_owned(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        for prefix in ["", "files/nested"] {
            let signer = signer(prefix);
            let url = signer
                .sign(&attachment, &download_disposition(&attachment))
                .await
                .unwrap();
            assert_eq!(url.host_str(), Some("uk-lon-1.katapultobjects.com"));
            assert_eq!(
                url.path(),
                format!(
                    "/lfsplanet/{}ab/file%20name.spr",
                    if prefix.is_empty() {
                        String::new()
                    } else {
                        format!("{prefix}/")
                    }
                )
            );
            let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
            assert_eq!(query["X-Amz-Expires"], "300");
            assert_eq!(query["response-content-type"], "application/octet-stream");
            assert_eq!(query["response-cache-control"], "private, no-store");
            assert_eq!(
                query["response-content-disposition"],
                download_disposition(&attachment)
            );
            assert!(!query["X-Amz-Signature"].is_empty());
        }
        attachment.extension = "png".to_owned();
        attachment.media_type = "image/png".to_owned();
        let url = signer("")
            .sign(&attachment, &download_disposition(&attachment))
            .await
            .unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(query["response-content-type"], "image/png");
        assert!(query["response-content-disposition"].starts_with("inline;"));
    }
}

#[cfg(all(test, feature = "aws"))]
pub(crate) use tests::signer as test_download_signer;

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
