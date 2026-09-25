//! Fresh LFS installation.

use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::{cli::Args, lfs::installations::installation_path, settings::Settings};
use lfsplanet_lfs::{Command as SandboxCommand, executable, validate_prefix};

use super::{checked_output, combined_output, redact};

#[allow(
    clippy::too_many_arguments,
    reason = "the arguments are the install subcommand's explicit operator inputs"
)]
pub(super) async fn run(
    args: &Args,
    installation_id: &str,
    download_url: &str,
    username: &str,
    unlock_code: &str,
    command_timeout_seconds: u64,
) -> anyhow::Result<()> {
    ensure!(
        command_timeout_seconds > 0,
        "command timeout must be greater than zero"
    );

    let settings = Settings::load(&args.config)?;
    let bubblewrap = executable(
        settings.lfs_runtime.bubblewrap_executable.path(),
        "Bubblewrap",
    )?;
    let wine = executable(settings.lfs_runtime.wine_executable.path(), "Wine")?;
    let installation_dir = new_installation_path(
        settings.lfs_runtime.installation_root.path(),
        installation_id,
    )?;

    fs::DirBuilder::new()
        .mode(0o700)
        .create(&installation_dir)
        .with_context(|| format!("failed to create {}", installation_dir.display()))?;

    let result = install_into(
        &settings,
        &bubblewrap,
        &wine,
        download_url,
        &installation_dir,
        username,
        unlock_code,
        Duration::from_secs(command_timeout_seconds),
    )
    .await;
    if let Err(error) = result {
        return Err(error.context(format!(
            "partial installation remains at {}",
            installation_dir.display()
        )));
    }

    tracing::info!(installation_id, path = %installation_dir.display(), "LFS installation completed");
    Ok(())
}

fn new_installation_path(root: &Path, installation_id: &str) -> anyhow::Result<std::path::PathBuf> {
    let installation_dir = installation_path(root, installation_id)?;
    match fs::symlink_metadata(&installation_dir) {
        Ok(_) => bail!(
            "LFS installation {installation_id:?} already exists at {}",
            installation_dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", installation_dir.display()));
        }
    }
    Ok(installation_dir)
}

#[allow(clippy::too_many_arguments)]
async fn install_into(
    settings: &Settings,
    bubblewrap: &Path,
    wine: &Path,
    download_url: &str,
    installation_dir: &Path,
    username: &str,
    unlock_code: &str,
    command_timeout: Duration,
) -> anyhow::Result<()> {
    let setup = cached_installer_path(
        settings.lfs_runtime.installer_download_root.path(),
        download_url,
        settings.lfs.outbound_http_timeout.duration(),
    )
    .await?;

    let (wine_prefix, initialize_prefix) = prepare_prefix(settings.lfs_runtime.wine_prefix.path())?;
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
            setup: &setup,
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
            if validate_prefix(requested).is_ok() {
                return Ok((
                    fs::canonicalize(requested).with_context(|| {
                        format!("failed to resolve Wine prefix {}", requested.display())
                    })?,
                    false,
                ));
            }

            tracing::warn!(path = %requested.display(), "removing incomplete Wine prefix");
            fs::remove_dir_all(requested).with_context(|| {
                format!(
                    "failed to remove incomplete Wine prefix {}",
                    requested.display()
                )
            })?;
            create_prefix(requested)?;
            Ok((
                fs::canonicalize(requested).with_context(|| {
                    format!("failed to resolve Wine prefix {}", requested.display())
                })?,
                true,
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_prefix(requested)?;
            Ok((
                fs::canonicalize(requested).with_context(|| {
                    format!("failed to resolve Wine prefix {}", requested.display())
                })?,
                true,
            ))
        }
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect Wine prefix {}", requested.display())),
    }
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

async fn cached_installer_path(
    root: &Path,
    url: &str,
    inactivity_timeout: Duration,
) -> anyhow::Result<PathBuf> {
    let destination = installer_cache_path(root, url);
    match tokio::fs::metadata(&destination).await {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
            tracing::info!(path = %destination.display(), "using cached LFS installer");
            return Ok(destination);
        }
        Ok(_) => {
            tokio::fs::remove_file(&destination)
                .await
                .with_context(|| {
                    format!(
                        "failed to remove invalid cache entry {}",
                        destination.display()
                    )
                })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", destination.display()));
        }
    }

    let temporary = destination.with_extension("part");
    tracing::info!(url, "downloading LFS installer");
    let client = lfsplanet_lfs_api::download_client(inactivity_timeout)
        .context("failed to build installer download client")?;
    let response = client
        .get(url)
        .send()
        .await
        .context("failed to download LFS installer")?
        .error_for_status()
        .context("LFS installer server returned an error")?;

    let mut file = tokio::fs::File::create(&temporary)
        .await
        .with_context(|| format!("failed to create {}", temporary.display()))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = tokio::time::timeout(inactivity_timeout, stream.next())
        .await
        .context("installer download stalled")?
    {
        let chunk = chunk.context("failed while streaming LFS installer")?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("failed to write {}", temporary.display()))?;
    }
    file.sync_all()
        .await
        .with_context(|| format!("failed to sync {}", temporary.display()))?;
    drop(file);
    tokio::fs::rename(&temporary, &destination)
        .await
        .with_context(|| format!("failed to cache LFS installer at {}", destination.display()))?;

    Ok(destination)
}

fn installer_cache_path(root: &Path, url: &str) -> PathBuf {
    root.join(format!(
        "{}.exe",
        hex::encode(Sha256::digest(url.as_bytes()))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_installations_use_an_explicit_nonexistent_path() {
        let parent = tempfile::tempdir().unwrap();
        let requested = parent.path().join("validator");
        assert_eq!(
            new_installation_path(parent.path(), "validator").unwrap(),
            requested
        );

        fs::create_dir(&requested).unwrap();
        assert!(
            new_installation_path(parent.path(), "validator")
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );
    }

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

    #[test]
    fn installer_cache_entries_are_keyed_by_url() {
        let root = Path::new("/srv/lfs/downloads");
        assert_eq!(
            installer_cache_path(root, "https://example.test/LFS_S3_8C20_setup.exe"),
            root.join("d2d673453813d67c674883f321d37a3ce07bcc4ffe52e91859f1e393de4f11dc.exe")
        );
        assert_ne!(
            installer_cache_path(root, "https://example.test/a.exe"),
            installer_cache_path(root, "https://example.test/b.exe")
        );
    }
}
