//! Catalogue definitions, local seeding, and upstream synchronisation.

mod apply;
pub mod assets;
pub mod badges;
pub mod eras;
#[cfg(any(test, feature = "test-support"))]
pub mod fixtures;
pub mod load;
pub mod locations;
mod sync_mods;
pub use apply::{Applied, PendingCatalogue, seed, seed_in};
pub use sync_mods::sync_mods;
pub mod tracks;
pub mod vehicles;

use crate::models::era::definition::EraDefinition;
use serde::{Deserialize, Serialize};

/// A catalogue document. Further kinds are added alongside their persistence support.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum CatalogueDefinition {
    Era(EraDefinition),
    Badge(badges::BadgeDefinition),
    Location(locations::LocationDefinition),
    Vehicle(vehicles::VehicleDefinition),
}

impl CatalogueDefinition {
    /// Apply one document inside the caller's catalogue write transaction.
    /// The source is the YAML file, used for diagnostics and resolving relative assets.
    pub async fn apply(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        object_store: Option<&dyn object_store::ObjectStore>,
        dry_run: bool,
        source: &std::path::Path,
    ) -> anyhow::Result<Applied> {
        use anyhow::Context;
        let mut applied = Applied::default();
        match self {
            Self::Badge(definition) => {
                if definition
                    .apply(transaction, object_store, dry_run, source)
                    .await?
                {
                    applied.badges.push(definition.id.clone());
                }
            }
            Self::Era(definition) => {
                let existing =
                    crate::models::Era::find_by_slug(transaction, &definition.id).await?;
                eras::persist(transaction, existing, definition)
                    .await
                    .with_context(|| {
                        format!("failed to apply catalogue definition {}", source.display())
                    })?;
                applied.eras.push(definition.id.clone());
            }
            Self::Vehicle(definition) => {
                if definition
                    .apply(transaction, object_store, dry_run, source)
                    .await?
                {
                    applied.vehicles.push(definition.id.to_string());
                }
            }
            Self::Location(definition) => {
                return definition
                    .apply(transaction, object_store, dry_run, source)
                    .await;
            }
        }
        Ok(applied)
    }
}
