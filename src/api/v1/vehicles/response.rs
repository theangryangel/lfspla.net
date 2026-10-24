//! Typed public specifications, deserialized from the cached upstream JSON.
use super::super::{attachments, eras::vehicles::VehicleSummary};
use crate::models::vehicle::Model;
use lfsplanet_lfs_api::{VehicleModMetadata, VehicleModSpecifications};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ExplorerVehicle {
    #[serde(flatten)]
    vehicle: VehicleSummary,
    class: Option<i16>,
    author: Option<String>,
    approved: Option<bool>,
    electric: Option<bool>,
    rating: Option<f64>,
    num_ratings: Option<u64>,
    downloads: Option<u64>,
    specifications: Option<VehicleSpecifications>,
}

#[derive(Debug, Default, Serialize, ToSchema)]
#[serde(default)]
pub(crate) struct VehicleSpecifications {
    displacement_cc: Option<f64>,
    cylinders: Option<u8>,
    /// 0: inline, 1: flat, 2: V.
    engine_layout: Option<u8>,
    /// 0: none, 1: RWD, 2: FWD, 3: AWD.
    drive: Option<u8>,
    transmission: Option<u8>,
    power_kw: Option<f64>,
    power_rpm: Option<u32>,
    torque_nm: Option<f64>,
    torque_rpm: Option<u32>,
    mass_kg: Option<f64>,
    power_weight_w_kg: Option<f64>,
    /// Litres for combustion vehicles; kWh for electric vehicles.
    tank_capacity: Option<f64>,
}

impl ExplorerVehicle {
    pub fn from_record(
        vehicle: Model,
        attachments: &HashMap<i64, attachments::Attachment>,
    ) -> Self {
        let metadata = vehicle.metadata.as_ref().and_then(|value| {
            match serde_json::from_value::<VehicleModMetadata>(value.clone()) {
                Ok(metadata) => Some(metadata),
                Err(error) => {
                    tracing::warn!(id = %vehicle.id, %error, "Cached vehicle metadata is invalid; next sync will retry details");
                    None
                }
            }
        });
        Self {
            class: vehicle.class,
            author: vehicle.author_username.clone(),
            approved: metadata.as_ref().map(|m| m.approved),
            electric: metadata.as_ref().map(|m| m.electric),
            rating: metadata.as_ref().map(|m| m.rating),
            num_ratings: metadata.as_ref().map(|m| m.num_ratings),
            downloads: metadata.as_ref().map(|m| m.num_downloads),
            specifications: metadata
                .as_ref()
                .and_then(|m| m.vehicle.as_ref())
                .map(VehicleSpecifications::from),
            vehicle: VehicleSummary::from_record(vehicle, attachments),
        }
    }
}

impl From<&VehicleModSpecifications> for VehicleSpecifications {
    fn from(specs: &VehicleModSpecifications) -> Self {
        Self {
            displacement_cc: Some(specs.ice_cc),
            cylinders: Some(specs.ice_num_cylinders),
            engine_layout: Some(specs.ice_layout),
            drive: Some(specs.drive),
            transmission: Some(specs.shift_type),
            power_kw: Some(specs.power),
            power_rpm: Some(specs.max_power_rpm),
            torque_nm: Some(specs.torque),
            torque_rpm: Some(specs.max_torque_rpm),
            mass_kg: Some(specs.mass),
            power_weight_w_kg: Some(specs.power_weight_ratio),
            tank_capacity: Some(specs.fuel_tank_size),
        }
    }
}
