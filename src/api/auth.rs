//! Browser authentication and session lifecycle routes.

use crate::api::{
    ApiError, ApiState,
    extractors::{browser_player, login, reset_csrf_token, secrets_match, verify_csrf},
};
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;

const OAUTH_STATE_KEY: &str = "oauth_state";

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

pub(crate) fn router() -> Router<ApiState> {
    Router::new()
        .route("/auth/lfs", get(start))
        .route("/auth/lfs/callback", get(callback))
        .route("/auth/logout", post(logout))
}

/// Redirects the browser to LFS authorization.
async fn start(State(state): State<ApiState>, session: Session) -> Result<Response, ApiError> {
    if browser_player(&session, &state.database).await?.is_some() {
        return Ok(Redirect::to("/").into_response());
    }

    let provider = state.oauth.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "oauth_not_configured",
            "LFS authentication is not configured",
        )
    })?;
    let (authorization_url, csrf_state) = provider.authorize_url();

    session
        .insert(OAUTH_STATE_KEY, csrf_state.secret())
        .await
        .map_err(ApiError::session)?;

    Ok(Redirect::to(authorization_url.as_str()).into_response())
}

/// Completes LFS OAuth and establishes an authenticated session.
async fn callback(
    State(state): State<ApiState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
) -> Result<Redirect, ApiError> {
    let expected_state = session
        .remove::<String>(OAUTH_STATE_KEY)
        .await
        .map_err(ApiError::session)?;
    if !secrets_match(expected_state.as_deref(), Some(query.state.as_str())) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_oauth_state",
            "OAuth state is invalid or has expired",
        ));
    }

    let provider = state.oauth.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "oauth_not_configured",
            "LFS authentication is not configured",
        )
    })?;
    let player = crate::auth::authenticate(&state.database, provider, &query.code)
        .await
        .map_err(ApiError::authentication)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "oauth_authentication_failed",
                "LFS authentication was not accepted",
            )
        })?;
    login(&session, player.id).await?;
    reset_csrf_token(&session).await?;
    tracing::info!(
        player_id = player.id,
        lfs_username = %player.lfs_username,
        "LFS session authenticated"
    );

    Ok(Redirect::to("/"))
}

/// Clears an API client's authenticated session.
async fn logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
    session: Session,
) -> Result<StatusCode, ApiError> {
    verify_csrf(&headers, &session).await?;
    let player = browser_player(&session, &state.database).await?;
    session.flush().await.map_err(ApiError::session)?;
    if let Some(player) = player {
        tracing::info!(
            player_id = player.id,
            lfs_username = %player.lfs_username,
            "LFS session logged out"
        );
    }
    Ok(StatusCode::NO_CONTENT)
}
