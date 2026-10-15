//! Locations own authored metadata and their nested canonical track definitions.

use super::{Applied, assets, tracks::TrackDefinition};
use crate::models::track::location::{self, ActiveModel, Entity};
use anyhow::{Context, ensure};
use object_store::ObjectStore;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, Set};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocationDefinition {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sequence: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<PathBuf>,
    #[serde(default)]
    pub(crate) tracks: Vec<TrackDefinition>,
}

impl LocationDefinition {
    pub(super) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        ensure!(
            !self.id.trim().is_empty(),
            "location ID is empty in {}",
            source.display()
        );
        ensure!(
            !self.name.trim().is_empty(),
            "location name is empty in {}",
            source.display()
        );
        if let Some(file) = &self.image {
            assets::read(source, file)?;
        }
        for track in &self.tracks {
            ensure!(
                location::canonical_code(track.id) == self.id,
                "track {} does not belong to location {} in {}",
                track.id,
                self.id,
                source.display()
            );
            track.validate(source)?;
        }
        Ok(())
    }

    pub(super) async fn apply(
        &self,
        transaction: &DatabaseTransaction,
        object_store: Option<&dyn ObjectStore>,
        dry_run: bool,
        source: &Path,
    ) -> anyhow::Result<Applied> {
        let existing = Entity::find_by_id(self.id.clone()).one(transaction).await?;
        let is_new = existing.is_none();
        let mut changed = existing.as_ref().is_none_or(|row| {
            row.name != self.name
                || self
                    .description
                    .as_ref()
                    .is_some_and(|text| row.description.as_ref() != Some(text))
                || self
                    .sequence
                    .is_some_and(|sequence| row.sequence != sequence)
        });
        let mut model = match existing {
            Some(model) => model.into_active_model(),
            None => ActiveModel {
                id: Set(self.id.clone()),
                name: Set(self.name.clone()),
                description: Set(None),
                sequence: Set(0),
                image_attachment_id: Set(None),
            },
        };
        model.name = Set(self.name.clone());
        if let Some(description) = &self.description {
            model.description = Set(Some(description.clone()));
        }
        if let Some(sequence) = self.sequence {
            model.sequence = Set(sequence);
        }
        if let Some(file) = &self.image {
            let asset = assets::read(source, file)?;
            match assets::store(
                transaction,
                object_store,
                asset,
                *model.image_attachment_id.as_ref(),
                file,
                dry_run,
            )
            .await?
            {
                assets::AssetChange::Unchanged => {}
                assets::AssetChange::WouldChange => changed = true,
                assets::AssetChange::Stored(id) => {
                    changed = true;
                    model.image_attachment_id = Set(Some(id));
                }
            }
        }
        if changed {
            if is_new {
                model.insert(transaction).await
            } else {
                model.update(transaction).await
            }
            .with_context(|| {
                format!(
                    "failed to apply location {} in {}",
                    self.id,
                    source.display()
                )
            })?;
        }
        let mut applied = Applied::default();
        if changed {
            applied.locations.push(self.id.clone());
        }
        for track in &self.tracks {
            if track
                .apply(transaction, object_store, dry_run, source, &self.id)
                .await?
            {
                applied.tracks.push(track.id.to_string());
            }
        }
        Ok(applied)
    }
}
