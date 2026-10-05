//! Explicit repair of persisted hotlap era classifications.

use crate::models::{
    Era,
    era::{Column as EraColumn, Entity as EraEntity},
};
use anyhow::Context;
use insim_core::game_version::GameVersion;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, QueryOrder,
    Statement, TransactionTrait, Value,
};
#[derive(FromQueryResult)]
struct StoredGameVersion {
    game_version: String,
}

#[derive(Debug, PartialEq, Eq)]
struct VersionEra {
    game_version: String,
    era_id: i64,
}

/// Outcome of a completed era classification repair.
pub(crate) struct RepairSummary {
    pub game_versions: usize,
    pub hotlaps_updated: u64,
}

/// Reclassifies every hotlap, rebuilding personal bests and chart positions.
pub(crate) async fn repair(database: &DatabaseConnection) -> anyhow::Result<RepairSummary> {
    let transaction = database.begin().await?;
    crate::db::lock_rebuild(&transaction).await?;
    let eras = EraEntity::find()
        .order_by_asc(EraColumn::Id)
        .all(&transaction)
        .await?;

    let versions = StoredGameVersion::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r"
        SELECT DISTINCT game_version
        FROM hotlap
        ORDER BY game_version
        ",
    ))
    .all(&transaction)
    .await
    .context("failed to list stored hotlap game versions")?;
    let classifications = classify_versions(
        versions.into_iter().map(|version| version.game_version),
        &eras,
    )?;

    // Clear projections before changing the hotlap/chart composite FK.
    transaction
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "DELETE FROM hotlap_personal_best".to_owned(),
        ))
        .await?;
    let mut hotlaps_updated = 0;
    for classification in &classifications {
        hotlaps_updated += transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
            UPDATE hotlap
            SET era_id = $1, chart_id = (
                SELECT id FROM chart WHERE era_id = $1
                  AND track_id = hotlap.track AND vehicle_id = hotlap.vehicle
            )
            WHERE game_version = $2
              AND era_id IS DISTINCT FROM $1
            ",
                [
                    Value::from(classification.era_id),
                    Value::from(classification.game_version.as_str()),
                ],
            ))
            .await
            .with_context(|| {
                format!(
                    "failed to reclassify hotlaps with game version {}",
                    classification.game_version
                )
            })?
            .rows_affected();
    }
    // A reclassification changes the PB key even when the lap remains valid.
    // Rebuild while the corrected hotlap rows and their projection are still
    // one atomic change. rebuild_era also reranks every chart before commit.
    for era in &eras {
        era.rebuild_personal_bests(&transaction).await?;
    }
    transaction
        .commit()
        .await
        .context("failed to commit hotlap era classifications")?;
    for era in &eras {
        era.rebuild_badges(database).await?;
    }

    Ok(RepairSummary {
        game_versions: classifications.len(),
        hotlaps_updated,
    })
}

fn classify_versions(
    versions: impl IntoIterator<Item = String>,
    eras: &[Era],
) -> anyhow::Result<Vec<VersionEra>> {
    versions
        .into_iter()
        .map(|version| {
            let parsed = version
                .parse::<GameVersion>()
                .with_context(|| format!("invalid stored hotlap game version {version:?}"))?;
            let era = eras
                .iter()
                .find(|era| era.accepts_replay_version(&parsed))
                .with_context(|| {
                    format!("stored hotlap game version {version} is not covered by an era")
                })?;
            Ok(VersionEra {
                game_version: version,
                era_id: era.id,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_stored_versions_before_writing() {
        let eras = crate::models::era::definition::test_definitions()
            .into_iter()
            .enumerate()
            .map(|(index, era)| Era {
                id: i64::try_from(index + 1).unwrap(),
                slug: era.id,
                title: era.title,
                installation_id: era.installation_id,
                version_requirement: era.version_requirement,
                open: era.open,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            classify_versions(["0.7D41".to_owned()], &eras).unwrap(),
            [VersionEra {
                game_version: "0.7D41".to_owned(),
                era_id: 3,
            }]
        );
        assert!(classify_versions(["invalid".to_owned()], &eras).is_err());
        assert!(classify_versions(["0.8A".to_owned()], &eras).is_err());
    }
}
