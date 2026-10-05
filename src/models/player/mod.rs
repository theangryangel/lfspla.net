//! Player persistence keyed by the stable LFS username.

use crate::country::CountryCode;
use crate::db::escaped_like::EscapedLike;

use lfsplanet_flags::FlagCode;

use sea_orm::entity::prelude::*;

use sea_orm::{
    Condition, QueryFilter, Select,
    sea_query::{Expr, Func, extension::postgres::PgExpr},
};

mod activity;
mod comparison;
mod profile;

pub(crate) use activity::PlayerChartResult;
pub(crate) use comparison::PlayerComparison;
pub(crate) use profile::PlayerProfile;

/// A Live for Speed player and their current profile.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player")]
pub struct Model {
    /// Database identity.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Stable LFS account name.
    #[sea_orm(unique)]
    pub lfs_username: String,
    /// Current display name returned by LFS.
    pub display_name: String,
    /// Whether every authentication path should reject this player.
    pub deny_auth: bool,
    /// Whether the hotlap upload route should reject this player.
    pub deny_uploads: bool,
    /// Nation selected for rankings, when present.
    ///
    /// Validated on read by [`CountryCode`], so no row loaded through this
    /// entity can carry a code that names no country.
    pub country_code: Option<CountryCode>,
    /// Optional FlagCDN display override; null follows the country.
    pub flag_code: Option<FlagCode>,
    /// Time at which this player was first created.
    pub created_at: TimeDateTimeWithTimeZone,
    /// Time at which LFS most recently authenticated this player.
    pub last_authenticated_at: Option<TimeDateTimeWithTimeZone>,
    /// Numeric identity from the old LFSWorld database, when imported.
    #[sea_orm(unique)]
    pub lfsworld_id: Option<i64>,
    #[sea_orm(has_many)]
    pub era_badges: HasMany<crate::models::badge::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
    #[sea_orm(has_many)]
    pub personal_access_tokens: HasMany<crate::models::personal_access_token::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Change authentication restrictions without overwriting other fields.
    pub(crate) async fn set_auth_denied(
        self,
        database: &impl ConnectionTrait,
        denied: bool,
    ) -> Result<Self, DbErr> {
        use sea_orm::{ActiveValue::Set, IntoActiveModel};
        let mut active = self.into_active_model();
        active.deny_auth = Set(denied);
        active.update(database).await
    }

    /// Change upload restrictions without overwriting other fields.
    pub(crate) async fn set_uploads_denied(
        self,
        database: &impl ConnectionTrait,
        denied: bool,
    ) -> Result<Self, DbErr> {
        use sea_orm::{ActiveValue::Set, IntoActiveModel};
        let mut active = self.into_active_model();
        active.deny_uploads = Set(denied);
        active.update(database).await
    }

    pub(crate) async fn find_by_username(
        database: &impl ConnectionTrait,
        username: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find().with_username(username).one(database).await
    }
    pub(crate) async fn record_authentication(
        database: &impl ConnectionTrait,
        account: &lfsplanet_lfs_api::LfsAccount,
    ) -> Result<Self, DbErr> {
        use sea_orm::ActiveValue::NotSet;
        use sea_orm::ActiveValue::Set;
        use sea_orm::sea_query::Expr;
        use sea_orm::sea_query::Func;
        use sea_orm::sea_query::OnConflict;
        // Keep the player's chosen country and original creation date on login.
        let player = Entity::insert(ActiveModel {
            id: NotSet,
            lfs_username: Set(account.username.clone()),
            display_name: Set(account.display_name.clone()),
            deny_auth: NotSet,
            deny_uploads: NotSet,
            country_code: NotSet,
            flag_code: NotSet,
            created_at: NotSet,
            last_authenticated_at: Set(Some(time::OffsetDateTime::now_utc())),
            lfsworld_id: Set(Some(account.lfsworld_id)),
        })
        .on_conflict(
            OnConflict::new()
                .expr(Func::lower(Expr::col(Column::LfsUsername)))
                .update_columns([
                    Column::DisplayName,
                    Column::LastAuthenticatedAt,
                    Column::LfsworldId,
                ])
                .to_owned(),
        )
        .exec_with_returning(database)
        .await?;

        Ok(player)
    }

    pub(crate) async fn update_preferences(
        self,
        database: &impl ConnectionTrait,
        country_code: Option<CountryCode>,
        flag_code: Option<FlagCode>,
    ) -> Result<Self, DbErr> {
        use sea_orm::ActiveModelTrait;
        use sea_orm::IntoActiveModel;
        use sea_orm::Set;
        let mut active = self.into_active_model();
        active.country_code = Set(country_code);
        active.flag_code = Set(flag_code);
        active.update(database).await
    }
}

/// Database filters for player queries.
pub(crate) trait PlayerFilter: QueryFilter + Sized {
    /// Matches the stable LFS account name without considering ASCII case.
    fn with_username(self, lfs_username: &str) -> Self {
        self.filter(sea_orm::sea_query::ExprTrait::eq(
            Expr::expr(Func::lower(Expr::col(Column::LfsUsername))),
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

        let pattern = EscapedLike::contains(query);
        self.filter(
            Condition::any()
                .add(Expr::col(Column::LfsUsername).ilike(pattern.clone()))
                .add(Expr::col(Column::DisplayName).ilike(pattern)),
        )
    }
}

impl PlayerFilter for Select<Entity> {}

#[cfg(test)]
mod filter_tests {
    use sea_orm::DbBackend;
    use sea_orm::EntityTrait;
    use sea_orm::QueryTrait;

    use super::*;

    fn sql(query: &str) -> String {
        Entity::find()
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
            Entity::find()
                .with_username("ExAmPlE")
                .build(DbBackend::Postgres)
        );
        assert!(
            query.find("LOWER(\"lfs_username\") = 'example'").is_some(),
            "{query}"
        );
    }
}
