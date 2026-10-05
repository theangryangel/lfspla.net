//! Object-store settings for replay files and cached catalogue images.

use std::path::{Path, PathBuf};

use object_store::ObjectStoreScheme;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use url::Url;

/// A filesystem root or object-store URL, serialized as one configuration string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageLocation {
    Local(PathBuf),
    Remote(Url),
}

impl StorageLocation {
    fn resolve(&mut self, base_dir: &Path) {
        if let Self::Local(path) = self
            && path.is_relative()
        {
            *path = base_dir.join(&*path);
        }
    }
}

impl<'de> Deserialize<'de> for StorageLocation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(D::Error::custom("storage location must not be empty"));
        }
        if !value.contains("://") {
            return Ok(Self::Local(PathBuf::from(value)));
        }

        let url = Url::parse(&value).map_err(D::Error::custom)?;
        let (scheme, prefix) = ObjectStoreScheme::parse(&url).map_err(D::Error::custom)?;
        if scheme == ObjectStoreScheme::Local {
            let path = url.to_file_path().map_err(|()| {
                D::Error::custom("file storage URL must identify a local filesystem path")
            })?;
            return Ok(Self::Local(path));
        }
        object_store::path::Path::parse(prefix).map_err(D::Error::custom)?;
        Ok(Self::Remote(url))
    }
}

impl Serialize for StorageLocation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Local(path) => path.serialize(serializer),
            Self::Remote(url) => url.serialize(serializer),
        }
    }
}

/// Where validated replay objects are kept.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageSettings {
    /// Filesystem root or object-store URL for replay files and catalogue images.
    pub object_store_root: StorageLocation,
}

impl StorageSettings {
    pub(super) fn resolve_paths(&mut self, base_dir: &Path) {
        self.object_store_root.resolve(base_dir);
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            object_store_root: StorageLocation::Local(PathBuf::from("data/storage")),
        }
    }
}
