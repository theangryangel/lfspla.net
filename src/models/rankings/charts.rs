//! Loads ordered charts owned directly by rankings.
use super::{RankingChartColumn, RankingChartEntity, RankingChartOrder, RankingEntity, RankingRow};
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Select};
use std::collections::HashMap;

pub(crate) async fn with_charts<C: ConnectionTrait>(
    database: &C,
    select: Select<RankingEntity>,
) -> Result<Vec<(RankingRow, Vec<super::RankingChartModel>)>, DbErr> {
    let rankings = select.all(database).await?;
    if rankings.is_empty() {
        return Ok(Vec::new());
    }
    let mut selected = Condition::any();
    for ranking in &rankings {
        selected = selected.add(
            Condition::all()
                .add(RankingChartColumn::EraId.eq(ranking.era_id))
                .add(RankingChartColumn::RankingId.eq(ranking.id)),
        );
    }
    let mut by_ranking: HashMap<_, Vec<_>> = HashMap::new();
    for chart in RankingChartEntity::find()
        .filter(selected)
        .by_position()
        .all(database)
        .await?
    {
        by_ranking
            .entry((chart.era_id, chart.ranking_id))
            .or_default()
            .push(chart);
    }
    Ok(rankings
        .into_iter()
        .map(|ranking| {
            let charts = by_ranking
                .remove(&(ranking.era_id, ranking.id))
                .unwrap_or_default();
            (ranking, charts)
        })
        .collect())
}
