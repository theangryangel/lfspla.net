//! Object-store settings for replay files and cached catalogue images.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::ConfigPath;

/// Where validated replay objects are kept.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageSettings {
    /// Root used by the local `object_store` backend for binary objects.
    pub object_store_root: ConfigPath,
}

impl StorageSettings {
    pub(super) fn resolve_paths(&mut self, base_dir: &Path) {
        self.object_store_root.resolve(base_dir);
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            object_store_root: ConfigPath::new("data/storage"),
        }
    }
}
