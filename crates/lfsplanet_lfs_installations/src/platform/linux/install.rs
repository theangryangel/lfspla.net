//! Installation, Wine prefix initialization, extraction and unlock.

use super::command::{Command as SandboxCommand, validate_prefix};
use super::{RuntimeConfig, checked_output, combined_output, redact};
use anyhow::{Context, ensure};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    time::Duration,
};

/// Install a downloaded Windows installer into a new destination.
pub async fn install(
    config: &RuntimeConfig,
    setup: &Path,
    installation_dir: &Path,
    username: &str,
    unlock_code: &str,
    command_timeout: Duration,
) -> anyhow::Result<()> {
    ensure!(
        !command_timeout.is_zero(),
        "command timeout must be greater than zero"
    );
    let (bubblewrap, wine) = config.programs()?;
    let setup = fs::canonicalize(setup).context("failed to resolve LFS installer")?;
    ensure!(setup.is_file(), "LFS installer must be a file");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(installation_dir)
        .with_context(|| format!("failed to create {}", installation_dir.display()))?;
    let installation_dir =
        fs::canonicalize(installation_dir).context("failed to resolve LFS installation")?;
    install_into(
        config,
        &setup,
        &installation_dir,
        &bubblewrap,
        &wine,
        username,
        unlock_code,
        command_timeout,
    )
    .await
    .with_context(|| {
        format!(
            "partial installation remains at {}",
            installation_dir.display()
        )
    })
}

#[allow(clippy::too_many_arguments)]
async fn install_into(
    config: &RuntimeConfig,
    setup: &Path,
    installation_dir: &Path,
    bubblewrap: &Path,
    wine: &Path,
    username: &str,
    unlock_code: &str,
    command_timeout: Duration,
) -> anyhow::Result<()> {
    let (wine_prefix, initialize_prefix) = prepare_prefix(&config.wine_prefix)?;
    if initialize_prefix {
        tracing::info!(path = %wine_prefix.display(), "initializing shared Wine prefix");
        let output = checked_output(
            SandboxCommand::Initialize {
                wine_prefix: &wine_prefix,
            }
            .build(bubblewrap, wine),
            command_timeout,
            "Wine prefix initialization",
            None,
        )
        .await?;
        let diagnostic = combined_output(&output);
        validate_prefix(&wine_prefix).with_context(|| {
            format!(
                "Wine prefix initialization completed without creating a valid prefix; captured output:\n{}",
                if diagnostic.trim().is_empty() {
                    "(no output)"
                } else {
                    diagnostic.trim()
                }
            )
        })?;
    }

    tracing::info!("installing LFS");
    checked_output(
        SandboxCommand::Install {
            installation_dir,
            wine_prefix: &wine_prefix,
            setup,
        }
        .build(bubblewrap, wine),
        command_timeout,
        "LFS installer",
        None,
    )
    .await?;
    ensure!(
        installation_dir.join("LFS.exe").is_file(),
        "LFS installer completed without creating LFS.exe"
    );

    tracing::info!("extracting LFS data");
    checked_output(
        SandboxCommand::Extract {
            installation_dir,
            wine_prefix: &wine_prefix,
        }
        .build(bubblewrap, wine),
        command_timeout,
        "LFS headless preparation",
        None,
    )
    .await?;

    tracing::info!("unlocking LFS");
    let output = checked_output(
        SandboxCommand::Unlock {
            installation_dir,
            wine_prefix: &wine_prefix,
            username,
            code: unlock_code,
        }
        .build(bubblewrap, wine),
        command_timeout,
        "LFS unlock",
        Some(unlock_code),
    )
    .await?;
    let diagnostic = redact(&combined_output(&output), unlock_code);
    ensure!(
        diagnostic.to_ascii_lowercase().contains("welcome to"),
        "LFS exited successfully but did not report a successful unlock: {}",
        diagnostic.trim()
    );

    ensure!(
        installation_dir.join("data/spr").is_dir(),
        "prepared LFS installation has no data/spr directory"
    );
    Ok(())
}

fn prepare_prefix(requested: &Path) -> anyhow::Result<(PathBuf, bool)> {
    match fs::symlink_metadata(requested) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink(),
                "Wine prefix {} must be a real directory, not a symlink",
                requested.display()
            );
            ensure!(
                metadata.is_dir(),
                "Wine prefix {} is not a directory",
                requested.display()
            );
            if let Ok(prefix) = validate_prefix(requested) {
                return Ok((prefix, false));
            }

            tracing::warn!(path = %requested.display(), "removing incomplete Wine prefix");
            fs::remove_dir_all(requested).with_context(|| {
                format!(
                    "failed to remove incomplete Wine prefix {}",
                    requested.display()
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect Wine prefix {}", requested.display()));
        }
    }
    create_prefix(requested)?;
    let prefix = fs::canonicalize(requested)
        .with_context(|| format!("failed to resolve Wine prefix {}", requested.display()))?;
    Ok((prefix, true))
}

fn create_prefix(requested: &Path) -> anyhow::Result<()> {
    let name = requested.file_name().with_context(|| {
        format!(
            "Wine prefix path {} has no final component",
            requested.display()
        )
    })?;
    let parent = requested
        .parent()
        .with_context(|| format!("Wine prefix path {} has no parent", requested.display()))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("failed to resolve Wine prefix parent {}", parent.display()))?;
    ensure!(parent.is_dir(), "Wine prefix parent is not a directory");
    let prefix = parent.join(name);
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&prefix)
        .with_context(|| format!("failed to create Wine prefix {}", prefix.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_redact_the_unlock_code() {
        assert_eq!(
            redact("unlock code secret-code was rejected", "secret-code"),
            "unlock code [REDACTED] was rejected"
        );
    }

    #[test]
    fn incomplete_prefixes_are_recreated() {
        let root = tempfile::tempdir().unwrap();
        let prefix = root.path().join("wine");
        fs::create_dir(&prefix).unwrap();
        fs::create_dir(prefix.join("drive_c")).unwrap();

        let (prepared, initialize) = prepare_prefix(&prefix).unwrap();
        assert_eq!(prepared, fs::canonicalize(&prefix).unwrap());
        assert!(initialize);
        assert!(!prepared.join("drive_c").exists());

        fs::create_dir(prepared.join("drive_c")).unwrap();
        fs::write(prepared.join("system.reg"), "registry").unwrap();
        fs::write(prepared.join("user.reg"), "registry").unwrap();
        assert!(!prepare_prefix(&prefix).unwrap().1);
    }
}
