//! Application settings loaded from YAML. Fields validate on read; see [`types`].

mod database;
mod hlvc;
mod hotlaps;
mod lfs;
mod lfs_runtime;
mod storage;
mod types;
mod web;
mod webhooks;

pub use database::DatabaseSettings;
pub use hlvc::HlvcSettings;
pub use hotlaps::HotlapSettings;
pub use lfs::LfsSettings;
pub use lfs_runtime::LfsRuntimeSettings;
pub use storage::StorageSettings;
pub use web::WebSettings;
pub use webhooks::WebhookSettings;

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Fully resolved settings used by backend processes.
///
/// `web` and `database` remain required in configuration files. Their Rust
/// defaults exist so the CLI can generate a complete development config;
/// every other section may be omitted entirely when loading YAML.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// How every process reaches PostgreSQL.
    pub database: DatabaseSettings,
    /// How the API process presents itself over HTTP.
    pub web: WebSettings,
    /// Paths and executables that define the local LFS runtime.
    #[serde(flatten)]
    pub lfs_runtime: LfsRuntimeSettings,
    /// Credentials and limits for outbound calls to LFS.
    #[serde(default)]
    pub lfs: LfsSettings,
    /// Policy applied to uploaded replays.
    #[serde(default)]
    pub hotlaps: HotlapSettings,
    /// Where replay files and cached catalogue images are kept.
    #[serde(default)]
    pub storage: StorageSettings,
    /// Background worker configuration.
    #[serde(default)]
    pub worker: WorkerSettings,
}

/// Scheduling and concurrency for each background processor.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkerSettings {
    /// Hotlap validation settings.
    pub hlvc: HlvcSettings,
    /// Outgoing webhook delivery settings.
    pub webhooks: WebhookSettings,
}

impl Settings {
    /// Loads and validates settings from a YAML configuration file.
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = std::path::absolute(path.as_ref())
            .with_context(|| format!("failed to resolve {}", path.as_ref().display()))?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let base_dir = path
            .parent()
            .with_context(|| format!("config path {} has no parent", path.display()))?;
        Self::from_yaml(&text, base_dir)
            .with_context(|| format!("failed to load config file {}", path.display()))
    }

    fn from_yaml(text: &str, base_dir: &Path) -> anyhow::Result<Self> {
        let mut settings: Self = serde_saphyr::from_str(text).context("failed to parse YAML")?;
        settings.hotlaps.validate()?;
        // Resolve paths here because deserialization has no config directory.
        settings.lfs_runtime.resolve_paths(base_dir);
        settings.storage.resolve_paths(base_dir);
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Builds a document from the two required sections, plus optional extra
    /// lines inside `web:` and optional extra top-level sections.
    fn yaml(web_extra: &str, sections: &str) -> String {
        format!(
            "database:\n  url: postgresql://localhost/test\nweb:\n  session_key: \"{}\"\n{web_extra}{sections}",
            "a5".repeat(64)
        )
    }

    fn load(sections: &str) -> Settings {
        Settings::from_yaml(&yaml("", sections), Path::new("/config")).unwrap()
    }

    fn load_web(web_extra: &str) -> Settings {
        Settings::from_yaml(&yaml(web_extra, ""), Path::new("/config")).unwrap()
    }

    fn assert_rejected(sections: &str) {
        assert!(
            Settings::from_yaml(&yaml("", sections), Path::new("/config")).is_err(),
            "accepted {sections:?}"
        );
    }

    #[test]
    fn optional_sections_may_be_omitted_entirely() {
        let settings = load("");
        assert_eq!(
            settings.lfs_runtime.installation_root.path(),
            Path::new("/srv/lfs")
        );
        assert_eq!(
            settings.lfs_runtime.installer_download_root.path(),
            Path::new("/srv/lfs/downloads")
        );
        assert_eq!(
            settings.lfs_runtime.wine_prefix.path(),
            Path::new("/srv/lfs/.wine")
        );
        assert_eq!(
            settings.storage.object_store_root.path(),
            Path::new("/config/data/storage")
        );
        assert!(settings.lfs.oauth.is_none());
        assert!(settings.hotlaps.enforce_replay_username);
        assert!(settings.web.site_notice.is_none());
    }

    #[test]
    fn the_required_sections_are_required() {
        let key = "a5".repeat(64);
        assert!(
            Settings::from_yaml(
                &format!("web:\n  session_key: \"{key}\"\n"),
                Path::new("/c")
            )
            .is_err(),
            "accepted a document with no database section"
        );
        assert!(
            Settings::from_yaml("database:\n  url: postgresql://x\n", Path::new("/c")).is_err(),
            "accepted a document with no web section"
        );
        assert!(
            Settings::from_yaml(
                &format!("database:\n  url: \"\"\nweb:\n  session_key: \"{key}\"\n"),
                Path::new("/c")
            )
            .is_err(),
            "accepted a blank database url"
        );
    }

    #[test]
    fn oauth_credentials_are_optional_but_must_not_be_blank() {
        assert!(load("").lfs.oauth.is_none());

        let settings = load("lfs:\n  oauth:\n    client_id: client\n    client_secret: secret\n");
        let oauth = settings.lfs.oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id.as_str(), "client");
        assert_eq!(oauth.client_secret.as_str(), "secret");

        assert_rejected("lfs:\n  oauth:\n    client_id: \"\"\n    client_secret: secret\n");
        assert_rejected("lfs:\n  oauth:\n    client_id: client\n");
    }

    #[test]
    fn callback_path_is_top_level() {
        assert_eq!(
            load("").web.oauth_callback_url().as_str(),
            "http://localhost:8000/auth/lfs/callback"
        );
    }

    #[test]
    fn relative_paths_are_resolved_from_the_config_directory() {
        let settings = load(
            "lfs_installation_root: lfs\ninstaller_download_root: downloads\nwine_prefix: prefix\nstorage:\n  object_store_root: objects\n",
        );
        assert_eq!(
            settings.lfs_runtime.installation_root.path(),
            Path::new("/config/lfs")
        );
        assert_eq!(
            settings.lfs_runtime.installer_download_root.path(),
            Path::new("/config/downloads")
        );
        assert_eq!(
            settings.lfs_runtime.wine_prefix.path(),
            Path::new("/config/prefix")
        );
        assert_eq!(
            settings.storage.object_store_root.path(),
            Path::new("/config/objects")
        );
    }

    #[test]
    fn system_executable_paths_are_not_resolved_against_the_config_directory() {
        let settings = load("wine_executable: /usr/bin/wine\n");
        assert_eq!(
            settings.lfs_runtime.wine_executable.path(),
            Path::new("/usr/bin/wine")
        );
    }

    #[test]
    fn validator_defaults_are_sane() {
        let settings = load("");
        assert_eq!(
            settings.lfs_runtime.wine_executable.path(),
            Path::new("/usr/bin/wine")
        );
        assert_eq!(
            settings.lfs_runtime.bubblewrap_executable.path(),
            Path::new("/usr/bin/bwrap")
        );
        assert_eq!(
            settings.worker.hlvc.poll_interval.duration(),
            Duration::from_secs(2)
        );
        assert_eq!(
            settings.worker.hlvc.timeout.duration(),
            Duration::from_secs(300)
        );
        assert_eq!(settings.worker.webhooks.workers.get(), 3);
    }

    #[test]
    fn webhook_worker_count_can_be_configured_and_must_be_positive() {
        assert_eq!(
            load("worker:\n  webhooks:\n    workers: 6\n")
                .worker
                .webhooks
                .workers
                .get(),
            6
        );
        assert_rejected("worker:\n  webhooks:\n    workers: 0\n");
    }

    #[test]
    fn unknown_settings_are_rejected_at_every_level() {
        assert_rejected("not_a_setting: true\n");
        assert_rejected("worker:\n  not_a_setting: true\n");
        assert_rejected("worker:\n  hlvc:\n    not_a_setting: true\n");
        assert_rejected("worker:\n  webhooks:\n    not_a_setting: true\n");
        assert_rejected("lfs:\n  oauth:\n    client_id: a\n    client_secret: b\n    extra: c\n");
    }

    #[test]
    fn hotlap_policy_defaults_can_be_overridden() {
        let settings = load("");
        assert!(settings.hotlaps.enforce_replay_username);
        assert_eq!(settings.hotlaps.max_queued_per_player.get(), 10);
        assert_eq!(settings.hotlaps.max_spr_upload_bytes(), 2 * 1024 * 1024);
        assert!(!settings.hotlaps.allow_test_validation);

        let settings = load(
            "hotlaps:\n  enforce_replay_username: false\n  max_queued_per_player: 25\n  max_spr_upload_bytes: 4 MiB\n  allow_test_validation: true\n",
        );
        assert!(!settings.hotlaps.enforce_replay_username);
        assert_eq!(settings.hotlaps.max_queued_per_player.get(), 25);
        assert_eq!(settings.hotlaps.max_spr_upload_bytes(), 4 * 1024 * 1024);
        assert!(settings.hotlaps.allow_test_validation);

        assert_rejected("hotlaps:\n  max_queued_per_player: 0\n");
        assert_rejected("hotlaps:\n  max_spr_upload_bytes: 0\n");
        assert_rejected("hotlaps:\n  max_spr_upload_bytes: -1 B\n");
    }

    #[test]
    fn cookie_secure_defaults_on_and_can_be_disabled() {
        assert!(load("").web.cookie_secure);
        assert!(!load_web("  cookie_secure: false\n").web.cookie_secure);
    }

    #[test]
    fn network_and_database_timeouts_have_sane_defaults() {
        let settings = load("");
        assert_eq!(
            settings.web.http_request_timeout.duration(),
            Duration::from_secs(30)
        );
        assert_eq!(
            settings.lfs.outbound_http_timeout.duration(),
            Duration::from_secs(15)
        );
        assert_eq!(
            settings.database.connect_timeout.duration(),
            Duration::from_secs(5)
        );
        assert_eq!(
            settings.database.acquire_timeout.duration(),
            Duration::from_secs(5)
        );
        assert_eq!(
            settings.database.statement_timeout.duration(),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn timeouts_must_be_positive_in_every_section() {
        assert_rejected("lfs:\n  outbound_http_timeout_seconds: 0\n");
        assert_rejected("database:\n  connect_timeout_seconds: 0\n");
        assert_rejected("worker:\n  hlvc:\n    timeout_seconds: 0\n");
        assert!(
            Settings::from_yaml(
                &yaml("  http_request_timeout_seconds: 0\n", ""),
                Path::new("/c")
            )
            .is_err(),
            "accepted a zero inbound request timeout"
        );
    }
}
