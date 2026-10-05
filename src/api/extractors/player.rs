//! Resolution of the player named by an API route.

use crate::api::{ApiError, ApiState};
use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

/// A player resolved from the route's `{lfs_username}` path parameter.
pub(crate) struct Player(pub(crate) crate::models::Player);

#[derive(Deserialize)]
struct PlayerParameter {
    lfs_username: String,
}

impl FromRequestParts<ApiState> for Player {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let Path(PlayerParameter { lfs_username }) = Path::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        crate::models::Player::find_by_username(&state.database, &lfs_username)
            .await
            .map_err(ApiError::database)
            .and_then(|player| {
                player.ok_or_else(|| ApiError::not_found("player_not_found", "Player"))
            })
            .map(Self)
            .map_err(IntoResponse::into_response)
    }
}
