//! Catalogue definitions, local seeding, and upstream synchronisation.

mod apply;
mod assets;
pub(crate) mod badges;
pub(crate) mod community_cards;
pub(crate) mod eras;
#[cfg(test)]
pub(crate) mod fixtures;
pub(crate) mod load;
pub(crate) mod locations;
mod sync_mods;
pub(crate) use apply::{Applied, seed};
pub(crate) use sync_mods::sync_mods;
pub(crate) mod tracks;
pub(crate) mod vehicles;

use crate::models::era::definition::EraDefinition;
use serde::{Deserialize, Serialize};

/// A catalogue document. Further kinds are added alongside their persistence support.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub(crate) enum CatalogueDefinition {
    Era(EraDefinition),
    Badge(badges::BadgeDefinition),
    Location(locations::LocationDefinition),
    Vehicle(vehicles::VehicleDefinition),
    CommunityCard(community_cards::CommunityCardDefinition),
}

impl CatalogueDefinition {
    /// Apply one document inside the caller's catalogue write transaction.
    /// The source is the YAML file, used for diagnostics and resolving relative assets.
    pub(crate) async fn apply(
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
            Self::CommunityCard(definition) => {
                if definition
                    .apply(transaction, object_store, dry_run, source)
                    .await?
                {
                    applied.community_cards.push(definition.id.clone());
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
