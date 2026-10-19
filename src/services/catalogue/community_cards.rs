//! Catalogue-owned homepage content. Each supplied document replaces one card.

use super::assets;
use crate::models::community_card::{self, CommunityCardContent, CommunityCardTheme};
use anyhow::{Context, ensure};
use object_store::ObjectStore;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommunityCardDefinition {
    pub(crate) id: String,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) sequence: i32,
    #[serde(default = "enabled")]
    pub(crate) enabled: bool,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub(crate) starts_at: Option<time::OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub(crate) ends_at: Option<time::OffsetDateTime>,
    #[serde(default)]
    pub(crate) sponsored: bool,
    #[serde(default)]
    pub(crate) theme: CommunityCardTheme,
    #[serde(default)]
    pub(crate) image: Option<PathBuf>,
    pub(crate) content: CommunityCardContent,
}

fn enabled() -> bool {
    true
}

impl CommunityCardDefinition {
    pub(super) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        let validate = || -> anyhow::Result<()> {
            ensure!(
                !self.id.is_empty()
                    && self.id.len() <= 80
                    && self
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "card id must contain 1 to 80 lowercase letters, digits or hyphens"
            );
            community_card::text(&self.title, "card title", 120)?;
            if let (Some(start), Some(end)) = (self.starts_at, self.ends_at) {
                ensure!(start < end, "starts_at must be earlier than ends_at");
            }
            self.content.validate(self.image.is_some())?;
            if let Some(file) = &self.image {
                assets::read(source, file)?;
            }
            Ok(())
        };
        validate().with_context(|| format!("invalid community card in {}", source.display()))
    }

    pub(super) async fn apply(
        &self,
        transaction: &DatabaseTransaction,
        object_store: Option<&dyn ObjectStore>,
        dry_run: bool,
        source: &Path,
    ) -> anyhow::Result<bool> {
        self.validate(source)?;
        let existing = community_card::Entity::find_by_id(&self.id)
            .one(transaction)
            .await?;
        let mut image_id = None;
        let mut artwork_changed = false;
        if let Some(file) = &self.image {
            let current = existing.as_ref().and_then(|card| card.image_attachment_id);
            match assets::store(
                transaction,
                object_store,
                assets::read(source, file)?,
                current,
                file,
                dry_run,
            )
            .await?
            {
                assets::AssetChange::Unchanged => image_id = current,
                assets::AssetChange::WouldChange => {
                    image_id = current;
                    artwork_changed = true;
                }
                assets::AssetChange::Stored(id) => image_id = Some(id),
            }
        }
        let desired = community_card::Model {
            id: self.id.clone(),
            title: self.title.clone(),
            sequence: self.sequence,
            enabled: self.enabled,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            sponsored: self.sponsored,
            theme: serde_json::to_value(self.theme)?
                .as_str()
                .unwrap()
                .to_owned(),
            content: serde_json::to_value(&self.content)?,
            image_attachment_id: image_id,
        };
        if existing.as_ref() == Some(&desired) {
            return Ok(artwork_changed);
        }
        // Set every field: omitted optional fields deliberately clear the old value.
        let active = desired.into_active_model().reset_all();
        if existing.is_some() {
            active.update(transaction).await?;
        } else {
            active.insert(transaction).await?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
