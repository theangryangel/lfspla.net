//! Advertised meetings, without individual practice or race session records.

pub(crate) mod list;
pub(crate) mod source;

use sea_orm::{Set, entity::prelude::*};
use source::{Change, Source};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "event")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub season_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: TimeDateTimeWithTimeZone,
    pub details_url: Option<String>,
    pub signup_url: Option<String>,
    pub track_label: Option<String>,
    pub cars_label: Option<String>,
    pub hidden: bool,
    pub source: Source,
    pub external_id: Option<String>,
    pub source_url: Option<String>,
    pub last_seen_at: Option<TimeDateTimeWithTimeZone>,
    pub last_synced_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
}
impl ActiveModelBehavior for ActiveModel {}

/// Source-owned fields. Local moderation and identity are deliberately absent.
#[derive(Debug, PartialEq)]
pub(crate) struct Import {
    pub league_id: i64,
    pub season_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: TimeDateTimeWithTimeZone,
    pub details_url: Option<String>,
    pub signup_url: Option<String>,
    pub track_label: Option<String>,
    pub cars_label: Option<String>,
}

impl From<&Model> for Import {
    fn from(row: &Model) -> Self {
        Self {
            league_id: row.league_id,
            season_id: row.season_id,
            title: row.title.clone(),
            description: row.description.clone(),
            starts_at: row.starts_at,
            details_url: row.details_url.clone(),
            signup_url: row.signup_url.clone(),
            track_label: row.track_label.clone(),
            cars_label: row.cars_label.clone(),
        }
    }
}

impl Model {
    pub(crate) async fn import(
        database: &impl ConnectionTrait,
        key: &str,
        input: Import,
        url: &str,
        now: TimeDateTimeWithTimeZone,
    ) -> Result<(Self, Change), DbErr> {
        use sea_orm::QueryFilter;
        let existing = Entity::find()
            .filter(Column::Source.eq(Source::LfsNet))
            .filter(Column::ExternalId.eq(key))
            .one(database)
            .await?;
        let change = match &existing {
            None => Change::Created,
            Some(old) if Import::from(old) == input => Change::Unchanged,
            Some(_) => Change::Updated,
        };
        let mut active = existing.map_or_else(
            || ActiveModel {
                created_at: Set(now),
                ..Default::default()
            },
            Into::into,
        );
        active.league_id = Set(input.league_id);
        active.season_id = Set(input.season_id);
        active.title = Set(input.title);
        active.description = Set(input.description);
        active.starts_at = Set(input.starts_at);
        active.details_url = Set(input.details_url);
        active.signup_url = Set(input.signup_url);
        active.track_label = Set(input.track_label);
        active.cars_label = Set(input.cars_label);
        active.source = Set(Source::LfsNet);
        active.external_id = Set(Some(key.to_owned()));
        active.source_url = Set(Some(url.to_owned()));
        active.last_seen_at = Set(Some(now));
        active.last_synced_at = Set(Some(now));
        if !matches!(change, Change::Unchanged) {
            active.updated_at = Set(now);
        }
        let row = match change {
            Change::Created => active.insert(database).await?,
            _ => active.update(database).await?,
        };
        Ok((row, change))
    }
}
