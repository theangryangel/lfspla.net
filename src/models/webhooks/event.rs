//! Durable event data and its generated subscription kind.

use sea_orm::{DeriveActiveEnum, EnumIter, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;
use utoipa::ToSchema;

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, EnumDiscriminants, FromJsonQueryResult,
)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum_discriminants(name(WebhookEventKind))]
#[strum_discriminants(derive(EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema))]
#[strum_discriminants(sea_orm(rs_type = "String", db_type = "Text"))]
#[strum_discriminants(serde(rename_all = "snake_case"))]
pub enum WebhookEvent {
    #[strum_discriminants(sea_orm(string_value = "hotlap_validated"))]
    HotlapValidated {
        hotlap_id: i64,
        driver: String,
        era: String,
        track: String,
        vehicle: String,
        lap_time_ms: i64,
        #[serde(default)]
        rank: Option<i64>,
    },
    #[strum_discriminants(sea_orm(string_value = "world_record_set"))]
    WorldRecordSet {
        hotlap_id: i64,
        driver: String,
        era: String,
        track: String,
        vehicle: String,
        lap_time_ms: i64,
        #[serde(default)]
        rank: Option<i64>,
    },
}
