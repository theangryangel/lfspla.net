//! Portable removal of expendable installation data.

use anyhow::{Context, ensure};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct TrimPlan {
    dds_entries: Vec<PathBuf>,
    pub dds_summary: Option<TrimSummary>,
    lgh_files: Vec<PathBuf>,
    pub lgh_summary: Option<TrimSummary>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrimSummary {
    pub files: u64,
    pub bytes: u64,
}

impl std::ops::AddAssign for TrimSummary {
    fn add_assign(&mut self, other: Self) {
        self.files += other.files;
        self.bytes += other.bytes;
    }
}

/// Execute a previously inspected plan. The caller obtains user confirmation.
pub fn trim(plan: TrimPlan) -> anyhow::Result<TrimSummary> {
    let summary = plan.summary();
    plan.execute()?;
    Ok(summary)
}

impl TrimPlan {
    pub fn new(installation_dir: &Path, dds: bool, lgh: bool) -> anyhow::Result<Self> {
        ensure!(dds || lgh, "select at least one of --dds or --lgh");
        let metadata =
            fs::symlink_metadata(installation_dir).context("failed to inspect LFS installation")?;
        ensure!(
            metadata.file_type().is_dir(),
            "LFS installation must be a real directory, not a symlink"
        );
        let install_dir = installation_dir.to_owned();
        let (dds_entries, dds_summary) = if dds {
            let directory = checked_directory(&install_dir, Path::new("data/dds"))?;
            let entries = directory_entries(&directory)?;
            let summary = measure_all(&entries)?;
            (entries, Some(summary))
        } else {
            (Vec::new(), None)
        };

        let (lgh_files, lgh_summary) = if lgh {
            let directory = checked_directory(&install_dir, Path::new("data/wld"))?;
            let files = direct_lgh_files(&directory)?;
            let summary = measure_all(&files)?;
            (files, Some(summary))
        } else {
            (Vec::new(), None)
        };

        Ok(Self {
            dds_entries,
            dds_summary,
            lgh_files,
            lgh_summary,
        })
    }

    #[must_use]
    pub fn summary(&self) -> TrimSummary {
        let mut summary = TrimSummary::default();
        if let Some(dds) = self.dds_summary {
            summary += dds;
        }
        if let Some(lgh) = self.lgh_summary {
            summary += lgh;
        }
        summary
    }

    fn execute(self) -> anyhow::Result<()> {
        for entry in self.dds_entries {
            remove_entry(&entry)?;
        }
        for file in self.lgh_files {
            fs::remove_file(&file)
                .with_context(|| format!("failed to remove {}", file.display()))?;
        }
        Ok(())
    }
}

fn checked_directory(install_dir: &Path, relative: &Path) -> anyhow::Result<PathBuf> {
    let mut directory = install_dir.to_owned();
    for component in relative.components() {
        directory.push(component);
        let metadata = fs::symlink_metadata(&directory).with_context(|| {
            format!("required directory {} does not exist", directory.display())
        })?;
        ensure!(
            metadata.file_type().is_dir(),
            "{} must be a real directory, not a file or symlink",
            directory.display()
        );
    }
    Ok(directory)
}

fn directory_entries(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .with_context(|| format!("failed to read an entry in {}", directory.display()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    entries.sort_unstable();
    Ok(entries)
}

fn direct_lgh_files(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in directory_entries(directory)? {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        let is_lgh = path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lgh"));
        if metadata.file_type().is_file() && is_lgh {
            files.push(path);
        }
    }
    Ok(files)
}

fn measure_all(paths: &[PathBuf]) -> anyhow::Result<TrimSummary> {
    let mut summary = TrimSummary::default();
    let mut pending = paths.to_vec();
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.file_type().is_file() {
            summary.files += 1;
            summary.bytes += metadata.len();
        } else if metadata.file_type().is_dir() {
            pending.extend(directory_entries(&path)?);
        }
    }
    Ok(summary)
}

fn remove_entry(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {} before removal", path.display()))?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installation() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("data/dds/nested")).unwrap();
        fs::create_dir_all(root.path().join("data/wld/nested")).unwrap();
        fs::write(root.path().join("LFS.exe"), b"executable").unwrap();
        fs::write(root.path().join("data/dds/texture.dds"), b"texture").unwrap();
        fs::write(root.path().join("data/dds/nested/texture.dds"), b"texture").unwrap();
        fs::write(root.path().join("data/wld/AS.lgh"), b"geometry").unwrap();
        fs::write(root.path().join("data/wld/BL.LGH"), b"geometry").unwrap();
        fs::write(root.path().join("data/wld/AS.wld"), b"world").unwrap();
        fs::write(root.path().join("data/wld/nested/keep.lgh"), b"geometry").unwrap();
        root
    }

    #[test]
    fn requires_an_explicit_trim_selection() {
        let root = installation();
        let error = TrimPlan::new(root.path(), false, false).unwrap_err();
        assert!(error.to_string().contains("--dds or --lgh"));
    }

    #[test]
    fn trims_only_the_selected_data() {
        let root = installation();
        trim(TrimPlan::new(root.path(), true, false).unwrap()).unwrap();

        assert!(root.path().join("data/dds").is_dir());
        assert_eq!(
            fs::read_dir(root.path().join("data/dds")).unwrap().count(),
            0
        );
        assert!(root.path().join("data/wld/AS.lgh").is_file());

        trim(TrimPlan::new(root.path(), false, true).unwrap()).unwrap();
        assert!(!root.path().join("data/wld/AS.lgh").exists());
        assert!(!root.path().join("data/wld/BL.LGH").exists());
        assert!(root.path().join("data/wld/AS.wld").is_file());
        assert!(root.path().join("data/wld/nested/keep.lgh").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_trim_directory() {
        use std::os::unix::fs::symlink;

        let root = installation();
        let outside = tempfile::tempdir().unwrap();
        fs::remove_dir_all(root.path().join("data/dds")).unwrap();
        symlink(outside.path(), root.path().join("data/dds")).unwrap();

        let error = TrimPlan::new(root.path(), true, false).unwrap_err();
        assert!(error.to_string().contains("must be a real directory"));
    }
}
