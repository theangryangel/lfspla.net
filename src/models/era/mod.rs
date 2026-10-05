//! Ranking-era persistence entities, policies, and validation queries.

use crate::era_slug::EraSlug;
use crate::installation_id::InstallationId;

use crate::models::{
    chart::{Column as ChartColumn, Entity as ChartEntity},
    track::{Column as TrackColumn, Entity as TrackEntity},
    vehicle::{Column as VehicleColumn, Entity as VehicleEntity},
};

use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};

use lfsplanet_game_version_req::GameVersionReq;

use sea_orm::{ConnectionTrait, QueryOrder, QuerySelect, QueryTrait, Select, entity::prelude::*};

mod activity;
mod badges;
mod coverage;
pub(crate) mod definition;
mod personal_bests;
mod rankings;

pub(crate) use activity::rank_world_record_holders;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "era")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Stable YAML and public API identifier. Database relations use `id`.
    pub slug: EraSlug,
    pub title: String,
    /// Named LFS installation used to validate this era's replays.
    pub installation_id: InstallationId,
    /// LFS replay versions this physics era covers.
    pub version_requirement: GameVersionReq,
    pub open: bool,
    #[sea_orm(has_many)]
    pub rankings: HasMany<crate::models::ranking::Entity>,
    #[sea_orm(has_many)]
    pub charts: HasMany<crate::models::chart::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
    #[sea_orm(has_many)]
    pub player_badges: HasMany<crate::models::badge::Entity>,
}

impl Model {
    /// Resolves the stable YAML/API slug at an application boundary.
    pub(crate) async fn find_by_slug(
        database: &impl ConnectionTrait,
        slug: &EraSlug,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::Slug.eq(slug.clone()))
            .one(database)
            .await
    }

    /// Whether this era covers a replay version.
    pub fn accepts_replay_version(&self, version: &GameVersion) -> bool {
        self.version_requirement.matches(version)
    }

    /// All distinct charts selected in this era.
    pub fn combinations(&self) -> Select<ChartEntity> {
        ChartEntity::find()
            .filter(ChartColumn::EraId.eq(self.id))
            .order_by_asc(ChartColumn::TrackId)
            .order_by_asc(ChartColumn::VehicleId)
    }

    /// The mirror of [`Self::vehicles_for_track`]. Mods narrow the catalogue
    /// like anything else, since a mod identifier is a `Vehicle` too.
    pub fn tracks_for_vehicle(&self, vehicle: Option<&Vehicle>) -> Select<TrackEntity> {
        let mut charts = ChartEntity::find().filter(ChartColumn::EraId.eq(self.id));
        if let Some(vehicle) = vehicle {
            charts = charts.filter(ChartColumn::VehicleId.eq(vehicle.to_string()));
        }
        TrackEntity::find().filter(
            TrackColumn::Id.in_subquery(
                charts
                    .select_only()
                    .column(ChartColumn::TrackId)
                    .into_query(),
            ),
        )
    }

    pub fn vehicles_for_track(&self, track: Option<&Track>) -> Select<VehicleEntity> {
        let mut charts = ChartEntity::find().filter(ChartColumn::EraId.eq(self.id));
        if let Some(track) = track {
            charts = charts.filter(ChartColumn::TrackId.eq(track.to_string()));
        }
        VehicleEntity::find()
            .filter(VehicleColumn::Available.eq(true))
            .filter(
                VehicleColumn::Id.in_subquery(
                    charts
                        .select_only()
                        .column(ChartColumn::VehicleId)
                        .into_query(),
                ),
            )
    }

    pub(crate) async fn accepting_version(
        database: &impl ConnectionTrait,
        version: &GameVersion,
    ) -> Result<Option<Self>, DbErr> {
        use sea_orm::QueryOrder;
        Ok(Entity::find()
            .order_by_asc(Column::Id)
            .all(database)
            .await?
            .into_iter()
            .find(|era| era.accepts_replay_version(version)))
    }
}
impl ActiveModelBehavior for ActiveModel {}
