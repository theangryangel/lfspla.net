//! Worker settings for outgoing webhook notifications.

use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

/// Concurrency for webhook delivery.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WebhookSettings {
    /// Maximum concurrent deliveries; each destination remains serialized.
    pub workers: NonZeroUsize,
}

impl Default for WebhookSettings {
    fn default() -> Self {
        Self {
            workers: NonZeroUsize::new(3).expect("3 is non-zero"),
        }
    }
}
