//! Era-wide ranking queries.

use sea_orm::{ConnectionTrait, DbBackend, FromQueryResult, Statement};

use crate::models::rankings::PersonalRankingProgress;

use super::EraModel;

impl EraModel {
    /// Counts one player's completed charts across every ranking in this era.
    pub(crate) async fn list_ranking_progresses(
        &self,
        database: &impl ConnectionTrait,
        player_id: i64,
    ) -> Result<Vec<PersonalRankingProgress>, sea_orm::DbErr> {
        PersonalRankingProgress::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"
SELECT ranking_chart.ranking_id,
       COUNT(*)::BIGINT AS total_combinations,
       COUNT(hotlap_personal_best.hotlap_id)::BIGINT AS completed_combinations
FROM ranking_chart
LEFT JOIN hotlap_personal_best
  ON hotlap_personal_best.era_id = ranking_chart.era_id
 AND hotlap_personal_best.track = ranking_chart.track_id
 AND hotlap_personal_best.vehicle = ranking_chart.vehicle_id
 AND hotlap_personal_best.player_id = $2
WHERE ranking_chart.era_id = $1
GROUP BY ranking_chart.ranking_id
",
            [self.id.into(), player_id.into()],
        ))
        .all(database)
        .await
    }
}
