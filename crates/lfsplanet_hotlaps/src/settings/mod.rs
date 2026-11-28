//! Configuration owned by hotlap submission, validation, installations and webhooks.

mod hotlaps;
mod validation;
mod webhooks;

pub use hotlaps::HotlapSettings;
pub use validation::ValidationSettings;
pub use webhooks::WebhookSettings;
