//! Badge calculation, rebuilding, and lookup for one era.

use crate::models::{
    Era,
    badge::{self, PlayerBadge, qualifies_for_ranking_badge},
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

type EraBadges = HashMap<i64, Vec<PlayerBadge>>;

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
        crate::db::lock_rebuild(&transaction).await?;
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
                        .push(PlayerBadge::Ranking {
                            ranking_id: ranking.slug.to_string(),
                            label: badge.label.clone(),
                            title: ranking.title.clone(),
                            ranking_position: row.position,
                        });
                }
                if let Some(completion) = &badge.completion
                    && qualifies_for_ranking_badge(
                        RankingBadgeQualification::Complete,
                        row.completed_charts,
                        charts.len(),
                        entry_index,
                    )
                {
                    badges_by_player.entry(row.player_id).or_default().push(
                        PlayerBadge::RankingCompletion {
                            ranking_id: ranking.slug.to_string(),
                            label: completion.label.clone(),
                            title: ranking.title.clone(),
                        },
                    );
                }
            }
        }

        let podium_counts = self.list_podium_counts(database).await?;
        for leader in rank_world_record_holders(&podium_counts) {
            if leader.position > WORLD_RECORD_LEADER_POSITIONS {
                break;
            }
            badges_by_player.entry(leader.player_id).or_default().push(
                PlayerBadge::WorldRecordLeader {
                    leader_position: i64::try_from(leader.position).unwrap_or(i64::MAX),
                    world_records: leader.world_records,
                },
            );
        }
        for podium in podium_counts {
            let level = if podium.firsts > 0 {
                badge::PodiumLevel::Gold
            } else if podium.seconds > 0 {
                badge::PodiumLevel::Silver
            } else {
                badge::PodiumLevel::Bronze
            };
            badges_by_player.entry(podium.player_id).or_default().push(
                PlayerBadge::WorldRecordPodium {
                    level,
                    firsts: podium.firsts,
                    seconds: podium.seconds,
                    thirds: podium.thirds,
                },
            );
        }
        for player_id in self.list_single_hotlap_player_ids(database).await? {
            badges_by_player
                .entry(player_id)
                .or_default()
                .push(PlayerBadge::Newbie);
        }
        Ok(badges_by_player)
    }
}
