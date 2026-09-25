//! Application models, business rules, and database operations, grouped by feature.
//!
//! HTTP and CLI concerns remain in their respective entrypoint modules.

use std::time::Duration;

pub(crate) mod badges;
pub(crate) mod country;
mod ordering;
pub use ordering::Ordering;
pub mod eras;
pub mod hotlaps;
pub(crate) mod personal_access_tokens;
pub(crate) mod player_flags;
pub(crate) mod players;
pub mod rankings;
pub(crate) mod stats;
pub(crate) mod track;
pub(crate) mod tracks;
pub(crate) mod vehicle;
pub(crate) mod vehicles;
pub(crate) mod version;

/// Escapes the wildcards a `LIKE`/`ILIKE` pattern would otherwise honour.
///
/// Free text typed by a caller must match literally, so `%`, `_` and the
/// escape character itself are neutralised before the pattern is built.
pub(crate) fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Converts a duration into the milliseconds a `BIGINT` column stores.
pub(crate) fn duration_millis(duration: Duration) -> Result<i64, sea_orm::DbErr> {
    i64::try_from(duration.as_millis())
        .map_err(|_| sea_orm::DbErr::Type("duration exceeds BIGINT milliseconds".to_owned()))
}

pub(crate) mod webhooks;
