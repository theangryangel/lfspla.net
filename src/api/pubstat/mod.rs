//! Era-scoped LFSWorld Pubstat compatibility boundary.
mod ch;
pub(super) mod error;
mod hl;
mod response;
mod version;
mod wr;

use crate::api::{
    ApiError, ApiState,
    extractors::{
        self,
        pubstat::{Action, PubstatQuery},
    },
};
use axum::{Router, extract::State, response::Response, routing::get};
use error::Error;
use response::respond;
use version::PubstatVersionExt;

pub(super) fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/pubstat", get(reference))
        .route("/api/pubstat/{era}/get_stat2.php", get(dispatch))
}

async fn reference() -> &'static str {
    include_str!("../../../docs/pubstat/reference.txt")
}

async fn dispatch(
    PubstatQuery {
        version,
        format,
        action,
        compression,
    }: PubstatQuery,
    era: Result<extractors::Era, ApiError>,
    State(state): State<ApiState>,
) -> Result<Response, Error> {
    let extractors::Era(era) = era.map_err(Error::from)?;
    match action {
        Action::Hotlaps(query) => respond(
            format,
            compression,
            &version.hl(&state.database, &era, query).await?,
        ),
        Action::Chart(query) => respond(
            format,
            compression,
            &version.ch(&state.database, &era, query).await?,
        ),
        Action::WorldRecords(query) => respond(
            format,
            compression,
            &version.wr(&state.database, &era, query).await?,
        ),
        action => Err(action.rejection()),
    }
}

#[cfg(test)]
mod tests;
