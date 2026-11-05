use super::*;
use crate::models::player;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use tower::ServiceExt;
#[test]
fn reports_require_a_known_reason_and_an_explanation() {
    assert!(
        serde_json::from_value::<SaveReport>(
            json!({"reason":"suspected_cheating", "body":"Evidence"})
        )
        .is_ok()
    );
    for value in [
        json!({"reason":"invalid", "body":"text"}),
        json!({"reason":"other_problem"}),
        json!({"body":"text"}),
        json!({"reason":"invalid_lap","body":"text","author_id":1}),
    ] {
        assert!(serde_json::from_value::<SaveReport>(value).is_err());
    }
}
#[test]
fn comments_and_reports_are_scoped_to_hotlaps() {
    let document = serde_json::to_value(crate::api::openapi()).unwrap();
    let paths = document["paths"].as_object().unwrap();
    assert!(!paths.keys().any(|path| path.starts_with("/api/v1/comments") || path.starts_with("/api/v1/reports")));
    for path in [
        "/api/v1/hotlaps/{hotlap}/comments/{comment}",
        "/api/v1/hotlaps/{hotlap}/comments/{comment}/replies",
        "/api/v1/hotlaps/{hotlap}/report",
        "/api/v1/hotlaps/{hotlap}/comments/{comment}/report",
        "/api/v1/hotlaps/reports/summaries",
    ] {
        assert!(paths.contains_key(path));
    }
    assert!(
        paths["/api/v1/hotlaps"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter["name"] == "reported")
    );
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    if !path.starts_with("/api/v1/hotlaps?") {
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        },
    )
}
#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn database_tests_unified_reports_and_nested_replies(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!(
        "../../../services/validate_hotlap/fixtures.sql"
    ))
    .execute(&pool)
    .await?;
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let author = player::Entity::find().one(&database).await?.unwrap();
    let visitor = crate::models::Player::record_authentication(
        &database,
        &lfsplanet_lfs_api::LfsAccount {
            username: "reporter".into(),
            display_name: "Reporter".into(),
            lfsworld_id: 123,
        },
    )
    .await?;
    let mut tokens = Vec::new();
    for player in [&author, &visitor] {
        tokens.push(
            crate::models::PersonalAccessToken::issue(
                &database,
                player.id,
                crate::models::personal_access_token::CreatePersonalAccessToken {
                    name: "conversation test".into(),
                    expires_in_days: Some(1),
                },
            )
            .await?
            .token,
        );
    }
    let state = ApiState {
        database: database.clone(),
        oauth: None,
        site_notice: None,
        hotlaps: Default::default(),
        object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
    };
    let app = router()
        .merge(crate::api::v1::comments::router())
        .merge(crate::api::v1::hotlaps::router())
        .split_for_parts()
        .0
        .with_state(state)
        .layer(tower_sessions::SessionManagerLayer::new(
            tower_sessions::MemoryStore::default(),
        ));
    let lap = hotlap::Entity::find().one(&database).await?.unwrap();
    let mine = format!("/api/v1/hotlaps/{}/report", lap.id);
    let comments = format!("/api/v1/hotlaps/{}/comments", lap.id);
    let evidence = json!({"reason":"suspected_cheating","body":"Please inspect this replay."});
    assert_eq!(
        request(&app, "PUT", &mine, Some(&tokens[0]), evidence.clone())
            .await
            .0,
        404
    );
    sqlx::query("UPDATE hotlap SET state = 'valid', hlvc_result_code = 1, finished_at = now()")
        .execute(&pool)
        .await?;
    assert_eq!(
        request(&app, "GET", &comments, None, Value::Null).await.0,
        200
    );
    assert_eq!(
        request(&app, "PUT", &mine, None, evidence.clone()).await.0,
        401
    );
    assert_eq!(
        request(&app, "POST", &comments, None, json!({"body":"anonymous"}))
            .await
            .0,
        401
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &mine,
            Some(&tokens[0]),
            json!({"reason":"invalid_lap","body":" "})
        )
        .await
        .0,
        400
    );
    let (first, duplicate) = tokio::join!(
        request(&app, "PUT", &mine, Some(&tokens[0]), evidence.clone()),
        request(&app, "PUT", &mine, Some(&tokens[0]), evidence.clone())
    );
    assert_eq!(first.0, 200);
    assert_eq!(duplicate.0, 200);
    assert_eq!(first.1["id"], duplicate.1["id"]);
    let report_id = first.1["id"].as_i64().unwrap();
    let report_path = format!("/api/v1/hotlaps/{}/comments/{report_id}/report", lap.id);
    let detail = format!("/api/v1/hotlaps/{}/comments/{report_id}", lap.id);
    let replies = format!("{detail}/replies");
    let roots = request(&app, "GET", &comments, None, Value::Null).await.1;
    assert_eq!(roots["pagination"]["total_items"], 1);
    assert_eq!(roots["items"][0]["reason"], "suspected_cheating");
    let reply = request(
        &app,
        "POST",
        &comments,
        Some(&tokens[1]),
        json!({"body":"Here is an explanation.","parent_id":report_id}),
    )
    .await;
    assert_eq!(reply.0, 201);
    assert_eq!(reply.1["root_id"], report_id);
    assert_eq!(reply.1["depth"], 1);
    let reply_id = reply.1["id"].as_i64().unwrap();
    let nested = request(
        &app,
        "POST",
        &comments,
        Some(&tokens[0]),
        json!({"body":"Thanks, what about this sector?","parent_id":reply_id}),
    )
    .await;
    assert_eq!(nested.0, 201);
    assert_eq!(nested.1["root_id"], report_id);
    assert_eq!(nested.1["depth"], 2);
    assert_eq!(
        request(&app, "GET", &comments, None, Value::Null).await.1["pagination"]["total_items"],
        1
    );
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["reply_count"],
        1
    );
    assert_eq!(
        request(&app, "GET", &replies, None, Value::Null).await.1["items"][0]["id"],
        reply_id
    );
    let tree = request(&app, "GET", &comments, None, Value::Null).await.1;
    assert_eq!(tree["pagination"]["total_items"], 1);
    assert_eq!(tree["items"][0]["replies"][0]["id"], reply_id);
    assert_eq!(
        tree["items"][0]["replies"][0]["replies"][0]["id"],
        nested.1["id"]
    );
    let invalid = request(
        &app,
        "POST",
        &comments,
        Some(&tokens[0]),
        json!({"body":"wrong parent","parent_id":999999}),
    )
    .await;
    assert_eq!(invalid.0, 404);
    // A reply cannot attach to a different lap, and immutable parents prevent cycles.
    let mut other = hotlap::Entity::find_by_id(lap.id)
        .one(&database)
        .await?
        .unwrap()
        .into_active_model();
    other.id = sea_orm::ActiveValue::NotSet;
    other.fingerprint = Set("b".repeat(64));
    let other = other.insert(&database).await?;
    assert_eq!(
        request(
            &app,
            "POST",
            &format!("/api/v1/hotlaps/{}/comments", other.id),
            Some(&tokens[0]),
            json!({"body":"cross-lap","parent_id":report_id})
        )
        .await
        .0,
        404
    );
    let wrong_scope = format!("/api/v1/hotlaps/{}/comments/{report_id}", other.id);
    assert_eq!(
        request(&app, "GET", &wrong_scope, None, Value::Null)
            .await
            .0,
        404
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{wrong_scope}/replies"),
            None,
            Value::Null
        )
        .await
        .0,
        404
    );
    request(&app, "DELETE", &wrong_scope, Some(&tokens[0]), Value::Null).await;
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["deleted"],
        false
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &report_path,
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        403
    );
    visitor
        .clone()
        .set_role(&database, crate::models::player::Role::Moderator)
        .await?;
    assert_eq!(
        request(
            &app,
            "PATCH",
            &report_path,
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        204
    );
    request(&app, "PUT", &mine, Some(&tokens[0]), evidence).await;
    assert!(request(&app, "GET", &detail, None, Value::Null).await.1["resolved_at"].is_string());
    request(
        &app,
        "PUT",
        &mine,
        Some(&tokens[0]),
        json!({"reason":"suspected_cheating","body":"New evidence."}),
    )
    .await;
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["resolved_at"],
        Value::Null
    );
    // Withdraw only the flag. The text and the complete conversation remain.
    request(&app, "DELETE", &report_path, Some(&tokens[1]), Value::Null).await;
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=true",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        1
    );
    request(&app, "DELETE", &report_path, Some(&tokens[0]), Value::Null).await;
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=true",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        0
    );
    let withdrawn = request(&app, "GET", &detail, None, Value::Null).await.1;
    assert_eq!(withdrawn["reason"], Value::Null);
    assert_eq!(withdrawn["body"], "New evidence.");
    assert_eq!(withdrawn["reply_count"], 1);
    // Deletion leaves a tombstone instead of orphaning or deleting descendants.
    request(&app, "DELETE", &detail, Some(&tokens[1]), Value::Null).await;
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["deleted"],
        false
    );
    request(&app, "DELETE", &detail, Some(&tokens[0]), Value::Null).await;
    let deleted = request(&app, "GET", &detail, None, Value::Null).await.1;
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["body"], Value::Null);
    assert_eq!(
        request(&app, "GET", &replies, None, Value::Null).await.1["pagination"]["total_items"],
        1
    );
    let mut parent = nested.1["id"].as_i64().unwrap();
    for depth in 3..=8 {
        let child = request(
            &app,
            "POST",
            &comments,
            Some(&tokens[0]),
            json!({"body":"nested","parent_id":parent}),
        )
        .await;
        assert_eq!(child.0, 201);
        assert_eq!(child.1["depth"], depth);
        parent = child.1["id"].as_i64().unwrap();
    }
    assert_eq!(
        request(
            &app,
            "POST",
            &comments,
            Some(&tokens[0]),
            json!({"body":"too deep","parent_id":parent})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(&app, "GET", &format!("{replies}?page=0"), None, Value::Null)
            .await
            .0,
        400
    );
    // A new report is allowed after withdrawal; archive remains readable after lap removal.
    let second = request(
        &app,
        "PUT",
        &mine,
        Some(&tokens[0]),
        json!({"reason":"invalid_lap","body":"Different concern."}),
    )
    .await;
    assert_eq!(second.0, 200);
    assert_ne!(second.1["id"], report_id);
    let duplicate = sqlx::query("INSERT INTO hotlap_comment (original_hotlap_id, hotlap_id, author_id, reason, body) VALUES ($1,$1,$2,'invalid_lap','duplicate')").bind(lap.id).bind(author.id).execute(&pool).await.unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    // After posting, locate the normal root page instead of isolating the comment.
    let added = sqlx::query_scalar::<_, i64>("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body) SELECT original_hotlap_id,hotlap_id,author_id,'page test' FROM hotlap_comment CROSS JOIN generate_series(1,55) WHERE id=$1 RETURNING id")
        .bind(report_id).fetch_all(&pool).await?;
    let last_id = *added.last().unwrap();
    let around = request(
        &app,
        "GET",
        &format!("{comments}?around={last_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(around.0, 200);
    assert_eq!(around.1["pagination"]["page"], 2);
    assert!(
        around.1["items"].as_array().unwrap().len() > 1,
        "surrounding conversations remain visible"
    );
    assert!(
        around.1["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == last_id)
    );
    let parent_page = request(
        &app,
        "GET",
        &format!("{comments}?around={reply_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(parent_page.1["pagination"]["page"], 1);
    assert!(
        parent_page.1["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == report_id)
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{comments}?around=999999"),
            None,
            Value::Null
        )
        .await
        .0,
        404
    );
    // Ordering happens before pagination; report status changes move whole roots.
    let second_id = second.1["id"].as_i64().unwrap();
    let later_report = request(
        &app,
        "PUT",
        &mine,
        Some(&tokens[1]),
        json!({"reason":"invalid_lap","body":"A newer report."}),
    )
    .await;
    assert_eq!(later_report.0, 200);
    let later_id = later_report.1["id"].as_i64().unwrap();
    let first_page = request(
        &app,
        "GET",
        &format!("{comments}?per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(first_page["items"][0]["id"], second_id);
    let second_page = request(
        &app,
        "GET",
        &format!("{comments}?page=2&per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(second_page["items"][0]["id"], later_id);
    let locate_report = request(
        &app,
        "GET",
        &format!("{comments}?around={later_id}&per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(locate_report["pagination"]["page"], 2);
    let locate_reply = request(
        &app,
        "GET",
        &format!("{comments}?around={reply_id}&per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(locate_reply["pagination"]["page"], 3);
    assert_eq!(locate_reply["items"][0]["id"], report_id);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &format!("/api/v1/hotlaps/{}/comments/{second_id}/report", lap.id),
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        204
    );
    let resolved_order = request(
        &app,
        "GET",
        &format!("{comments}?per_page=3"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(resolved_order["items"][0]["id"], later_id);
    assert_eq!(resolved_order["items"][1]["id"], report_id);
    assert_eq!(resolved_order["items"][2]["id"], second_id);
    let locate_resolved = request(
        &app,
        "GET",
        &format!("{comments}?around={second_id}&per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(locate_resolved["pagination"]["page"], 3);
    assert_eq!(locate_resolved["items"][0]["id"], second_id);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &format!("/api/v1/hotlaps/{}/comments/{second_id}/report", lap.id),
            Some(&tokens[1]),
            json!({"resolved":false})
        )
        .await
        .0,
        204
    );
    let queue = request(
        &app,
        "GET",
        "/api/v1/hotlaps?reported=true",
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(
        queue["pagination"]["total_items"], 1,
        "multiple reports yield one hotlap"
    );
    assert_eq!(queue["items"][0]["id"], lap.id);
    let wrong_moderation = format!("/api/v1/hotlaps/{}/comments/{second_id}/report", other.id);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &wrong_moderation,
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        404
    );
    for id in [second_id, later_id] {
        assert_eq!(
            request(
                &app,
                "PATCH",
                &format!("/api/v1/hotlaps/{}/comments/{id}/report", lap.id),
                Some(&tokens[1]),
                json!({"resolved":true})
            )
            .await
            .0,
            204
        );
    }
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=true",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        0
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=false",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        2
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &format!("/api/v1/hotlaps/{}/comments/{second_id}/report", lap.id),
            Some(&tokens[1]),
            json!({"resolved":false})
        )
        .await
        .0,
        204
    );
    let reopened_order = request(
        &app,
        "GET",
        &format!("{comments}?per_page=1"),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(reopened_order["items"][0]["id"], second_id);
    hotlap::Entity::delete_by_id(lap.id).exec(&database).await?;
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["hotlap_id"],
        Value::Null
    );
    assert_eq!(
        request(&app, "GET", &replies, None, Value::Null).await.1["items"][0]["body"],
        "Here is an explanation."
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=true",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        0
    );

    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.1["original_hotlap_id"],
        lap.id
    );
    let archived_reply = format!("/api/v1/hotlaps/{}/comments/{reply_id}", lap.id);
    assert_eq!(
        request(
            &app,
            "DELETE",
            &archived_reply,
            Some(&tokens[1]),
            Value::Null
        )
        .await
        .0,
        204
    );
    let archived = request(&app, "GET", &archived_reply, None, Value::Null).await;
    assert_eq!(archived.0, 200);
    assert_eq!(archived.1["deleted"], true);
    assert!(!archived.1["replies"].as_array().unwrap().is_empty());
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps?reported=false",
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        1,
        "archived null references must not break the inverse filter"
    );
    Ok(())
}
