//! Site-wide totals and homepage spotlights from one database snapshot.
use crate::{era_slug::EraSlug, models::Player, track_id::TrackId, vehicle_id::VehicleId};
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
    pub spotlights: Vec<Spotlight>,
}

#[derive(Debug)]
pub(crate) struct Spotlight {
    pub id: &'static str,
    pub title: &'static str,
    pub label: &'static str,
    pub value: i64,
    pub milliseconds: bool,
    /// Percentage in basis points (100 = 1%).
    pub percentage: bool,
    pub combo: Option<ComboSpotlight>,
    pub driver: Option<DriverSpotlight>,
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct ComboSpotlight {
    pub chart_id: i64,
    pub era_id: EraSlug,
    pub era_title: String,
    pub track: TrackId,
    pub track_name: String,
    pub track_location: String,
    pub track_reverse: bool,
    pub track_open_configuration: bool,
    pub track_image_public_id: Option<sea_orm::prelude::Uuid>,
    pub track_image_original_filename: Option<String>,
    pub track_outline_public_id: Option<sea_orm::prelude::Uuid>,
    pub track_outline_original_filename: Option<String>,
    pub vehicle: VehicleId,
    pub vehicle_name: String,
    pub vehicle_license: String,
    pub vehicle_image_original_filename: Option<String>,
    pub vehicle_image_public_id: Option<sea_orm::prelude::Uuid>,
    pub recent_uploads: i64,
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct DriverSpotlight {
    #[sea_orm(nested)]
    pub player: Player,
    pub recent_personal_bests: i64,
}

impl SiteStats {
    /// Selects activity highlights from the latest 100 validated uploads and
    /// daily discovery candidates from open, available catalogue charts.
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

        let popular_combo = load_combo(
            &transaction,
            &format!(
                "{RECENT_UPLOADS}, selected AS (
            SELECT chart_id, COUNT(*)::BIGINT AS recent_uploads FROM recent GROUP BY chart_id
            ORDER BY COUNT(DISTINCT player_id) DESC, COUNT(*) DESC, MAX(id) DESC, chart_id LIMIT 1
        )"
            ),
        )
        .await?;
        let recent_driver = DriverSpotlight::find_by_statement(Statement::from_sql_and_values(
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
        // Daily deterministic choices vary even when upload volume is low. Only
        // charts in the published catalogue are eligible for discovery cards.
        let mut used_charts: Vec<i64> = popular_combo.iter().map(|combo| combo.chart_id).collect();
        for (id, title, label, selection, milliseconds) in [
            (
                "close-at-the-top",
                "Close at the top",
                "Gap to second",
                "SELECT first.chart_id, (second_lap.lap_time_ms - first_lap.lap_time_ms)::BIGINT AS recent_uploads FROM hotlap_personal_best first JOIN hotlap first_lap ON first_lap.id = first.hotlap_id JOIN hotlap_personal_best second ON second.chart_id = first.chart_id AND second.position = 2 JOIN hotlap second_lap ON second_lap.id = second.hotlap_id WHERE first.position = 1 AND second_lap.lap_time_ms - first_lap.lap_time_ms BETWEEN 0 AND 500",
                true,
            ),
            (
                "be-the-first",
                "Be the first",
                "Drivers",
                "SELECT chart.id AS chart_id, 0::BIGINT AS recent_uploads FROM chart WHERE NOT EXISTS (SELECT 1 FROM hotlap_personal_best pb WHERE pb.chart_id = chart.id)",
                false,
            ),
            (
                "room-on-the-grid",
                "Room on the grid",
                "Drivers",
                "SELECT chart_id, COUNT(*)::BIGINT AS recent_uploads FROM hotlap_personal_best GROUP BY chart_id HAVING COUNT(*) BETWEEN 1 AND 5",
                false,
            ),
        ] {
            let excluded = used_charts
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let exclusion = if excluded.is_empty() {
                String::new()
            } else {
                format!("AND candidates.chart_id NOT IN ({excluded})")
            };
            let prefix = format!("WITH candidates AS ({selection}), selected AS (
                SELECT candidates.* FROM candidates
                WHERE EXISTS (SELECT 1 FROM ranking_chart_membership membership
                    JOIN chart eligible ON eligible.id = membership.chart_id
                    JOIN era ON era.id = eligible.era_id
                    JOIN vehicle ON vehicle.id = eligible.vehicle_id
                    WHERE membership.chart_id = candidates.chart_id AND era.open AND vehicle.available)
                {exclusion}
                ORDER BY md5(candidates.chart_id::TEXT || CURRENT_DATE::TEXT), candidates.chart_id LIMIT 1
            )");
            if let Some(combo) = load_combo(&transaction, &prefix).await? {
                used_charts.push(combo.chart_id);
                stats.spotlights.push(Spotlight {
                    id,
                    title,
                    label,
                    value: combo.recent_uploads,
                    milliseconds,
                    percentage: false,
                    combo: Some(combo),
                    driver: None,
                });
            }
        }
        // Each player/chart is compared with the PB before the entire sample,
        // so repeated uploads cannot inflate either achievement.
        let mut featured_drivers = Vec::<i64>::new();
        for (id, title, label, selection, percentage) in [
            ("biggest-improvement", "Biggest improvement", "PB improvement", "
                SELECT pb.player_id,
                    ROUND((previous.lap_time_ms - current.lap_time_ms)::NUMERIC * 10000 / previous.lap_time_ms)::BIGINT AS value
                FROM hotlap_personal_best pb
                JOIN recent ON recent.id = pb.hotlap_id
                JOIN hotlap current ON current.id = pb.hotlap_id
                JOIN LATERAL (
                    SELECT MIN(lap_time_ms) AS lap_time_ms FROM hotlap earlier
                    WHERE earlier.player_id = pb.player_id AND earlier.chart_id = pb.chart_id
                        AND earlier.state = 'valid' AND earlier.id < (SELECT MIN(id) FROM recent)
                ) previous ON previous.lap_time_ms > current.lap_time_ms AND previous.lap_time_ms > 0
            ", true),
            ("exploring-new-combos", "Exploring new combos", "New combos", "
                SELECT pb.player_id, COUNT(DISTINCT pb.chart_id)::BIGINT AS value
                FROM hotlap_personal_best pb JOIN recent ON recent.id = pb.hotlap_id
                WHERE NOT EXISTS (
                    SELECT 1 FROM hotlap earlier WHERE earlier.player_id = pb.player_id
                        AND earlier.chart_id = pb.chart_id AND earlier.state = 'valid'
                        AND earlier.id < (SELECT MIN(id) FROM recent)
                ) GROUP BY pb.player_id HAVING COUNT(DISTINCT pb.chart_id) >= 2
            ", false),
        ] {
            let excluded = featured_drivers.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            let exclusion = if excluded.is_empty() { String::new() } else { format!("AND player.id NOT IN ({excluded})") };
            let candidate = DriverSpotlight::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!("{RECENT_UPLOADS}, candidates AS ({selection}), best AS (
                    SELECT player_id, MAX(value) AS value FROM candidates GROUP BY player_id
                ) SELECT player.*, best.value AS recent_personal_bests FROM best
                JOIN player ON player.id = best.player_id WHERE best.value > 0 {exclusion}
                ORDER BY best.value DESC, md5(player.id::TEXT || CURRENT_DATE::TEXT), player.id LIMIT 1"),
                [SPOTLIGHT_UPLOAD_LIMIT.into()],
            )).one(&transaction).await?;
            if let Some(driver) = candidate {
                featured_drivers.push(driver.player.id);
                stats.spotlights.push(Spotlight {
                    id, title, label, value: driver.recent_personal_bests,
                    milliseconds: false, percentage, combo: None, driver: Some(driver),
                });
            }
        }
        // A newcomer has no valid historical/imported lap before their first
        // actual submission, and that first submission is still in the sample.
        featured_drivers.extend(recent_driver.iter().map(|driver| driver.player.id));
        let excluded = featured_drivers
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let exclusion = if excluded.is_empty() {
            String::new()
        } else {
            format!("WHERE player.id NOT IN ({excluded})")
        };
        let newcomer = DriverSpotlight::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("{RECENT_UPLOADS}, candidates AS (
                SELECT recent.player_id, MIN(recent.id) AS first_id FROM recent
                WHERE NOT EXISTS (SELECT 1 FROM hotlap earlier WHERE earlier.player_id = recent.player_id AND earlier.state = 'valid' AND earlier.id < recent.id)
                GROUP BY recent.player_id
            ) SELECT player.*, 1::BIGINT AS recent_personal_bests FROM candidates
            JOIN player ON player.id = candidates.player_id
            {exclusion}
            ORDER BY md5(player.id::TEXT || CURRENT_DATE::TEXT), player.id LIMIT 1"),
            [SPOTLIGHT_UPLOAD_LIMIT.into()],
        )).one(&transaction).await?;
        if let Some(driver) = newcomer {
            stats.spotlights.push(Spotlight {
                id: "new-arrival",
                title: "New arrival",
                label: "First validated lap",
                value: 1,
                milliseconds: false,
                percentage: false,
                combo: None,
                driver: Some(driver),
            });
        }
        if let Some(driver) = recent_driver
            && !stats
                .spotlights
                .iter()
                .filter_map(|card| card.driver.as_ref())
                .any(|featured| featured.player.id == driver.player.id)
        {
            stats.spotlights.insert(
                0,
                Spotlight {
                    id: "recent-pbs",
                    title: "Recent PBs",
                    label: "Recent PBs",
                    value: driver.recent_personal_bests,
                    milliseconds: false,
                    percentage: false,
                    combo: None,
                    driver: Some(driver),
                },
            );
        }
        if let Some(combo) = popular_combo {
            stats.spotlights.insert(
                0,
                Spotlight {
                    id: "popular-right-now",
                    title: "Popular right now",
                    label: "Recent uploads",
                    value: combo.recent_uploads,
                    milliseconds: false,
                    percentage: false,
                    combo: Some(combo),
                    driver: None,
                },
            );
        }
        transaction.commit().await?;
        Ok(stats)
    }
}

async fn load_combo(
    transaction: &sea_orm::DatabaseTransaction,
    prefix: &str,
) -> Result<Option<ComboSpotlight>, DbErr> {
    ComboSpotlight::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres, format!("{prefix} SELECT chart.id AS chart_id, era.slug AS era_id, era.title AS era_title,
       chart.track_id AS track, track.name AS track_name,
       track.location AS track_location, track.reverse AS track_reverse,
       track.open_configuration AS track_open_configuration,
       track_image.public_id AS track_image_public_id,
       track_image.original_filename AS track_image_original_filename,
       track_outline.public_id AS track_outline_public_id,
       track_outline.original_filename AS track_outline_original_filename,
       chart.vehicle_id AS vehicle, vehicle.name AS vehicle_name, vehicle.license AS vehicle_license,
       CASE WHEN vehicle.available THEN image.public_id END AS vehicle_image_public_id,
       CASE WHEN vehicle.available THEN image.original_filename END AS vehicle_image_original_filename,
       selected.recent_uploads
FROM selected
JOIN chart ON chart.id = selected.chart_id
JOIN era ON era.id = chart.era_id
JOIN track ON track.id = chart.track_id
JOIN vehicle ON vehicle.id = chart.vehicle_id
LEFT JOIN attachment image ON image.id = vehicle.image_attachment_id
LEFT JOIN attachment track_image ON track_image.id = track.image_attachment_id
LEFT JOIN attachment track_outline ON track_outline.id = track.outline_attachment_id"),
        if prefix.contains("$1") { vec![SPOTLIGHT_UPLOAD_LIMIT.into()] } else { vec![] },
    )).one(transaction).await
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
        sqlx::query("UPDATE era SET open = TRUE")
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
    async fn progress_cards_compare_the_whole_sample_and_count_distinct_combos(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = published_fixture(&pool).await?;
        // Move the established PB outside the sample with repeated slower laps.
        sqlx::raw_sql(
            r"
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    65000, 0, 0, 0, 0, 'demo', 'demo:progress:' || sequence, 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart CROSS JOIN generate_series(1,100) sequence;
",
        )
        .execute(&pool)
        .await?;
        let before = SiteStats::load(&database).await?;
        assert!(
            !before
                .spotlights
                .iter()
                .any(|card| card.id == "biggest-improvement" || card.id == "exploring-new-combos")
        );
        sqlx::raw_sql(r"
INSERT INTO vehicle (id, kind, name, normalized_name, license)
VALUES ('RB4', 'standard', 'RB4 GT', 'rb4 gt', 's2'), ('XRG', 'standard', 'XR GT', 'xr gt', 'demo') ON CONFLICT DO NOTHING;
INSERT INTO chart (era_id, track_id, vehicle_id) SELECT era.id, 'BL1', vehicle.id FROM era CROSS JOIN vehicle WHERE vehicle.id IN ('RB4', 'XRG');
INSERT INTO player (lfs_username, display_name) VALUES ('explorer', 'Explorer');
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'Test car',
    60000 + sequence, 0, 0, 0, 0, 'demo', 'demo:explorer:' || chart.id || ':' || sequence, 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart CROSS JOIN generate_series(1,3) sequence
WHERE player.lfs_username = 'explorer' AND chart.vehicle_id IN ('RB4', 'XRG');
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    54000, 0, 0, 0, 0, 'demo', 'demo:improved', 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart WHERE player.lfs_username = 'worker-test' AND chart.vehicle_id = 'XFG';
").execute(&pool).await?;
        crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap()
            .rebuild_personal_bests(&database)
            .await?;
        let stats = SiteStats::load(&database).await?;
        let improvement = stats
            .spotlights
            .iter()
            .find(|card| card.id == "biggest-improvement")
            .unwrap();
        assert!(improvement.percentage);
        assert_eq!(improvement.value, 1000); // 10%, against 60s rather than recent 65s laps.
        assert_eq!(
            improvement.driver.as_ref().unwrap().player.lfs_username,
            "worker-test"
        );
        let explorer = stats
            .spotlights
            .iter()
            .find(|card| card.id == "exploring-new-combos")
            .unwrap();
        assert_eq!(explorer.value, 2); // Six uploads across two combos.
        assert!(!explorer.percentage);
        assert_eq!(
            explorer.driver.as_ref().unwrap().player.lfs_username,
            "explorer"
        );
        assert!(!stats.spotlights.iter().any(|card| card.id == "new-arrival"));
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn discovery_cards_use_published_charts_and_do_not_repeat_combos(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = published_fixture(&pool).await?;
        sqlx::raw_sql(
            r"
INSERT INTO vehicle (id, kind, name, normalized_name, license)
VALUES ('RB4', 'standard', 'RB4 GT', 'rb4 gt', 's2') ON CONFLICT DO NOTHING;
INSERT INTO chart (era_id, track_id, vehicle_id) SELECT id, 'BL1', 'RB4' FROM era;
",
        )
        .execute(&pool)
        .await?;
        // A chart without catalogue membership must not be advertised.
        let unpublished = SiteStats::load(&database).await?;
        assert!(
            !unpublished
                .spotlights
                .iter()
                .any(|card| card.id == "be-the-first")
        );
        sqlx::raw_sql(r"
INSERT INTO ranking_chart_membership (era_id, ranking_id, position, chart_id)
SELECT ranking.era_id, ranking.id, 1, chart.id FROM ranking JOIN chart ON chart.era_id = ranking.era_id WHERE chart.vehicle_id = 'RB4';
").execute(&pool).await?;
        let stats = SiteStats::load(&database).await?;
        let first = stats
            .spotlights
            .iter()
            .find(|card| card.id == "be-the-first")
            .unwrap();
        assert_eq!(first.value, 0);
        assert_eq!(first.combo.as_ref().unwrap().vehicle.to_string(), "RB4");
        assert!(
            !stats
                .spotlights
                .iter()
                .any(|card| card.id == "room-on-the-grid")
        );
        // The one driver is already featured by Recent PBs.
        assert!(!stats.spotlights.iter().any(|card| card.id == "new-arrival"));
        let again = SiteStats::load(&database).await?;
        assert_eq!(
            first.combo.as_ref().unwrap().chart_id,
            again
                .spotlights
                .iter()
                .find(|card| card.id == "be-the-first")
                .unwrap()
                .combo
                .as_ref()
                .unwrap()
                .chart_id
        );
        sqlx::query("UPDATE era SET open = FALSE")
            .execute(&pool)
            .await?;
        let closed = SiteStats::load(&database).await?;
        assert!(
            !closed
                .spotlights
                .iter()
                .any(|card| card.id == "be-the-first")
        );
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn empty_site_has_no_spotlights(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let stats = SiteStats::load(&database).await?;
        assert_eq!(stats.spotlight_uploads, 0);
        assert!(!stats.spotlights.iter().any(|card| card.id == "recent-pbs"));
        assert!(stats.spotlights.is_empty());
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
-- Another ranking of the same chart must not duplicate its sample.
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
    source, fingerprint, steering, player_flags, game_version, state, spr_attachment_id)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'XF GTI',
    50000, 0, 0, 0, 0, 'upload', repeat(CASE state WHEN 'pending' THEN 'b' ELSE 'c' END,64), 'wheel', 0, '0.7A', state, (SELECT id FROM attachment WHERE object_key = 'missing.spr')
FROM player CROSS JOIN chart CROSS JOIN (VALUES ('pending'), ('invalid')) states(state) WHERE player.lfs_username = 'worker-test';
").execute(&pool).await?;
        let era: Era = crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        era.rebuild_personal_bests(&database).await?;
        let stats = SiteStats::load(&database).await?;
        assert_eq!(stats.spotlight_uploads, 3);
        let combo = stats
            .spotlights
            .iter()
            .find(|card| card.id == "popular-right-now")
            .unwrap()
            .combo
            .as_ref()
            .unwrap();
        assert_eq!(combo.era_id.as_str(), "2026-09-23");
        assert_eq!(combo.track.to_string(), "BL1");
        assert_eq!(combo.vehicle.to_string(), "XFG");
        assert_eq!(combo.recent_uploads, 3);
        // Equal PB counts favor the most recent submission, then stable IDs.
        let driver = stats
            .spotlights
            .iter()
            .find(|card| card.id == "recent-pbs")
            .unwrap()
            .driver
            .as_ref()
            .unwrap();
        let newcomer = stats
            .spotlights
            .iter()
            .find(|card| card.id == "new-arrival")
            .unwrap();
        assert_ne!(
            newcomer.driver.as_ref().unwrap().player.id,
            driver.player.id
        );
        assert_eq!(newcomer.value, 1);
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
        let combo = stats
            .spotlights
            .iter()
            .find(|card| card.id == "popular-right-now")
            .unwrap()
            .combo
            .as_ref()
            .unwrap();
        assert_eq!(combo.recent_uploads, 100);
        assert!(!stats.spotlights.iter().any(|card| card.id == "recent-pbs"));

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
        let combo = stats
            .spotlights
            .iter()
            .find(|card| card.id == "popular-right-now")
            .unwrap()
            .combo
            .as_ref()
            .unwrap();
        assert_eq!(combo.vehicle.to_string(), "XFG");
        assert_eq!(combo.recent_uploads, 97);
        assert_eq!(
            stats
                .spotlights
                .iter()
                .find(|card| card.id == "recent-pbs")
                .unwrap()
                .driver
                .as_ref()
                .unwrap()
                .recent_personal_bests,
            1
        );
        sqlx::raw_sql(
            r"
INSERT INTO player (lfs_username, display_name) VALUES ('challenger', 'Challenger');
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, chart.track_id, chart.vehicle_id, 'RB4 GT',
    65251, 0, 0, 0, 0, 'lfsworld_v1', '2026-09-23:999', 'wheel', 0, '0.7A', 'valid'
FROM player CROSS JOIN chart WHERE chart.vehicle_id = 'RB4' AND player.lfs_username = 'challenger';
",
        )
        .execute(&pool)
        .await?;
        crate::models::era::Entity::find()
            .one(&database)
            .await?
            .unwrap()
            .rebuild_personal_bests(&database)
            .await?;
        let stats = SiteStats::load(&database).await?;
        let close = stats
            .spotlights
            .iter()
            .find(|card| card.id == "close-at-the-top")
            .unwrap();
        assert!(close.milliseconds);
        assert_eq!(close.value, 250);
        assert_eq!(close.combo.as_ref().unwrap().vehicle.to_string(), "RB4");
        assert!(
            !stats
                .spotlights
                .iter()
                .any(|card| card.id == "room-on-the-grid")
        );
        sqlx::query("UPDATE hotlap SET lap_time_ms = 66001 WHERE fingerprint = '2026-09-23:999'")
            .execute(&pool)
            .await?;
        let stats = SiteStats::load(&database).await?;
        assert!(
            !stats
                .spotlights
                .iter()
                .any(|card| card.id == "close-at-the-top")
        );
        assert!(
            stats
                .spotlights
                .iter()
                .any(|card| card.id == "room-on-the-grid")
        );

        Ok(())
    }
}
