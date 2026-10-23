use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request as HttpRequest, StatusCode},
};
use sea_orm::DatabaseConnection;
use tower::ServiceExt;

fn state(database: DatabaseConnection) -> ApiState {
    ApiState {
        database,
        oauth: None,
        object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
        hotlaps: Default::default(),
        site_notice: None,
    }
}

#[test]
fn spr_finish_split_is_removed_to_match_captured_pubstat_checkpoints() {
    use crate::{models::hotlap::pubstat::Row, track_id::TrackId, vehicle_id::VehicleId};
    use insim_core::{track::Track, vehicle::Vehicle};
    use lfsplanet_pubstat::{Version, wr::WorldRecords};
    // Same result as the captured WR, first with its zero padding, then with
    // the stored SPR finish value occupying the third split slot.
    for split3 in [0, 92_900] {
        let row = Row {
            id: 2397,
            track: TrackId(Track::Bl1),
            car: VehicleId(Vehicle::Xfg),
            split1: 30_100,
            split2: 66_480,
            split3,
            laptime: 92_900,
            flags_hlaps: 1,
            racername: "manolixsvouros".into(),
            created_at: time::OffsetDateTime::from_unix_timestamp(1_751_243_059).unwrap(),
        };
        let lap = version::wire_lap(row)
            .map_err(|error| error.message)
            .unwrap();
        let response = WorldRecords::new(Version::V1_5, vec![lap]);
        assert_eq!(
            lfsplanet_pubstat::ser::to_string(&response).unwrap(),
            include_str!("../../../crates/lfsplanet_pubstat/fixtures/lfsworld/wr-1.5.txt")
        );
    }
}

#[tokio::test]
async fn action_queries_validate_before_database_resolution() {
    let app = router().with_state(state(DatabaseConnection::default()));
    let cases = [
        ("", StatusCode::BAD_REQUEST),
        ("action=hl", StatusCode::BAD_REQUEST),
        ("action=hl&racer=", StatusCode::BAD_REQUEST),
        ("action=ch&track=000", StatusCode::BAD_REQUEST),
        ("action=ch&track=999&car=XFG", StatusCode::BAD_REQUEST),
        ("action=ch&track=invalid&car=XFG", StatusCode::BAD_REQUEST),
        ("action=ch&track=65536&car=XFG", StatusCode::BAD_REQUEST),
        (
            "action=ch&track=000&car=XFG&control=no",
            StatusCode::BAD_REQUEST,
        ),
        (
            "action=ch&track=000&car=XFG&country=GB&version=1.4",
            StatusCode::BAD_REQUEST,
        ),
        (
            "action=ch&track=000&car=XFG&country=invalid",
            StatusCode::BAD_REQUEST,
        ),
        ("action=wr&version=1.3&car=XFG", StatusCode::BAD_REQUEST),
        ("action=wr&version=1.6", StatusCode::BAD_REQUEST),
        ("action=wr&action=hl", StatusCode::BAD_REQUEST),
        ("action=wr&s=1&version=1.3", StatusCode::BAD_REQUEST),
        ("action=wr&s=3&version=1.3", StatusCode::BAD_REQUEST),
        ("action=wr&s=4", StatusCode::BAD_REQUEST),
        ("action=wr&s=2", StatusCode::NOT_IMPLEMENTED),
        ("action=wr&c=4", StatusCode::BAD_REQUEST),
        ("action=wr&c=invalid", StatusCode::BAD_REQUEST),
        ("action=pb&racer=somebody", StatusCode::NOT_IMPLEMENTED),
        ("action=unknown", StatusCode::BAD_REQUEST),
    ];
    for (query, expected) in cases {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/api/pubstat/2026-09-23/get_stat2.php?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{query}");
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/plain")
        );
    }
}

#[tokio::test]
async fn shared_era_extractor_preserves_each_api_error_format() {
    let app = crate::api::router(1024).with_state(state(DatabaseConnection::default()));
    // A decoded slash fails slug validation before any database access.
    for (uri, content_type) in [
        ("/api/pubstat/%2F/get_stat2.php?action=wr&s=1", "text/plain"),
        ("/api/pubstat/%2F/get_stat2.php?action=wr&c=1", "text/plain"),
        ("/api/v1/eras/%2F", "application/json"),
    ] {
        let response = app
            .clone()
            .oneshot(HttpRequest::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with(content_type)
        );
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        if content_type == "application/json" {
            let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(error["error"]["code"], "era_not_found");
        } else {
            assert_eq!(body.as_ref(), b"era_not_found: Era was not found\n");
        }
    }
}

#[tokio::test]
async fn action_specific_extractors_accept_legacy_queries() {
    async fn inspect(
        PubstatQuery {
            version,
            format: _,
            action,
            ..
        }: PubstatQuery,
    ) -> String {
        let action = match action {
            Action::Hotlaps(query) => format!("hl {}", query.racer),
            Action::Chart(query) => format!("ch {} {} {:?}", query.track, query.car, query.country),
            Action::WorldRecords(query) => format!(
                "wr {:?} {:?}",
                query.track.map(|track| track.to_string()),
                query.car.map(|car| car.to_string())
            ),
            action => return action.rejection().message,
        };
        format!("{version} {action}")
    }
    let app = Router::new()
        .route("/", get(inspect))
        .with_state(state(DatabaseConnection::default()));
    for (query, expected) in [
        (
            "action=hl&racer=a+racer&version=1.0&idk=unused&ps=1",
            "1.0 hl a racer",
        ),
        (
            "action=ch&track=000&car=XFG&country=GB&control=ks",
            "1.5 ch BL1 XFG Some(\"GB\")",
        ),
        ("action=wr&track=410&s=1", "1.5 wr Some(\"KY2\") None"),
        ("action=wr&track=410&s=3", "1.5 wr Some(\"KY2\") None"),
        ("action=ch&track=000&car=ABCDEF", "1.5 ch BL1 ABCDEF None"),
        ("action=wr&car=123abc", "1.5 wr None Some(\"123ABC\")"),
        ("action=wr&track=0", "1.5 wr Some(\"BL1\") None"),
        ("action=wr&track=0000", "1.5 wr Some(\"BL1\") None"),
        ("action=wr&car=XFG&version=1.4", "1.4 wr None Some(\"XFG\")"),
        (
            "action=wr&track=7100&car=FBM",
            "1.5 wr Some(\"RO11\") Some(\"FBM\")",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{query}");
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), expected);
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn published_bests_are_complete_filtered_and_era_scoped(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    use sea_orm::EntityTrait;
    sqlx::raw_sql(include_str!("../../services/validate_hotlap/fixtures.sql"))
        .execute(&pool)
        .await?;
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let hotlap = crate::models::hotlap::Entity::find()
        .one(&database)
        .await?
        .unwrap();
    // Pending laps do not appear, even when the chart exists.
    let app = router().with_state(state(database.clone()));
    let request = |query: &str| {
        HttpRequest::builder()
            .uri(format!("/api/pubstat/2026-09-23/get_stat2.php?{query}"))
            .body(Body::empty())
            .unwrap()
    };
    let response = app
        .clone()
        .oneshot(request("action=ch&track=000&car=XFG"))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 4096).await?.is_empty());
    crate::models::Hotlap::validate_owned_for_testing(&database, hotlap.id, hotlap.player_id)
        .await?
        .unwrap();
    sqlx::raw_sql(
        r#"
UPDATE player SET country_code = 'GB' WHERE lfs_username = 'worker-test';
INSERT INTO player (lfs_username, display_name, country_code)
SELECT 'import-' || n, 'Imported', 'FR' FROM generate_series(1,120) n;
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT p.id, h.era_id, h.chart_id, h.track, h.vehicle, h.raw_vehicle_name,
    61000 + p.id, 10000, 20000, 0, 0,
    'lfsworld_v1', '2026-09-23:' || p.id, 'mouse', 1024, h.game_version, 'valid'
FROM player p CROSS JOIN hotlap h WHERE p.lfs_username LIKE 'import-%';
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id, position)
SELECT chart_id, player_id, id, ROW_NUMBER() OVER (ORDER BY lap_time_ms, id) + 1
FROM hotlap WHERE source = 'lfsworld_v1';
INSERT INTO era (slug, title, version_requirement, installation_id)
VALUES ('2020-01-01', 'Other era', '>=0.7A', '0.7');
INSERT INTO chart (era_id, track_id, vehicle_id)
SELECT id, 'BL1', 'XFG' FROM era WHERE slug = '2020-01-01';
"#,
    )
    .execute(&pool)
    .await?;

    for (query, count) in [
        ("action=ch&track=000&car=XFG&s=1", 121),
        ("action=ch&track=000&car=XFG&s=1&control=m", 120),
        ("action=ch&track=000&car=XFG&s=1&country=GB", 1),
        ("action=ch&track=000&car=XFG&s=1&country=FR&control=w", 0),
        ("action=wr&s=1", 1),
        ("action=wr&s=1&car=XRG", 0),
        ("action=hl&racer=IMPORT-1&s=1", 1),
    ] {
        let response = app.clone().oneshot(request(query)).await?;
        assert_eq!(response.status(), StatusCode::OK, "{query}");
        let body = to_bytes(response.into_body(), 100_000).await?;
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
        assert_eq!(rows.len(), count, "{query}");
        if query == "action=wr&s=1" {
            assert_eq!(rows[0]["racername"], "worker-test");
        }
    }
    for (query, root, count) in [
        ("action=hl&racer=worker-test&s=3", "hl_data", 1),
        ("action=ch&track=000&car=XFG&s=3", "ch_data", 121),
        ("action=wr&s=3&version=1.4", "wr_data", 1),
        ("action=wr&car=XRG&s=3", "wr_data", 0),
    ] {
        let response = app.clone().oneshot(request(query)).await?;
        assert_eq!(response.status(), StatusCode::OK, "{query}");
        assert_eq!(
            response.headers()["content-type"],
            "text/xml; charset=utf-8"
        );
        let body = to_bytes(response.into_body(), 100_000).await?;
        let xml = std::str::from_utf8(&body)?;
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains(&format!("<{root}>")) || xml.contains(&format!("<{root} />")));
        assert_eq!(xml.matches("<entry>").count(), count);
        if query.contains("version=1.4") {
            assert!(!xml.contains("<timestamp>"));
        }
    }
    for query in [
        "action=hl&racer=worker-test&s=1",
        "action=ch&track=000&car=XFG&s=1",
        "action=wr&s=1",
    ] {
        let response = app.clone().oneshot(request(query)).await?;
        let baseline = to_bytes(response.into_body(), 100_000).await?;
        for mode in 1..=3 {
            let response = app
                .clone()
                .oneshot(request(&format!("{query}&c={mode}")))
                .await?;
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), 100_000).await?;
            assert_eq!(
                inflate_query_body(mode, &body),
                baseline,
                "{query}&c={mode}"
            );
        }
    }
    for (version, columns) in [
        ("1.0", 4),
        ("1.1", 4),
        ("1.2", 5),
        ("1.3", 8),
        ("1.4", 9),
        ("1.5", 9),
    ] {
        let response = app
            .clone()
            .oneshot(request(&format!(
                "action=hl&racer=worker-test&version={version}"
            )))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4096).await?;
        assert_eq!(
            std::str::from_utf8(&body)?.split_whitespace().count(),
            columns
        );
    }
    let response = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/pubstat/2020-01-01/get_stat2.php?action=wr&s=1")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(to_bytes(response.into_body(), 4096).await?.as_ref(), b"[]");
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn mods_are_available_in_every_action_and_output(pool: sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("../../services/validate_hotlap/fixtures.sql"))
        .execute(&pool)
        .await?;
    sqlx::raw_sql(
        r#"
INSERT INTO vehicle (id, kind, name, normalized_name, license, version)
VALUES ('ABCDEF', 'mod', 'Test mod', 'test mod', 's3', 1);
INSERT INTO chart (era_id, track_id, vehicle_id)
SELECT id, 'BL1', 'ABCDEF' FROM era WHERE slug = '2026-09-23';
INSERT INTO hotlap (player_id, era_id, chart_id, track, vehicle, raw_vehicle_name, mod_version,
    lap_time_ms, split_1_ms, split_2_ms, split_3_ms, split_4_ms,
    source, fingerprint, steering, player_flags, game_version, state)
SELECT player.id, chart.era_id, chart.id, 'BL1', 'ABCDEF', 'Test mod', 1,
    65000, 30000, 65000, 0, 0, 'lfsworld_v1', '2026-09-23:999999', 'wheel', 0, '0.7A', 'valid'
FROM player JOIN chart ON chart.vehicle_id = 'ABCDEF'
WHERE player.lfs_username = 'worker-test';
INSERT INTO hotlap_personal_best (chart_id, player_id, hotlap_id, position)
SELECT chart_id, player_id, id, 1 FROM hotlap WHERE vehicle = 'ABCDEF';
"#,
    )
    .execute(&pool)
    .await?;
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let app = router().with_state(state(database));
    for (query, content_type, needle) in [
        ("action=hl&racer=worker-test", "text/plain", "000 ABCDEF"),
        (
            "action=hl&racer=worker-test&s=1",
            "application/json",
            "\"car\":\"ABCDEF\"",
        ),
        (
            "action=ch&track=000&car=ABCDEF&s=1",
            "application/json",
            "\"laptime\":65000",
        ),
        (
            "action=wr&car=abcdef&s=1",
            "application/json",
            "\"car\":\"ABCDEF\"",
        ),
        ("action=wr&car=ABCDEF&s=3", "text/xml", "<car>ABCDEF</car>"),
    ] {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/api/pubstat/2026-09-23/get_stat2.php?{query}"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK, "{query}");
        assert!(
            response.headers()["content-type"]
                .to_str()?
                .starts_with(content_type)
        );
        let body = to_bytes(response.into_body(), 4096).await?;
        assert!(std::str::from_utf8(&body)?.contains(needle), "{query}");
    }
    Ok(())
}

fn compression_test_app() -> Router {
    async fn sample(
        PubstatQuery {
            version,
            format,
            compression,
            action,
        }: PubstatQuery,
    ) -> Result<Response, Error> {
        use insim_core::{track::Track, vehicle::Vehicle};
        use lfsplanet_pubstat::{Lap, wr::WorldRecords};
        let empty =
            matches!(action, Action::WorldRecords(ref query) if query.car == Some(Vehicle::Xrg));
        let rows = if empty {
            vec![]
        } else {
            vec![
                Lap {
                    id: 42,
                    track: Track::Bl1,
                    car: Vehicle::Xfg,
                    split1: 10000,
                    split2: 20000,
                    split3: 0,
                    laptime: 30000,
                    flags_hlaps: 1,
                    racername: "compression fixture".into(),
                    timestamp: 1000000,
                };
                32
            ]
        };
        response::respond(format, compression, &WorldRecords::new(version, rows))
    }
    Router::new()
        .route("/", get(sample))
        .layer(
            tower_http::compression::CompressionLayer::new()
                .compress_when(crate::api::compressible()),
        )
        .with_state(state(DatabaseConnection::default()))
}

fn inflate_query_body(mode: u8, body: &[u8]) -> Vec<u8> {
    use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
    use std::io::Read;
    let mut decoded = Vec::new();
    match mode {
        1 => GzDecoder::new(body).read_to_end(&mut decoded),
        2 => ZlibDecoder::new(body).read_to_end(&mut decoded),
        3 => DeflateDecoder::new(body).read_to_end(&mut decoded),
        _ => panic!("Invalid test compression mode"),
    }
    .unwrap();
    decoded
}

#[tokio::test]
async fn query_compression_preserves_every_format_and_empty_collections() {
    let app = compression_test_app();
    for car in ["XFG", "XRG"] {
        for format in [0, 1, 3] {
            let uri = format!("/?action=wr&car={car}&s={format}");
            let baseline = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .uri(&uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(baseline.status(), StatusCode::OK);
            let content_type = baseline.headers()["content-type"].clone();
            let baseline = to_bytes(baseline.into_body(), 100_000).await.unwrap();
            for mode in 1..=3 {
                let response = app
                    .clone()
                    .oneshot(
                        HttpRequest::builder()
                            .uri(format!("{uri}&c={mode}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                if mode == 1 {
                    assert_eq!(response.headers()["content-encoding"], "gzip");
                    assert_eq!(response.headers()["content-type"], content_type);
                } else {
                    assert!(!response.headers().contains_key("content-encoding"));
                    assert_eq!(
                        response.headers()["content-type"],
                        "application/octet-stream"
                    );
                }
                let body = to_bytes(response.into_body(), 100_000).await.unwrap();
                assert_eq!(
                    inflate_query_body(mode, &body),
                    baseline,
                    "car={car}, s={format}, c={mode}"
                );
            }
        }
    }
}

#[tokio::test]
async fn query_compression_avoids_double_encoding_and_keeps_http_negotiation() {
    let app = compression_test_app();
    let baseline = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/?action=wr&c=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let baseline = to_bytes(baseline.into_body(), 100_000).await.unwrap();
    for mode in 1..=3 {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/?action=wr&c={mode}"))
                    .header("accept-encoding", "gzip, br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        if mode == 1 {
            assert_eq!(response.headers()["content-encoding"], "gzip");
        } else {
            assert!(!response.headers().contains_key("content-encoding"));
        }
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        assert_eq!(inflate_query_body(mode, &body), baseline, "c={mode}");
    }
    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/?action=wr&c=0")
                .header("accept-encoding", "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["content-encoding"], "gzip");
    let body = to_bytes(response.into_body(), 100_000).await.unwrap();
    assert_eq!(inflate_query_body(1, &body), baseline);
}

#[tokio::test]
async fn query_compression_is_available_to_old_plain_text_clients() {
    let app = compression_test_app();
    let baseline = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/?action=wr&version=1.0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let baseline = to_bytes(baseline.into_body(), 100_000).await.unwrap();
    for mode in 1..=3 {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri(format!("/?action=wr&version=1.0&c={mode}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 100_000).await.unwrap();
        assert_eq!(inflate_query_body(mode, &body), baseline);
    }
}
