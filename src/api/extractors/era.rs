//! Resolution of the era named by an API route.

use crate::api::{ApiError, ApiState};
use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

/// An era resolved from the route's `{era}` path parameter.
pub(crate) struct Era(pub(crate) crate::models::Era);

#[derive(Deserialize)]
struct EraParameter {
    era: String,
}

impl FromRequestParts<ApiState> for Era {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let Path(EraParameter { era }) = Path::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        resolve_era(&state.database, &era)
            .await
            .map(Self)
            .map_err(IntoResponse::into_response)
    }
}

/// Loads one era and maps persistence outcomes to the public API contract.
///
/// This deliberately avoids loading the whole catalogue: the schema enforces
/// the structural invariants, while operator writes validate the remaining
/// cross-era invariants under the catalogue advisory lock.
pub(crate) async fn resolve_era(
    database: &DatabaseConnection,
    id: &str,
) -> Result<crate::models::Era, ApiError> {
    let slug = id
        .parse::<crate::era_slug::EraSlug>()
        .map_err(|_| ApiError::not_found("era_not_found", "Era"))?;
    crate::models::Era::find_by_slug(database, &slug)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("era_not_found", "Era"))
}
