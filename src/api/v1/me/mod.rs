//! Resources belonging to the currently authenticated player.

mod personal_access_tokens;
mod webhook_notifications;
mod webhooks;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
};
use celes::Country;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse,
        extractors::{AuthenticatedPlayer, BrowserAuthenticatedPlayer, csrf_token},
        v1::PlayerSummary,
    },
    models::country::CountryCode,
};

/// Stable authentication-state envelope returned to the frontend.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct MeResponse {
    /// Whether a current player was found.
    authenticated: bool,
    /// Current player, or null when logged out.
    #[schema(required)]
    player: Option<PlayerSummary>,
    /// Token required in `X-CSRF-Token` for state-changing requests. Empty
    /// when logged out, because every such request requires authentication.
    csrf_token: String,
    /// Whether immediate hotlap validation is enabled for testing.
    allow_test_validation: bool,
    /// Whether the current account may submit hotlap uploads.
    allow_uploads: bool,
    /// Maximum accepted size of one uploaded SPR replay, in bytes.
    max_spr_upload_bytes: usize,
}

/// Editable personal profile fields.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct UpdateMeRequest {
    /// ISO 3166-1 alpha-2 nation code, or null to leave the nation unset.
    country_code: Option<String>,
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get))
        .routes(routes!(update))
        .routes(routes!(personal_access_tokens::list))
        .routes(routes!(personal_access_tokens::create))
        .routes(routes!(personal_access_tokens::revoke))
        .routes(routes!(webhooks::list))
        .routes(routes!(webhook_notifications::list))
        .routes(routes!(webhooks::options))
        .routes(routes!(webhooks::create))
        .routes(routes!(webhooks::update))
        .routes(routes!(webhooks::remove))
}

/// Returns ordinary logged-in or logged-out frontend state.
#[utoipa::path(
    get,
    path = "/api/v1/me",
    tag = "authentication",
    responses((status = 200, body = MeResponse))
)]
pub(crate) async fn get(
    State(state): State<ApiState>,
    headers: HeaderMap,
    player: Option<AuthenticatedPlayer>,
    session: Session,
) -> Result<Json<MeResponse>, ApiError> {
    let player = player.map(|AuthenticatedPlayer(player)| player);
    let issue_csrf_token = !headers.contains_key(header::AUTHORIZATION);
    let allow_uploads = player.as_ref().is_some_and(|player| !player.deny_uploads);
    let player = player.map(PlayerSummary::from);
    // Every route that verifies a CSRF token also requires an authenticated
    // player, so a token issued to an anonymous caller could never authorise
    // anything. Minting one would persist a session row for every logged-out
    // visitor.
    let csrf_token = if player.is_some() && issue_csrf_token {
        csrf_token(&session).await?
    } else {
        String::new()
    };

    Ok(Json(MeResponse {
        authenticated: player.is_some(),
        player,
        csrf_token,
        allow_test_validation: state.hotlaps.allow_test_validation,
        allow_uploads,
        max_spr_upload_bytes: state.hotlaps.max_spr_upload_bytes(),
    }))
}

/// Updates editable fields for the authenticated player.
#[utoipa::path(
    patch,
    path = "/api/v1/me",
    tag = "authentication",
    security(("cookie_session" = [])),
    params(("X-CSRF-Token" = String, Header, description = "Session CSRF token")),
    request_body = UpdateMeRequest,
    responses(
        (status = 200, description = "Updated authenticated player", body = PlayerSummary),
        (status = 400, description = "Invalid nation code", body = ErrorResponse),
        (status = 401, description = "Browser authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse)
    )
)]
pub(crate) async fn update(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<UpdateMeRequest>,
) -> Result<Json<PlayerSummary>, ApiError> {
    let country_code = request
        .country_code
        .as_deref()
        .map(str::trim)
        .map(Country::from_alpha2)
        .transpose()
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_country_code",
                "Choose a valid ISO 3166-1 nation code",
            )
        })?;
    // Updated from the row the session extractor already loaded, so no second
    // read is needed: sea-orm writes only the fields set below.
    let mut active = player.into_active_model();
    active.country_code = Set(country_code.map(CountryCode));
    let updated = active
        .update(&state.database)
        .await
        .map_err(ApiError::database)?;
    tracing::info!(
        player_id = updated.id,
        country_code = updated
            .country_code
            .map_or("unset", |country| country.as_str()),
        "personal profile updated"
    );
    Ok(Json(updated.into()))
}
