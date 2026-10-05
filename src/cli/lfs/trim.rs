//! Trim command presentation and deletion confirmation.

use crate::{
    services::manage_lfs::{self, resolve_installation},
    settings::Settings,
};
use anyhow::ensure;
use lfsplanet_lfs::TrimPlan;
use size::Size;
use std::path::Path;

pub(super) fn run(
    settings: &Settings,
    installation_id: &str,
    dds: bool,
    lgh: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let installation_dir =
        resolve_installation(settings.lfs.installation_root.path(), installation_id)?;
    trim(&installation_dir, installation_id, dds, lgh, yes)
}

fn trim(
    installation_dir: &Path,
    installation_id: &str,
    dds: bool,
    lgh: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let summary = manage_lfs::trim(installation_dir, dds, lgh, |plan| {
        warn(installation_dir, plan);
        ensure!(
            yes,
            "trimming permanently deletes LFS data; rerun with --yes to confirm"
        );
        Ok(())
    })?;
    tracing::info!(
        installation_id,
        path = %installation_dir.display(),
        files = summary.files,
        bytes = summary.bytes,
        "LFS installation trimmed"
    );
    Ok(())
}

fn warn(installation_dir: &Path, plan: &TrimPlan) {
    eprintln!(
        "WARNING: trimming {} permanently removes the selected data:",
        installation_dir.display()
    );
    if let Some(summary) = plan.dds_summary {
        eprintln!(
            "  data/dds contents: {} files, {}",
            summary.files,
            Size::from_bytes(summary.bytes)
        );
    }
    if let Some(summary) = plan.lgh_summary {
        eprintln!(
            "  data/wld/*.lgh: {} files, {}",
            summary.files,
            Size::from_bytes(summary.bytes)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn installation() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("data/dds")).unwrap();
        fs::create_dir_all(root.path().join("data/wld")).unwrap();
        fs::write(root.path().join("data/dds/texture.dds"), b"texture").unwrap();
        fs::write(root.path().join("data/wld/AS.lgh"), b"geometry").unwrap();
        root
    }

    #[test]
    fn refuses_without_confirmation_and_changes_nothing() {
        let root = installation();
        let error = trim(root.path(), "test", true, true, false).unwrap_err();
        assert!(error.to_string().contains("--yes"));
        assert!(root.path().join("data/dds/texture.dds").exists());
        assert!(root.path().join("data/wld/AS.lgh").exists());
    }
}
