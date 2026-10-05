//! Shared SQL binding and literal-pattern helpers.
use std::time::Duration;

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
