//! SeaORM entity for the unified standard and mod vehicle catalogue.

use std::collections::HashSet;

use object_store::ObjectMeta;
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

use crate::storage::Storage;

/// Canonical vehicle identity and presentation/upstream metadata.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub kind: String,
    pub sequence: i32,
    pub name: String,
    pub normalized_name: String,
    pub license: String,
    pub version: Option<i16>,
    pub class: Option<i16>,
    pub author_username: Option<String>,
    pub work_in_progress: bool,
    pub published_at: Option<TimeDateTimeWithTimeZone>,
    pub available: bool,
    pub metadata: Option<Json>,
    pub fetched_at: Option<TimeDateTimeWithTimeZone>,
    pub last_seen_at: Option<TimeDateTimeWithTimeZone>,
    pub image_object_key: Option<String>,
    pub image_content_type: Option<String>,
    pub image_fetched_at: Option<TimeDateTimeWithTimeZone>,
    /// Mod revision whose cover was successfully cached; independent of metadata.
    pub image_version: Option<i16>,
    #[sea_orm(has_many)]
    pub ranking_charts: HasMany<crate::models::rankings::chart_entity::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlaps::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Storage for Entity {
    const PREFIX: &'static str = "vehicle-images";

    async fn should_gc(
        database: &DatabaseConnection,
        objects: &[ObjectMeta],
    ) -> Result<Vec<ObjectMeta>, DbErr> {
        let candidates = objects
            .iter()
            .map(|object| object.location.to_string())
            .collect::<Vec<_>>();
        let retained = Entity::find()
            .select_only()
            .column(Column::ImageObjectKey)
            .filter(Column::Available.eq(true))
            .filter(Column::ImageObjectKey.is_in(candidates))
            .into_tuple::<String>()
            .all(database)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        Ok(objects
            .iter()
            .filter(|object| !retained.contains(object.location.as_ref()))
            .cloned()
            .collect())
    }
}
