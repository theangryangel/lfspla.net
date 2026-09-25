//! axum router assembly and HTTP boundary models.

pub mod v1;

mod auth;
mod error;
mod pagination;
mod state;

pub(crate) mod extractors;

pub(crate) use crate::models::Ordering;
pub use error::{ApiError, ErrorResponse};
pub(crate) use pagination::{ListResponse, PaginatedResponse, PaginationQuery};
pub use state::ApiState;

/// Default number of results returned by paginated API endpoints.
pub(crate) const PER_PAGE: u32 = 50;

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    http::{HeaderValue, StatusCode, header},
    routing::get,
};
use sea_orm::DatabaseConnection;
use time::Duration;
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate},
    },
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{Key, SameSite},
};
use tower_sessions_sqlx_store::PostgresStore;
use utoipa::openapi::ContactBuilder;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;

use crate::{cli::Args, lfs::api_client, settings::Settings, startup, storage};
use lfsplanet_lfs_api::OAuthProvider;

/// Runs the web application until a shutdown signal is received.
pub async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = startup::connect(&settings.database, 10).await?;

    let app = application(database, &settings)?;
    let listener = tokio::net::TcpListener::bind(settings.web.listen)
        .await
        .with_context(|| format!("failed to listen on {}", settings.web.listen))?;

    tracing::info!(listen = %settings.web.listen, "API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(?error, "failed to install shutdown signal handler");
    }
}

/// Builds the public routes and their OpenAPI document.
fn parts(max_spr_upload_bytes: usize) -> (Router<ApiState>, utoipa::openapi::OpenApi) {
    let (v1_router, mut openapi) = v1::router(max_spr_upload_bytes).split_for_parts();
    let router = Router::new().merge(v1_router);

    openapi.info.title = "lfspla.net API".into();
    openapi.info.version = env!("CARGO_PKG_VERSION").into();
    openapi.info.description = Some(
        "Public HTTP API for lfspla.net, a community hotlap leaderboard for \
         Live for Speed.\n\nAnonymous access is read-only. \
         Writing requires a session cookie or a personal access token."
            .into(),
    );
    openapi.info.contact = Some(
        ContactBuilder::new()
            .name("lfspla.net contributors".into())
            .url(env!("CARGO_PKG_HOMEPAGE").into())
            .build(),
    );
    let components = openapi.components.get_or_insert_default();
    components.add_security_scheme(
        "cookie_session",
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("lfsplanet.sid"))),
    );
    components.add_security_scheme(
        "personal_access_token",
        SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
    );

    (router, openapi)
}

/// Generates the public API document without starting the HTTP server.
pub(crate) fn openapi() -> utoipa::openapi::OpenApi {
    parts(crate::settings::HotlapSettings::default().max_spr_upload_bytes()).1
}

/// Builds the public routes, including the generated OpenAPI document and Swagger UI.
pub fn router(max_spr_upload_bytes: usize) -> Router<ApiState> {
    let (router, openapi) = parts(max_spr_upload_bytes);
    // Disable caching by default: responses may contain private data or tokens.
    router
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", openapi))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

/// Wires the public routes to runtime dependencies and middleware.
pub fn application(database: DatabaseConnection, settings: &Settings) -> anyhow::Result<Router> {
    let object_store = storage::build(&settings.storage)?;
    let oauth = api_client(&settings.lfs)?
        .map(|client| OAuthProvider::new(client, settings.web.oauth_callback_url().to_string()))
        .transpose()?;
    let state = ApiState {
        database: database.clone(),
        oauth: oauth.clone(),
        object_store,
        hotlaps: settings.hotlaps.clone(),
    };

    // Share SeaORM's SQLx 0.9 connection pool with the session store.
    let session_store = PostgresStore::new(database.get_postgres_connection_pool().clone());
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("lfsplanet.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_path("/")
        .with_secure(settings.web.cookie_secure)
        .with_expiry(Expiry::OnInactivity(Duration::days(7)))
        .with_private(Key::from(settings.web.session_key.as_slice()));
    let app = router(settings.hotlaps.max_spr_upload_bytes())
        .merge(auth::router())
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<Body>| {
                // Deliberately omit the query string: OAuth callback URLs carry
                // short-lived authorization codes and state values there.
                tracing::debug_span!(
                    "http_request",
                    method = %request.method(),
                    path = %request.uri().path(),
                    version = ?request.version(),
                )
            }),
        )
        .layer(session_layer)
        .layer(CompressionLayer::new().compress_when(compressible()))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            settings.web.http_request_timeout.duration(),
        ))
        .with_state(state);

    Ok(app)
}

/// Skip compression for replay downloads to keep Content-Length for progress
/// reporting. Caddy serves the frontend's precompressed files separately.
fn compressible() -> impl Predicate {
    DefaultPredicate::new().and(NotForContentType::const_new("application/octet-stream"))
}

async fn live() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn ready(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Result<StatusCode, ApiError> {
    state.database.ping().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compresses(content_type: &str) -> bool {
        let response = axum::http::Response::builder()
            .header(axum::http::header::CONTENT_TYPE, content_type)
            .body(axum::body::Body::from("x".repeat(1024)))
            .expect("response builds");
        compressible().should_compress(&response)
    }

    #[test]
    fn json_is_compressed_but_replay_downloads_keep_their_length() {
        assert!(compresses("application/json"));
        // Compressing would strip Content-Length and Accept-Ranges, costing
        // downloads their progress reporting and resumability.
        assert!(!compresses("application/octet-stream"));
    }

    #[test]
    fn every_operation_id_is_unique() {
        const METHODS: [&str; 7] = ["get", "put", "post", "delete", "options", "head", "patch"];

        let (_, document) =
            parts(crate::settings::HotlapSettings::default().max_spr_upload_bytes());
        let mut seen = std::collections::HashMap::new();

        for (path, item) in document.paths.paths {
            let item = serde_json::to_value(item).expect("path item serialises");
            for method in METHODS {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let id = operation["operationId"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{method} {path} has no operationId"));
                if let Some(previous) = seen.insert(id.to_owned(), format!("{method} {path}")) {
                    panic!("operationId `{id}` is used by both {previous} and {method} {path}");
                }
            }
        }
    }
    #[test]
    fn collection_schemas_and_page_parameters_match_the_wire_contract() {
        let (_, document) =
            parts(crate::settings::HotlapSettings::default().max_spr_upload_bytes());
        let document = serde_json::to_value(document).unwrap();
        let paths = document["paths"].as_object().unwrap();
        for (path, item) in paths {
            let Some(schema) = item.pointer("/get/responses/200/content/application~1json/schema")
            else {
                continue;
            };
            let schema = schema["$ref"].as_str().map_or(schema, |reference| {
                document
                    .pointer(reference.strip_prefix('#').unwrap())
                    .unwrap()
            });
            assert_ne!(schema["type"], "array", "{path} returns a bare array");
        }
        for path in [
            "/api/v1/eras/{era}/charts/{track}/{vehicle}",
            "/api/v1/eras/{era}/combinations",
            "/api/v1/hotlaps",
        ] {
            let parameters = paths[path]["get"]["parameters"].as_array().unwrap();
            let names: Vec<_> = parameters
                .iter()
                .map(|parameter| parameter["name"].as_str().unwrap())
                .collect();
            assert!(names.contains(&"page"), "{path}");
            assert!(names.contains(&"per_page"), "{path}");
            assert!(!names.contains(&"cursor"), "{path}");
            assert!(!names.contains(&"limit"), "{path}");
        }
        for path in ["/api/v1/countries", "/api/v1/eras/{era}/tracks"] {
            for parameter in paths[path]["get"]["parameters"].as_array().unwrap() {
                assert!(
                    !["page", "per_page", "cursor", "limit"]
                        .contains(&parameter["name"].as_str().unwrap()),
                    "{path}"
                );
            }
        }
    }
}
