//! Canonical metadata and authored artwork for each supplied standard vehicle.

use super::assets;
use crate::models::vehicle::{ActiveModel, Column, Entity, KIND_STANDARD, STANDARD_VEHICLES};
use anyhow::{Context, ensure};
use insim_core::vehicle::Vehicle;
use object_store::ObjectStore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter,
    Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VehicleDefinition {
    pub(crate) id: Vehicle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<PathBuf>,
}

impl VehicleDefinition {
    pub(super) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        ensure!(
            STANDARD_VEHICLES
                .iter()
                .any(|(vehicle, _)| *vehicle == self.id),
            "Vehicle definitions require a standard vehicle ID in {}",
            source.display()
        );
        if let Some(file) = &self.image {
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
    ) -> anyhow::Result<bool> {
        let id = crate::vehicle_id::VehicleId(self.id);
        let (sequence, (_, name)) = STANDARD_VEHICLES
            .iter()
            .enumerate()
            .find(|(_, (vehicle, _))| *vehicle == self.id)
            .with_context(|| {
                format!(
                    "Vehicle definitions require a standard vehicle ID in {}",
                    source.display()
                )
            })?;
        let sequence = i32::try_from(sequence)?;
        let normalized_name = crate::models::Vehicle::normalize_name(name);
        let license = self.id.license().to_string().to_ascii_lowercase();
        let vehicle = Entity::find_by_id(id).one(transaction).await?;
        let mut changed = vehicle.as_ref().is_none_or(|vehicle| {
            vehicle.kind != KIND_STANDARD
                || vehicle.name != *name
                || vehicle.normalized_name != normalized_name
                || vehicle.license != license
                || vehicle.sequence != sequence
                || !vehicle.available
        });
        if changed {
            let mut model = vehicle
                .clone()
                .map(IntoActiveModel::into_active_model)
                .unwrap_or(ActiveModel {
                    id: Set(id),
                    version: Set(None),
                    class: Set(None),
                    author_username: Set(None),
                    author_player_id: Set(None),
                    work_in_progress: Set(false),
                    published_at: Set(None),
                    metadata: Set(None),
                    fetched_at: Set(None),
                    last_seen_at: Set(None),
                    image_attachment_id: Set(None),
                    image_fetched_at: Set(None),
                    image_version: Set(None),
                    ..Default::default()
                });
            model.kind = Set(KIND_STANDARD.to_owned());
            model.sequence = Set(sequence);
            model.name = Set((*name).to_owned());
            model.normalized_name = Set(normalized_name);
            model.license = Set(license);
            model.available = Set(true);
            if vehicle.is_some() {
                model.update(transaction).await
            } else {
                model.insert(transaction).await
            }
            .with_context(|| format!("failed to apply vehicle {id} in {}", source.display()))?;
        }
        let Some(file) = &self.image else {
            return Ok(changed);
        };
        let asset = assets::read(source, file)?;
        match assets::store(
            transaction,
            object_store,
            asset,
            vehicle
                .as_ref()
                .and_then(|vehicle| vehicle.image_attachment_id),
            file,
            dry_run,
        )
        .await?
        {
            assets::AssetChange::Unchanged => {}
            assets::AssetChange::WouldChange => changed = true,
            assets::AssetChange::Stored(attachment_id) => {
                Entity::update_many()
                    .filter(Column::Id.eq(id))
                    .col_expr(Column::ImageAttachmentId, Expr::value(attachment_id))
                    .col_expr(
                        Column::ImageFetchedAt,
                        Expr::value(time::OffsetDateTime::now_utc()),
                    )
                    .exec(transaction)
                    .await?;
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests;
