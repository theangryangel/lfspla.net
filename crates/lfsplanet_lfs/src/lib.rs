//! High-level operations on local Live for Speed installations.

mod result;
mod trim;
mod validation;

#[cfg(target_os = "linux")]
#[path = "platform/linux/mod.rs"]
mod platform;
#[cfg(not(target_os = "linux"))]
#[path = "platform/unsupported.rs"]
mod platform;

pub use platform::{RuntimeConfig, ensure_supported, install, update, validate};
pub use result::HlvcResult;
pub use trim::{TrimPlan, TrimSummary, trim};
pub use validation::{ValidationDiagnostic, ValidationResult};

// Compile and exercise the unsupported implementation on Linux too.
#[cfg(all(test, target_os = "linux"))]
#[path = "platform/unsupported.rs"]
mod unsupported;
