//! Resolving configured names to safe LFS installation paths.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, ensure};

use super::valid_installation_id;

fn installation_root(root: &Path) -> anyhow::Result<PathBuf> {
    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to resolve LFS installation root {}", root.display()))?;
    ensure!(
        root.is_dir(),
        "LFS installation root {} is not a directory",
        root.display()
    );
    Ok(root)
}

/// Resolves the configured path of an installation, which need not exist yet.
pub(crate) fn installation_path(root: &Path, installation_id: &str) -> anyhow::Result<PathBuf> {
    ensure!(
        valid_installation_id(installation_id),
        "{installation_id:?} is not a valid installation identifier"
    );
    Ok(installation_root(root)?.join(installation_id))
}

/// Resolves and checks an existing named LFS installation.
pub(crate) fn resolve_installation(root: &Path, installation_id: &str) -> anyhow::Result<PathBuf> {
    ensure!(
        valid_installation_id(installation_id),
        "{installation_id:?} is not a valid installation identifier"
    );
    let root = installation_root(root)?;
    let configured_path = root.join(installation_id);
    let metadata = fs::symlink_metadata(&configured_path)
        .with_context(|| format!("no LFS installation named {installation_id:?}"))?;
    ensure!(
        metadata.file_type().is_dir(),
        "LFS installation {} must be a real directory, not a file or symlink",
        configured_path.display()
    );
    let installation_dir = fs::canonicalize(&configured_path)
        .with_context(|| format!("no LFS installation named {installation_id:?}"))?;
    ensure!(
        installation_dir.starts_with(&root),
        "LFS installation {installation_id:?} escapes its configured root"
    );
    ensure!(
        installation_dir.is_dir(),
        "LFS installation {} is not a directory",
        installation_dir.display()
    );
    ensure!(
        installation_dir.join("LFS.exe").is_file(),
        "LFS installation {} has no LFS.exe",
        installation_dir.display()
    );
    Ok(installation_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_installations_resolve_beneath_the_configured_root() {
        let root = tempfile::tempdir().unwrap();
        let installation = root.path().join("0.7");
        fs::create_dir(&installation).unwrap();
        fs::write(installation.join("LFS.exe"), b"executable").unwrap();

        assert_eq!(
            resolve_installation(root.path(), "0.7").unwrap(),
            fs::canonicalize(installation).unwrap()
        );
        assert!(resolve_installation(root.path(), "../other").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn named_installations_cannot_be_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::write(target.path().join("LFS.exe"), b"executable").unwrap();
        symlink(target.path(), root.path().join("0.7")).unwrap();
        assert!(resolve_installation(root.path(), "0.7").is_err());
    }

    #[test]
    fn installation_paths_do_not_require_the_named_installation_to_exist() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            installation_path(root.path(), "0.8").unwrap(),
            fs::canonicalize(root.path()).unwrap().join("0.8")
        );
    }
}
