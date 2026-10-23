//! Optional named seasons grouping a league's event advertisements.

use super::event::source::{Change, Source};
use sea_orm::{Set, entity::prelude::*};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "season")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub league_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source: Source,
    pub external_id: Option<String>,
    pub source_url: Option<String>,
    pub last_seen_at: Option<TimeDateTimeWithTimeZone>,
    pub last_synced_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub(crate) async fn import(
        database: &impl ConnectionTrait,
        key: &str,
        league_id: i64,
        name: &str,
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
            Some(old) if old.name == name && old.league_id == league_id => Change::Unchanged,
            Some(_) => Change::Updated,
        };
        let mut active = existing.map_or_else(
            || ActiveModel {
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
            Into::into,
        );
        active.name = Set(name.to_owned());
        active.league_id = Set(league_id);
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
