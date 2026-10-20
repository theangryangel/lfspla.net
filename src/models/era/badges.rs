//! Badge calculation, rebuilding, and lookup for one era.

use crate::models::{
    Era,
    badge::{self, CalculatedAward, PlayerBadge, qualifies_for_ranking_badge},
    era::rank_world_record_holders,
    ranking::{self, RankingBadgeDefinition, RankingBadgeQualification, RankingFilter},
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait,
};
use std::collections::HashMap;

/// How many leading world record positions are awarded a badge.
///
/// Positions are a competition rank, so equal record counts share a position
/// and a wide tie can award more than three drivers.
const WORLD_RECORD_LEADER_POSITIONS: u64 = 3;

type EraBadges = HashMap<i64, Vec<CalculatedAward>>;

impl Era {
    /// Recalculates this era's badges, then stores them in one transaction.
    pub(crate) async fn rebuild_badges(
        &self,
        database: &DatabaseConnection,
    ) -> Result<usize, sea_orm::DbErr> {
        let transaction = database.begin().await?;
        // Acquire before reading: publication and catalogue edits use this same
        // lock. It also serializes rebuilds, without taking a stale snapshot
        // before waiting for another rebuild to finish.
        crate::db::lock_catalogue_write(&transaction).await?;
        let calculated = self.calculate_badges(&transaction).await?;
        let count =
            crate::models::badge::Model::replace_era(&transaction, self.id, calculated).await?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM era_badge_refresh WHERE era_id = $1",
                [self.id.into()],
            ))
            .await?;
        transaction.commit().await?;
        Ok(count)
    }

    /// Returns stored badge vectors for requested players in this era.
    pub(crate) async fn list_badges_for_players(
        &self,
        database: &DatabaseConnection,
        player_ids: impl IntoIterator<Item = i64>,
    ) -> Result<HashMap<i64, Vec<PlayerBadge>>, sea_orm::DbErr> {
        badge::Model::list_for_players(database, self.id, player_ids).await
    }

    #[allow(clippy::too_many_lines)]
    async fn calculate_badges(
        &self,
        database: &impl ConnectionTrait,
    ) -> Result<EraBadges, sea_orm::DbErr> {
        let mut badges_by_player = EraBadges::new();
        let rankings =
            crate::models::Ranking::with_charts(database, ranking::Entity::find().in_era(self.id))
                .await?;
        for loaded in rankings {
            let ranking = loaded.definition;
            let charts = loaded.charts;
            let Some(badge) = ranking
                .badge
                .as_ref()
                .map(|value| serde_json::from_value::<RankingBadgeDefinition>(value.clone()))
                .transpose()
                .map_err(|error| {
                    sea_orm::DbErr::Type(format!(
                        "invalid badge configuration for ranking {}: {error}",
                        ranking.id
                    ))
                })?
            else {
                continue;
            };
            if charts.is_empty() {
                return Err(sea_orm::DbErr::Type(format!(
                    "ranking {} has no charts",
                    ranking.id
                )));
            }
            let rows = ranking
                .personal_scores(database, badge.result_limit())
                .await?;
            for (entry_index, row) in rows.into_iter().enumerate() {
                if qualifies_for_ranking_badge(
                    badge.qualification,
                    row.completed_charts,
                    charts.len(),
                    entry_index,
                ) {
                    badges_by_player
                        .entry(row.player_id)
                        .or_default()
                        .push(CalculatedAward::new(
                            &badge.id,
                            position_variant(row.position),
                            format!("ranking:{}:primary", ranking.id),
                            format!("{} - #{}", ranking.title, row.position),
                        ));
                }
                if let Some(completion) = &badge.completion
                    && qualifies_for_ranking_badge(
                        RankingBadgeQualification::Complete,
                        row.completed_charts,
                        charts.len(),
                        entry_index,
                    )
                {
                    badges_by_player
                        .entry(row.player_id)
                        .or_default()
                        .push(CalculatedAward::new(
                            &completion.id,
                            "standard",
                            format!("ranking:{}:completion", ranking.id),
                            format!("{} 100% complete", ranking.title),
                        ));
                }
            }
        }

        let podium_counts = self.list_podium_counts(database).await?;
        for leader in rank_world_record_holders(&podium_counts) {
            if leader.position > WORLD_RECORD_LEADER_POSITIONS {
                break;
            }
            badges_by_player
                .entry(leader.player_id)
                .or_default()
                .push(CalculatedAward::new(
                    "world-record-leader",
                    position_variant(i64::try_from(leader.position).unwrap_or(i64::MAX)),
                    "world-record-leader",
                    format!(
                        "#{} with {} world record{}",
                        leader.position,
                        leader.world_records,
                        if leader.world_records == 1 { "" } else { "s" }
                    ),
                ));
        }
        for podium in podium_counts {
            let level = if podium.firsts > 0 {
                "gold"
            } else if podium.seconds > 0 {
                "silver"
            } else {
                "bronze"
            };
            badges_by_player
                .entry(podium.player_id)
                .or_default()
                .push(CalculatedAward::new(
                    "world-record-podium",
                    level,
                    "world-record-podium",
                    format!(
                        "{} first, {} second, {} third places",
                        podium.firsts, podium.seconds, podium.thirds
                    ),
                ));
        }
        for player_id in self.list_single_hotlap_player_ids(database).await? {
            badges_by_player
                .entry(player_id)
                .or_default()
                .push(CalculatedAward::new("newbie", "standard", "newbie", None));
        }
        Ok(badges_by_player)
    }
}

/// Presentation variant selection is application policy, independent of catalogue artwork.
fn position_variant(position: i64) -> &'static str {
    match position {
        1 => "gold",
        2 => "silver",
        3 => "bronze",
        _ => "standard",
    }
}
