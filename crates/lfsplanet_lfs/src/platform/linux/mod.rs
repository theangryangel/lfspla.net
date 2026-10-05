//! Wine and Bubblewrap execution on Linux.

mod command;
mod install;
mod validate;

pub use install::install;
pub use validate::validate;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::{Output, Stdio},
    time::Duration,
};
use tokio::process::Command;

/// Linux runtime paths. Prefix paths are relative to the configuration file;
/// executable paths are used as given.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub wine_prefix: PathBuf,
    pub wine_executable: PathBuf,
    pub bubblewrap_executable: PathBuf,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            wine_prefix: "/srv/lfs/.wine".into(),
            wine_executable: "/usr/bin/wine".into(),
            bubblewrap_executable: "/usr/bin/bwrap".into(),
        }
    }
}

impl RuntimeConfig {
    pub fn resolve_paths(&mut self, base: &Path) -> anyhow::Result<()> {
        for path in [
            &self.wine_prefix,
            &self.wine_executable,
            &self.bubblewrap_executable,
        ] {
            ensure!(
                !path.as_os_str().is_empty(),
                "LFS runtime paths must not be empty"
            );
        }
        if self.wine_prefix.is_relative() {
            self.wine_prefix = base.join(&self.wine_prefix);
        }
        Ok(())
    }

    fn programs(&self) -> anyhow::Result<(PathBuf, PathBuf)> {
        Ok((
            command::executable(&self.bubblewrap_executable, "Bubblewrap")?,
            command::executable(&self.wine_executable, "Wine")?,
        ))
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "match the unsupported platform API"
)]
pub fn ensure_supported() -> anyhow::Result<()> {
    Ok(())
}

/// Update an existing installation, allowing the updater to start a successor.
pub async fn update(
    config: &RuntimeConfig,
    installation: &Path,
    timeout: Duration,
) -> anyhow::Result<()> {
    ensure!(
        !timeout.is_zero(),
        "command timeout must be greater than zero"
    );
    let (bubblewrap, wine) = config.programs()?;
    let installation =
        std::fs::canonicalize(installation).context("failed to resolve LFS installation")?;
    let prefix = command::validate_prefix(&config.wine_prefix)?;
    checked_output(
        command::Command::Update {
            installation_dir: &installation,
            wine_prefix: &prefix,
        }
        .build(&bubblewrap, &wine),
        timeout,
        "LFS update",
        None,
    )
    .await?;
    Ok(())
}

async fn capture_output(
    mut command: Command,
    timeout: Duration,
    description: &str,
) -> anyhow::Result<Output> {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start {description}"))?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .with_context(|| {
            format!(
                "{description} exceeded its {} second deadline",
                timeout.as_secs()
            )
        })?
        .with_context(|| format!("failed while waiting for {description}"))?;
    Ok(output)
}

async fn checked_output(
    command: Command,
    timeout: Duration,
    description: &str,
    secret: Option<&str>,
) -> anyhow::Result<Output> {
    let output = capture_output(command, timeout, description).await?;
    let mut diagnostic = combined_output(&output);
    if let Some(secret) = secret {
        diagnostic = redact(&diagnostic, secret);
    }
    ensure!(
        output.status.success(),
        "{description} exited {}: {}",
        output.status,
        diagnostic.trim()
    );
    Ok(output)
}

fn combined_output(output: &Output) -> String {
    let mut diagnostic = String::from_utf8_lossy(&output.stdout).into_owned();
    diagnostic.push_str(&String::from_utf8_lossy(&output.stderr));
    diagnostic
}

fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "[REDACTED]")
    }
}
