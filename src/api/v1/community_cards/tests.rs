use super::*;
use crate::services::catalogue;
use sea_orm::SqlxPostgresConnector;
use std::sync::Arc;

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn endpoint_returns_only_visible_cards_in_stable_order_with_public_images(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let store = Arc::new(object_store::memory::InMemory::new());
    let directory = tempfile::tempdir()?;
    std::fs::write(directory.path().join("banner.svg"), "<svg/>")?;
    let mut paths = Vec::new();
    for (id, enabled, sequence) in [
        ("hidden", false, 0),
        ("last", true, 20),
        ("beta", true, 10),
        ("alpha", true, 10),
    ] {
        let path = directory.path().join(format!("{id}.yaml"));
        std::fs::write(
            &path,
            format!(
                "kind: CommunityCard\nid: {id}\ntitle: {id}\nenabled: {enabled}\nsequence: {sequence}\nimage: banner.svg\ncontent: {{ type: image, alt: Banner }}\n"
            ),
        )?;
        paths.push(path);
    }
    let sources = catalogue::load::files(&paths)?;
    catalogue::seed(&database, &sources, Some(store.as_ref()), false, |_| Ok(())).await?;
    sqlx::raw_sql(r#"
        INSERT INTO community_card (id, title, starts_at, ends_at, content) VALUES
        ('future', 'Future card', now() + interval '1 day', NULL, '{"type":"announcement","body":"Not yet visible"}'),
        ('expired', 'Expired card', NULL, now() - interval '1 day', '{"type":"announcement","body":"No longer visible"}');
    "#).execute(&pool).await?;
    let state = ApiState {
        database,
        object_store: store,
        oauth: None,
        hotlaps: crate::settings::HotlapSettings::default(),
        site_notice: None,
    };
    // Exercise the routed HTTP representation, including the tagged enum schema.
    use tower::ServiceExt;
    let app = router().with_state(state).split_for_parts().0;
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/community-cards")
                .body(axum::body::Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    let items = json["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|card| card["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["alpha", "beta", "last"]
    );
    assert_eq!(items[0]["content"]["type"], "image");
    assert!(
        items[0]["image"]["url"]
            .as_str()
            .unwrap()
            .starts_with("/api/v1/attachments/")
    );
    assert!(items[0].get("image_attachment_id").is_none());
    Ok(())
}
