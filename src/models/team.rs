//! Racing teams and their recruitment policy.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub lfsworld_id: Option<i64>,
    pub name: String,
    pub tag: Option<String>,
    pub website_url: Option<String>,
    pub description: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub tags: Tags,
    pub accepts_applications: bool,
    pub status: Status,
    /// Unknown when the source export has a zero timestamp.
    pub lfsworld_updated_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    #[sea_orm(has_many)]
    pub members: HasMany<super::team_member::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Race,
    Drift,
    Drag,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
#[serde(transparent)]
pub struct Tags(pub Vec<Tag>);

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "disbanded")]
    Disbanded,
}

#[cfg(test)]
mod database_tests {
    use super::*;

    #[sqlx::test(migrations = false)]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn snapshot_preserves_profiles_and_imports_membership_roles(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let before = sqlx::migrate::Migrator::with_migrations(
            sqlx::migrate!()
                .iter()
                .filter(|migration| migration.version < 20261125120000)
                .cloned()
                .collect(),
        );
        before.run(&pool).await?;
        sqlx::query(
            "INSERT INTO player (lfs_username, display_name, deny_auth)
             VALUES ('orion', 'Existing display name', true)",
        )
        .execute(&pool)
        .await?;
        sqlx::migrate!().run(&pool).await?;
        // Re-running migrations must leave the imported snapshot alone.
        sqlx::migrate!().run(&pool).await?;

        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM team), (SELECT count(*) FROM player),
             (SELECT count(*) FROM team_member),
             (SELECT count(*) FROM team_member WHERE role = 'owner')",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(counts, (1388, 8053, 8686, 1388));
        let existing: (String, bool, String) = sqlx::query_as(
            "SELECT display_name, deny_auth, role FROM player
             JOIN team_member ON player_id = player.id
             JOIN team ON team.id = team_id
             WHERE lfs_username = 'orion' AND team.lfsworld_id = 1",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            existing,
            ("Existing display name".into(), true, "owner".into())
        );

        let missing_owner: String = sqlx::query_scalar(
            "SELECT role FROM team_member JOIN player ON player.id = player_id
             JOIN team ON team.id = team_id
             WHERE team.name = 'Norwegian smoke drifters' AND lfs_username = 'aleks1985'",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(missing_owner, "owner");
        let memberships: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM team_member JOIN player ON player.id = player_id
             WHERE lfs_username = 'Locovich'",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(memberships, 5);

        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let team = Entity::find()
            .filter(Column::LfsworldId.eq(1_i64))
            .one(&database)
            .await?
            .unwrap();
        assert_eq!(team.tag.as_deref(), Some("[ZT]"));
        assert_eq!(team.tags, Tags(vec![Tag::Race]));
        assert_eq!(
            team.description.as_deref(),
            Some("Do you feel the SPEEEEED?!")
        );
        assert!(team.accepts_applications);
        assert_eq!(team.status, Status::Active);
        assert_eq!(team.lfsworld_updated_at, None);
        assert!(
            sqlx::query("UPDATE team_member SET role = 'admin' WHERE team_id = $1")
                .bind(team.id)
                .execute(&pool)
                .await
                .is_err()
        );
        Ok(())
    }
}
