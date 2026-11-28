//! Validation queries over era replay-version coverage.

use crate::models::era::{Column as EraColumn, Entity as EraEntity};
use anyhow::ensure;
use sea_orm::{ConnectionTrait, EntityTrait, QueryOrder};

/// Requires at least one era and rejects overlapping version ranges.
impl crate::models::Era {
    pub async fn validate_version_requirements<C>(database: &C) -> anyhow::Result<()>
    where
        C: ConnectionTrait,
    {
        let eras = EraEntity::find()
            .order_by_asc(EraColumn::Id)
            .all(database)
            .await?;
        ensure!(!eras.is_empty(), "at least one era must be configured");
        for (index, left) in eras.iter().enumerate() {
            for right in &eras[index + 1..] {
                ensure!(
                    !left
                        .version_requirement
                        .intersects(&right.version_requirement),
                    "version requirements overlap for eras {} and {}",
                    left.id,
                    right.id
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::models::Era;

    #[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn version_requirements_must_exist_and_not_overlap(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let insert = async |slug: &str, requirement: &str| {
            sqlx::query(
                "INSERT INTO era (slug, title, installation_id, version_requirement, open) \
                 VALUES ($1, $1, '0.7', $2, false)",
            )
            .bind(slug)
            .bind(requirement)
            .execute(&pool)
            .await
        };
        sqlx::query("DELETE FROM era").execute(&pool).await?;
        assert!(Era::validate_version_requirements(&database).await.is_err());
        insert("early", ">=0.5Y,<0.6A").await?;
        insert("current", ">=0.7A,<0.8A").await?;
        Era::validate_version_requirements(&database).await?;
        insert("overlapping", ">=0.7F,<0.9A").await?;
        let error = Era::validate_version_requirements(&database)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("overlap"), "{error}");
        Ok(())
    }
}
