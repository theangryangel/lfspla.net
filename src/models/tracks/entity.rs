//! SeaORM entity for canonical track presentation metadata.

use sea_orm::entity::prelude::*;

/// One canonical LFS track configuration.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "track")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub license: String,
    pub location: crate::models::tracks::TrackLocation,
    pub reverse: bool,
    pub open_configuration: bool,
    pub sequence: i32,
    #[sea_orm(has_many)]
    pub ranking_charts: HasMany<crate::models::rankings::chart_entity::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlaps::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
