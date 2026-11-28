//! Catalogue documents for homepage cards. Each supplied document replaces one card.

use crate::community_card::{self, CommunityCardContent, CommunityCardTheme};
use anyhow::{Context, ensure};
use lfsplanet_hotlaps::services::catalogue::assets;
use object_store::ObjectStore;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

/// The catalogue document kind handled here rather than by the hotlaps catalogue.
const KIND: &str = "CommunityCard";

/// A parsed card and the YAML file against which its image path is resolved.
pub struct SourceCard {
    pub source: PathBuf,
    pub definition: CommunityCardDefinition,
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum Document {
    CommunityCard(CommunityCardDefinition),
}

#[derive(Deserialize)]
struct DocumentKind {
    kind: Option<String>,
}

/// Whether a catalogue file is a community card document.
pub fn is_card(path: &Path) -> anyhow::Result<bool> {
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read catalogue file {}", path.display()))?;
    let document = serde_saphyr::from_str::<DocumentKind>(&input)
        .with_context(|| format!("failed to parse catalogue definition {}", path.display()))?;
    Ok(document.kind.as_deref() == Some(KIND))
}

/// Load and validate every card before the catalogue changes.
pub fn load(paths: &[PathBuf]) -> anyhow::Result<Vec<SourceCard>> {
    let mut ids = HashSet::new();
    let mut cards = Vec::with_capacity(paths.len());
    for source in paths {
        let input = std::fs::read_to_string(source)
            .with_context(|| format!("failed to read catalogue file {}", source.display()))?;
        let Document::CommunityCard(definition) = serde_saphyr::from_str::<Document>(&input)
            .with_context(|| {
                format!("failed to parse catalogue definition {}", source.display())
            })?;
        definition.validate(source)?;
        ensure!(
            ids.insert(definition.id.clone()),
            "duplicate community card {} in {}",
            definition.id,
            source.display()
        );
        cards.push(SourceCard {
            source: source.clone(),
            definition,
        });
    }
    Ok(cards)
}

/// Apply cards inside the caller's catalogue write transaction, returning changed ids.
pub async fn apply(
    transaction: &DatabaseTransaction,
    cards: &[SourceCard],
    object_store: Option<&dyn ObjectStore>,
    dry_run: bool,
) -> anyhow::Result<Vec<String>> {
    let mut changed = Vec::new();
    for card in cards {
        if card
            .definition
            .apply(transaction, object_store, dry_run, &card.source)
            .await?
        {
            changed.push(card.definition.id.clone());
        }
    }
    Ok(changed)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityCardDefinition {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub sequence: i32,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub starts_at: Option<time::OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub ends_at: Option<time::OffsetDateTime>,
    #[serde(default)]
    pub sponsored: bool,
    #[serde(default)]
    pub theme: CommunityCardTheme,
    #[serde(default)]
    pub image: Option<PathBuf>,
    pub content: CommunityCardContent,
}

fn enabled() -> bool {
    true
}

impl CommunityCardDefinition {
    fn validate(&self, source: &Path) -> anyhow::Result<()> {
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
            lfsplanet_core::validate::bounded_text(&self.title, "card title", 120)?;
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

    async fn apply(
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
