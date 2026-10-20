//! Load complete input batches before changing the catalogue.

use super::CatalogueDefinition;
use crate::models::era::definition::normalise_file;
use anyhow::{Context, ensure};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use validator::Validate;

/// A parsed document and the YAML file against which asset paths are resolved.
pub(crate) struct SourceDefinition {
    pub(crate) source: PathBuf,
    pub(crate) definition: CatalogueDefinition,
}

/// Load tagged files selected explicitly by the caller (including shell globs).
pub(crate) fn files(paths: &[PathBuf]) -> anyhow::Result<Vec<SourceDefinition>> {
    ensure!(!paths.is_empty(), "at least one catalogue file is required");
    ensure!(
        paths.iter().all(|path| path != Path::new("-")),
        "stdin is not supported; supply catalogue YAML file paths"
    );
    let mut ids = HashSet::new();
    let mut documents = Vec::with_capacity(paths.len());
    for source in paths {
        let input = std::fs::read_to_string(source)
            .with_context(|| format!("failed to read catalogue file {}", source.display()))?;
        let mut definition =
            serde_saphyr::from_str::<CatalogueDefinition>(&input).with_context(|| {
                format!("failed to parse catalogue definition {}", source.display())
            })?;
        match &mut definition {
            CatalogueDefinition::Badge(badge) => {
                badge.validate(source)?;
                ensure!(
                    ids.insert(("Badge", badge.id.clone())),
                    "duplicate badge {} in {}",
                    badge.id,
                    source.display()
                );
            }
            CatalogueDefinition::CommunityCard(card) => {
                card.validate(source)?;
                ensure!(
                    ids.insert(("CommunityCard", card.id.clone())),
                    "duplicate community card {} in {}",
                    card.id,
                    source.display()
                );
            }
            CatalogueDefinition::Era(era) => {
                normalise_file(era);
                era.validate()
                    .with_context(|| format!("invalid era in {}", source.display()))?;
                ensure!(
                    ids.insert(("Era", era.id.to_string())),
                    "duplicate era {} in {}",
                    era.id,
                    source.display()
                );
            }
            CatalogueDefinition::Location(location) => {
                location.validate(source)?;
                ensure!(
                    ids.insert(("Location", location.id.clone())),
                    "duplicate location {} in {}",
                    location.id,
                    source.display()
                );
                for track in &location.tracks {
                    ensure!(
                        ids.insert(("Track", track.id.to_string())),
                        "duplicate track {} in {}",
                        track.id,
                        source.display()
                    );
                }
            }
            CatalogueDefinition::Vehicle(vehicle) => {
                vehicle.validate(source)?;
                ensure!(
                    ids.insert(("Vehicle", vehicle.id.to_string())),
                    "duplicate vehicle {} in {}",
                    vehicle.id,
                    source.display()
                );
            }
        }
        documents.push(SourceDefinition {
            source: source.clone(),
            definition,
        });
    }
    Ok(documents)
}

#[cfg(test)]
mod tests;
