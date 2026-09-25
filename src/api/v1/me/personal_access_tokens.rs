//! Browser-managed personal access tokens for API clients.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::{ModelTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, ListResponse, extractors::BrowserAuthenticatedPlayer,
    },
    models::personal_access_tokens::{
        self, CreatePersonalAccessToken, PersonalAccessTokenColumn, PersonalAccessTokenEntity,
        PersonalAccessTokenModel,
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct CreatePersonalAccessTokenRequest {
    /// Human-readable purpose shown in the token list.
    name: String,
    /// Lifetime from creation, from 1 to 365 days. Defaults to 90.
    expires_in_days: Option<u16>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PersonalAccessTokenResponse {
    id: i64,
    name: String,
    token_hint: String,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    expires_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    last_used_at: Option<time::OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime, required)]
    revoked_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CreatedPersonalAccessTokenResponse {
    /// Full bearer token. It is returned once and cannot be retrieved later.
    token: String,
    credential: PersonalAccessTokenResponse,
}

#[utoipa::path(
    get,
    path = "/api/v1/me/personal-access-tokens",
    operation_id = "list_personal_access_tokens",
    tag = "personal access tokens",
    security(("cookie_session" = [])),
    responses(
        (status = 200, description = "Current player's personal access tokens", body = ListResponse<PersonalAccessTokenResponse>),
        (status = 401, description = "Browser authentication required", body = ErrorResponse)
    )
)]
pub(crate) async fn list(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<Json<ListResponse<PersonalAccessTokenResponse>>, ApiError> {
    let tokens = player
        .find_related(PersonalAccessTokenEntity)
        .order_by_desc(PersonalAccessTokenColumn::CreatedAt)
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(ListResponse::from(
        tokens.into_iter().map(Into::into).collect::<Vec<_>>(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/me/personal-access-tokens",
    tag = "personal access tokens",
    security(("cookie_session" = [])),
    params(("X-CSRF-Token" = String, Header, description = "Session CSRF token")),
    request_body = CreatePersonalAccessTokenRequest,
    responses(
        (status = 201, description = "Token created; the full token is returned once", body = CreatedPersonalAccessTokenResponse),
        (status = 400, description = "Invalid token name or lifetime", body = ErrorResponse),
        (status = 401, description = "Browser authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse)
    )
)]
pub(crate) async fn create(
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
    Json(request): Json<CreatePersonalAccessTokenRequest>,
) -> Result<(StatusCode, Json<CreatedPersonalAccessTokenResponse>), ApiError> {
    let created = personal_access_tokens::issue(
        &state.database,
        player.id,
        CreatePersonalAccessToken {
            name: request.name,
            expires_in_days: request.expires_in_days,
        },
    )
    .await
    .map_err(|error| match error {
        personal_access_tokens::CreateError::Validation(error) => error.into(),
        personal_access_tokens::CreateError::Database(error) => ApiError::database(error),
    })?;

    tracing::info!(
        player_id = player.id,
        token_id = created.record.id,
        "personal access token created"
    );
    Ok((
        StatusCode::CREATED,
        Json(CreatedPersonalAccessTokenResponse {
            token: created.token,
            credential: created.record.into(),
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/me/personal-access-tokens/{token}",
    tag = "personal access tokens",
    security(("cookie_session" = [])),
    params(
        ("token" = i64, Path, description = "Personal access token identity"),
        ("X-CSRF-Token" = String, Header, description = "Session CSRF token")
    ),
    responses(
        (status = 204, description = "Token revoked"),
        (status = 401, description = "Browser authentication required", body = ErrorResponse),
        (status = 403, description = "CSRF token missing or invalid", body = ErrorResponse),
        (status = 404, description = "Token not found", body = ErrorResponse)
    )
)]
pub(crate) async fn revoke(
    Path(token_id): Path<i64>,
    State(state): State<ApiState>,
    BrowserAuthenticatedPlayer(player): BrowserAuthenticatedPlayer,
) -> Result<StatusCode, ApiError> {
    let revoked = personal_access_tokens::revoke(&state.database, player.id, token_id)
        .await
        .map_err(ApiError::database)?;
    if !revoked {
        return Err(ApiError::not_found(
            "personal_access_token_not_found",
            "Personal access token",
        ));
    }

    tracing::info!(
        player_id = player.id,
        token_id,
        "personal access token revoked"
    );
    Ok(StatusCode::NO_CONTENT)
}

impl From<PersonalAccessTokenModel> for PersonalAccessTokenResponse {
    fn from(token: PersonalAccessTokenModel) -> Self {
        Self {
            id: token.id,
            name: token.name,
            token_hint: token.token_hint,
            created_at: token.created_at,
            expires_at: token.expires_at,
            last_used_at: token.last_used_at,
            revoked_at: token.revoked_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    fn command(name: &str, expires_in_days: Option<u16>) -> CreatePersonalAccessToken {
        CreatePersonalAccessToken {
            name: name.to_owned(),
            expires_in_days,
        }
    }

    #[test]
    fn token_name_validation_rejects_unsafe_values_and_counts_characters() {
        for name in ["", " \t", "label\n"] {
            assert!(command(name, None).validate().is_err(), "{name:?}");
        }
        assert!(command(&"é".repeat(100), None).validate().is_ok());
        assert!(command(&"é".repeat(101), None).validate().is_err());
    }

    #[test]
    fn token_request_derives_all_field_validation() {
        assert!(command(" useful name ", Some(365)).validate().is_ok());
        assert!(command("useful name", Some(0)).validate().is_err());
    }
}
