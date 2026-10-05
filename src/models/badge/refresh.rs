//! Durable badge refresh requests, retries, and qualification rules.

use crate::models::{era::Entity as EraEntity, ranking::RankingBadgeQualification};
use anyhow::Context;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement};

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
