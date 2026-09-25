//! Explicit removal of version-dependent, expendable LFS data.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, ensure};

use crate::{cli::Args, lfs::installations::resolve_installation, settings::Settings};

struct TrimPlan {
    install_dir: PathBuf,
    dds_entries: Vec<PathBuf>,
    dds_summary: Option<Summary>,
    lgh_files: Vec<PathBuf>,
    lgh_summary: Option<Summary>,
}

#[derive(Clone, Copy, Default)]
struct Summary {
    files: u64,
    bytes: u64,
}

impl std::ops::AddAssign for Summary {
    fn add_assign(&mut self, other: Self) {
        self.files += other.files;
        self.bytes += other.bytes;
    }
}

pub(super) fn run(
    args: &Args,
    installation_id: &str,
    dds: bool,
    lgh: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let installation_dir = resolve_installation(
        settings.lfs_runtime.installation_root.path(),
        installation_id,
    )?;
    trim(&installation_dir, installation_id, dds, lgh, yes)
}

fn trim(
    installation_dir: &Path,
    installation_id: &str,
    dds: bool,
    lgh: bool,
    yes: bool,
) -> anyhow::Result<()> {
    ensure!(dds || lgh, "select at least one of --dds or --lgh");
    let plan = TrimPlan::discover(installation_dir.to_owned(), dds, lgh)?;
    plan.warn();
    ensure!(
        yes,
        "trimming permanently deletes LFS data; rerun with --yes to confirm"
    );

    let summary = plan.summary();
    plan.execute()?;
    tracing::info!(
        installation_id,
        path = %installation_dir.display(),
        files = summary.files,
        bytes = summary.bytes,
        "LFS installation trimmed"
    );
    Ok(())
}

impl TrimPlan {
    fn discover(install_dir: PathBuf, dds: bool, lgh: bool) -> anyhow::Result<Self> {
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
            install_dir,
            dds_entries,
            dds_summary,
            lgh_files,
            lgh_summary,
        })
    }

    fn warn(&self) {
        eprintln!(
            "WARNING: trimming {} permanently removes the selected data:",
            self.install_dir.display()
        );
        if let Some(summary) = self.dds_summary {
            eprintln!(
                "  data/dds contents: {} files, {}",
                summary.files,
                display_bytes(summary.bytes)
            );
        }
        if let Some(summary) = self.lgh_summary {
            eprintln!(
                "  data/wld/*.lgh: {} files, {}",
                summary.files,
                display_bytes(summary.bytes)
            );
        }
    }

    fn summary(&self) -> Summary {
        let mut summary = Summary::default();
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

fn measure_all(paths: &[PathBuf]) -> anyhow::Result<Summary> {
    let mut summary = Summary::default();
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

fn display_bytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;
    if bytes >= GIB {
        format!("{} GiB ({bytes} bytes)", scaled_hundredths(bytes, GIB))
    } else if bytes >= MIB {
        format!("{} MiB ({bytes} bytes)", scaled_hundredths(bytes, MIB))
    } else {
        format!("{bytes} bytes")
    }
}

fn scaled_hundredths(bytes: u64, unit: u64) -> String {
    let hundredths = (u128::from(bytes) * 100 + u128::from(unit) / 2) / u128::from(unit);
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
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
        let error = trim(root.path(), "test", false, false, true).unwrap_err();
        assert!(error.to_string().contains("--dds or --lgh"));
    }

    #[test]
    fn refuses_without_confirmation_and_changes_nothing() {
        let root = installation();
        let error = trim(root.path(), "test", true, true, false).unwrap_err();
        assert!(error.to_string().contains("--yes"));
        assert!(root.path().join("data/dds/texture.dds").exists());
        assert!(root.path().join("data/wld/AS.lgh").exists());
    }

    #[test]
    fn trims_only_the_selected_data() {
        let root = installation();
        trim(root.path(), "test", true, false, true).unwrap();

        assert!(root.path().join("data/dds").is_dir());
        assert_eq!(
            fs::read_dir(root.path().join("data/dds")).unwrap().count(),
            0
        );
        assert!(root.path().join("data/wld/AS.lgh").is_file());

        trim(root.path(), "test", false, true, true).unwrap();
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

        let error = trim(root.path(), "test", true, false, true).unwrap_err();
        assert!(error.to_string().contains("must be a real directory"));
    }

    #[test]
    fn formats_large_sizes_for_the_warning() {
        assert_eq!(display_bytes(42), "42 bytes");
        assert_eq!(display_bytes(2 * 1024 * 1024), "2.00 MiB (2097152 bytes)");
        assert_eq!(
            display_bytes(3 * 1024 * 1024 * 1024),
            "3.00 GiB (3221225472 bytes)"
        );
    }
}
