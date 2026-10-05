//! Output and application verdict from one validation attempt.

use crate::HlvcResult;
use std::process::Output;

/// Raw diagnostics remain available even when no complete verdict was produced.
pub struct ValidationDiagnostic {
    pub output: Output,
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
