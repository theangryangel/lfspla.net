//! Operator CLI for installing and maintaining named LFS installations.

mod args;

#[cfg(target_os = "linux")]
use std::{
    process::{Output, Stdio},
    time::Duration,
};

#[cfg(target_os = "linux")]
use anyhow::{Context, ensure};
#[cfg(target_os = "linux")]
use tokio::process::Command;

#[cfg(target_os = "linux")]
mod install;
#[cfg(target_os = "linux")]
mod installation_path;
#[cfg(target_os = "linux")]
mod trim;
#[cfg(target_os = "linux")]
mod update;

pub(crate) use args::LfsCommand;

use crate::cli::Args;

/// Runs one LFS installation command.
#[cfg(target_os = "linux")]
pub(crate) async fn run(args: &Args, action: &LfsCommand) -> anyhow::Result<()> {
    match action {
        LfsCommand::Path { installation_id } => installation_path::run(args, installation_id),
        LfsCommand::Install {
            installation_id,
            download_url,
            username,
            unlock_code,
            command_timeout_seconds,
        } => {
            install::run(
                args,
                installation_id,
                download_url,
                username,
                unlock_code,
                *command_timeout_seconds,
            )
            .await
        }
        LfsCommand::Update {
            installation_id,
            command_timeout_seconds,
        } => update::run(args, installation_id, *command_timeout_seconds).await,
        LfsCommand::Trim {
            installation_id,
            dds,
            lgh,
            yes,
        } => trim::run(args, installation_id, *dds, *lgh, *yes),
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) async fn run(_args: &Args, _action: &LfsCommand) -> anyhow::Result<()> {
    tracing::error!(
        "LFS installation management is unsupported on this platform; it requires Linux and Bubblewrap"
    );
    anyhow::bail!(
        "LFS installation management is unsupported on this platform; it requires Linux and Bubblewrap"
    )
}

#[cfg(target_os = "linux")]
async fn checked_output(
    mut command: Command,
    timeout: Duration,
    description: &str,
    secret: Option<&str>,
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

#[cfg(target_os = "linux")]
fn combined_output(output: &Output) -> String {
    let mut diagnostic = String::from_utf8_lossy(&output.stdout).into_owned();
    diagnostic.push_str(&String::from_utf8_lossy(&output.stderr));
    diagnostic
}

#[cfg(target_os = "linux")]
fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_owned()
    } else {
        text.replace(secret, "[REDACTED]")
    }
}
