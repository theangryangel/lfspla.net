//! Apply reviewed era catalogue changes within one locked transaction.

use crate::{
    db::locks,
    models::{
        Era,
        era::{
            ActiveModel as EraMutation, Column as EraColumn, Entity as EraEntity,
            definition::{self, EraDefinition, combination_keys},
        },
        ranking::{
            ActiveModel as RankingMutation, Column as RankingColumn, Entity as RankingEntity,
        },
        ranking_chart::{ActiveModel as RankingChartMutation, Entity as RankingChartEntity},
    },
};
use anyhow::{Context, ensure};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, IntoActiveModel, QueryFilter, Set, Statement,
    TransactionTrait,
};
use std::collections::{HashMap, HashSet};
use validator::Validate;
pub(crate) struct AffectedHotlaps {
    pub(crate) track: String,
    pub(crate) vehicle: String,
    pub(crate) state: String,
    pub(crate) laps: i64,
}
pub(crate) struct EraChange {
    pub(crate) existing: Option<EraDefinition>,
    pub(crate) desired: EraDefinition,
    pub(crate) affected: Vec<AffectedHotlaps>,
}

/// Review runs with the catalogue lock held, before any changes are persisted.
/// Returning an error cancels the transaction.
pub(crate) async fn apply(
    database: &DatabaseConnection,
    desired: &[EraDefinition],
    review: impl FnOnce(&[EraChange]) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<String>> {
    let mut ids = HashSet::new();
    for definition in desired {
        definition
            .validate()
            .with_context(|| format!("invalid era definition {}", definition.id))?;
        ensure!(
            ids.insert(&definition.id),
            "duplicate era {} in input",
            definition.id
        );
    }
    let transaction = database.begin().await?;
    locks::lock_rebuild(&transaction).await?;
    let existing_models = EraEntity::find().all(&transaction).await?;
    let existing_definitions = definition::list(&transaction).await?;
    let mut changes = Vec::new();
    for desired in desired {
        let model = existing_models
            .iter()
            .find(|model| model.slug == desired.id);
        let existing = existing_definitions
            .iter()
            .find(|era| era.id == desired.id)
            .cloned();
        let affected = if let Some(model) = model {
            affected_hotlaps(&transaction, model.id, desired).await?
        } else {
            Vec::new()
        };
        changes.push(EraChange {
            existing,
            desired: desired.clone(),
            affected,
        });
    }
    review(&changes)?;
    let changed = changes
        .iter()
        .filter(|change| change.existing.as_ref() != Some(&change.desired))
        .collect::<Vec<_>>();
    for change in &changed {
        let existing = existing_models
            .iter()
            .find(|model| model.slug == change.desired.id)
            .cloned();
        persist(&transaction, existing, &change.desired).await?;
    }
    crate::models::Era::validate_version_requirements(&transaction)
        .await
        .context("the proposed era would make the catalogue invalid")?;
    let mut changed_eras = Vec::new();
    for change in &changed {
        let era = Era::find_by_slug(&transaction, &change.desired.id)
            .await?
            .expect("persisted era must exist");
        era.rebuild_personal_bests(&transaction).await?;
        changed_eras.push(era);
    }
    transaction.commit().await?;
    let mut applied = Vec::new();
    for era in changed_eras {
        era.rebuild_badges(database).await.with_context(|| {
            format!(
                "era {} was applied, but badge rebuilding failed; run hotlap rebadge --all",
                era.slug
            )
        })?;
        applied.push(era.slug);
    }
    Ok(applied)
}

pub(crate) async fn delete(database: &DatabaseConnection, id: &str) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    locks::lock_rebuild(&transaction).await?;
    let result = EraEntity::delete_many()
        .filter(EraColumn::Slug.eq(id))
        .exec(&transaction)
        .await?;
    ensure!(result.rows_affected == 1, "era {id:?} was not found");
    crate::models::Era::validate_version_requirements(&transaction)
        .await
        .context("deleting the era would make the catalogue invalid")?;
    transaction
        .commit()
        .await
        .context("failed to delete era; stored hotlaps may still reference it")?;
    Ok(())
}

pub(crate) async fn set_open(
    database: &DatabaseConnection,
    id: &str,
    open: bool,
) -> anyhow::Result<bool> {
    let transaction = database.begin().await?;
    locks::lock_rebuild(&transaction).await?;
    let model = Era::find_by_slug(&transaction, id)
        .await?
        .with_context(|| format!("era {id:?} was not found"))?;
    if model.open == open {
        transaction.rollback().await?;
        return Ok(false);
    }
    let mut active = model.into_active_model();
    active.open = Set(open);
    active.update(&transaction).await?;
    transaction.commit().await?;
    Ok(true)
}

#[allow(clippy::too_many_lines)]
async fn persist(
    transaction: &DatabaseTransaction,
    existing: Option<Era>,
    desired: &EraDefinition,
) -> anyhow::Result<()> {
    let version_requirement = desired.version_requirement.clone();
    let era = if let Some(existing) = existing {
        let mut active = existing.into_active_model();
        active.title = Set(desired.title.trim().to_owned());
        active.installation_id = Set(desired.installation_id.clone());
        active.version_requirement = Set(version_requirement);
        active.open = Set(desired.open);
        active.update(transaction).await?
    } else {
        EraMutation {
            id: NotSet,
            slug: Set(desired.id.clone()),
            title: Set(desired.title.trim().to_owned()),
            installation_id: Set(desired.installation_id.clone()),
            version_requirement: Set(version_requirement),
            open: Set(desired.open),
        }
        .insert(transaction)
        .await?
    };

    let existing_rankings = RankingEntity::find()
        .filter(RankingColumn::EraId.eq(era.id))
        .all(transaction)
        .await?
        .into_iter()
        .map(|ranking| (ranking.slug.clone(), ranking))
        .collect::<HashMap<_, _>>();
    for (position, definition) in desired.rankings.iter().enumerate() {
        let position = i32::try_from(position).context("too many rankings for one era")?;
        let badge = definition
            .badge
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .context("failed to serialize ranking badge")?;
        let ranking = if let Some(existing) = existing_rankings.get(definition.id.as_str()) {
            let mut active = existing.clone().into_active_model();
            active.position = Set(position);
            active.title = Set(definition.title.trim().to_owned());
            active.description = Set(definition.description.trim().to_owned());
            active.benchmark_percent = Set(definition.rules.benchmark_percent);
            active.nation_max_points = Set(definition.rules.nation_max_points);
            active.nation_driver_limit = Set(definition.rules.nation_driver_limit);
            active.badge = Set(badge);
            active.update(transaction).await?
        } else {
            RankingMutation {
                id: NotSet,
                era_id: Set(era.id),
                slug: Set(definition.id.to_string()),
                position: Set(position),
                title: Set(definition.title.trim().to_owned()),
                description: Set(definition.description.trim().to_owned()),
                benchmark_percent: Set(definition.rules.benchmark_percent),
                nation_max_points: Set(definition.rules.nation_max_points),
                nation_driver_limit: Set(definition.rules.nation_driver_limit),
                badge: Set(badge),
            }
            .insert(transaction)
            .await?
        };

        RankingChartEntity::delete_many()
            .filter(crate::models::ranking_chart::Column::RankingId.eq(ranking.id))
            .exec(transaction)
            .await?;

        let charts = definition
            .combinations
            .pairs()
            .enumerate()
            .map(|(chart_position, chart)| {
                Ok(RankingChartMutation {
                    era_id: Set(era.id),
                    ranking_id: Set(ranking.id),
                    position: Set(
                        i32::try_from(chart_position).context("too many charts for one ranking")?
                    ),
                    track_id: Set(chart.track.to_string()),
                    vehicle_id: Set(chart.vehicle.to_string()),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        RankingChartEntity::insert_many(charts)
            .exec(transaction)
            .await?;
    }
    let desired_slugs = desired
        .rankings
        .iter()
        .map(|ranking| ranking.id.to_string())
        .collect::<Vec<_>>();
    RankingEntity::delete_many()
        .filter(RankingColumn::EraId.eq(era.id))
        .filter(RankingColumn::Slug.is_not_in(desired_slugs))
        .exec(transaction)
        .await?;
    Ok(())
}

async fn affected_hotlaps<C: ConnectionTrait>(
    database: &C,
    era_id: i64,
    desired: &EraDefinition,
) -> anyhow::Result<Vec<AffectedHotlaps>> {
    let pairs: Vec<_> = combination_keys(desired)
        .into_iter()
        .map(|(track, vehicle)| serde_json::json!({"track": track, "vehicle": vehicle}))
        .collect();
    let rows = database
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT track, vehicle, state, COUNT(*)::BIGINT AS laps
         FROM hotlap
         WHERE era_id = $1 AND vehicle IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM jsonb_to_recordset($2::jsonb) AS pair(track TEXT, vehicle TEXT)
             WHERE pair.track = hotlap.track AND pair.vehicle = hotlap.vehicle)
         GROUP BY track, vehicle, state ORDER BY track, vehicle, state",
            [era_id.into(), serde_json::json!(pairs).into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(AffectedHotlaps {
                track: row.try_get("", "track")?,
                vehicle: row.try_get("", "vehicle")?,
                state: row.try_get("", "state")?,
                laps: row.try_get("", "laps")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
