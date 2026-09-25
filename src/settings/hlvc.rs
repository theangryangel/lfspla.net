//! Scheduling and timeout policy for replay validation.

use serde::{Deserialize, Serialize};

use super::types::Seconds;

/// How the queued HLVC worker schedules and bounds validation.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HlvcSettings {
    /// Delay between checks when the replay queue is empty.
    #[serde(rename = "poll_interval_seconds")]
    pub poll_interval: Seconds<2>,
    /// Hard deadline for a single LFS HLVC invocation.
    #[serde(rename = "timeout_seconds")]
    pub timeout: Seconds<300>,
}
