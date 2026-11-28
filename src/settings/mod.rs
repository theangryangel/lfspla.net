//! Application configuration composition and YAML loading.

mod types;
mod web;

#[cfg(test)]
use lfsplanet_core::settings::StorageLocation;
pub use lfsplanet_core::settings::{DatabaseSettings, StorageSettings};
pub use lfsplanet_hotlaps::settings::HotlapSettings;
pub(crate) use types::PublicBaseUrl;
pub use web::WebSettings;

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
    /// How the application reaches lfs.net: API credentials and request limits.
    #[serde(default)]
    pub lfs_net: lfsplanet_lfs_net_api::Config,
    /// Upload policy, LFS installations, validation and webhook delivery.
    #[serde(default)]
    pub hotlaps: HotlapSettings,
    /// Where replay files and cached catalogue images are kept.
    #[serde(default)]
    pub storage: StorageSettings,
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
        settings.hotlaps.resolve_paths(base_dir)?;
        settings.storage.resolve_paths(base_dir);
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a document from the two required sections, plus optional extra
    /// lines inside `web:` and optional extra top-level sections.
    fn yaml(web_extra: &str, sections: &str) -> String {
        format!(
            "database:\n  url: postgresql://localhost/test\nweb:\n  session_key: \"{}\"\n{web_extra}{sections}",
            "a5".repeat(64)
        )
    }

    fn config_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("lfsplanet-settings")
    }

    fn load(sections: &str) -> Settings {
        Settings::from_yaml(&yaml("", sections), &config_dir()).unwrap()
    }

    fn load_web(web_extra: &str) -> Settings {
        Settings::from_yaml(&yaml(web_extra, ""), &config_dir()).unwrap()
    }

    fn assert_rejected(sections: &str) {
        assert!(
            Settings::from_yaml(&yaml("", sections), &config_dir()).is_err(),
            "accepted {sections:?}"
        );
    }

    #[test]
    fn optional_sections_may_be_omitted_entirely() {
        let settings = load("");
        let mut defaults = HotlapSettings::default();
        defaults.resolve_paths(&config_dir()).unwrap();
        assert_eq!(
            settings.hotlaps.installation_root.path(),
            defaults.installation_root.path()
        );
        assert_eq!(
            settings.hotlaps.installer_cache_root(),
            defaults.installation_root.path().join(".cache/installers")
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            settings.hotlaps.runtime.wine_prefix.as_path(),
            Path::new("/srv/lfs/.wine")
        );
        assert_eq!(
            settings.storage.object_store_root,
            StorageLocation::Local(config_dir().join("data/storage"))
        );
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
            "hotlaps:\n  installation_root: lfs\n  runtime:\n    wine_prefix: prefix\nstorage:\n  object_store_root: objects\n",
        );
        assert_eq!(
            settings.hotlaps.installation_root.path(),
            config_dir().join("lfs")
        );
        assert_eq!(
            settings.hotlaps.installer_cache_root(),
            config_dir().join("lfs/.cache/installers")
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            settings.hotlaps.runtime.wine_prefix.as_path(),
            config_dir().join("prefix")
        );
        assert_eq!(
            settings.storage.object_store_root,
            StorageLocation::Local(config_dir().join("objects"))
        );
    }

    #[test]
    fn crate_aligned_configuration_round_trips() {
        let settings = load(
            "lfs_net:\n  outbound_http_timeout_seconds: 25\n  oauth:\n    client_id: client\n    client_secret: secret\nhotlaps:\n  installation_root: games\n  runtime:\n    wine_prefix: prefix\n    wine_executable: /usr/bin/wine\n    bubblewrap_executable: /usr/bin/bwrap\n  validation:\n    poll_interval_seconds: 3\n    timeout_seconds: 120\n  webhooks:\n    workers: 4\n",
        );
        let serialized = serde_saphyr::to_string(&settings).unwrap();
        Settings::from_yaml(&serialized, &config_dir()).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runtime_rejects_empty_paths_and_unknown_settings() {
        assert_rejected("hotlaps:\n  runtime:\n    wine_prefix: \"\"\n");
        assert_rejected("hotlaps:\n  runtime:\n    wine_executable: \"\"\n");
        assert_rejected("hotlaps:\n  runtime:\n    bubblewrap_executable: \"\"\n");
        assert_rejected("hotlaps:\n  runtime:\n    not_a_setting: true\n");
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn unsupported_runtime_ignores_extra_fields_without_losing_application_settings() {
        let settings = load(
            "lfs_net:\n  outbound_http_timeout_seconds: 25\nhotlaps:\n  installation_root: games\n  runtime:\n    wine_prefix: ignored\n    future_runtime: ignored\n",
        );
        assert_eq!(
            settings.hotlaps.installation_root.path(),
            config_dir().join("games")
        );
        assert_eq!(
            settings.lfs_net.timeout(),
            std::time::Duration::from_secs(25)
        );
        assert_rejected("lfs_net:\n  outbound_http_timeout_seconds: 0\n");
    }

    #[test]
    fn unknown_settings_are_rejected_at_every_level() {
        assert_rejected("not_a_setting: true\n");
        assert_rejected("lfs_net:\n  not_a_setting: true\n");
        assert_rejected("hotlaps:\n  not_a_setting: true\n");
        assert_rejected("hotlaps:\n  validation:\n    not_a_setting: true\n");
        assert_rejected("hotlaps:\n  webhooks:\n    not_a_setting: true\n");
        assert_rejected(
            "lfs_net:\n  oauth:\n    client_id: a\n    client_secret: b\n    extra: c\n",
        );
    }

    #[test]
    fn hotlap_policy_can_be_overridden() {
        let settings = load(
            "hotlaps:\n  enforce_replay_username: false\n  max_queued_per_player: 25\n  max_spr_upload_bytes: 4 MiB\n  allow_test_validation: true\n",
        );
        assert!(!settings.hotlaps.enforce_replay_username);
        assert_eq!(settings.hotlaps.max_queued_per_player.get(), 25);
        assert_eq!(settings.hotlaps.max_spr_upload_bytes(), 4 * 1024 * 1024);
        assert!(settings.hotlaps.allow_test_validation);

        assert_rejected("hotlaps:\n  max_spr_upload_bytes: 0\n");
        assert_rejected("hotlaps:\n  max_spr_upload_bytes: -1 B\n");
    }

    #[test]
    fn cookie_secure_defaults_on_and_can_be_disabled() {
        assert!(load("").web.cookie_secure);
        assert!(!load_web("  cookie_secure: false\n").web.cookie_secure);
    }
}
