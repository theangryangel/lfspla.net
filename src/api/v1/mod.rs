//! Version 1 HTTP routes and boundary models.

pub mod countries;
pub mod eras;
pub mod hotlaps;
pub mod me;
pub mod player_comparison;
pub mod players;
pub mod stats;
pub mod vehicles;

use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

use crate::api::ApiState;

/// Public player details shared by API responses.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PlayerSummary {
    pub id: i64,
    pub lfs_username: String,
    pub display_name: String,
    #[schema(example = "GB", required)]
    pub country_code: Option<String>,
}

impl From<crate::models::players::PlayerModel> for PlayerSummary {
    fn from(player: crate::models::players::PlayerModel) -> Self {
        Self {
            id: player.id,
            lfs_username: player.lfs_username,
            display_name: player.display_name,
            country_code: player
                .country_code
                .map(|country_code| country_code.as_str().to_owned()),
        }
    }
}

/// Builds the version 1 API routes.
pub(crate) fn router(max_spr_upload_bytes: usize) -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .merge(countries::router())
        .merge(eras::router(max_spr_upload_bytes))
        .merge(hotlaps::router())
        .merge(me::router())
        .merge(players::router())
        .merge(stats::router())
        .merge(player_comparison::router())
        .merge(vehicles::router())
}
