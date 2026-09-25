//! SPR hotlap ingestion and the public hotlap resource.

mod detail;
mod list;
mod remove;
pub(super) mod replay;
pub(super) mod response;
mod validate;

use utoipa_axum::{router::OpenApiRouter, routes};

use crate::api::ApiState;

/// Hotlap routes. Handlers check ownership where needed.
/// Uploads use `/eras/{era}/hotlaps`.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list::list))
        .routes(routes!(detail::detail))
        .merge(replay::router())
        .routes(routes!(remove::remove))
        .routes(routes!(validate::validate_for_testing))
}
