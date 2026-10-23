//! Pubstat has a separate, plain-text error boundary.
use crate::api::ApiError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(crate) struct Error {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl Error {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request",
            message: message.into(),
        }
    }
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            code: "unsupported",
            message: message.into(),
        }
    }
    pub fn internal(error: &impl std::fmt::Debug) -> Self {
        ApiError::internal(error, "Pubstat").into()
    }
}
impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Self {
            status: error.status,
            code: error.code,
            message: error.message,
        }
    }
}
impl From<sea_orm::DbErr> for Error {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::internal(&error)
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.status, format!("{}: {}\n", self.code, self.message)).into_response()
    }
}
