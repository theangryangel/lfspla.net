//! Stable track metadata, synchronization, and eligibility.

mod entity;
mod filter;
mod location;

pub(crate) use filter::{TrackFilter, TrackOrder};
pub use location::TrackLocation;

pub(crate) use entity::*;
#[allow(unused_imports, reason = "part of the standard entity re-export set")]
pub use entity::{
    ActiveModel as TrackMutation, Column as TrackColumn, Entity as TrackEntity, Model as TrackModel,
};

use insim_core::track::Track;
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait, sea_query::OnConflict};

/// Synchronizes every canonical track known by this binary.
pub(crate) async fn sync(database: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    TrackEntity::insert_many(Track::ALL.iter().enumerate().map(|(sequence, track)| {
        TrackMutation {
            id: Set(track.to_string()),
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
        OnConflict::column(TrackColumn::Id)
            .update_columns([
                TrackColumn::Name,
                TrackColumn::License,
                TrackColumn::Location,
                TrackColumn::Reverse,
                TrackColumn::OpenConfiguration,
                TrackColumn::Sequence,
            ])
            .to_owned(),
    )
    .exec(database)
    .await?;

    tracing::info!(tracks = Track::ALL.len(), "canonical tracks synchronized");
    Ok(())
}
