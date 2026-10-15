//! Canonical metadata and authored artwork for each supplied track.

use super::assets;
use crate::models::track::{ActiveModel, Column, Entity};
use anyhow::Context;
use insim_core::track::Track;
use object_store::ObjectStore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter,
    Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackDefinition {
    pub(crate) id: Track,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outline: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<PathBuf>,
}

impl TrackDefinition {
    pub(super) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        for file in self.outline.iter().chain(&self.image) {
            assets::read(source, file)?;
        }
        Ok(())
    }

    pub(crate) async fn apply(
        &self,
        transaction: &DatabaseTransaction,
        object_store: Option<&dyn ObjectStore>,
        dry_run: bool,
        source: &Path,
        location: &str,
    ) -> anyhow::Result<bool> {
        let id = self.id.to_string();
        let track = Entity::find_by_id(crate::track_id::TrackId(self.id))
            .one(transaction)
            .await?;
        let name = self.id.complete_name().to_owned();
        let license = self.id.license().to_string().to_ascii_lowercase();
        let sequence = i32::try_from(
            Track::ALL
                .iter()
                .position(|track| *track == self.id)
                .context("track is missing from the canonical catalogue")?,
        )?;
        let mut changed = track.as_ref().is_none_or(|track| {
            track.name != name
                || track.license != license
                || track.location != location
                || track.reverse != self.id.is_reverse()
                || track.open_configuration != self.id.is_open()
                || track.sequence != sequence
        });
        if changed {
            let mut model = track
                .clone()
                .map(IntoActiveModel::into_active_model)
                .unwrap_or(ActiveModel {
                    id: Set(self.id.into()),
                    outline_attachment_id: Set(None),
                    image_attachment_id: Set(None),
                    ..Default::default()
                });
            model.name = Set(name);
            model.license = Set(license);
            model.location = Set(location.to_owned());
            model.reverse = Set(self.id.is_reverse());
            model.open_configuration = Set(self.id.is_open());
            model.sequence = Set(sequence);
            if track.is_some() {
                model.update(transaction).await
            } else {
                model.insert(transaction).await
            }
            .with_context(|| format!("failed to apply track {id} in {}", source.display()))?;
        }
        for (file, current, column) in [
            (
                &self.outline,
                track.as_ref().and_then(|track| track.outline_attachment_id),
                Column::OutlineAttachmentId,
            ),
            (
                &self.image,
                track.as_ref().and_then(|track| track.image_attachment_id),
                Column::ImageAttachmentId,
            ),
        ] {
            let Some(file) = file else { continue };
            let asset = assets::read(source, file)?;
            match assets::store(transaction, object_store, asset, current, file, dry_run).await? {
                assets::AssetChange::Unchanged => {}
                assets::AssetChange::WouldChange => changed = true,
                assets::AssetChange::Stored(attachment_id) => {
                    changed = true;
                    Entity::update_many()
                        .filter(Column::Id.eq(&id))
                        .col_expr(column, Expr::value(attachment_id))
                        .exec(transaction)
                        .await?;
                }
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests;
