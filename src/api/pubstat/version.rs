//! Application-local database operations extending the protocol version.
use super::error::Error;
use crate::api::extractors::pubstat::{ChartQuery, HlQuery, WrQuery};
use crate::models::{Era, hotlap::pubstat};
use lfsplanet_pubstat::track::TrackPubstatExt;
use lfsplanet_pubstat::{Lap, Version, ch::Chart, hl::Hotlaps, wr::WorldRecords};
use sea_orm::DatabaseConnection;

pub(super) trait PubstatVersionExt {
    async fn hl(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: HlQuery,
    ) -> Result<Hotlaps, Error>;
    async fn ch(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: ChartQuery,
    ) -> Result<Chart, Error>;
    async fn wr(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: WrQuery,
    ) -> Result<WorldRecords, Error>;
}

impl PubstatVersionExt for Version {
    async fn hl(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: HlQuery,
    ) -> Result<Hotlaps, Error> {
        super::hl::get(self, database, era, query).await
    }
    async fn ch(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: ChartQuery,
    ) -> Result<Chart, Error> {
        super::ch::get(self, database, era, query).await
    }
    async fn wr(
        self,
        database: &DatabaseConnection,
        era: &Era,
        query: WrQuery,
    ) -> Result<WorldRecords, Error> {
        super::wr::get(self, database, era, query).await
    }
}

pub(super) async fn load(
    database: &DatabaseConnection,
    era: &Era,
    filters: pubstat::Filters<'_>,
) -> Result<Vec<Lap>, Error> {
    pubstat::list(database, era.id, filters)
        .await?
        .into_iter()
        .filter(|row| row.track.0.to_pubstat_id().is_some())
        .map(wire_lap)
        .collect()
}

pub(super) fn wire_lap(row: pubstat::Row) -> Result<Lap, Error> {
    // Stored SPR splits include the finish in their last meaningful slot;
    // Pubstat exposes checkpoints only and pads unused columns with zero.
    let checkpoint = |split| if split == row.laptime { 0 } else { split };
    Ok(Lap {
        id: row.id,
        track: row.track.0,
        car: row.car.0,
        split1: checkpoint(row.split1),
        split2: checkpoint(row.split2),
        split3: checkpoint(row.split3),
        laptime: row.laptime,
        flags_hlaps: u16::try_from(row.flags_hlaps).map_err(|e| Error::internal(&e))?,
        racername: row.racername,
        timestamp: row.created_at.unix_timestamp(),
    })
}
