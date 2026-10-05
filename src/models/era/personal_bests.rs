//! Rebuilding and reranking the personal-best projection for one era.

use crate::models::Era;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement, Value};

impl Era {
    /// Rebuilds this era's personal bests after its chart catalogue or
    /// classification changes. The caller holds the exclusive catalogue lock.
    pub(crate) async fn rebuild_personal_bests<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<(), DbErr> {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM hotlap_personal_best WHERE chart_id IN (SELECT id FROM chart WHERE era_id = $1)",
                [Value::from(self.id)],
            ))
            .await?;
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id)
SELECT DISTINCT ON (hotlap.chart_id, hotlap.player_id)
    hotlap.chart_id, hotlap.player_id, hotlap.id
FROM hotlap
WHERE hotlap.era_id = $1
  AND hotlap.state = 'valid'
ORDER BY
    hotlap.chart_id, hotlap.player_id,
    hotlap.lap_time_ms, hotlap.created_at, hotlap.id
",
                [Value::from(self.id)],
            ))
            .await?;
        self.rerank_personal_bests(database).await?;
        crate::models::badge::request_badge_refresh(database, self.id).await
    }

    /// Re-ranks every chart in this era.
    /// Call inside a transaction holding the chart lock or exclusive catalogue lock.
    pub(crate) async fn rerank_personal_bests<C: ConnectionTrait>(
        &self,
        database: &C,
    ) -> Result<(), DbErr> {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                crate::models::chart::RERANK_SQL,
                [Value::from(self.id), Value::from(None::<i64>)],
            ))
            .await?;
        Ok(())
    }
}
