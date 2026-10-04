//! Add a named LFS installation from a downloaded installer.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail, ensure};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::{cli::Args, lfs::installations::installation_path, settings::Settings};

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

    lfsplanet_lfs::ensure_supported()?;
    let settings = Settings::load(&args.config)?;
    let installation_dir =
        new_installation_path(settings.lfs.installation_root.path(), installation_id)?;

    let cache_root = settings.lfs.installer_cache_root();
    let setup = cached_installer_path(
        &cache_root,
        download_url,
        settings.lfs.outbound_http_timeout.duration(),
    )
    .await?;
    lfsplanet_lfs::install(
        &settings.lfs.runtime,
        &setup,
        &installation_dir,
        username,
        unlock_code,
        Duration::from_secs(command_timeout_seconds),
    )
    .await?;

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

async fn cached_installer_path(
    root: &Path,
    url: &str,
    inactivity_timeout: Duration,
) -> anyhow::Result<PathBuf> {
    tokio::fs::create_dir_all(root)
        .await
        .with_context(|| format!("failed to create installer cache {}", root.display()))?;
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

    #[tokio::test]
    async fn installer_cache_is_created_and_existing_downloads_are_reused() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join(".cache/installers");
        // Invalid URLs fail before making a network request.
        assert!(
            cached_installer_path(&cache, "invalid-url", Duration::from_secs(1))
                .await
                .is_err()
        );
        assert!(cache.is_dir());

        let url = "https://example.test/setup.exe";
        let installer = installer_cache_path(&cache, url);
        fs::write(&installer, b"cached installer").unwrap();
        assert_eq!(
            cached_installer_path(&cache, url, Duration::from_secs(1))
                .await
                .unwrap(),
            installer
        );
        assert_eq!(fs::read(&installer).unwrap(), b"cached installer");
    }

    #[test]
    fn installer_cache_entries_are_keyed_by_url() {
        let root = Path::new("/srv/lfs/.cache/installers");
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
