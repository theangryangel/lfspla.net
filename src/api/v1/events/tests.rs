use super::*;
use sea_orm::SqlxPostgresConnector;
use std::sync::Arc;
use tower::ServiceExt;

#[test]
fn event_filters_require_positive_ids_and_explicit_date_offsets() {
    let now = time::macros::datetime!(2030-01-01 12:00 UTC);
    let empty: EventQuery = serde_urlencoded::from_str("").unwrap();
    assert_eq!(empty.filter(now).unwrap().starts_from, now);
    for query in [
        "league_id=0",
        "season_id=-1",
        "starts_from=2030-01-01",
        "starts_from=2030-01-01T12:00:00",
        "starts_before=2029-01-01T00:00:00Z",
        "starts_from=2030-01-01T12:00:00Z&starts_before=2030-01-01T12:00:00Z",
    ] {
        assert!(
            serde_urlencoded::from_str::<EventQuery>(query)
                .unwrap()
                .filter(now)
                .is_err(),
            "accepted {query}"
        );
    }
    let query: EventQuery = serde_urlencoded::from_str(
        "starts_from=2030-01-01T13:00:00%2B01:00&starts_before=2030-01-02T00:00:00Z",
    )
    .unwrap();
    assert_eq!(query.filter(now).unwrap().starts_from, now);
}

async fn request(state: ApiState, query: &str) -> axum::response::Response {
    router()
        .with_state(state)
        .split_for_parts()
        .0
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/v1/events{query}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn body(state: ApiState, query: &str) -> serde_json::Value {
    let response = request(state, query).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn events_api_filters_paginates_and_hides_moderated_or_past_meetings(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let league: i64 =
        sqlx::query_scalar("INSERT INTO league (name) VALUES ('League A') RETURNING id")
            .fetch_one(&pool)
            .await?;
    let other: i64 =
        sqlx::query_scalar("INSERT INTO league (name) VALUES ('League B') RETURNING id")
            .fetch_one(&pool)
            .await?;
    let season: i64 = sqlx::query_scalar(
        "INSERT INTO season (league_id, name) VALUES ($1, 'Season A') RETURNING id",
    )
    .bind(league)
    .fetch_one(&pool)
    .await?;
    sqlx::query("INSERT INTO event (league_id, season_id, title, starts_at, details_url) VALUES ($1, $2, 'First', now() + interval '1 day', 'https://www.lfs.net/leagues/432')").bind(league).bind(season).execute(&pool).await?;
    sqlx::query("INSERT INTO event (league_id, title, starts_at) VALUES ($1, 'One off', now() + interval '1 day'), ($2, 'Other', now() + interval '2 days'), ($1, 'Past', now() - interval '1 day')").bind(league).bind(other).execute(&pool).await?;
    sqlx::query("INSERT INTO event (league_id, title, starts_at, hidden) VALUES ($1, 'Hidden', now() + interval '1 day', true)").bind(league).execute(&pool).await?;
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let transaction = database.begin().await?;
    let logo = crate::models::attachment::Model::store(
        &transaction,
        &*store,
        b"logo".to_vec(),
        "png",
        "image/png",
        None,
    )
    .await?;
    transaction.commit().await?;
    sqlx::query("UPDATE league SET image_attachment_id = $1 WHERE id = $2")
        .bind(logo.id)
        .bind(league)
        .execute(&pool)
        .await?;
    let state = ApiState {
        database,
        object_store: store,
        oauth: None,
        hotlaps: crate::settings::HotlapSettings::default(),
        site_notice: None,
    };
    let all = body(state.clone(), "").await;
    assert_eq!(all["pagination"]["total_items"], 3);
    assert_eq!(all["items"][0]["title"], "First");
    assert_eq!(all["items"][0]["league"]["name"], "League A");
    assert_eq!(
        all["items"][0]["league"]["image"]["url"],
        format!("/api/v1/attachments/{}", logo.public_id)
    );
    assert_eq!(all["items"][2]["league"]["image"], serde_json::Value::Null);
    assert_eq!(all["items"][0]["season"]["name"], "Season A");
    assert_eq!(all["items"][1]["season"], serde_json::Value::Null);
    assert_eq!(
        all["items"][0]["details_url"],
        "https://www.lfs.net/leagues/432"
    );
    assert!(
        all["items"][0]["starts_at"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    assert!(all["items"][0].get("hidden").is_none());
    let page = body(state.clone(), "?page=2&per_page=1").await;
    assert_eq!(page["items"][0]["title"], "One off");
    assert_eq!(page["pagination"]["total_pages"], 3);
    let filtered = body(state.clone(), &format!("?league_id={league}")).await;
    assert_eq!(filtered["pagination"]["total_items"], 2);
    let filtered = body(state.clone(), &format!("?season_id={season}")).await;
    assert_eq!(filtered["pagination"]["total_items"], 1);
    let empty = body(state.clone(), "?league_id=999999").await;
    assert_eq!(empty["pagination"]["total_items"], 0);
    let before: time::OffsetDateTime = sqlx::query_scalar("SELECT now() + interval '36 hours'")
        .fetch_one(&pool)
        .await?;
    let query = serde_urlencoded::to_string([("starts_before", before.format(&Rfc3339)?)])?;
    assert_eq!(
        body(state.clone(), &format!("?{query}")).await["pagination"]["total_items"],
        2
    );
    let past = body(state.clone(), "?starts_from=2000-01-01T00:00:00Z").await;
    assert_eq!(past["pagination"]["total_items"], 4);
    for query in ["?league_id=0", "?page=0", "?starts_from=2030-01-01"] {
        assert_eq!(
            request(state.clone(), query).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn event_windows_include_ongoing_events_and_exclude_touching_boundaries(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let league: i64 =
        sqlx::query_scalar("INSERT INTO league (name) VALUES ('Window test') RETURNING id")
            .fetch_one(&pool)
            .await?;
    sqlx::query(
        "INSERT INTO event (league_id, title, starts_at, ends_at) VALUES
        ($1, 'Ongoing', '2030-01-01T12:00:00Z', '2030-01-03T12:00:00Z'),
        ($1, 'Ended at boundary', '2030-01-01T12:00:00Z', '2030-01-02T00:00:00Z'),
        ($1, 'Starts at boundary', '2030-01-03T00:00:00Z', NULL),
        ($1, 'Point inside', '2030-01-02T00:00:00Z', NULL),
        ($1, 'Point before', '2030-01-01T12:00:00Z', NULL)",
    )
    .bind(league)
    .execute(&pool)
    .await?;
    let state = ApiState {
        database: SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone()),
        object_store: Arc::new(object_store::memory::InMemory::new()),
        oauth: None,
        hotlaps: crate::settings::HotlapSettings::default(),
        site_notice: None,
    };
    let response = body(
        state,
        "?starts_from=2030-01-02T00:00:00Z&starts_before=2030-01-03T00:00:00Z",
    )
    .await;
    assert_eq!(response["pagination"]["total_items"], 2);
    assert_eq!(response["items"][0]["title"], "Ongoing");
    assert_eq!(response["items"][0]["ends_at"], "2030-01-03T12:00:00Z");
    assert_eq!(response["items"][1]["title"], "Point inside");
    assert!(response["items"][1]["ends_at"].is_null());
    assert!(sqlx::query("INSERT INTO event (league_id, title, starts_at, ends_at) VALUES ($1, 'Invalid', '2030-01-02T00:00:00Z', '2030-01-02T00:00:00Z')")
        .bind(league).execute(&pool).await.is_err());
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn event_details_and_results_respect_visibility_and_expose_source(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let league: i64 = sqlx::query_scalar("INSERT INTO league(name) VALUES ('Detail') RETURNING id")
        .fetch_one(&pool)
        .await?;
    let attachment: i64 = sqlx::query_scalar("INSERT INTO attachment(object_key,media_type,extension) VALUES ('layout.lyt','application/octet-stream','lyt') RETURNING id").fetch_one(&pool).await?;
    let game: i64 = sqlx::query_scalar("INSERT INTO event(league_id,title,starts_at,ends_at,source,catalogue_id,game_config,layout_attachment_id) VALUES ($1,'Bomb',now()+interval '1 day',now()+interval '2 days','bomb','bomb-detail','{\"source\":{\"type\":\"bomb\",\"checkpoint_timeout_secs\":30,\"checkpoint_penalty_ms\":250,\"collision_max_penalty_ms\":500},\"track\":\"BL1\",\"vehicles\":[]}',$2) RETURNING id").bind(league).bind(attachment).fetch_one(&pool).await?;
    let imported: i64 = sqlx::query_scalar("INSERT INTO event(league_id,title,starts_at,source,external_id,source_url) VALUES ($1,'External',now()+interval '1 day','lfs_net','detail-source','https://www.lfs.net/leagues/432') RETURNING id").bind(league).fetch_one(&pool).await?;
    let state = ApiState {
        database: SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone()),
        object_store: Arc::new(object_store::memory::InMemory::new()),
        oauth: None,
        hotlaps: crate::settings::HotlapSettings::default(),
        site_notice: None,
    };
    let listing = body(state.clone(), "").await;
    let listed_external = listing["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == imported)
        .unwrap();
    assert_eq!(listed_external["source"], "lfs_net");
    assert_eq!(
        listed_external["source_url"],
        "https://www.lfs.net/leagues/432"
    );
    let listed_game = listing["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == game)
        .unwrap();
    assert_eq!(listed_game["source"], "bomb");
    assert!(listed_game["source_url"].is_null());
    let detail = body(state.clone(), &format!("/{game}")).await;
    assert_eq!(detail["source"], "bomb");
    assert_eq!(detail["game"]["source"]["type"], "bomb");
    assert!(detail.get("layout_url").is_none());
    let results = body(state.clone(), &format!("/{game}/results")).await;
    assert_eq!(results["pagination"]["total_items"], 0);
    let external = body(state.clone(), &format!("/{imported}")).await;
    assert_eq!(external["source"], "lfs_net");
    assert_eq!(external["source_url"], "https://www.lfs.net/leagues/432");
    assert!(external["game"].is_null());
    assert_eq!(
        request(state.clone(), &format!("/{game}/results?page=0"))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    sqlx::query("UPDATE event SET hidden=true WHERE id=$1")
        .bind(game)
        .execute(&pool)
        .await?;
    assert_eq!(
        request(state.clone(), &format!("/{game}")).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(state, &format!("/{game}/results")).await.status(),
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn land_speed_record_exposes_speed_and_paginated_fastest_runs(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let league: i64 =
        sqlx::query_scalar("INSERT INTO league(name) VALUES ('Speed records') RETURNING id")
            .fetch_one(&pool)
            .await?;
    let attachment: i64 = sqlx::query_scalar("INSERT INTO attachment(object_key,media_type,extension) VALUES ('speed.lyt','application/octet-stream','lyt') RETURNING id").fetch_one(&pool).await?;
    let event: i64 = sqlx::query_scalar("INSERT INTO event(league_id,title,starts_at,ends_at,source,catalogue_id,game_config,layout_attachment_id) VALUES ($1,'Land Speed Record',now()-interval '1 hour',now()+interval '1 hour','land_speed_record','speed-record','{\"source\":{\"type\":\"land_speed_record\"},\"track\":\"BL1\",\"vehicles\":[]}', $2) RETURNING id").bind(league).bind(attachment).fetch_one(&pool).await?;
    for (uname, speed_kph) in [("Alpha", 200.123), ("Beta", 250.456), ("Alpha", 210.789)] {
        crate::models::event::results::record(
            &pool,
            event,
            crate::models::event::results::Submission::LandSpeedRecord(
                lfsplanet_land_speed_record::LandSpeedRecordResult {
                    uname: uname.into(),
                    pname: uname.into(),
                    vehicle: "XFG".into(),
                    speed_kph,
                },
            ),
        )
        .await?;
    }
    let state = ApiState {
        database: SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone()),
        object_store: Arc::new(object_store::memory::InMemory::new()),
        oauth: None,
        hotlaps: crate::settings::HotlapSettings::default(),
        site_notice: None,
    };
    let detail = body(state.clone(), &format!("/{event}")).await;
    assert_eq!(detail["source"], "land_speed_record");
    assert_eq!(detail["game"]["source"]["type"], "land_speed_record");
    let first = body(state.clone(), &format!("/{event}/results?per_page=1")).await;
    assert_eq!(first["pagination"]["total_items"], 2);
    assert_eq!(first["items"][0]["username"], "Beta");
    assert_eq!(first["items"][0]["speed_kph"], 250.456);
    assert!(first["items"][0]["time_ms"].is_null());
    let second = body(state, &format!("/{event}/results?per_page=1&page=2")).await;
    assert_eq!(second["items"][0]["username"], "Alpha");
    assert_eq!(second["items"][0]["speed_kph"], 210.789);
    assert_eq!(second["items"][0]["position"], 2);
    Ok(())
}
