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
