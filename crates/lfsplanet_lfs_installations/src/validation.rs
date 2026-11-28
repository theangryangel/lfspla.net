//! Output and application verdict from one validation attempt.

use crate::HlvcResult;
use std::process::Output;

/// Captured telemetry with its original output filename.
#[derive(Debug, PartialEq)]
pub struct RafFile {
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// Raw diagnostics remain available even when no complete verdict was produced.
pub struct ValidationDiagnostic {
    pub output: Output,
    /// Complete RAF output, when available. Capture failures do not affect the verdict.
    pub raf: Option<RafFile>,
    pub(crate) verdict: anyhow::Result<ValidationResult>,
}

impl ValidationDiagnostic {
    pub fn result(&self) -> anyhow::Result<ValidationResult> {
        self.verdict
            .as_ref()
            .copied()
            .map_err(|error| anyhow::anyhow!("{error:#}"))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ValidationResult {
    pub child_pid: u32,
    pub process_exit_code: i32,
    pub runtime_exit_code: i32,
    pub lfs: HlvcResult,
}
