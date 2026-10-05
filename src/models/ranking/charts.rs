//! Loads each ranking’s shared charts in membership order.
use crate::models::{
    Ranking,
    ranking::Entity as RankingEntity,
    ranking::RankingFilter,
    ranking_chart_membership::{Column as MembershipColumn, Entity as MembershipEntity},
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Select,
};
use std::collections::HashMap;
/// One era-scoped ranking together with its ordered chart selection.
pub(crate) struct RankingWithCharts {
    pub(crate) definition: Ranking,
    pub(crate) charts: Vec<crate::models::Chart>,
}

impl RankingWithCharts {
    /// Loads one complete ranking definition.
    pub(crate) async fn find(
        database: &DatabaseConnection,
        era_id: i64,
        ranking_slug: &str,
    ) -> Result<Option<Self>, sea_orm::DbErr> {
        Ok(Ranking::with_charts(
            database,
            RankingEntity::find().in_era(era_id).with_slug(ranking_slug),
        )
        .await?
        .into_iter()
        .next())
    }
}
impl Ranking {
    pub(crate) async fn with_charts<C: ConnectionTrait>(
        database: &C,
        select: Select<RankingEntity>,
    ) -> Result<Vec<super::RankingWithCharts>, DbErr> {
        let rankings = select.all(database).await?;
        if rankings.is_empty() {
            return Ok(Vec::new());
        }
        let mut by_ranking: HashMap<_, Vec<_>> = HashMap::new();
        for (membership, chart) in MembershipEntity::find()
            .filter(MembershipColumn::RankingId.is_in(rankings.iter().map(|ranking| ranking.id)))
            .order_by_asc(MembershipColumn::Position)
            .find_also_related(crate::models::chart::Entity)
            .all(database)
            .await?
        {
            by_ranking
                .entry(membership.ranking_id)
                .or_default()
                .push(chart.ok_or_else(|| DbErr::RecordNotFound("ranking chart missing".into()))?);
        }
        Ok(rankings
            .into_iter()
            .map(|ranking| {
                let charts = by_ranking.remove(&ranking.id).unwrap_or_default();
                super::RankingWithCharts {
                    definition: ranking,
                    charts,
                }
            })
            .collect())
    }
}
