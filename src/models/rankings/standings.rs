//! A loaded ranking definition and the standings derived from it.

use sea_orm::{DatabaseConnection, EntityTrait};

use crate::models::{badges::PlayerBadge, eras::EraModel, players::PlayerModel};

use super::aggregate::{nation_contributions, personal_chart_bests};
use super::{
    NationContribution, NationRankingRow, PersonalChartBest, PersonalRankingRow, RankingEntity,
    RankingFilter, RankingRow, RankingRules, nations, personal,
};

const STANDINGS_SIZE: u64 = 100;

/// One era-scoped ranking together with its ordered chart selection.
pub(crate) struct LoadedRanking {
    pub(crate) definition: RankingRow,
    pub(crate) charts: Vec<crate::models::rankings::RankingChartModel>,
}

/// A personal ranking row with its presentation badges.
pub(crate) struct PersonalStanding {
    pub(crate) row: PersonalRankingRow,
    pub(crate) badges: Vec<PlayerBadge>,
}

impl LoadedRanking {
    /// Loads one complete ranking definition.
    pub(crate) async fn find(
        database: &DatabaseConnection,
        era_id: i64,
        ranking_slug: &str,
    ) -> Result<Option<Self>, sea_orm::DbErr> {
        Ok(super::with_charts(
            database,
            RankingEntity::find().in_era(era_id).with_slug(ranking_slug),
        )
        .await?
        .into_iter()
        .next()
        .map(|(definition, charts)| Self { definition, charts }))
    }

    pub(crate) const fn rules(&self) -> RankingRules {
        self.definition.rules()
    }

    pub(crate) async fn personal(
        &self,
        database: &DatabaseConnection,
        era: &EraModel,
    ) -> Result<Vec<PersonalStanding>, sea_orm::DbErr> {
        let rows = personal(
            database,
            self.rules(),
            self.definition.era_id,
            self.definition.id,
            STANDINGS_SIZE,
        )
        .await?;
        let mut badges = era
            .list_badges_for_players(database, rows.iter().map(|row| row.player_id))
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| PersonalStanding {
                badges: badges.remove(&row.player_id).unwrap_or_default(),
                row,
            })
            .collect())
    }

    pub(crate) async fn nations(
        &self,
        database: &DatabaseConnection,
    ) -> Result<Vec<NationRankingRow>, sea_orm::DbErr> {
        nations(
            database,
            self.rules(),
            self.definition.era_id,
            self.definition.id,
            STANDINGS_SIZE,
        )
        .await
    }

    /// Lists this player's best lap for each chart in this ranking.
    pub(crate) async fn list_personal_chart_bests(
        &self,
        database: &DatabaseConnection,
        player: &PlayerModel,
    ) -> Result<Vec<PersonalChartBest>, sea_orm::DbErr> {
        personal_chart_bests(
            database,
            self.definition.era_id,
            self.definition.id,
            player.id,
        )
        .await
    }

    /// Lists the players whose laps contribute to this country's score.
    pub(crate) async fn list_nation_contributions(
        &self,
        database: &DatabaseConnection,
        country: &str,
    ) -> Result<Vec<NationContribution>, sea_orm::DbErr> {
        nation_contributions(
            database,
            self.rules(),
            self.definition.era_id,
            self.definition.id,
            country,
        )
        .await
    }
}
