//! Site-wide catalogue and activity totals.
use sea_orm::{ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement};

#[derive(Debug, FromQueryResult)]
pub(crate) struct SiteStats {
    pub validated_hotlaps: i64,
    pub drivers: i64,
    pub combinations: i64,
    pub eras: i64,
}

impl SiteStats {
    /// One statement gives all totals the same database snapshot. Combinations
    /// count distinct pairs across eras.
    pub(crate) async fn load(database: &impl ConnectionTrait) -> Result<Self, DbErr> {
        Self::find_by_statement(Statement::from_string(
            DbBackend::Postgres,
            r"
        SELECT
            (SELECT COUNT(*) FROM hotlap WHERE state = 'valid') AS validated_hotlaps,
            (SELECT COUNT(*) FROM player) AS drivers,
            (SELECT COUNT(DISTINCT (track_id, vehicle_id)) FROM ranking_chart) AS combinations,
            (SELECT COUNT(*) FROM era) AS eras
    ",
        ))
        .one(database)
        .await?
        .ok_or_else(|| DbErr::Custom("site totals query returned no row".to_owned()))
    }
}
