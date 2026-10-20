//! Badge presentation only; automatic qualification remains in Rust.
use super::assets;
use crate::models::badge::{definition, variant};
use anyhow::{Context, ensure};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, QueryFilter,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BadgeDefinition {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) sequence: i32,
    pub(crate) variants: BTreeMap<String, BadgeVariant>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BadgeVariant {
    #[serde(default)]
    pub(crate) image: Option<PathBuf>,
}

pub(crate) fn validate_slug(id: &str) -> anyhow::Result<()> {
    crate::validate::validate_badge_slug(id).map_err(|_| anyhow::anyhow!(
        "badge and variant ids must contain 1 to 80 lowercase letters, digits or hyphens, starting with a letter or digit"))?;
    Ok(())
}

impl BadgeDefinition {
    pub(super) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        let validate = || -> anyhow::Result<()> {
            validate_slug(&self.id)?;
            crate::models::community_card::text(&self.name, "badge name", 120)?;
            crate::models::community_card::text(&self.description, "badge description", 1000)?;
            ensure!(
                self.variants.contains_key("standard"),
                "badge requires a standard variant"
            );
            for (id, variant) in &self.variants {
                validate_slug(id)?;
                if let Some(image) = &variant.image {
                    assets::read(source, image)?;
                }
            }
            Ok(())
        };
        validate().with_context(|| format!("invalid badge in {}", source.display()))
    }

    pub(super) async fn apply(
        &self,
        transaction: &DatabaseTransaction,
        object_store: Option<&dyn object_store::ObjectStore>,
        dry_run: bool,
        source: &Path,
    ) -> anyhow::Result<bool> {
        self.validate(source)?;
        let existing = definition::Entity::find_by_id(&self.id)
            .one(transaction)
            .await?;
        let desired = definition::Model {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            sequence: self.sequence,
        };
        let mut changed = existing.as_ref() != Some(&desired);
        if changed {
            let active = desired.into_active_model().reset_all();
            if existing.is_some() {
                active.update(transaction).await?;
            } else {
                active.insert(transaction).await?;
            }
        }
        for (id, desired) in &self.variants {
            let existing = variant::Entity::find_by_id((self.id.clone(), id.clone()))
                .one(transaction)
                .await?;
            let current = existing.as_ref().and_then(|v| v.image_attachment_id);
            let mut image_id = None;
            if let Some(file) = &desired.image {
                image_id = match assets::store(
                    transaction,
                    object_store,
                    assets::read(source, file)?,
                    current,
                    file,
                    dry_run,
                )
                .await?
                {
                    assets::AssetChange::Unchanged => current,
                    assets::AssetChange::WouldChange => {
                        changed = true;
                        current
                    }
                    assets::AssetChange::Stored(id) => Some(id),
                };
            }
            let desired = variant::Model {
                badge_id: self.id.clone(),
                id: id.clone(),
                image_attachment_id: image_id,
            };
            if existing.as_ref() != Some(&desired) {
                let active = desired.into_active_model().reset_all();
                if existing.is_some() {
                    active.update(transaction).await?;
                } else {
                    active.insert(transaction).await?;
                }
                changed = true;
            }
        }
        // Refuse removal of a variant still held by a player (FK RESTRICT).
        let removed = variant::Entity::delete_many()
            .filter(variant::Column::BadgeId.eq(&self.id))
            .filter(variant::Column::Id.is_not_in(self.variants.keys().cloned()))
            .exec(transaction)
            .await
            .context("cannot remove an awarded badge variant")?;
        Ok(changed || removed.rows_affected > 0)
    }
}

/// Check ranking references before committing an era, even if it has no players.
pub(super) async fn validate_ranking_references(
    transaction: &DatabaseTransaction,
) -> anyhow::Result<()> {
    use crate::models::ranking::{self, RankingBadgeDefinition};
    for ranking in ranking::Entity::find().all(transaction).await? {
        let Some(value) = ranking.badge else { continue };
        let badge: RankingBadgeDefinition = serde_json::from_value(value)?;
        for variant_id in ["standard", "gold", "silver", "bronze"] {
            ensure!(
                variant::Entity::find_by_id((badge.id.clone(), variant_id.to_owned()))
                    .one(transaction)
                    .await?
                    .is_some(),
                "ranking {} references missing badge variant {}/{}",
                ranking.slug,
                badge.id,
                variant_id
            );
        }
        if let Some(completion) = badge.completion {
            ensure!(
                variant::Entity::find_by_id((completion.id.clone(), "standard".to_owned()))
                    .one(transaction)
                    .await?
                    .is_some(),
                "ranking {} references missing completion badge {}",
                ranking.slug,
                completion.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
