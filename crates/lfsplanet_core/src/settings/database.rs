//! PostgreSQL connection settings.

use serde::{Deserialize, Serialize};

use super::types::{NonEmptyString, Seconds};

/// How every process reaches PostgreSQL.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSettings {
    /// PostgreSQL connection URL. Required, so this section is required.
    pub url: NonEmptyString,
    /// Maximum time allowed to connect to PostgreSQL.
    #[serde(default, rename = "connect_timeout_seconds")]
    pub connect_timeout: Seconds<5>,
    /// Maximum time allowed to acquire a pooled connection.
    #[serde(default, rename = "acquire_timeout_seconds")]
    pub acquire_timeout: Seconds<5>,
    /// Statement timeout applied to every pooled connection.
    #[serde(default, rename = "statement_timeout_seconds")]
    pub statement_timeout: Seconds<30>,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            url: NonEmptyString::new("postgresql://lfsplanet:lfsplanet@localhost:5432/lfsplanet"),
            connect_timeout: Seconds::default(),
            acquire_timeout: Seconds::default(),
            statement_timeout: Seconds::default(),
        }
    }
}
