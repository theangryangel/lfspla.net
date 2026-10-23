use super::{error::Error, version::load};
use crate::api::extractors::pubstat::HlQuery;
use crate::{
    api::ApiError,
    models::{Era, Player, hotlap::pubstat::Filters},
};
use lfsplanet_pubstat::{Version, hl::Hotlaps};
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
