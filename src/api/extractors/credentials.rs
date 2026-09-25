//! Session and bearer-token authentication.
//!
//! Cookie-authenticated writes require a CSRF token. Each extractor checks
//! this from the request method.

use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use oauth2::CsrfToken;
use sea_orm::DatabaseConnection;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use subtle::ConstantTimeEq;
use tower_sessions::Session;

use crate::{
    api::{ApiError, ApiState},
    models::{
        personal_access_tokens,
        players::{PlayerColumn, PlayerEntity, PlayerModel},
    },
};

const CSRF_TOKEN_KEY: &str = "csrf_token";
const PLAYER_ID_KEY: &str = "player_id";

macro_rules! extractor {
    ($extractor:ty) => {
        impl FromRequestParts<ApiState> for $extractor {
            type Rejection = Response;

            async fn from_request_parts(
                parts: &mut Parts,
                state: &ApiState,
            ) -> Result<Self, Self::Rejection> {
                Self::extract(parts, state)
                    .await
                    .map_err(IntoResponse::into_response)
            }
        }
    };
}

/// The player behind a session cookie or a personal access token.
pub(crate) struct AuthenticatedPlayer(pub(crate) PlayerModel);

impl AuthenticatedPlayer {
    async fn extract(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        let session = session(parts, state).await?;
        let authenticated = resolve(&parts.headers, &session, &state.database).await?;
        guard_csrf(parts, state, authenticated.credential).await?;
        Ok(Self(authenticated.player))
    }

    async fn optional_extract(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Option<Self>, ApiError> {
        let session = session(parts, state).await?;
        if parts.headers.get(header::AUTHORIZATION).is_none() {
            let Some(player) = browser_player(&session, &state.database).await? else {
                return Ok(None);
            };
            guard_csrf(parts, state, Credential::BrowserSession).await?;
            return Ok(Some(Self(player)));
        }
        let authenticated = resolve(&parts.headers, &session, &state.database).await?;
        guard_csrf(parts, state, authenticated.credential).await?;
        Ok(Some(Self(authenticated.player)))
    }
}

extractor!(AuthenticatedPlayer);

impl OptionalFromRequestParts<ApiState> for AuthenticatedPlayer {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Option<Self>, Self::Rejection> {
        Self::optional_extract(parts, state)
            .await
            .map_err(IntoResponse::into_response)
    }
}

/// Requires browser authentication. A leaked API token cannot create more tokens.
pub(crate) struct BrowserAuthenticatedPlayer(pub(crate) PlayerModel);

impl BrowserAuthenticatedPlayer {
    async fn extract(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        let session = session(parts, state).await?;
        let player = browser_player(&session, &state.database)
            .await?
            .ok_or_else(authentication_required)?;
        guard_csrf(parts, state, Credential::BrowserSession).await?;
        Ok(Self(player))
    }

    async fn optional_extract(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Option<Self>, ApiError> {
        let session = session(parts, state).await?;
        let Some(player) = browser_player(&session, &state.database).await? else {
            return Ok(None);
        };
        guard_csrf(parts, state, Credential::BrowserSession).await?;
        Ok(Some(Self(player)))
    }
}

extractor!(BrowserAuthenticatedPlayer);

impl OptionalFromRequestParts<ApiState> for BrowserAuthenticatedPlayer {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Option<Self>, Self::Rejection> {
        Self::optional_extract(parts, state)
            .await
            .map_err(IntoResponse::into_response)
    }
}

/// The credential used to authenticate the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Credential {
    BrowserSession,
    PersonalAccessToken,
}

/// The authenticated player and credential type.
struct Authenticated {
    player: PlayerModel,
    credential: Credential,
}

/// Resolves whichever credential the request carries into its player.
async fn resolve(
    headers: &HeaderMap,
    session: &Session,
    database: &DatabaseConnection,
) -> Result<Authenticated, ApiError> {
    let Some(token) = bearer_token(headers)? else {
        return Ok(Authenticated {
            player: browser_player(session, database)
                .await?
                .ok_or_else(authentication_required)?,
            credential: Credential::BrowserSession,
        });
    };
    let player = personal_access_tokens::authenticate(database, token)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(invalid_access_token)?;
    Ok(Authenticated {
        player,
        credential: Credential::PersonalAccessToken,
    })
}

/// Checks CSRF for cookie-authenticated writes. Browsers send cookies
/// automatically, so the request must also supply the session's CSRF token.
async fn guard_csrf(
    parts: &mut Parts,
    state: &ApiState,
    credential: Credential,
) -> Result<(), ApiError> {
    if !needs_csrf(&parts.method, credential) {
        return Ok(());
    }
    let session = session(parts, state).await?;
    verify_csrf(&parts.headers, &session).await
}

fn needs_csrf(method: &axum::http::Method, credential: Credential) -> bool {
    !method.is_safe() && credential == Credential::BrowserSession
}

async fn session(parts: &mut Parts, state: &ApiState) -> Result<Session, ApiError> {
    Session::from_request_parts(parts, state)
        .await
        .map_err(|(_, reason)| ApiError::internal(&reason, "session"))
}

pub(crate) async fn browser_player(
    session: &Session,
    database: &DatabaseConnection,
) -> Result<Option<PlayerModel>, ApiError> {
    let Some(player_id) = session
        .get::<i64>(PLAYER_ID_KEY)
        .await
        .map_err(ApiError::session)?
    else {
        return Ok(None);
    };
    PlayerEntity::find_by_id(player_id)
        .filter(PlayerColumn::DenyAuth.eq(false))
        .one(database)
        .await
        .map_err(ApiError::database)
}

pub(crate) async fn login(session: &Session, player_id: i64) -> Result<(), ApiError> {
    session.cycle_id().await.map_err(ApiError::session)?;
    session
        .insert(PLAYER_ID_KEY, player_id)
        .await
        .map_err(ApiError::session)
}

fn authentication_required() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "authentication_required",
        "Authentication is required",
    )
}

fn bearer_token(headers: &HeaderMap) -> Result<Option<&str>, ApiError> {
    let Some(authorization) = headers.get(header::AUTHORIZATION) else {
        return Ok(None);
    };
    let authorization = authorization.to_str().map_err(|_| invalid_access_token())?;
    let (scheme, token) = authorization
        .split_once(' ')
        .ok_or_else(invalid_access_token)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return Err(invalid_access_token());
    }
    Ok(Some(token))
}

fn invalid_access_token() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_access_token",
        "The personal access token is missing, invalid, expired, or revoked",
    )
}

/// Returns the session's CSRF token, minting one if it has none.
pub(crate) async fn csrf_token(session: &Session) -> Result<String, ApiError> {
    if let Some(token) = session
        .get::<String>(CSRF_TOKEN_KEY)
        .await
        .map_err(ApiError::session)?
    {
        return Ok(token);
    }

    let token = CsrfToken::new_random().secret().clone();
    session
        .insert(CSRF_TOKEN_KEY, &token)
        .await
        .map_err(ApiError::session)?;
    Ok(token)
}

/// Replaces the session's CSRF token, so that a token observed before sign-in
/// cannot be replayed against the session that sign-in established.
pub(crate) async fn reset_csrf_token(session: &Session) -> Result<(), ApiError> {
    session
        .insert(CSRF_TOKEN_KEY, CsrfToken::new_random().secret())
        .await
        .map_err(ApiError::session)
}

/// Requires the request to echo the session's CSRF token.
pub(crate) async fn verify_csrf(headers: &HeaderMap, session: &Session) -> Result<(), ApiError> {
    let expected = session
        .get::<String>(CSRF_TOKEN_KEY)
        .await
        .map_err(ApiError::session)?;
    let provided = headers
        .get("x-csrf-token")
        .and_then(|header| header.to_str().ok());

    if !secrets_match(expected.as_deref(), provided) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_csrf_token",
            "CSRF token is missing or invalid",
        ));
    }
    Ok(())
}

/// Compares a stored secret with a supplied one in constant time.
///
/// Used for both the CSRF token and the OAuth `state`: both are secrets the
/// caller echoes back, so both get the same comparison.
pub(crate) fn secrets_match(expected: Option<&str>, provided: Option<&str>) -> bool {
    let (Some(expected), Some(provided)) = (expected, provided) else {
        return false;
    };
    if expected.is_empty() {
        return false;
    }
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tower_sessions::{MemoryStore, Session};

    #[test]
    fn csrf_requires_matching_non_empty_tokens() {
        assert!(secrets_match(Some("token"), Some("token")));
        assert!(!secrets_match(None, None));
        assert!(!secrets_match(None, Some("")));
        assert!(!secrets_match(None, Some("token")));
        assert!(!secrets_match(Some("token"), None));
        assert!(!secrets_match(Some(""), Some("")));
        assert!(!secrets_match(Some("token"), Some("other")));
        // Differing lengths must not match either.
        assert!(!secrets_match(Some("token"), Some("token-longer")));
    }

    #[tokio::test]
    async fn browser_login_rotates_the_session_id_and_preserves_oauth_state() {
        let store = Arc::new(MemoryStore::default());
        let session = Session::new(None, store, None);
        session.insert("oauth_state", "pending").await.unwrap();
        session.save().await.unwrap();
        let before_login = session.id();

        login(&session, 42).await.unwrap();
        assert_eq!(
            session.get::<String>("oauth_state").await.unwrap(),
            Some("pending".into())
        );
        assert_eq!(session.get::<i64>(PLAYER_ID_KEY).await.unwrap(), Some(42));
        session.save().await.unwrap();

        assert_ne!(session.id(), before_login);
    }

    #[test]
    fn csrf_is_required_of_exactly_the_forgeable_requests() {
        use axum::http::Method;

        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(needs_csrf(&method, Credential::BrowserSession), "{method}");
            // A browser never attaches an `Authorization` header of its own
            // accord, so a token-authenticated request is not forgeable.
            assert!(
                !needs_csrf(&method, Credential::PersonalAccessToken),
                "{method}"
            );
        }

        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert!(!needs_csrf(&method, Credential::BrowserSession), "{method}");
            assert!(
                !needs_csrf(&method, Credential::PersonalAccessToken),
                "{method}"
            );
        }
    }

    #[test]
    fn bearer_authorization_is_parsed_strictly() {
        let token = format!("lfspla_{}", "a".repeat(64));
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers).unwrap(), None);

        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        assert_eq!(bearer_token(&headers).unwrap(), Some(token.as_str()));

        for value in [
            format!("Basic {token}"),
            format!("Bearer  {token}"),
            "Bearer".to_owned(),
            "Bearer ".to_owned(),
        ] {
            headers.insert(header::AUTHORIZATION, value.parse().unwrap());
            assert!(bearer_token(&headers).is_err());
        }
    }
}
