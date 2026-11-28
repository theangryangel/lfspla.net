use super::{error::Error, version::load};
use crate::api::ApiError;
use crate::api::extractors::pubstat::HlQuery;
use lfsplanet_core::models::Player;
use lfsplanet_hotlaps::models::{Era, hotlap::pubstat::Filters};
use lfsplanet_pubstat_types::{Version, hl::Hotlaps};
use sea_orm::DatabaseConnection;

pub(super) async fn get(
    version: Version,
    database: &DatabaseConnection,
    era: &Era,
    query: HlQuery,
) -> Result<Hotlaps, Error> {
    Player::find_by_username(database, &query.racer)
        .await?
        .ok_or_else(|| ApiError::not_found("player_not_found", "Player"))?;
    let rows = load(
        database,
        era,
        Filters {
            racer: Some(&query.racer),
            ..Filters::default()
        },
    )
    .await?;
    Ok(Hotlaps::new(version, rows))
}
