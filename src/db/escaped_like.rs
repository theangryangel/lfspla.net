//! Literal search patterns for PostgreSQL LIKE and ILIKE expressions.

use sea_orm::sea_query::LikeExpr;

/// A pattern that escapes caller-supplied wildcards and backslashes.
#[derive(Clone, Debug)]
pub(crate) struct EscapedLike(String);

impl EscapedLike {
    /// Matches the supplied text anywhere within a value, literally.
    pub(crate) fn contains(value: &str) -> Self {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        Self(format!("%{escaped}%"))
    }
}

impl From<EscapedLike> for LikeExpr {
    fn from(value: EscapedLike) -> Self {
        // PostgreSQL uses backslash as the default LIKE escape character.
        Self::new(value.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::{
        Expr, PostgresQueryBuilder, Query, Value, extension::postgres::PgExpr,
    };

    #[test]
    fn literal_search_patterns_bind_escaped_text() {
        for (input, expected) in [
            ("driver", "%driver%"),
            (r"driver_100%\name", r"%driver\_100\%\\name%"),
            ("", "%%"),
            ("café", "%café%"),
        ] {
            let (sql, values) = Query::select()
                .column("name")
                .from("player")
                .and_where(Expr::col("name").ilike(EscapedLike::contains(input)))
                .build(PostgresQueryBuilder);
            assert_eq!(
                sql,
                "SELECT \"name\" FROM \"player\" WHERE \"name\" ILIKE $1"
            );
            assert_eq!(values.0, vec![Value::from(expected)]);
        }
    }
}
