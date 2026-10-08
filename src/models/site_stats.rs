//! Site-wide totals and homepage spotlights from one database snapshot.
use crate::{
    era_slug::EraSlug, milliseconds::Milliseconds, models::Player, track_id::TrackId,
    vehicle_id::VehicleId,
};
use sea_orm::{
    AccessMode, DatabaseConnection, DbBackend, DbErr, FromQueryResult, IsolationLevel, Statement,
    TransactionTrait,
};

const SPOTLIGHT_UPLOAD_LIMIT: i64 = 100;
// Demo laps exercise the same homepage behavior; historical imports do not
// displace actual upload activity. IDs order submissions independently of age.
const RECENT_UPLOADS: &str = "WITH recent AS (
    SELECT id, chart_id, player_id FROM hotlap
    WHERE source IN ('upload', 'demo') AND state = 'valid'
    ORDER BY id DESC LIMIT $1
)";

#[derive(Debug, FromQueryResult)]
pub(crate) struct SiteStats {
    pub validated_hotlaps: i64,
    pub drivers: i64,
    pub combinations: i64,
    pub eras: i64,
    pub spotlight_uploads: i64,
    #[sea_orm(skip)]
    pub combo_spotlight: Option<ComboSpotlight>,
    #[sea_orm(skip)]
    pub driver_spotlight: Option<DriverSpotlight>,
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct ComboSpotlight {
    pub chart_id: i64,
    pub era_id: EraSlug,
    pub era_title: String,
    pub track: TrackId,
    pub track_name: String,
    pub track_location: crate::models::track::TrackLocation,
    pub track_reverse: bool,
    pub track_open_configuration: bool,
    pub vehicle: VehicleId,
    pub vehicle_name: String,
    pub vehicle_license: String,
    pub vehicle_has_image: bool,
    pub recent_uploads: i64,
    #[sea_orm(skip)]
    pub leaders: Vec<SpotlightBest>,
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct SpotlightBest {
    #[sea_orm(nested)]
    pub player: Player,
    pub position: i64,
    pub lap_time_ms: Milliseconds,
    pub distance_to_world_record_ms: Milliseconds,
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct DriverSpotlight {
    #[sea_orm(nested)]
    pub player: Player,
    pub recent_personal_bests: i64,
}

impl SiteStats {
    /// Selects the busiest combo and the driver with most current PBs among
    /// the latest 100 validated uploads, including demo laps. No age cutoff.
    pub(crate) async fn load(database: &DatabaseConnection) -> Result<Self, DbErr> {
        let transaction = database
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let mut stats = Self::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "{RECENT_UPLOADS}
        SELECT
            (SELECT COUNT(*) FROM hotlap WHERE state = 'valid') AS validated_hotlaps,
            (SELECT COUNT(*) FROM player) AS drivers,
            (SELECT COUNT(DISTINCT (track_id, vehicle_id)) FROM chart) AS combinations,
            (SELECT COUNT(*) FROM era) AS eras,
            (SELECT COUNT(*) FROM recent) AS spotlight_uploads"
            ),
            [SPOTLIGHT_UPLOAD_LIMIT.into()],
        ))
        .one(&transaction)
        .await?
        .ok_or_else(|| DbErr::Custom("site totals query returned no row".to_owned()))?;

        stats.combo_spotlight = ComboSpotlight::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "{RECENT_UPLOADS}, selected AS (
    SELECT chart_id, COUNT(*)::BIGINT AS recent_uploads
    FROM recent GROUP BY chart_id
    ORDER BY COUNT(*) DESC, MAX(id) DESC, chart_id LIMIT 1
)
SELECT chart.id AS chart_id, era.slug AS era_id, era.title AS era_title,
       chart.track_id AS track, track.name AS track_name,
       track.location AS track_location, track.reverse AS track_reverse,
       track.open_configuration AS track_open_configuration,
       chart.vehicle_id AS vehicle, vehicle.name AS vehicle_name, vehicle.license AS vehicle_license,
       (vehicle.available AND vehicle.image_object_key IS NOT NULL) AS vehicle_has_image,
       selected.recent_uploads
FROM selected
JOIN chart ON chart.id = selected.chart_id
JOIN era ON era.id = chart.era_id
JOIN track ON track.id = chart.track_id
JOIN vehicle ON vehicle.id = chart.vehicle_id"
            ),
            [SPOTLIGHT_UPLOAD_LIMIT.into()],
        ))
        .one(&transaction)
        .await?;
        if let Some(combo) = &mut stats.combo_spotlight {
            combo.leaders = SpotlightBest::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT player.*, pb.position, hotlap.lap_time_ms,
                        hotlap.lap_time_ms - record.lap_time_ms AS distance_to_world_record_ms
                 FROM hotlap_personal_best pb
                 JOIN hotlap ON hotlap.id = pb.hotlap_id
                 JOIN player ON player.id = pb.player_id
                 JOIN hotlap_personal_best record_best
                   ON record_best.chart_id = pb.chart_id AND record_best.position = 1
                 JOIN hotlap record ON record.id = record_best.hotlap_id
                 WHERE pb.chart_id = $1
                 ORDER BY pb.position LIMIT 3",
                [combo.chart_id.into()],
            ))
            .all(&transaction)
            .await?;
        }
        stats.driver_spotlight =
            DriverSpotlight::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    "{RECENT_UPLOADS}, selected AS (
    SELECT recent.player_id, COUNT(*)::BIGINT AS recent_personal_bests
    FROM recent JOIN hotlap_personal_best pb ON pb.hotlap_id = recent.id
    GROUP BY recent.player_id
    ORDER BY COUNT(*) DESC, MAX(recent.id) DESC, recent.player_id LIMIT 1
)
SELECT player.*, selected.recent_personal_bests
FROM selected JOIN player ON player.id = selected.player_id"
                ),
                [SPOTLIGHT_UPLOAD_LIMIT.into()],
            ))
            .one(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Era, Hotlap};
    use sea_orm::EntityTrait;

    async fn published_fixture(pool: &sqlx::PgPool) -> anyhow::Result<DatabaseConnection> {
        sqlx::raw_sql(include_str!("../services/validate_hotlap/fixtures.sql"))
            .execute(pool)
            .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let lap = crate::models::hotlap::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        Hotlap::validate_owned_for_testing(&database, lap.id, lap.player_id).await?;
        // Quiet sites still have spotlights; calendar age is immaterial.
        sqlx::query("UPDATE hotlap SET created_at = '2000-01-01'")
            .execute(pool)
            .await?;
        Ok(database)
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn empty_site_has_no_spotlights(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let stats = SiteStats::load(&database).await?;
        assert_eq!(stats.spotlight_uploads, 0);
        assert!(stats.combo_spotlight.is_none());
        assert!(stats.driver_spotlight.is_none());
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn spotlights_ignore_imports_and_unvalidated_laps_and_share_chart_identity(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = published_fixture(&pool).await?;
        sqlx::raw_sql(r"
INSERT INTO player (lfs_username, display_name) VALUES ('second', 'Second'), ('third', 'Third');
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state, created_at)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    CASE player.lfs_username WHEN 'second' THEN 61000 ELSE 62000 END, 0, 0, 0, 0,
    'demo', 'demo:' || player.lfs_username, 'wheel', 0, '0.7A', 'valid', '2000-01-01'
FROM player CROSS JOIN chart WHERE player.lfs_username IN ('second', 'third');
-- Another ranking of the same chart must not duplicate its sample or leaders.
INSERT INTO ranking (era_id, slug, position, title, description, benchmark_percent, nation_max_points, nation_driver_limit)
SELECT id, 'another', 1, 'Another', 'Another ranking', 103, 10, 3 FROM era;
INSERT INTO ranking_chart_membership (era_id, ranking_id, position, chart_id)
SELECT ranking.era_id, ranking.id, 0, chart.id FROM ranking JOIN chart ON chart.era_id = ranking.era_id WHERE ranking.slug = 'another';
-- These imports have newer IDs, but must not displace the upload sample.
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    65000, 0, 0, 0, 0, 'lfsworld_v1', '2026-09-23:' || sequence, 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart CROSS JOIN generate_series(1,150) sequence WHERE player.lfs_username = 'worker-test';
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state, original_filename, spr_object_key)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    50000, 0, 0, 0, 0, 'upload', repeat(CASE state WHEN 'pending' THEN 'b' ELSE 'c' END,64), 'wheel', 0, '0.7A', state, 'test.spr', 'missing.spr'
FROM player CROSS JOIN chart CROSS JOIN (VALUES ('pending'), ('invalid')) states(state) WHERE player.lfs_username = 'worker-test';
").execute(&pool).await?;
        let era: Era = crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        era.rebuild_personal_bests(&database).await?;
        let stats = SiteStats::load(&database).await?;
        assert_eq!(stats.spotlight_uploads, 3);
        let combo = stats.combo_spotlight.unwrap();
        assert_eq!(combo.era_id.as_str(), "2026-09-23");
        assert_eq!(combo.track.to_string(), "BL1");
        assert_eq!(combo.vehicle.to_string(), "XFG");
        assert_eq!(combo.recent_uploads, 3);
        assert_eq!(combo.leaders.len(), 3);
        assert_eq!(combo.leaders[0].player.lfs_username, "worker-test");
        assert_eq!(combo.leaders[0].lap_time_ms.as_millis(), 60000);
        assert_eq!(
            combo.leaders[1].distance_to_world_record_ms.as_millis(),
            1000
        );
        assert_eq!(
            combo.leaders[2].distance_to_world_record_ms.as_millis(),
            2000
        );
        // Equal PB counts favor the most recent submission, then stable IDs.
        let driver = stats.driver_spotlight.unwrap();
        let latest: String = sqlx::query_scalar("SELECT player.lfs_username FROM hotlap JOIN player ON player.id = hotlap.player_id WHERE source = 'demo' ORDER BY hotlap.id DESC LIMIT 1").fetch_one(&pool).await?;
        assert_eq!(driver.player.lfs_username, latest);
        assert_eq!(driver.recent_personal_bests, 1);
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn sample_is_bounded_and_slower_uploads_do_not_count_as_personal_bests(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = published_fixture(&pool).await?;
        sqlx::raw_sql(
            r"
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state, created_at)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    65000, 0, 0, 0, 0, 'demo', 'demo:' || sequence, 'wheel', 0, '0.7A', 'valid', '2000-01-01'
FROM player CROSS JOIN chart CROSS JOIN generate_series(1,101) sequence;
",
        )
        .execute(&pool)
        .await?;
        let stats = SiteStats::load(&database).await?;
        assert_eq!(stats.spotlight_uploads, 100);
        let combo = stats.combo_spotlight.unwrap();
        assert_eq!(combo.recent_uploads, 100);
        assert_eq!(combo.leaders.len(), 1);
        assert_eq!(combo.leaders[0].lap_time_ms.as_millis(), 60000);
        assert!(stats.driver_spotlight.is_none());

        // A newer, less busy chart must not replace the busiest combo. Its
        // fastest recent lap does qualify its driver for the PB spotlight.
        sqlx::raw_sql(r"
INSERT INTO vehicle (id, kind, name, normalized_name, license)
VALUES ('RB4', 'standard', 'RB4 GT', 'rb4 gt', 's2') ON CONFLICT DO NOTHING;
INSERT INTO chart (era_id, track_id, vehicle_id) SELECT id, 'BL1', 'RB4' FROM era;
INSERT INTO ranking_chart_membership (era_id, ranking_id, position, chart_id)
SELECT ranking.era_id, ranking.id, 1, chart.id FROM ranking JOIN chart ON chart.era_id = ranking.era_id WHERE chart.vehicle_id = 'RB4';
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'RB4 GT',
    65000 + sequence, 0, 0, 0, 0, 'demo', 'demo:rb4:' || sequence, 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart CROSS JOIN generate_series(1,3) sequence WHERE chart.vehicle_id = 'RB4';
").execute(&pool).await?;
        crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap()
            .rebuild_personal_bests(&database)
            .await?;
        let stats = SiteStats::load(&database).await?;
        let combo = stats.combo_spotlight.unwrap();
        assert_eq!(combo.vehicle.to_string(), "XFG");
        assert_eq!(combo.recent_uploads, 97);
        assert_eq!(stats.driver_spotlight.unwrap().recent_personal_bests, 1);
        Ok(())
    }
}
