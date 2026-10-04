//! Local execution is not implemented on this platform.

#![allow(
    clippy::unused_async,
    reason = "match the supported platform's asynchronous API"
)]

use crate::ValidationDiagnostic;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

/// Accepts and ignores settings for runtimes unavailable on this platform.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct RuntimeConfig {}

impl RuntimeConfig {
    #[allow(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        reason = "match the supported platform's configuration API"
    )]
    pub fn resolve_paths(&mut self, _base: &Path) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn ensure_supported() -> anyhow::Result<()> {
    anyhow::bail!("local LFS execution is currently supported only on Linux")
}

pub async fn install(
    _config: &RuntimeConfig,
    _setup: &Path,
    _installation: &Path,
    _username: &str,
    _unlock_code: &str,
    _timeout: Duration,
) -> anyhow::Result<()> {
    anyhow::bail!("local LFS installation is currently supported only on Linux")
}

pub async fn update(
    _config: &RuntimeConfig,
    _installation: &Path,
    _timeout: Duration,
) -> anyhow::Result<()> {
    anyhow::bail!("local LFS updates are currently supported only on Linux")
}

pub async fn validate(
    _config: &RuntimeConfig,
    _installation: &Path,
    _replay: &[u8],
    _timeout: Duration,
) -> anyhow::Result<ValidationDiagnostic> {
    anyhow::bail!("local LFS validation is currently supported only on Linux")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_fields_can_be_flattened_alongside_application_settings() {
        #[derive(Deserialize, Serialize)]
        struct Settings {
            installation_root: String,
            #[serde(flatten)]
            runtime: RuntimeConfig,
        }
        let mut settings: Settings = serde_json::from_str(r#"{"installation_root":"games","wine_prefix":"prefix","wine_executable":"wine","bubblewrap_executable":"bwrap","future_runtime":{"command":[]}}"#).unwrap();
        assert_eq!(settings.installation_root, "games");
        settings.runtime.resolve_paths(Path::new("config")).unwrap();
        assert_eq!(
            serde_json::to_value(&settings).unwrap(),
            serde_json::json!({"installation_root": "games"})
        );
    }

    #[tokio::test]
    async fn accepts_linux_configuration_and_rejects_execution() {
        assert!(
            ensure_supported()
                .unwrap_err()
                .to_string()
                .contains("supported only on Linux")
        );
        let mut config: RuntimeConfig = serde_json::from_str(r#"{"wine_prefix":"missing-prefix","wine_executable":"missing-wine","bubblewrap_executable":"missing-bwrap"}"#).unwrap();
        config
            .resolve_paths(Path::new("missing-config-directory"))
            .unwrap();
        let installation = Path::new("missing-installation");
        let timeout = Duration::from_secs(1);
        assert!(
            install(
                &config,
                Path::new("missing-installer"),
                installation,
                "driver",
                "code",
                timeout
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("supported only on Linux")
        );
        assert!(
            update(&config, installation, timeout)
                .await
                .unwrap_err()
                .to_string()
                .contains("supported only on Linux")
        );
        let error = validate(&config, installation, b"replay", timeout)
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("supported only on Linux"));
    }
}
