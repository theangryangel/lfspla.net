//! Shared filtered hotlap pages, with chart positions and ranking context.

use crate::milliseconds::Milliseconds;
use crate::models::{
    Chart, Era, Hotlap, Player,
    badge::PlayerBadge,
    era::Entity as EraEntity,
    hotlap::{Column as HotlapColumn, Entity as HotlapEntity, HotlapListColumn},
    player::Entity as PlayerEntity,
};
use crate::ordering::Ordering;
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    ExprTrait, IsolationLevel, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select,
    Statement, TransactionTrait,
    sea_query::{Alias, Expr, JoinType, NullOrdering},
};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub(crate) struct RankingContribution {
    pub(crate) id: String,
    pub(crate) title: String,
}
pub(crate) struct HotlapListEntry {
    pub(crate) hotlap: Hotlap,
    pub(crate) player: Player,
    pub(crate) era: Era,
    pub(crate) position: Option<i64>,
    pub(crate) distance_to_world_record_ms: Option<Milliseconds>,
    pub(crate) distance_to_benchmark_ms: Option<Milliseconds>,
    pub(crate) badges: Option<Vec<PlayerBadge>>,
    pub(crate) contributes_to: Vec<RankingContribution>,
}
pub(crate) struct HotlapPage {
    pub(crate) entries: Vec<HotlapListEntry>,
    pub(crate) total: u64,
    pub(crate) chart_contributions: Option<Vec<RankingContribution>>,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum HotlapListError {
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error("hotlap owner could not be loaded")]
    MissingPlayer,
    #[error("hotlap era could not be loaded")]
    MissingEra,
}
/// Ordering and pagination, resolved by the caller.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HotlapListPage {
    pub(crate) column: HotlapListColumn,
    pub(crate) order: Ordering,
    pub(crate) offset: u64,
    pub(crate) limit: u64,
}

impl Hotlap {
    pub(crate) async fn list(
        database: &DatabaseConnection,
        filters: HotlapListFilters<'_>,
        page: HotlapListPage,
    ) -> Result<HotlapPage, HotlapListError> {
        let hotlaps = filters.select();
        let transaction = database
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let total = hotlaps.clone().count(&transaction).await?;
        // Keep the sort narrow: full replay/profile fields are loaded only for the page.
        let ids = ordered(hotlaps, page.column, page.order)
            .select_only()
            .column(HotlapColumn::Id)
            .offset(page.offset)
            .limit(page.limit)
            .into_tuple::<i64>()
            .all(&transaction)
            .await?;
        let rows = HotlapEntity::find()
            .filter(HotlapColumn::Id.is_in(ids.iter().copied()))
            .find_also_related(PlayerEntity)
            // `HotlapEntity` declares `era_id` as an Era relation, so load the
            // public slug with the page rather than leaking the internal key.
            .find_also_related(EraEntity)
            .all(&transaction)
            .await?;
        let mut chart_data = HashMap::<i64, (i64, Milliseconds, Option<Milliseconds>)>::new();
        let mut ranking_contributions = HashMap::<i64, Vec<RankingContribution>>::new();
        let chart_contributions = match filters.chart {
            Some(chart) => Some(
                chart
                    .rankings()
                    .all(&transaction)
                    .await?
                    .into_iter()
                    .map(|ranking| RankingContribution {
                        id: ranking.slug.to_string(),
                        title: ranking.title,
                    })
                    .collect::<Vec<_>>(),
            ),
            None => None,
        };
        let mut badges = match filters.chart {
            Some(chart) => Some(
                crate::models::badge::Model::list_for_players(
                    &transaction,
                    chart.era_id,
                    rows.iter().map(|(hotlap, _, _)| hotlap.player_id),
                )
                .await?,
            ),
            None => None,
        };
        let benchmark_gap = if filters.chart.is_some() {
            format!(
                "current_lap.lap_time_ms - {}",
                crate::models::chart::benchmark_sql("record_lap.lap_time_ms", 103)
            )
        } else {
            "NULL::BIGINT".to_owned()
        };

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
                             {benchmark_gap} AS distance_to_benchmark_ms,
                             current_lap.lap_time_ms - record_lap.lap_time_ms AS distance_to_world_record_ms
                      FROM hotlap_personal_best current
                      JOIN hotlap current_lap ON current_lap.id = current.hotlap_id
                      JOIN hotlap_personal_best record
                        ON record.chart_id = current.chart_id
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
                        row.try_get("", "distance_to_benchmark_ms")?,
                    ),
                );
            }
            if chart_contributions.is_none() {
                let ranking_rows = transaction
                    .query_all_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        format!(
                            r"SELECT current.hotlap_id, ranking.slug, ranking.title
                       FROM hotlap_personal_best current
                       JOIN ranking_chart_membership
                         ON ranking_chart_membership.chart_id = current.chart_id
                       JOIN ranking
                         ON ranking.id = ranking_chart_membership.ranking_id
                        AND ranking.era_id = ranking_chart_membership.era_id
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
        }
        transaction.commit().await?;
        let mut rows_by_id: HashMap<_, _> = rows.into_iter().map(|row| (row.0.id, row)).collect();
        let entries = ids
            .into_iter()
            .map(|id| {
                let (hotlap, player, era) = rows_by_id.remove(&id).ok_or_else(|| {
                    sea_orm::DbErr::RecordNotFound(format!("listed hotlap {id} missing"))
                })?;
                let player = player.ok_or(HotlapListError::MissingPlayer)?;
                let era = era.ok_or(HotlapListError::MissingEra)?;
                let chart = chart_data.get(&hotlap.id).copied();
                let contributes_to = chart_contributions.as_ref().cloned().unwrap_or_else(|| {
                    ranking_contributions.remove(&hotlap.id).unwrap_or_default()
                });
                let player_badges = badges
                    .as_mut()
                    .map(|by_player| by_player.remove(&hotlap.player_id).unwrap_or_default());
                Ok(HotlapListEntry {
                    hotlap,
                    player,
                    era,
                    position: chart.map(|c| c.0),
                    distance_to_world_record_ms: chart.map(|c| c.1),
                    distance_to_benchmark_ms: chart.and_then(|c| c.2),
                    contributes_to,
                    badges: player_badges,
                })
            })
            .collect::<Result<Vec<_>, HotlapListError>>()?;

        Ok(HotlapPage {
            entries,
            total,
            chart_contributions,
        })
    }
}

use crate::models::{
    hotlap::HotlapFilter,
    player::{Column as PlayerColumn, PlayerFilter},
};
use sea_orm::QueryTrait;

/// Resolved filters. The caller determines the permitted owner/state scope.
#[derive(Debug, Default)]
pub(crate) struct HotlapListFilters<'a> {
    pub(crate) era_id: Option<i64>,
    pub(crate) chart: Option<&'a Chart>,
    pub(crate) country: Option<celes::Country>,
    pub(crate) controller: Option<super::SteeringInput>,
    pub(crate) state: Option<super::HotlapState>,
    pub(crate) unpublished: bool,
    pub(crate) owner: Option<i64>,
    pub(crate) lfs_username: Option<&'a str>,
    pub(crate) track: Option<insim_core::track::Track>,
    pub(crate) vehicle: Option<insim_core::vehicle::Vehicle>,
    pub(crate) ranked_only: bool,
    pub(crate) rank: Option<i64>,
}
impl<'a> HotlapListFilters<'a> {
    /// A public leaderboard has a fixed chart and only its ranked valid personal bests.
    pub(crate) fn for_chart(chart: &'a Chart) -> Self {
        Self {
            era_id: Some(chart.era_id),
            chart: Some(chart),
            state: Some(super::HotlapState::Valid),
            ranked_only: true,
            ..Self::default()
        }
    }

    pub(crate) fn select(&self) -> Select<HotlapEntity> {
        let mut hotlaps = HotlapEntity::find();
        if let Some(era_id) = self.era_id {
            hotlaps = hotlaps.in_era(era_id);
        }
        if let Some(chart) = self.chart {
            hotlaps = hotlaps.filter(HotlapColumn::ChartId.eq(chart.id));
        }
        if let Some(controller) = self.controller {
            hotlaps = hotlaps.filter(HotlapColumn::Steering.eq(controller));
        }
        if let Some(country) = &self.country {
            hotlaps = hotlaps.filter(
                HotlapColumn::PlayerId.in_subquery(
                    PlayerEntity::find()
                        .filter(PlayerColumn::CountryCode.eq(country.alpha2))
                        .select_only()
                        .column(PlayerColumn::Id)
                        .into_query(),
                ),
            );
        }

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

/// Rank joins use the unique hotlap identity; the personal-best table is chart-scoped.
fn ordered(
    mut hotlaps: Select<HotlapEntity>,
    column: HotlapListColumn,
    direction: Ordering,
) -> Select<HotlapEntity> {
    let order = match direction {
        Ordering::Asc => Order::Asc,
        Ordering::Desc => Order::Desc,
    };
    QueryTrait::query(&mut hotlaps).join_as(
        JoinType::LeftJoin,
        Alias::new("hotlap_personal_best"),
        Alias::new("sort_rank"),
        Expr::col((Alias::new("sort_rank"), Alias::new("hotlap_id")))
            .equals((HotlapEntity, HotlapColumn::Id)),
    );
    if column == HotlapListColumn::Driver {
        QueryTrait::query(&mut hotlaps).join_as(
            JoinType::LeftJoin,
            PlayerEntity,
            Alias::new("sort_player"),
            Expr::col((Alias::new("sort_player"), Alias::new("id")))
                .equals((HotlapEntity, HotlapColumn::PlayerId)),
        );
    }
    hotlaps = match column {
        HotlapListColumn::Submitted => hotlaps.order_by(HotlapColumn::CreatedAt, order),
        HotlapListColumn::LapTime => hotlaps.order_by(HotlapColumn::LapTimeMs, order),
        HotlapListColumn::Driver => hotlaps.order_by(
            Expr::cust("LOWER(sort_player.display_name) COLLATE \"C\""),
            order,
        ),
        HotlapListColumn::Rank => {
            hotlaps.order_by_with_nulls(Expr::cust("sort_rank.position"), order, NullOrdering::Last)
        }
    };
    hotlaps
        .order_by_with_nulls(
            Expr::cust("sort_rank.position"),
            Order::Asc,
            NullOrdering::Last,
        )
        .order_by_desc(HotlapColumn::CreatedAt)
        .order_by_desc(HotlapColumn::Id)
}
