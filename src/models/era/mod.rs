//! Ranking-era persistence entities, policies, and validation queries.

use validator::ValidationError;

use crate::models::{
    hotlap::HotlapRankable,
    ranking::RankingChartOrder,
    ranking_chart::{Column as RankingChartColumn, Entity as RankingChartEntity},
    track::{Column as TrackColumn, Entity as TrackEntity, TrackFilter},
    vehicle::{Column as VehicleColumn, Entity as VehicleEntity},
};

use insim_core::{game_version::GameVersion, track::Track, vehicle::Vehicle};

use lfsplanet_game_version_req::GameVersionReq;

use sea_orm::{ConnectionTrait, QuerySelect, QueryTrait, Select, entity::prelude::*};

mod activity;
mod coverage;
pub(crate) mod definition;
mod rankings;

pub(crate) use activity::rank_world_record_holders;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "era")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Stable YAML and public API identifier. Database relations use `id`.
    pub slug: String,
    pub title: String,
    /// Named LFS installation used to validate this era's replays.
    pub installation_id: String,
    /// LFS replay versions this physics era covers.
    pub version_requirement: GameVersionReq,
    pub open: bool,
    #[sea_orm(has_many)]
    pub rankings: HasMany<crate::models::ranking::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
    #[sea_orm(has_many)]
    pub player_badges: HasMany<crate::models::badge::Entity>,
}

impl Model {
    /// Resolves the stable YAML/API slug at an application boundary.
    pub(crate) async fn find_by_slug(
        database: &impl ConnectionTrait,
        slug: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::Slug.eq(slug))
            .one(database)
            .await
    }

    /// Whether this era covers a replay version.
    pub fn accepts_replay_version(&self, version: &GameVersion) -> bool {
        self.version_requirement.matches(version)
    }

    /// One representative chart for every distinct pair selected in this era.
    pub fn combinations(&self) -> Select<RankingChartEntity> {
        RankingChartEntity::find()
            .filter(RankingChartColumn::EraId.eq(self.id))
            .distinct_on([RankingChartColumn::TrackId, RankingChartColumn::VehicleId])
            .in_combination_order()
    }

    /// Catalogue projections for navigation only; admission must check the pair.
    pub fn eligible_tracks(&self) -> Select<TrackEntity> {
        self.tracks_for_vehicle(None)
    }

    /// The mirror of [`Self::vehicles_for_track`]. Mods narrow the catalogue
    /// like anything else, since a mod identifier is a `Vehicle` too.
    pub fn tracks_for_vehicle(&self, vehicle: Option<&Vehicle>) -> Select<TrackEntity> {
        let mut charts = RankingChartEntity::find().filter(RankingChartColumn::EraId.eq(self.id));
        if let Some(vehicle) = vehicle {
            charts = charts.filter(RankingChartColumn::VehicleId.eq(vehicle.to_string()));
        }
        TrackEntity::find().closed_circuit().filter(
            TrackColumn::Id.in_subquery(
                charts
                    .select_only()
                    .column(RankingChartColumn::TrackId)
                    .into_query(),
            ),
        )
    }

    pub fn eligible_vehicles(&self) -> Select<VehicleEntity> {
        self.vehicles_for_track(None)
    }

    pub fn vehicles_for_track(&self, track: Option<&Track>) -> Select<VehicleEntity> {
        let mut charts = RankingChartEntity::find().filter(RankingChartColumn::EraId.eq(self.id));
        if let Some(track) = track {
            charts = charts.filter(RankingChartColumn::TrackId.eq(track.to_string()));
        }
        VehicleEntity::find()
            .filter(VehicleColumn::Available.eq(true))
            .filter(
                VehicleColumn::Id.in_subquery(
                    charts
                        .select_only()
                        .column(RankingChartColumn::VehicleId)
                        .into_query(),
                ),
            )
    }

    pub async fn admits_combination<C: ConnectionTrait>(
        &self,
        database: &C,
        track: &Track,
        vehicle: &Vehicle,
    ) -> Result<bool, DbErr> {
        if !track.is_hotlap_rankable() || !vehicle.is_hotlap_rankable() {
            return Ok(false);
        }
        let vehicle_id = vehicle.to_string();
        if VehicleEntity::find_by_id(&vehicle_id)
            .filter(VehicleColumn::Available.eq(true))
            .one(database)
            .await?
            .is_none()
        {
            return Ok(false);
        }
        Ok(RankingChartEntity::find()
            .filter(RankingChartColumn::EraId.eq(self.id))
            .filter(RankingChartColumn::TrackId.eq(track.to_string()))
            .filter(RankingChartColumn::VehicleId.eq(vehicle_id))
            .one(database)
            .await?
            .is_some())
    }

    /// Whether this era admits one canonical track.
    pub async fn admits_track<C>(&self, database: &C, track: &Track) -> Result<bool, DbErr>
    where
        C: ConnectionTrait,
    {
        if !track.is_hotlap_rankable() {
            return Ok(false);
        }
        Ok(self
            .eligible_tracks()
            .filter(TrackColumn::Id.eq(track.to_string()))
            .one(database)
            .await?
            .is_some())
    }

    /// Whether this era admits one canonical vehicle.
    pub async fn admits_vehicle<C>(&self, database: &C, vehicle: &Vehicle) -> Result<bool, DbErr>
    where
        C: ConnectionTrait,
    {
        if !vehicle.is_hotlap_rankable() {
            return Ok(false);
        }
        Ok(self
            .eligible_vehicles()
            .filter(VehicleColumn::Id.eq(vehicle.to_string()))
            .one(database)
            .await?
            .is_some())
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
/// Whether a value can safely be used as one era path component.
pub(crate) fn validate_era_slug(slug: &str) -> Result<(), ValidationError> {
    if !slug.is_empty()
        && std::path::Path::new(slug).components().count() == 1
        && !matches!(slug, "." | "..")
        && !slug.contains(std::path::MAIN_SEPARATOR)
    {
        return Ok(());
    }
    Err(ValidationError::new("invalid_era_slug"))
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::validate_era_slug;

    #[test]
    fn era_slug_must_be_one_safe_path_component() {
        for slug in ["2007-12-21", "s2", "current-era"] {
            assert!(validate_era_slug(slug).is_ok(), "{slug}");
        }
        for slug in ["", ".", "..", "era/other"] {
            assert!(validate_era_slug(slug).is_err(), "{slug}");
        }
    }
}
