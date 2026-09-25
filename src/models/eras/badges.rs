//! Badge calculation and lookup scoped to one ranking era.

use std::collections::HashMap;

use anyhow::Context;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait,
};

use crate::models::{
    badges::{self, BadgeFilter, PlayerBadge, PlayerEraBadgesEntity},
    rankings::{self, RankingBadgeDefinition, RankingBadgeQualification, RankingFilter},
};

use super::{EraEntity, EraModel, rank_world_record_holders};

/// How many leading world record positions are awarded a badge.
///
/// Positions are a competition rank, so equal record counts share a position
/// and a wide tie can award more than three drivers.
const WORLD_RECORD_LEADER_POSITIONS: u64 = 3;

type EraBadges = HashMap<i64, Vec<PlayerBadge>>;

/// Persist alongside the publication change, so a crash cannot lose its refresh.
/// Callers hold the catalogue admission or rebuild lock until commit.
pub(crate) async fn request_badge_refresh(
    database: &impl ConnectionTrait,
    era_id: i64,
) -> Result<(), sea_orm::DbErr> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO era_badge_refresh (era_id) VALUES ($1) ON CONFLICT DO NOTHING",
            [era_id.into()],
        ))
        .await?;
    Ok(())
}

/// Maintenance retries every dirty era, including work interrupted after commit.
pub(crate) async fn retry_badge_refreshes(
    database: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    let pending = database
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT era_id FROM era_badge_refresh ORDER BY era_id",
        ))
        .await?;
    let mut failure = None;
    for row in pending {
        let era_id: i64 = row.try_get("", "era_id")?;
        if let Some(era) = EraEntity::find_by_id(era_id).one(database).await?
            && let Err(error) = era.rebuild_badges(database).await
        {
            tracing::error!(?error, era_id, "badge refresh remains pending");
            failure = Some(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

/// Rebuilds an era's badges after a published hotlap changes its results.
/// Errors leave the transactional refresh request for maintenance to retry.
pub(crate) async fn rebuild_published_badges(database: &DatabaseConnection, era_id: i64) {
    let rebuilt = async {
        let era = EraEntity::find_by_id(era_id)
            .one(database)
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound("era".to_owned()))?;
        era.rebuild_badges(database)
            .await
            .context("failed to rebuild player badges")
    }
    .await;

    match rebuilt {
        Ok(players) => tracing::info!(era_id, players, "player badges rebuilt"),
        Err(error) => tracing::error!(?error, era_id, "failed to rebuild player badges"),
    }
}

impl EraModel {
    /// Recalculates this era's badges, then stores them in one transaction.
    pub(crate) async fn rebuild_badges(
        &self,
        database: &DatabaseConnection,
    ) -> Result<usize, sea_orm::DbErr> {
        let transaction = database.begin().await?;
        // Acquire before reading: publication and catalogue edits use this same
        // lock. It also serializes rebuilds, without taking a stale snapshot
        // before waiting for another rebuild to finish.
        super::lock_rebuild(&transaction).await?;
        let calculated = self.calculate_badges(&transaction).await?;
        let count = badges::replace_era(&transaction, self.id, calculated).await?;
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
        let player_ids = player_ids.into_iter().collect::<Vec<_>>();
        if player_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = PlayerEraBadgesEntity::find()
            .in_era(self.id)
            .owned_by_any(player_ids)
            .all(database)
            .await?;

        rows.into_iter()
            .map(|row| Ok((row.player_id, badges::decode(row)?)))
            .collect()
    }

    #[allow(clippy::too_many_lines)]
    async fn calculate_badges(
        &self,
        database: &impl ConnectionTrait,
    ) -> Result<EraBadges, sea_orm::DbErr> {
        let mut badges_by_player = EraBadges::new();
        let rankings =
            rankings::with_charts(database, rankings::RankingEntity::find().in_era(self.id))
                .await?;
        for (ranking, charts) in rankings {
            let rules = ranking.rules();
            let Some(badge) = ranking
                .badge
                .map(serde_json::from_value::<RankingBadgeDefinition>)
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
            let rows =
                rankings::personal(database, rules, self.id, ranking.id, badge.result_limit())
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
                            ranking_id: ranking.slug.clone(),
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
                            ranking_id: ranking.slug.clone(),
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
                badges::PodiumLevel::Gold
            } else if podium.seconds > 0 {
                badges::PodiumLevel::Silver
            } else {
                badges::PodiumLevel::Bronze
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

pub(crate) fn qualifies_for_ranking_badge(
    qualification: RankingBadgeQualification,
    completed_charts: i64,
    total_charts: usize,
    entry_index: usize,
) -> bool {
    match qualification {
        RankingBadgeQualification::Ranked { .. } => {
            (entry_index as u64) < qualification.result_limit()
        }
        RankingBadgeQualification::Complete => {
            usize::try_from(completed_charts).is_ok_and(|completed| completed == total_charts)
        }
    }
}
