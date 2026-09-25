//! Public API errors and their wire representation.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use validator::ValidationErrors;

/// Machine-readable error envelope.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub(super) error: ErrorBody,
}

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ErrorBody {
    pub(super) code: &'static str,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) details: Option<Value>,
}

/// HTTP error with a stable client-facing code.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ApiError {
    pub(super) status: StatusCode,
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) details: Option<Value>,
}

// Accepts errors by value for use with `map_err`.
#[allow(
    clippy::needless_pass_by_value,
    reason = "by-value is required for point-free map_err"
)]
impl ApiError {
    /// Creates a client-facing HTTP error.
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    pub(crate) fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// A failure the caller cannot act on and must not see the detail of.
    ///
    /// `what` names the failing subsystem for the log only; the response body
    /// is deliberately identical for every internal failure.
    pub(crate) fn internal(error: &dyn std::fmt::Debug, what: &'static str) -> Self {
        tracing::error!(?error, "{what} failed");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The request could not be completed",
        )
    }

    /// A missing resource, named by the noun the caller asked for.
    pub(crate) fn not_found(code: &'static str, noun: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, format!("{noun} was not found"))
    }

    pub(crate) fn replay_not_available() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "replay_not_available",
            "A replay is not available for this hotlap",
        )
    }

    pub(crate) fn invalid_upload(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_upload", message)
    }

    pub(crate) fn unsupported_track() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_track",
            "The replay's track is not available in this era",
        )
    }

    pub(crate) fn unsupported_combination() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_combination",
            "The replay's track and vehicle combination is not available in this era",
        )
    }

    pub(crate) fn hotlap_queue_full(outstanding: u64, limit: u64) -> Self {
        let replay_label = if outstanding == 1 {
            "replay"
        } else {
            "replays"
        };
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "hotlap_queue_full",
            format!(
                "You already have {outstanding} {replay_label} awaiting processing (limit {limit}). Wait for one to finish or remove one before uploading another."
            ),
        )
    }

    pub(crate) fn uploads_banned() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "uploads_banned",
            "You are not permitted to upload replays",
        )
    }

    pub(crate) fn database(error: sea_orm::DbErr) -> Self {
        Self::internal(&error, "database request")
    }

    pub(crate) fn session(error: tower_sessions::session::Error) -> Self {
        Self::internal(&error, "session request")
    }

    pub(crate) fn authentication(error: impl std::fmt::Debug) -> Self {
        tracing::warn!(?error, "LFS authentication failed");
        Self::new(
            StatusCode::BAD_GATEWAY,
            "oauth_provider_error",
            "LFS authentication could not be completed",
        )
    }

    fn storage(error: &object_store::Error, message: &'static str) -> Self {
        tracing::error!(?error, "object store request failed");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "storage_error", message)
    }

    pub(crate) fn object_store(error: object_store::Error) -> Self {
        Self::storage(&error, "The replay could not be stored")
    }

    pub(crate) fn object_store_read(error: object_store::Error) -> Self {
        Self::storage(&error, "The replay could not be retrieved")
    }
}

impl From<ValidationErrors> for ApiError {
    fn from(errors: ValidationErrors) -> Self {
        let mut details = serde_json::to_value(errors).unwrap_or_else(|error| {
            tracing::error!(?error, "validation errors could not be serialized");
            serde_json::json!({})
        });
        remove_rejected_values(&mut details);

        Self::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Request validation failed",
        )
        .with_details(details)
    }
}

fn remove_rejected_values(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(params)) = object.get_mut("params") {
                params.remove("value");
            }
            for nested in object.values_mut() {
                remove_rejected_values(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                remove_rejected_values(nested);
            }
        }
        _ => {}
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                    details: self.details,
                },
            }),
        )
            .into_response()
    }
}
