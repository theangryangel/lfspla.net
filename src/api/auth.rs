//! Browser authentication and session lifecycle routes.

use crate::api::{
    ApiError, ApiState, ErrorResponse,
    extractors::{browser_player, login, reset_csrf_token, secrets_match, verify_csrf},
};
use crate::models::Player;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;
use tower_sessions::Session;
use utoipa_axum::{router::OpenApiRouter, routes};
const OAUTH_STATE_KEY: &str = "oauth_state";

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .route("/auth/lfs", get(start))
        .route("/auth/lfs/callback", get(callback))
        .routes(routes!(logout))
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
    let account = provider
        .authenticate(&query.code)
        .await
        .map_err(ApiError::authentication)?;
    let player = Player::record_authentication(&state.database, &account)
        .await
        .map_err(ApiError::authentication)?;
    if player.deny_auth {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "oauth_authentication_failed",
            "LFS authentication was not accepted",
        ));
    }
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
#[utoipa::path(
    post,
    path = "/auth/logout",
    operation_id = "logout",
    tag = "authentication",
    security(("cookie_session" = [])),
    params(("X-CSRF-Token" = String, Header, description = "Session CSRF token")),
    responses(
        (status = 204, description = "Session cleared"),
        (status = 403, description = "CSRF token is missing or invalid", body = ErrorResponse),
        (status = 500, description = "Session or database failure", body = ErrorResponse)
    )
)]
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
