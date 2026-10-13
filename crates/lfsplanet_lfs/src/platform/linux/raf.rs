//! Capture the first RAF from a fresh, per-invocation output directory.

use anyhow::{Context, ensure};
use std::{fs, io::Read, path::Path};

const MAX_RAF_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn capture(directory: &Path) -> anyhow::Result<Option<crate::RafFile>> {
    for entry in fs::read_dir(directory).context("failed to list RAF output")? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || !entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("raf"))
        {
            continue;
        }
        let mut bytes = Vec::new();
        fs::File::open(entry.path())?
            .take(MAX_RAF_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_RAF_BYTES,
            "RAF output exceeds size limit"
        );
        validate(&bytes)?;
        return Ok(Some(crate::RafFile {
            filename: entry.file_name().to_string_lossy().into_owned(),
            bytes,
        }));
    }
    Ok(None)
}

fn validate(bytes: &[u8]) -> anyhow::Result<()> {
    ensure!(bytes.len() >= 1024, "RAF header is truncated");
    ensure!(&bytes[..6] == b"LFSRAF", "invalid RAF signature");
    ensure!(bytes[8] == 2, "unsupported RAF version");
    ensure!(bytes[9] != 0, "RAF update interval is zero");
    let word = |offset| usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
    let header_size = word(12);
    let block_size = word(14);
    let wheel_size = word(16);
    let wheel_offset = word(18);
    let blocks = i32::from_le_bytes(bytes[20..24].try_into().unwrap());
    ensure!(
        header_size >= 1024 && block_size >= 64,
        "invalid RAF block sizes"
    );
    ensure!(
        wheel_size >= 32
            && wheel_offset >= 64
            && wheel_offset + usize::from(bytes[169]) * wheel_size <= block_size,
        "invalid RAF wheel layout"
    );
    let blocks = usize::try_from(blocks).context("negative RAF block count")?;
    ensure!(blocks > 0, "RAF contains no samples");
    let expected = blocks
        .checked_mul(block_size)
        .and_then(|size| size.checked_add(header_size))
        .context("RAF size overflow")?;
    ensure!(
        bytes.len() == expected,
        "RAF data length does not match its header"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raf(header: u16, block: u16, wheel: u16, offset: u16) -> Vec<u8> {
        let mut bytes = vec![0; usize::from(header) + usize::from(block)];
        bytes[..6].copy_from_slice(b"LFSRAF");
        bytes[8] = 2;
        bytes[9] = 100;
        for (position, value) in [(12, header), (14, block), (16, wheel), (18, offset)] {
            bytes[position..position + 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes[20..24].copy_from_slice(&1_i32.to_le_bytes());
        bytes[169] = 4;
        bytes
    }

    #[test]
    fn captures_any_raf_name_from_an_isolated_directory() {
        let directory = tempfile::tempdir().unwrap();
        assert!(capture(directory.path()).unwrap().is_none());
        fs::write(directory.path().join("log.txt"), b"ignored").unwrap();
        let bytes = raf(1024, 192, 32, 64);
        fs::write(directory.path().join("whatever-LFS-calls-it.RAF"), &bytes).unwrap();
        assert_eq!(
            capture(directory.path()).unwrap(),
            Some(crate::RafFile {
                filename: "whatever-LFS-calls-it.RAF".to_owned(),
                bytes
            })
        );
        assert!(
            capture(tempfile::tempdir().unwrap().path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn honours_extended_sizes_and_rejects_incomplete_output() {
        assert!(validate(&raf(1100, 224, 36, 80)).is_ok());
        let bytes = raf(1024, 192, 32, 64);
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        for (offset, value) in [(0, 0), (8, 3), (9, 0), (20, 2), (169, 5)] {
            let mut invalid = bytes.clone();
            invalid[offset] = value;
            assert!(validate(&invalid).is_err());
        }
    }

    #[test]
    fn ignores_symlinks_and_directories() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("folder.raf")).unwrap();
        std::os::unix::fs::symlink("/does/not/exist", directory.path().join("link.raf")).unwrap();
        assert!(capture(directory.path()).unwrap().is_none());
    }
}
