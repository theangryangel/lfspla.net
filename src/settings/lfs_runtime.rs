//! Paths and executables shared by every local LFS operation.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::ConfigPath;

/// The installation store, Wine state, and programs used to run LFS locally.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LfsRuntimeSettings {
    /// Root containing persistent named LFS installations.
    #[serde(rename = "lfs_installation_root")]
    pub installation_root: ConfigPath,
    /// Disk-backed scratch directory for installer downloads.
    pub installer_download_root: ConfigPath,
    /// Wine prefix shared by every LFS installation.
    pub wine_prefix: ConfigPath,
    /// Wine executable used to run LFS.
    pub wine_executable: ConfigPath,
    /// Bubblewrap executable used to isolate LFS.
    pub bubblewrap_executable: ConfigPath,
}

impl LfsRuntimeSettings {
    // Data paths are resolved against the config directory. Wine and
    // Bubblewrap name system executables and are looked up as given.
    pub(super) fn resolve_paths(&mut self, base_dir: &Path) {
        self.installation_root.resolve(base_dir);
        self.installer_download_root.resolve(base_dir);
        self.wine_prefix.resolve(base_dir);
    }
}

impl Default for LfsRuntimeSettings {
    fn default() -> Self {
        Self {
            installation_root: ConfigPath::new("/srv/lfs"),
            installer_download_root: ConfigPath::new("/srv/lfs/downloads"),
            wine_prefix: ConfigPath::new("/srv/lfs/.wine"),
            wine_executable: ConfigPath::new("/usr/bin/wine"),
            bubblewrap_executable: ConfigPath::new("/usr/bin/bwrap"),
        }
    }
}
