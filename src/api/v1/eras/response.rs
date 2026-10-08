//! Shared chart identity and catalogue metadata used by era and spotlight responses.

use super::{tracks::TrackSummary, vehicles::VehicleSummary};
use crate::{
    api::ApiError,
    era_slug::EraSlug,
    models::{
        Chart as ChartModel, Era,
        track::{Column as TrackColumn, Entity as TrackEntity},
        vehicle::{Column as VehicleColumn, Entity as VehicleEntity},
    },
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

/// An offered competition identified by its era, track and vehicle.
/// Player progress and leaderboard results belong to the surrounding response.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Chart {
    pub(crate) era_id: EraSlug,
    pub(crate) era_title: String,
    pub(crate) track: TrackSummary,
    pub(crate) vehicle: VehicleSummary,
}

impl Chart {
    pub(crate) fn new(
        era_id: EraSlug,
        era_title: String,
        track: TrackSummary,
        vehicle: VehicleSummary,
    ) -> Self {
        Self {
            era_id,
            era_title,
            track,
            vehicle,
        }
    }

    /// Loads catalogue metadata in two queries for all charts, retaining input order.
    pub(crate) async fn load(
        database: &impl ConnectionTrait,
        era: &Era,
        charts: &[ChartModel],
    ) -> Result<Vec<Self>, ApiError> {
        if charts.is_empty() {
            return Ok(Vec::new());
        }
        if charts.iter().any(|chart| chart.era_id != era.id) {
            return Err(ApiError::database(sea_orm::DbErr::Type(
                "chart belongs to another era".into(),
            )));
        }
        let (tracks, vehicles) = tokio::try_join!(
            TrackEntity::find()
                .filter(TrackColumn::Id.is_in(charts.iter().map(|chart| chart.track_id)))
                .all(database),
            VehicleEntity::find()
                .filter(VehicleColumn::Id.is_in(charts.iter().map(|chart| chart.vehicle_id)))
                .all(database),
        )
        .map_err(ApiError::database)?;
        let tracks: HashMap<_, _> = tracks
            .into_iter()
            .map(|track| (track.id, TrackSummary::from(track)))
            .collect();
        let vehicles: HashMap<_, _> = vehicles
            .into_iter()
            .map(|vehicle| (vehicle.id, VehicleSummary::from(vehicle)))
            .collect();
        charts
            .iter()
            .map(|chart| {
                let track = tracks.get(&chart.track_id).ok_or_else(|| {
                    ApiError::database(sea_orm::DbErr::RecordNotFound("chart track missing".into()))
                })?;
                let vehicle = vehicles.get(&chart.vehicle_id).ok_or_else(|| {
                    ApiError::database(sea_orm::DbErr::RecordNotFound(
                        "chart vehicle missing".into(),
                    ))
                })?;
                Ok(Self::new(
                    era.slug.clone(),
                    era.title.clone(),
                    track.clone(),
                    vehicle.clone(),
                ))
            })
            .collect()
    }
}
