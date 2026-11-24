//! Public era totals from a single database snapshot.
use super::Model;
use sea_orm::{ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement};

#[derive(Debug, FromQueryResult)]
pub(crate) struct EraStats {
    pub validated_hotlaps: i64,
    pub rankings: i64,
    pub charts: i64,
}

impl Model {
    pub(crate) async fn stats(&self, database: &impl ConnectionTrait) -> Result<EraStats, DbErr> {
        EraStats::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"SELECT
                (SELECT COUNT(*) FROM hotlap WHERE era_id = $1 AND state = 'valid') AS validated_hotlaps,
                (SELECT COUNT(*) FROM ranking WHERE era_id = $1) AS rankings,
                (SELECT COUNT(*) FROM chart
                 WHERE era_id = $1 AND EXISTS (
                    SELECT 1 FROM ranking_chart_membership membership
                    WHERE membership.era_id = chart.era_id AND membership.chart_id = chart.id
                 )) AS charts",
            [self.id.into()],
        ))
        .one(database)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("era totals missing".into()))
    }
}
