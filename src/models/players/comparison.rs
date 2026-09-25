use sea_orm::DatabaseConnection;

use crate::models::players::PlayerChartResult;

use super::PlayerModel;

#[derive(Debug)]
pub(crate) struct ComparedPlayer {
    pub(crate) player: PlayerModel,
    pub(crate) results: Vec<PlayerChartResult>,
}

#[derive(Debug)]
pub(crate) struct PlayerComparison {
    pub(crate) left: ComparedPlayer,
    pub(crate) right: ComparedPlayer,
}

impl PlayerModel {
    pub(crate) async fn compare_with(
        self,
        database: &DatabaseConnection,
        other: Self,
        era_id: i64,
        track: Option<&str>,
        vehicle: Option<&str>,
    ) -> Result<PlayerComparison, sea_orm::DbErr> {
        let (left_results, right_results) = tokio::try_join!(
            self.list_chart_results(database, Some(era_id), track, vehicle),
            other.list_chart_results(database, Some(era_id), track, vehicle),
        )?;

        Ok(PlayerComparison {
            left: ComparedPlayer {
                player: self,
                results: left_results,
            },
            right: ComparedPlayer {
                player: other,
                results: right_results,
            },
        })
    }
}
