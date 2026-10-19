//! Authored community cards and their supported presentation templates.

use anyhow::{Context, ensure};
use sea_orm::entity::prelude::*;
use sea_orm::{QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "community_card")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub title: String,
    pub sequence: i32,
    pub enabled: bool,
    pub starts_at: Option<time::OffsetDateTime>,
    pub ends_at: Option<time::OffsetDateTime>,
    pub sponsored: bool,
    pub theme: String,
    pub content: Json,
    pub image_attachment_id: Option<i64>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Start-inclusive, end-exclusive publication window. Disabled cards never appear.
    pub(crate) fn visible_at(now: time::OffsetDateTime) -> sea_orm::Select<Self> {
        Self::find()
            .filter(Column::Enabled.eq(true))
            .filter(
                sea_orm::Condition::any()
                    .add(Column::StartsAt.is_null())
                    .add(Column::StartsAt.lte(now)),
            )
            .filter(
                sea_orm::Condition::any()
                    .add(Column::EndsAt.is_null())
                    .add(Column::EndsAt.gt(now)),
            )
            .order_by_asc(Column::Sequence)
            .order_by_asc(Column::Id)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommunityCardTheme {
    #[default]
    Neutral,
    Cyan,
    Purple,
    Yellow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommunityImageFit {
    #[default]
    Cover,
    Contain,
}

impl CommunityImageFit {
    fn contain() -> Self {
        Self::Contain
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommunityLink {
    pub(crate) label: String,
    /// Absolute HTTP(S) URL or a root-relative site path.
    pub(crate) url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CommunityCardContent {
    Partner {
        description: String,
        link: CommunityLink,
        #[serde(default = "CommunityImageFit::contain")]
        fit: CommunityImageFit,
    },
    Image {
        alt: String,
        #[serde(default)]
        #[schema(required)]
        description: Option<String>,
        #[serde(default)]
        #[schema(required)]
        link: Option<CommunityLink>,
        #[serde(default)]
        fit: CommunityImageFit,
    },
    Announcement {
        body: String,
        #[serde(default = "CommunityImageFit::contain")]
        fit: CommunityImageFit,
        #[serde(default)]
        #[schema(required)]
        link: Option<CommunityLink>,
    },
}

impl CommunityLink {
    fn validate(&self) -> anyhow::Result<()> {
        text(&self.label, "link label", 100)?;
        ensure!(
            !self
                .url
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || c == '\\'),
            "link URL contains whitespace or an invalid character"
        );
        if self.url.starts_with('/') && !self.url.starts_with("//") {
            return Ok(());
        }
        let url =
            url::Url::parse(&self.url).context("link URL must be HTTP(S) or root-relative")?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none(),
            "link URL must be HTTP(S), without credentials, or root-relative"
        );
        Ok(())
    }
}

impl CommunityCardContent {
    pub(crate) fn validate(&self, has_image: bool) -> anyhow::Result<()> {
        match self {
            Self::Partner {
                description, link, ..
            } => {
                text(description, "partner description", 500)?;
                link.validate()?;
            }
            Self::Image {
                alt,
                description,
                link,
                ..
            } => {
                ensure!(has_image, "image cards require an image");
                text(alt, "image alt text", 300)?;
                if let Some(description) = description {
                    text(description, "image description", 500)?;
                }
                if let Some(link) = link {
                    link.validate()?;
                }
            }
            Self::Announcement { body, link, .. } => {
                text(body, "announcement body", 1000)?;
                if let Some(link) = link {
                    link.validate()?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn text(value: &str, name: &str, max: usize) -> anyhow::Result<()> {
    ensure!(
        !value.trim().is_empty() && value.chars().count() <= max,
        "{name} must contain 1 to {max} characters"
    );
    Ok(())
}
