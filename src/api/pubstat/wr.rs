use super::{error::Error, version::load};
use crate::api::extractors::pubstat::WrQuery;
use crate::models::{Era, hotlap::pubstat::Filters};
use lfsplanet_pubstat::{Version, wr::WorldRecords};
use sea_orm::DatabaseConnection;

pub(super) async fn get(
    version: Version,
    database: &DatabaseConnection,
    era: &Era,
    query: WrQuery,
) -> Result<WorldRecords, Error> {
    let rows = load(
        database,
        era,
        Filters {
            track: query.track,
            car: query.car,
            leaders_only: true,
            ..Filters::default()
        },
    )
    .await?;
    Ok(WorldRecords::new(version, rows))
}
