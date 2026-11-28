//! Reverse relations keep shared entities independent of hotlap entities.
use crate::models::{chart, era, hotlap};
use lfsplanet_core::models::{badge, player, track, vehicle};
use sea_orm::{EntityTrait, Related, RelationDef, RelationTrait};

impl Related<hotlap::Entity> for player::Entity {
    fn to() -> RelationDef {
        hotlap::Relation::Player.def().rev()
    }
}
impl Related<hotlap::Entity> for track::Entity {
    fn to() -> RelationDef {
        hotlap::Relation::Track.def().rev()
    }
}
impl Related<hotlap::Entity> for vehicle::Entity {
    fn to() -> RelationDef {
        hotlap::Relation::Vehicle.def().rev()
    }
}
impl Related<chart::Entity> for track::Entity {
    fn to() -> RelationDef {
        chart::Relation::Track.def().rev()
    }
}
impl Related<chart::Entity> for vehicle::Entity {
    fn to() -> RelationDef {
        chart::Relation::Vehicle.def().rev()
    }
}
impl Related<era::Entity> for badge::Entity {
    fn to() -> RelationDef {
        badge::Entity::belongs_to(era::Entity)
            .from(badge::Column::EraId)
            .to(era::Column::Id)
            .on_update(sea_orm::sea_query::ForeignKeyAction::Cascade)
            .on_delete(sea_orm::sea_query::ForeignKeyAction::Cascade)
            .into()
    }
}
