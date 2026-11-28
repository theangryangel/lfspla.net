//! Apply catalogue batches under one shared write transaction.

use super::{
    CatalogueDefinition,
    eras::{self, EraChange},
    load::SourceDefinition,
};
use crate::{
    era_slug::EraSlug,
    models::{
        Era,
        era::{Entity as EraEntity, definition},
    },
};
use anyhow::{Context, ensure};
use lfsplanet_core::db;
use sea_orm::{DatabaseConnection, DatabaseTransaction, EntityTrait, TransactionTrait};
use std::collections::HashSet;
use validator::Validate;

#[derive(Default)]
pub struct Applied {
    pub eras: Vec<EraSlug>,
    pub badges: Vec<String>,
    pub locations: Vec<String>,
    pub tracks: Vec<String>,
    pub vehicles: Vec<String>,
}

impl Applied {
    fn extend(&mut self, other: Self) {
        self.eras.extend(other.eras);
        self.badges.extend(other.badges);
        self.locations.extend(other.locations);
        self.tracks.extend(other.tracks);
        self.vehicles.extend(other.vehicles);
    }
}

/// A batch applied inside a caller's transaction, with badge rebuilds outstanding.
pub struct PendingCatalogue {
    pub applied: Applied,
    changed_eras: Vec<Era>,
}

impl PendingCatalogue {
    /// Rebuild badges for changed eras. Call after the caller's transaction commits:
    /// rebuilds take the catalogue write lock themselves.
    pub async fn finish(self, database: &DatabaseConnection) -> anyhow::Result<Applied> {
        for era in self.changed_eras {
            era.rebuild_badges(database).await.with_context(|| {
                format!(
                    "era {} was applied, but badge rebuilding failed; run hotlaps rebadge --all",
                    era.slug
                )
            })?;
        }
        Ok(self.applied)
    }
}

/// Apply loaded documents under one lock, with review before persistence.
/// Dry runs validate proposed rows and roll back without rebuilding projections or badges.
pub async fn seed(
    database: &DatabaseConnection,
    sources: &[SourceDefinition],
    object_store: Option<&dyn object_store::ObjectStore>,
    dry_run: bool,
    review: impl FnOnce(&[EraChange]) -> anyhow::Result<()>,
) -> anyhow::Result<Applied> {
    let transaction = database.begin().await?;
    db::lock_catalogue_write(&transaction).await?;
    let pending = seed_in(&transaction, sources, object_store, dry_run, review).await?;
    if dry_run {
        transaction.rollback().await?;
        return Ok(pending.applied);
    }
    transaction.commit().await?;
    pending.finish(database).await
}

/// Apply loaded documents inside the caller's transaction, which must already hold
/// [`db::lock_catalogue_write`]. The caller rolls back dry runs, otherwise commits and
/// then calls [`PendingCatalogue::finish`].
pub async fn seed_in(
    transaction: &DatabaseTransaction,
    sources: &[SourceDefinition],
    object_store: Option<&dyn object_store::ObjectStore>,
    dry_run: bool,
    review: impl FnOnce(&[EraChange]) -> anyhow::Result<()>,
) -> anyhow::Result<PendingCatalogue> {
    let era_documents = sources
        .iter()
        .filter_map(|source| match &source.definition {
            CatalogueDefinition::Era(era) => Some((source, era)),
            CatalogueDefinition::Badge(_)
            | CatalogueDefinition::Location(_)
            | CatalogueDefinition::Vehicle(_) => None,
        })
        .collect::<Vec<_>>();
    let mut ids = HashSet::new();
    for (_, definition) in &era_documents {
        definition
            .validate()
            .with_context(|| format!("invalid era definition {}", definition.id))?;
        ensure!(
            ids.insert(&definition.id),
            "duplicate era {} in input",
            definition.id
        );
    }
    let mut changes = Vec::new();
    if !era_documents.is_empty() {
        let existing_models = EraEntity::find().all(transaction).await?;
        let existing_definitions = definition::list(transaction).await?;
        for (_, desired) in &era_documents {
            let model = existing_models
                .iter()
                .find(|model| model.slug == desired.id);
            let existing = existing_definitions
                .iter()
                .find(|era| era.id == desired.id)
                .cloned();
            let affected = if let Some(model) = model {
                eras::affected_hotlaps(transaction, model.id, desired).await?
            } else {
                Vec::new()
            };
            changes.push(EraChange {
                existing,
                desired: (*desired).clone(),
                affected,
            });
        }
    }
    review(&changes)?;
    let mut applied = Applied::default();
    for source in sources {
        if !matches!(source.definition, CatalogueDefinition::Era(_)) {
            applied.extend(
                source
                    .definition
                    .apply(transaction, object_store, dry_run, &source.source)
                    .await?,
            );
        }
    }
    for ((source, _), change) in era_documents.iter().zip(&changes) {
        if change.existing.as_ref() != Some(&change.desired) {
            applied.extend(
                source
                    .definition
                    .apply(transaction, object_store, dry_run, &source.source)
                    .await?,
            );
        }
    }
    if !era_documents.is_empty() {
        Era::validate_version_requirements(transaction)
            .await
            .context("the proposed era would make the catalogue invalid")?;
    }
    super::badges::validate_ranking_references(transaction).await?;
    if dry_run {
        applied.eras.clear();
        return Ok(PendingCatalogue {
            applied,
            changed_eras: Vec::new(),
        });
    }
    let mut changed_eras = Vec::new();
    for slug in &applied.eras {
        let era = Era::find_by_slug(transaction, slug)
            .await?
            .expect("persisted era must exist");
        era.rebuild_personal_bests(transaction).await?;
        changed_eras.push(era);
    }
    Ok(PendingCatalogue {
        applied,
        changed_eras,
    })
}
