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
