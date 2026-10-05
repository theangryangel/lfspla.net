//! Stable track metadata, synchronization, and eligibility.

use insim_core::track::Track;

use sea_orm::{ActiveValue::Set, ConnectionTrait, EntityTrait, sea_query::OnConflict};

use sea_orm::entity::prelude::*;

use sea_orm::{QueryOrder, Select};

mod location;

pub use location::TrackLocation;

/// One canonical LFS track configuration.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "track")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: crate::track_id::TrackId,
    pub name: String,
    pub license: String,
    pub location: crate::models::track::TrackLocation,
    pub reverse: bool,
    pub open_configuration: bool,
    pub sequence: i32,
    #[sea_orm(has_many)]
    pub charts: HasMany<crate::models::chart::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
}

/// Synchronizes every canonical track known by this binary.
impl Model {
    pub(crate) async fn sync(database: &impl ConnectionTrait) -> Result<(), sea_orm::DbErr> {
        Entity::insert_many(Track::ALL.iter().enumerate().map(|(sequence, track)| {
            ActiveModel {
                id: Set((*track).into()),
                name: Set(track.complete_name().to_owned()),
                license: Set(track.license().to_string().to_ascii_lowercase()),
                location: Set((*track).into()),
                reverse: Set(track.is_reverse()),
                open_configuration: Set(track.is_open()),
                sequence: Set(i32::try_from(sequence)
                    .expect("the compiled track catalogue is far smaller than i32::MAX")),
            }
        }))
        .on_conflict(
            OnConflict::column(Column::Id)
                .update_columns([
                    Column::Name,
                    Column::License,
                    Column::Location,
                    Column::Reverse,
                    Column::OpenConfiguration,
                    Column::Sequence,
                ])
                .to_owned(),
        )
        .exec(database)
        .await?;

        tracing::info!(tracks = Track::ALL.len(), "canonical tracks synchronized");
        Ok(())
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical presentation ordering for track catalogue queries.
pub(crate) trait TrackOrder: QueryOrder + Sized {
    fn in_catalogue_order(self) -> Self {
        self.order_by_asc(Column::Sequence).order_by_asc(Column::Id)
    }
}

impl TrackOrder for Select<Entity> {}
