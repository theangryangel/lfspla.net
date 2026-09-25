//! Named predicates over the `player` table.

use sea_orm::{
    Condition, QueryFilter, Select,
    sea_query::{Expr, Func, extension::postgres::PgExpr},
};

use super::{PlayerColumn, PlayerEntity};

/// Database filters for player queries.
pub(crate) trait PlayerFilter: QueryFilter + Sized {
    /// Matches the stable LFS account name without considering ASCII case.
    fn with_username(self, lfs_username: &str) -> Self {
        self.filter(sea_orm::sea_query::ExprTrait::eq(
            Expr::expr(Func::lower(Expr::col(PlayerColumn::LfsUsername))),
            lfs_username.to_ascii_lowercase(),
        ))
    }

    /// Searches usernames and display names. Blank searches match nothing.
    /// Search text is escaped so wildcards match literally.
    fn matching(self, query: &str) -> Self {
        let query = query.trim();
        if query.is_empty() {
            return self.filter(Expr::value(false));
        }

        let pattern = format!("%{}%", crate::models::escape_like(query));
        self.filter(
            Condition::any()
                .add(Expr::col(PlayerColumn::LfsUsername).ilike(pattern.clone()))
                .add(Expr::col(PlayerColumn::DisplayName).ilike(pattern)),
        )
    }
}

impl PlayerFilter for Select<PlayerEntity> {}

#[cfg(test)]
mod tests {
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};

    use super::*;

    fn sql(query: &str) -> String {
        PlayerEntity::find()
            .matching(query)
            .build(DbBackend::Postgres)
            .to_string()
    }

    #[test]
    fn an_empty_finder_query_matches_nothing_rather_than_everything() {
        for empty in ["", "   ", "\t\n"] {
            let sql = sql(empty);
            assert!(
                sql.find("FALSE").is_some(),
                "an empty finder query must not become an unbounded listing: {sql}"
            );
            assert!(sql.find("ILIKE").is_none(), "{sql}");
        }
    }

    #[test]
    fn finder_text_cannot_smuggle_in_its_own_wildcards() {
        let sql = sql("100%_x");
        // Rendered as a Postgres E-string, so each escaping backslash is
        // itself doubled: the pattern Postgres receives is `%100\%\_x%`.
        assert!(sql.find(r"E'%100\\%\\_x%'").is_some(), "{sql}");
    }

    #[test]
    fn a_real_query_matches_both_the_username_and_the_display_name() {
        let sql = sql("driver");
        assert!(sql.find(r#""lfs_username" ILIKE"#).is_some(), "{sql}");
        assert!(sql.find(r#""display_name" ILIKE"#).is_some(), "{sql}");
    }

    #[test]
    fn username_lookup_ignores_case() {
        let query = format!(
            "{}",
            PlayerEntity::find()
                .with_username("ExAmPlE")
                .build(DbBackend::Postgres)
        );
        assert!(
            query.find("LOWER(\"lfs_username\") = 'example'").is_some(),
            "{query}"
        );
    }
}
