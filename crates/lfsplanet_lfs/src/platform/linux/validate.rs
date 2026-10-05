//! Replay preparation, sandbox execution and status decoding.

use super::{
    RuntimeConfig, capture_output,
    command::{Command as SandboxCommand, validate_prefix},
};
use crate::{HlvcResult, ValidationDiagnostic, ValidationResult};
use anyhow::{Context, ensure};
use serde::{Deserialize, Deserializer, de::Error as _};
use std::{fs, io::Write, path::Path, process::Output, time::Duration};
use tempfile::NamedTempFile;

const VALIDATOR_BATCH: &[u8] = include_bytes!("validate.bat");

/// Validate a replay while retaining diagnostics even when execution produces
/// no complete HLVC verdict.
pub async fn validate(
    config: &RuntimeConfig,
    installation_dir: &Path,
    replay: &[u8],
    timeout: Duration,
) -> anyhow::Result<ValidationDiagnostic> {
    ensure!(
        !timeout.is_zero(),
        "validation timeout must be greater than zero"
    );
    let (bubblewrap, wine) = config.programs()?;
    let wine_prefix = validate_prefix(&config.wine_prefix)?;
    let installation_dir =
        fs::canonicalize(installation_dir).context("failed to resolve LFS installation")?;
    run_with(
        &installation_dir,
        &wine_prefix,
        &bubblewrap,
        &wine,
        timeout,
        replay,
    )
    .await
}

async fn run_with(
    installation_dir: &Path,
    wine_prefix: &Path,
    bubblewrap: &Path,
    wine: &Path,
    timeout: Duration,
    replay: &[u8],
) -> anyhow::Result<ValidationDiagnostic> {
    ensure!(
        installation_dir.join("data/spr").is_dir(),
        "LFS installation has no data/spr directory"
    );
    fs::create_dir_all(installation_dir.join("mods/vehicles")).with_context(|| {
        format!(
            "failed to create mod cache in {}",
            installation_dir.display()
        )
    })?;
    fs::create_dir_all(installation_dir.join("cache"))
        .with_context(|| format!("failed to create cache in {}", installation_dir.display()))?;

    let batch = temporary_file("embedded validator batch", VALIDATOR_BATCH)?;
    let replay_file = temporary_file("replay", replay)?;
    let result_file = temporary_file("HLVC result", b"")?;
    let command = SandboxCommand::Validate {
        installation_dir,
        wine_prefix,
        replay: replay_file.path(),
        batch: batch.path(),
        result: result_file.path(),
    }
    .build(bubblewrap, wine);
    let output = capture_output(command, timeout, "sandboxed validation").await?;
    let contents = fs::read(result_file.path()).context("failed to read the HLVC result file");
    let verdict = contents.and_then(|contents| ValidationResult::from_output(&output, &contents));
    Ok(ValidationDiagnostic { output, verdict })
}

enum StatusDocument {
    ChildStarted(u32),
    SandboxExited(i32),
    Unknown,
}

impl<'de> Deserialize<'de> for StatusDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let child_pid = object.remove("child-pid");
        let sandbox_exit_code = object.remove("exit-code");

        match (child_pid, sandbox_exit_code) {
            (Some(value), None) => serde_json::from_value(value)
                .map(Self::ChildStarted)
                .map_err(D::Error::custom),
            (None, Some(value)) => serde_json::from_value(value)
                .map(Self::SandboxExited)
                .map_err(D::Error::custom),
            (None, None) => Ok(Self::Unknown),
            _ => Err(D::Error::custom(
                "status document contains more than one recognized event",
            )),
        }
    }
}

impl ValidationResult {
    fn from_output(output: &Output, result_file: &[u8]) -> anyhow::Result<Self> {
        let mut child_pid = None;
        let mut sandbox_exit_code = None;
        let documents =
            serde_json::Deserializer::from_slice(&output.stdout).into_iter::<StatusDocument>();

        for document in documents {
            match document.context("Bubblewrap returned invalid JSON status output")? {
                StatusDocument::ChildStarted(pid) => ensure!(
                    child_pid.replace(pid).is_none(),
                    "Bubblewrap returned more than one child process"
                ),
                StatusDocument::SandboxExited(code) => ensure!(
                    sandbox_exit_code.replace(code).is_none(),
                    "Bubblewrap returned more than one exit status"
                ),
                StatusDocument::Unknown => {}
            }
        }
        let child_pid = child_pid.with_context(|| {
            format!("Bubblewrap failed before starting Wine ({})", output.status)
        })?;
        let runtime_exit_code = sandbox_exit_code.with_context(|| {
            format!(
                "Bubblewrap failed before the validation child exited ({})",
                output.status
            )
        })?;
        let process_exit_code = output
            .status
            .code()
            .context("Bubblewrap was terminated by a signal")?;
        ensure!(
            process_exit_code == runtime_exit_code,
            "Bubblewrap exited with {process_exit_code} but reported Wine exited with \
             {runtime_exit_code}"
        );
        let exit_code = lfs_exit_code_from_file(result_file)?;
        let lfs = HlvcResult::try_from(exit_code)
            .map_err(|code| anyhow::anyhow!("LFS returned unsupported HLVC result {code}"))?;
        Ok(Self {
            child_pid,
            process_exit_code,
            runtime_exit_code,
            lfs,
        })
    }
}

fn lfs_exit_code_from_file(contents: &[u8]) -> anyhow::Result<i32> {
    let text = String::from_utf8_lossy(contents);
    ensure!(
        !text.trim().is_empty(),
        "validator did not write an HLVC result file"
    );
    text.trim()
        .parse::<i32>()
        .context("validator wrote an invalid HLVC result file")
}

fn temporary_file(label: &str, contents: &[u8]) -> anyhow::Result<NamedTempFile> {
    let mut file =
        NamedTempFile::new().with_context(|| format!("failed to create {label} file"))?;
    file.write_all(contents)
        .with_context(|| format!("failed to write {label} file"))?;
    file.flush()
        .with_context(|| format!("failed to flush {label} file"))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn output(code: i32, stdout: &[u8]) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn result_file_supplies_the_hlvc_verdict() {
        for (code, expected) in [(1, HlvcResult::Ok), (21, HlvcResult::FailOutOfBounds)] {
            let stdout = format!("{{\"child-pid\":123}}\n{{\"exit-code\":{code}}}\n");
            let result = ValidationResult::from_output(
                &output(code, stdout.as_bytes()),
                format!("{code}\r\n").as_bytes(),
            )
            .unwrap();
            assert_eq!(result.child_pid, 123);
            assert_eq!(result.process_exit_code, code);
            assert_eq!(result.runtime_exit_code, code);
            assert_eq!(result.lfs, expected);
        }
    }

    #[test]
    fn validation_requires_a_complete_result_file() {
        let output = output(
            1,
            b"{\"child-pid\":123}\n{\"lfs-exit-code\":1}\n{\"exit-code\":1}\n",
        );
        for contents in [b"".as_slice(), b"unfinished", b"1\n21", b"23"] {
            assert!(ValidationResult::from_output(&output, contents).is_err());
        }
    }

    #[test]
    fn process_status_must_be_complete_and_consistent() {
        for stdout in [
            b"".as_slice(),
            b"invalid json",
            b"{\"child-pid\":123}\n",
            b"{\"exit-code\":1}\n",
            b"{\"child-pid\":123}\n{\"exit-code\":21}\n",
            b"{\"child-pid\":123}\n{\"child-pid\":124}\n{\"exit-code\":1}\n",
            b"{\"child-pid\":123}\n{\"exit-code\":1}\n{\"exit-code\":1}\n",
            b"{\"child-pid\":123,\"exit-code\":1}\n",
        ] {
            assert!(ValidationResult::from_output(&output(1, stdout), b"1").is_err());
        }
        let mut signalled = output(1, b"{\"child-pid\":123}\n{\"exit-code\":1}\n");
        signalled.status = std::process::ExitStatus::from_raw(9);
        assert!(ValidationResult::from_output(&signalled, b"1").is_err());
    }
}
