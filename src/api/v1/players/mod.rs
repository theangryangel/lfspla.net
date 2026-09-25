//! Public player profiles and competitive summaries.

mod detail;
pub(super) mod response;
mod search;

use utoipa_axum::{router::OpenApiRouter, routes};

use crate::api::ApiState;

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(search::search))
        .routes(routes!(detail::detail))
}
