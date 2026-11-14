use std::{collections::HashMap, path::Path, sync::Arc};

use object_store::{ObjectStore, ObjectStoreExt, path::Path as StorePath};

/// Application object-storage configuration.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Local directory or object-store URL, such as `s3://bucket/prefix`.
    pub url: String,
    /// Backend-specific options, such as an endpoint or region.
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// Object storage rooted at a configurable prefix.
#[derive(Clone)]
pub struct Storage {
    store: Arc<dyn ObjectStore>,
    prefix: StorePath,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Storage")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl Storage {
    /// Opens the configured local or remote object store.
    pub fn new(config: &StorageConfig) -> Result<Self, object_store::Error> {
        let (store, prefix) = if config.url.contains("://") {
            let url =
                url::Url::parse(&config.url).map_err(|error| object_store::Error::Generic {
                    store: "layout",
                    source: Box::new(error),
                })?;
            object_store::parse_url_opts(&url, config.options.clone())?
        } else {
            std::fs::create_dir_all(&config.url).map_err(|error| object_store::Error::Generic {
                store: "layout",
                source: Box::new(error),
            })?;
            (
                Box::new(object_store::local::LocalFileSystem::new_with_prefix(
                    Path::new(&config.url),
                )?) as Box<dyn ObjectStore>,
                StorePath::default(),
            )
        };
        Ok(Self {
            store: Arc::from(store),
            prefix,
        })
    }

    /// Returns a handle rooted beneath this store's current prefix.
    #[must_use]
    pub fn scoped(&self, prefix: &str) -> Self {
        Self {
            store: self.store.clone(),
            prefix: self.prefix.clone().join(prefix),
        }
    }

    fn path(&self, key: &str) -> Result<StorePath, object_store::Error> {
        let path = if self.prefix.as_ref().is_empty() {
            key.to_string()
        } else {
            format!("{}/{key}", self.prefix.as_ref())
        };
        Ok(StorePath::parse(path)?)
    }

    /// Writes an immutable layout blob.
    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), object_store::Error> {
        let _ = self.store.put(&self.path(key)?, bytes.into()).await?;
        Ok(())
    }

    /// Reads a layout blob in full.
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, object_store::Error> {
        let path = self.path(key)?;
        let result = self.store.get(&path).await?;
        Ok(result.bytes().await?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_preserve_path_segments() {
        let storage = Storage {
            store: Arc::new(object_store::memory::InMemory::new()),
            prefix: StorePath::parse("layouts").expect("valid prefix"),
        };

        let path = storage.path("2c/ad/digest.lyt").expect("valid object path");

        assert_eq!(path.as_ref(), "layouts/2c/ad/digest.lyt");
    }
}
