//! Resources belonging to the currently authenticated player.

mod personal_access_tokens;
mod webhook_notifications;
mod webhooks;

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse,
        extractors::{AuthenticatedPlayer, BrowserAuthenticatedPlayer, csrf_token},
        v1::PlayerSummary,
    },
    country::CountryCode,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
};
use celes::Country;
use lfsplanet_flags::{CountryFlagsExt, FlagCode};

use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

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
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    #[schema(value_type = Option<String>)]
    country_code: Option<Option<String>>,
    /// FlagCDN code. Null uses the country flag; omitted keeps the current choice.
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    #[schema(value_type = Option<String>)]
    flag_code: Option<Option<String>>,
}

// Missing leaves the field unchanged; null clears it.
fn deserialize_patch_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
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
    responses((status = 200, description = "Current authentication and frontend state", body = MeResponse))
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
        (status = 400, description = "Invalid country or display flag", body = ErrorResponse),
        (status = 401, description = "Browser authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse)
    )
)]
pub(crate) async fn update(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<UpdateMeRequest>,
) -> Result<Json<PlayerSummary>, ApiError> {
    let (country_code, flag_code) =
        request.resolve(player.country_code.map(|code| code.0), player.flag_code)?;
    let updated = player
        .update_preferences(&state.database, country_code.map(CountryCode), flag_code)
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

impl UpdateMeRequest {
    fn resolve(
        self,
        current_country: Option<Country>,
        current_flag: Option<FlagCode>,
    ) -> Result<(Option<Country>, Option<FlagCode>), ApiError> {
        let country_code = match self.country_code {
            None => current_country,
            Some(code) => code
                .as_deref()
                .map(str::trim)
                .map(|code| Country::from_alpha2(code.to_ascii_uppercase()))
                .transpose()
                .map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_country_code",
                        "Choose a valid country code",
                    )
                })?,
        };
        let flag_code = match self.flag_code {
            Some(Some(code)) => {
                let flag = FlagCode::parse(&code)
                    .filter(|flag| country_code.is_some_and(|country| country.allows_flag(*flag)))
                    .ok_or_else(|| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_flag_code",
                            "Choose a flag for your country",
                        )
                    })?;
                Some(flag)
            }
            Some(None) => None,
            None => current_flag
                .filter(|flag| country_code.is_some_and(|country| country.allows_flag(*flag))),
        };
        Ok((country_code, flag_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn patch_distinguishes_missing_and_null() {
        let gb = Some(Country::from_alpha2("GB").unwrap());
        let request: UpdateMeRequest = serde_json::from_value(json!({})).unwrap();
        assert_eq!(
            request.resolve(gb, FlagCode::parse("gb-sct")).unwrap(),
            (gb, FlagCode::parse("gb-sct"))
        );
        let request: UpdateMeRequest = serde_json::from_value(json!({"flag_code": null})).unwrap();
        assert_eq!(
            request.resolve(gb, FlagCode::parse("gb-sct")).unwrap(),
            (gb, None)
        );
        let request: UpdateMeRequest =
            serde_json::from_value(json!({"country_code": null})).unwrap();
        assert_eq!(
            request.resolve(gb, FlagCode::parse("gb-sct")).unwrap(),
            (None, None)
        );
    }

    #[test]
    fn country_change_keeps_only_allowed_flags() {
        let gb = Some(Country::from_alpha2("GB").unwrap());
        for (old_flag, expected) in [("gb-sct", None), ("un", FlagCode::parse("un"))] {
            let request: UpdateMeRequest =
                serde_json::from_value(json!({"country_code": "NO"})).unwrap();
            let (country, flag) = request.resolve(gb, FlagCode::parse(old_flag)).unwrap();
            assert_eq!(country.unwrap().alpha2, "NO");
            assert_eq!(flag, expected);
        }
    }

    #[test]
    fn validates_flags_for_the_new_country() {
        for value in [
            json!({"country_code": "US", "flag_code": "gb-sct"}),
            json!({"country_code": "US", "flag_code": "eu"}),
            json!({"country_code": "GB", "flag_code": "eu"}),
            json!({"flag_code": "eu"}),
            json!({"country_code": "GB", "flag_code": "invalid"}),
            json!({"country_code": "invalid"}),
        ] {
            let request: UpdateMeRequest = serde_json::from_value(value).unwrap();
            assert!(request.resolve(None, None).is_err());
        }
        let request: UpdateMeRequest =
            serde_json::from_value(json!({"country_code": "gb", "flag_code": "GB-SCT"})).unwrap();
        let (country, flag) = request.resolve(None, None).unwrap();
        assert_eq!(country.unwrap().alpha2, "GB");
        assert_eq!(flag.map(FlagCode::as_str), Some("gb-sct"));
    }
}
