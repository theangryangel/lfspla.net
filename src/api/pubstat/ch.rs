use super::{error::Error, version::load};
use crate::api::extractors::pubstat::ChartQuery;
use crate::{
    api::ApiError,
    models::{Chart as ChartModel, Era, hotlap::pubstat::Filters},
};
use lfsplanet_pubstat::{Version, ch::Chart};
use sea_orm::DatabaseConnection;

pub(super) async fn get(
    version: Version,
    database: &DatabaseConnection,
    era: &Era,
    query: ChartQuery,
) -> Result<Chart, Error> {
    ChartModel::find_combination(
        database,
        era.id,
        &query.track.to_string(),
        &query.car.to_string(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("chart_not_found", "Chart"))?;
    let rows = load(
        database,
        era,
        Filters {
            track: Some(query.track),
            car: Some(query.car),
            steering: query.control.map(Into::into),
            country: query.country.as_deref(),
            ..Filters::default()
        },
    )
    .await?;
    Ok(Chart::new(version, rows))
}
