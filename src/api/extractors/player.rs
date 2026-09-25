//! Resolution of the player named by an API route.

use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use sea_orm::EntityTrait;
use serde::Deserialize;

use crate::{
    api::{ApiError, ApiState},
    models::players::{self, PlayerFilter},
};

/// A player resolved from the route's `{lfs_username}` path parameter.
pub(crate) struct Player(pub(crate) players::PlayerModel);

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

        players::PlayerEntity::find()
            .with_username(&lfs_username)
            .one(&state.database)
            .await
            .map_err(ApiError::database)
            .and_then(|player| {
                player.ok_or_else(|| ApiError::not_found("player_not_found", "Player"))
            })
            .map(Self)
            .map_err(IntoResponse::into_response)
    }
}
