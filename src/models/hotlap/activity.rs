//! Hotlap activity pages, with chart positions and ranking contributions.
use crate::models::{
    Era, Hotlap, Ordering, Player,
    era::Entity as EraEntity,
    hotlap::{Column as HotlapColumn, Entity as HotlapEntity, HotlapListColumn, HotlapOrder},
    player::Entity as PlayerEntity,
};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    IsolationLevel, PaginatorTrait, QueryFilter, QuerySelect, Select, Statement, TransactionTrait,
};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub(crate) struct RankingContribution {
    pub(crate) id: String,
    pub(crate) title: String,
}
pub(crate) struct HotlapActivityEntry {
    pub(crate) hotlap: Hotlap,
    pub(crate) player: Player,
    pub(crate) era: Era,
    pub(crate) position: Option<i64>,
    pub(crate) distance_to_world_record_ms: Option<i64>,
    pub(crate) contributes_to: Vec<RankingContribution>,
}
pub(crate) struct HotlapActivity {
    pub(crate) entries: Vec<HotlapActivityEntry>,
    pub(crate) total: u64,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum ActivityError {
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error("hotlap owner could not be loaded")]
    MissingPlayer,
    #[error("hotlap era could not be loaded")]
    MissingEra,
}
impl HotlapActivity {
    pub(crate) async fn load(
        database: &DatabaseConnection,
        hotlaps: Select<HotlapEntity>,
        column: HotlapListColumn,
        order: Ordering,
        offset: u64,
        limit: u64,
    ) -> Result<Self, ActivityError> {
        let transaction = database
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let total = hotlaps.clone().count(&transaction).await?;
        // Keep the sort narrow: full replay/profile fields are loaded only for the page.
        let ids = hotlaps
            .ordered(column, order)
            .select_only()
            .column(HotlapColumn::Id)
            .offset(offset)
            .limit(limit)
            .into_tuple::<i64>()
            .all(&transaction)
            .await?;
        let rows = HotlapEntity::find()
            .filter(HotlapColumn::Id.is_in(ids))
            .ordered(column, order)
            .find_also_related(PlayerEntity)
            // `HotlapEntity` declares `era_id` as an Era relation, so load the
            // public slug with the page rather than leaking the internal key.
            .find_also_related(EraEntity)
            .all(&transaction)
            .await?;
        let mut chart_data = HashMap::<i64, (i64, i64)>::new();
        let mut ranking_contributions = HashMap::<i64, Vec<RankingContribution>>::new();
        if !rows.is_empty() {
            let placeholders = (1..=rows.len())
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>()
                .join(", ");
            let ranked = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    r"SELECT current.hotlap_id,
                             current.position,
                             current_lap.lap_time_ms - record_lap.lap_time_ms AS distance_to_world_record_ms
                      FROM hotlap_personal_best current
                      JOIN hotlap current_lap ON current_lap.id = current.hotlap_id
                      JOIN hotlap_personal_best record
                        ON record.era_id = current.era_id
                       AND record.track = current.track
                       AND record.vehicle = current.vehicle
                       AND record.position = 1
                      JOIN hotlap record_lap ON record_lap.id = record.hotlap_id
                      WHERE current.hotlap_id IN ({placeholders})"
                ),
                rows.iter().map(|(hotlap, _, _)| hotlap.id.into()),
            ))
            .await?;
            for row in ranked {
                chart_data.insert(
                    row.try_get("", "hotlap_id")?,
                    (
                        row.try_get("", "position")?,
                        row.try_get("", "distance_to_world_record_ms")?,
                    ),
                );
            }
            let ranking_rows = transaction
                .query_all_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    format!(
                        r"SELECT current.hotlap_id, ranking.slug, ranking.title
                       FROM hotlap_personal_best current
                       JOIN ranking_chart
                         ON ranking_chart.era_id = current.era_id
                        AND ranking_chart.track_id = current.track
                        AND ranking_chart.vehicle_id = current.vehicle
                       JOIN ranking
                         ON ranking.id = ranking_chart.ranking_id
                        AND ranking.era_id = ranking_chart.era_id
                       WHERE current.hotlap_id IN ({placeholders})
                       ORDER BY ranking.position, ranking.id"
                    ),
                    rows.iter().map(|(hotlap, _, _)| hotlap.id.into()),
                ))
                .await?;
            for row in ranking_rows {
                ranking_contributions
                    .entry(row.try_get("", "hotlap_id")?)
                    .or_default()
                    .push(RankingContribution {
                        id: row.try_get("", "slug")?,
                        title: row.try_get("", "title")?,
                    });
            }
        }
        transaction.commit().await?;
        let entries = rows
            .into_iter()
            .map(|(hotlap, player, era)| {
                let player = player.ok_or(ActivityError::MissingPlayer)?;
                let era = era.ok_or(ActivityError::MissingEra)?;
                let chart = chart_data.get(&hotlap.id).copied();
                let contributes_to = ranking_contributions.remove(&hotlap.id).unwrap_or_default();
                Ok(HotlapActivityEntry {
                    hotlap,
                    player,
                    era,
                    position: chart.map(|c| c.0),
                    distance_to_world_record_ms: chart.map(|c| c.1),
                    contributes_to,
                })
            })
            .collect::<Result<Vec<_>, ActivityError>>()?;
        Ok(Self { entries, total })
    }
}

use crate::models::{
    hotlap::HotlapFilter,
    player::{Column as PlayerColumn, PlayerFilter},
};
use sea_orm::QueryTrait;

/// Resolved activity filters. The adapter determines the permitted owner/state scope.
pub(crate) struct HotlapActivityFilter<'a> {
    pub(crate) state: Option<super::HotlapState>,
    pub(crate) unpublished: bool,
    pub(crate) owner: Option<i64>,
    pub(crate) lfs_username: Option<&'a str>,
    pub(crate) track: Option<insim_core::track::Track>,
    pub(crate) vehicle: Option<insim_core::vehicle::Vehicle>,
    pub(crate) ranked_only: bool,
    pub(crate) rank: Option<i64>,
}
impl HotlapActivityFilter<'_> {
    pub(crate) fn select(&self) -> Select<HotlapEntity> {
        let mut hotlaps = HotlapEntity::find();
        if let Some(hotlap_state) = self.state {
            hotlaps = hotlaps.in_state(hotlap_state);
        } else if self.unpublished {
            hotlaps =
                hotlaps.filter(crate::models::hotlap::Column::State.ne(super::HotlapState::Valid));
        }
        if let Some(owner) = self.owner {
            hotlaps = hotlaps.uploads().owned_by(owner);
        }
        if let Some(username) = self.lfs_username {
            hotlaps = hotlaps.filter(
                HotlapColumn::PlayerId.in_subquery(
                    PlayerEntity::find()
                        .with_username(username)
                        .select_only()
                        .column(PlayerColumn::Id)
                        .into_query(),
                ),
            );
        }
        if let Some(track) = self.track {
            hotlaps = hotlaps.filter(HotlapColumn::Track.eq(track.to_string()));
        }
        if let Some(vehicle) = self.vehicle {
            hotlaps = hotlaps.filter(HotlapColumn::Vehicle.eq(vehicle.to_string()));
        }
        if self.ranked_only {
            hotlaps = hotlaps.filter(sea_orm::sea_query::Expr::cust(
                "EXISTS (SELECT 1 FROM hotlap_personal_best WHERE hotlap_id = hotlap.id)",
            ));
        }
        if let Some(rank) = self.rank {
            hotlaps = hotlaps.filter(sea_orm::sea_query::Expr::cust(format!(
                "EXISTS (SELECT 1 FROM hotlap_personal_best WHERE hotlap_id = hotlap.id AND position <= {rank})"
            )));
        }
        hotlaps
    }
}
