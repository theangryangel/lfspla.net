//! Bubblewrap boundary for LFS replay validation.

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Output, Stdio},
};

use anyhow::{Context, ensure};
use serde::{Deserialize, Deserializer, de::Error as _};
use tempfile::NamedTempFile;

use crate::{
    lfs::installations::resolve_installation,
    settings::{HlvcSettings, LfsRuntimeSettings},
};
use lfsplanet_lfs::{Command as SandboxCommand, executable, validate_prefix};

use super::HlvcResult;

const VALIDATOR_BATCH: &[u8] = include_bytes!("validate.bat");

/// Resolves a named installation and validates one replay in ephemeral LFS
/// and Wine-prefix overlays.
pub(crate) async fn validate(
    runtime: &LfsRuntimeSettings,
    settings: &HlvcSettings,
    installation_id: &str,
    replay: &[u8],
) -> anyhow::Result<ValidationResult> {
    let installation_dir = resolve_installation(runtime.installation_root.path(), installation_id)?;
    let wine_prefix = validate_prefix(runtime.wine_prefix.path())?;
    let bubblewrap = executable(runtime.bubblewrap_executable.path(), "Bubblewrap")?;
    let wine = executable(runtime.wine_executable.path(), "Wine")?;
    validate_with(
        &installation_dir,
        &wine_prefix,
        &bubblewrap,
        &wine,
        settings.timeout.duration(),
        replay,
    )
    .await
}

/// Runs one validation while retaining Wine and Bubblewrap diagnostics for an
/// operator to inspect. This does not write any database or object-store state.
pub(crate) async fn diagnose(
    runtime: &LfsRuntimeSettings,
    settings: &HlvcSettings,
    installation_id: &str,
    replay: &[u8],
) -> anyhow::Result<ValidationDiagnostic> {
    let installation_dir = resolve_installation(runtime.installation_root.path(), installation_id)?;
    let wine_prefix = validate_prefix(runtime.wine_prefix.path())?;
    let bubblewrap = executable(runtime.bubblewrap_executable.path(), "Bubblewrap")?;
    let wine = executable(runtime.wine_executable.path(), "Wine")?;
    run_with(
        &installation_dir,
        &wine_prefix,
        &bubblewrap,
        &wine,
        settings.timeout.duration(),
        replay,
    )
    .await
}

async fn validate_with(
    installation_dir: &Path,
    wine_prefix: &Path,
    bubblewrap: &Path,
    wine: &Path,
    timeout: std::time::Duration,
    replay: &[u8],
) -> anyhow::Result<ValidationResult> {
    let diagnostic = run_with(
        installation_dir,
        wine_prefix,
        bubblewrap,
        wine,
        timeout,
        replay,
    )
    .await?;
    if !diagnostic.output.stderr.is_empty() {
        tracing::debug!(
            stderr = %String::from_utf8_lossy(&diagnostic.output.stderr),
            "validation command diagnostics"
        );
    }
    let result = diagnostic.result()?;
    tracing::debug!(
        child_pid = result.child_pid,
        bubblewrap_exit_code = result.bubblewrap_exit_code,
        wine_exit_code = result.wine_exit_code,
        lfs_exit_code = result.lfs.code(),
        "validation command completed"
    );
    Ok(result)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the arguments configure one sandbox execution"
)]
async fn run_with(
    installation_dir: &Path,
    wine_prefix: &Path,
    bubblewrap: &Path,
    wine: &Path,
    timeout: std::time::Duration,
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
    let mut command = SandboxCommand::Validate {
        installation_dir,
        wine_prefix,
        replay: replay_file.path(),
        batch: batch.path(),
        result: result_file.path(),
    }
    .build(bubblewrap, wine);
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start the Bubblewrap sandbox")?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .with_context(|| {
            format!(
                "sandboxed validation exceeded its {} second deadline",
                timeout.as_secs()
            )
        })?
        .context("failed while waiting for sandboxed validation")?;
    let result_file = fs::read(result_file.path());
    Ok(ValidationDiagnostic {
        output,
        result_file,
    })
}

/// Raw process output and the dedicated result file from one validation.
pub(crate) struct ValidationDiagnostic {
    pub(crate) output: Output,
    result_file: std::io::Result<Vec<u8>>,
}

impl ValidationDiagnostic {
    pub(crate) fn result(&self) -> anyhow::Result<ValidationResult> {
        let result_file = self
            .result_file
            .as_ref()
            .map_err(|error| anyhow::anyhow!("failed to read the HLVC result file: {error}"))?;
        ValidationResult::from_output(&self.output, Some(result_file))
    }
}

enum StatusDocument {
    ChildStarted(u32),
    SandboxExited(i32),
    LfsExited(i32),
    Unknown,
}

#[derive(Debug)]
pub(crate) struct ValidationResult {
    pub(crate) child_pid: u32,
    pub(crate) bubblewrap_exit_code: i32,
    pub(crate) wine_exit_code: i32,
    pub(crate) lfs: HlvcResult,
}

impl<'de> Deserialize<'de> for StatusDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let child_pid = object.remove("child-pid");
        let sandbox_exit_code = object.remove("exit-code");
        let lfs_exit_code = object.remove("lfs-exit-code");

        match (child_pid, sandbox_exit_code, lfs_exit_code) {
            (Some(value), None, None) => serde_json::from_value(value)
                .map(Self::ChildStarted)
                .map_err(D::Error::custom),
            (None, Some(value), None) => serde_json::from_value(value)
                .map(Self::SandboxExited)
                .map_err(D::Error::custom),
            (None, None, Some(value)) => serde_json::from_value(value)
                .map(Self::LfsExited)
                .map_err(D::Error::custom),
            (None, None, None) => Ok(Self::Unknown),
            _ => Err(D::Error::custom(
                "status document contains more than one recognized event",
            )),
        }
    }
}

impl TryFrom<&Output> for ValidationResult {
    type Error = anyhow::Error;

    fn try_from(output: &Output) -> Result<Self, Self::Error> {
        Self::from_output(output, None)
    }
}

impl ValidationResult {
    fn from_output(output: &Output, result_file: Option<&[u8]>) -> anyhow::Result<Self> {
        let mut child_pid = None;
        let mut sandbox_exit_code = None;
        let mut lfs_exit_code = None;
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
                StatusDocument::LfsExited(code) => ensure!(
                    lfs_exit_code.replace(code).is_none(),
                    "validator returned more than one HLVC result"
                ),
                StatusDocument::Unknown => {}
            }
        }
        let child_pid = child_pid.with_context(|| {
            format!("Bubblewrap failed before starting Wine ({})", output.status)
        })?;
        let wine_exit_code = sandbox_exit_code.with_context(|| {
            format!(
                "Bubblewrap failed before the validation child exited ({})",
                output.status
            )
        })?;
        let bubblewrap_exit_code = output
            .status
            .code()
            .context("Bubblewrap was terminated by a signal")?;
        ensure!(
            bubblewrap_exit_code == wine_exit_code,
            "Bubblewrap exited with {bubblewrap_exit_code} but reported Wine exited with \
             {wine_exit_code}"
        );
        let file_lfs_exit_code = result_file.map(lfs_exit_code_from_file).transpose()?;
        if let (Some(stdout_code), Some(file_code)) = (lfs_exit_code, file_lfs_exit_code) {
            ensure!(
                stdout_code == file_code,
                "validator returned conflicting HLVC results: {stdout_code} on stdout and {file_code} in its result file"
            );
        }
        let exit_code = lfs_exit_code.or(file_lfs_exit_code).with_context(|| {
            format!(
                "Wine failed before LFS returned an HLVC result ({})",
                output.status
            )
        })?;
        let lfs = HlvcResult::try_from(exit_code)
            .map_err(|code| anyhow::anyhow!("LFS returned unsupported HLVC result {code}"))?;
        Ok(Self {
            child_pid,
            bubblewrap_exit_code,
            wine_exit_code,
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

    #[test]
    fn json_status_stream_distinguishes_execution_stages() {
        let failure = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 127"])
            .status()
            .unwrap();
        let lfs_ok = std::process::Command::new("/bin/false").status().unwrap();
        let exit_21 = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 21"])
            .status()
            .unwrap();

        let output = Output {
            status: failure,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert!(ValidationResult::try_from(&output).is_err());

        let output = Output {
            status: failure,
            stdout: b"{\"child-pid\":123}\n{\"exit-code\":1}\n".to_vec(),
            stderr: Vec::new(),
        };
        assert!(ValidationResult::try_from(&output).is_err());

        let output = Output {
            status: lfs_ok,
            stdout: b"{\"child-pid\":123}\n{\"lfs-exit-code\":1}\n{\"exit-code\":1}\n".to_vec(),
            stderr: Vec::new(),
        };
        let result = ValidationResult::try_from(&output).unwrap();
        assert_eq!(result.child_pid, 123);
        assert_eq!(result.bubblewrap_exit_code, 1);
        assert_eq!(result.wine_exit_code, 1);
        assert_eq!(result.lfs, HlvcResult::Ok);

        let output = Output {
            status: exit_21,
            stdout: b"{\"child-pid\":123}\n{\"lfs-exit-code\":21}\n{\"exit-code\":21}\n".to_vec(),
            stderr: Vec::new(),
        };
        let result = ValidationResult::try_from(&output).unwrap();
        assert_eq!(result.child_pid, 123);
        assert_eq!(result.bubblewrap_exit_code, 21);
        assert_eq!(result.wine_exit_code, 21);
        assert_eq!(result.lfs, HlvcResult::FailOutOfBounds);

        let output = Output {
            status: exit_21,
            stdout: b"{\"child-pid\":123}\n{\"exit-code\":21}\n".to_vec(),
            stderr: Vec::new(),
        };
        let result = ValidationResult::from_output(&output, Some(b"21\r\n")).unwrap();
        assert_eq!(result.lfs, HlvcResult::FailOutOfBounds);
    }
}
