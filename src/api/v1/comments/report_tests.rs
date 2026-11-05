use super::*;
use crate::models::player;
use axum::http::header;
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
        serde_json::from_value::<CreateHotlapComment>(
            json!({"reason":"suspected_cheating", "body":"Evidence"})
        )
        .is_ok()
    );
    for value in [
        json!({"reason":"invalid", "body":"text"}),
        json!({"reason":"other_problem"}),
        json!({"reason":"invalid_lap","body":"text","author_id":1}),
    ] {
        assert!(serde_json::from_value::<CreateHotlapComment>(value).is_err());
    }
}
#[test]
fn patch_distinguishes_an_omitted_reason_from_withdrawal() {
    let omitted: UpdateHotlapComment = serde_json::from_value(json!({"body":"Edited"})).unwrap();
    let withdrawn: UpdateHotlapComment = serde_json::from_value(json!({"reason":null})).unwrap();
    let flagged: UpdateHotlapComment =
        serde_json::from_value(json!({"reason":"invalid_lap"})).unwrap();
    assert_eq!(omitted.reason, None);
    assert_eq!(withdrawn.reason, Some(None));
    assert_eq!(
        flagged.reason,
        Some(Some(crate::models::report_reason::ReportReason::InvalidLap))
    );
    for body in [
        json!({"parent_id":42}),
        json!({"author_id":42}),
        json!({"reason":"unknown"}),
    ] {
        assert!(serde_json::from_value::<UpdateHotlapComment>(body).is_err());
    }
}
#[test]
fn comments_and_reports_are_scoped_to_hotlaps() {
    let document = serde_json::to_value(crate::api::openapi()).unwrap();
    let paths = document["paths"].as_object().unwrap();
    assert!(!paths.keys().any(|path| path.starts_with("/api/v1/comments") || path.starts_with("/api/v1/reports")));
    for path in [
        "/api/v1/hotlaps/{hotlap}/comments/{comment}",
        "/api/v1/hotlaps/{hotlap}/comments",
    ] {
        assert!(paths.contains_key(path));
    }
    assert!(!paths.keys().any(|path| path.ends_with("/report")
        || path.ends_with("/replies")
        || path.contains("/reports/")));
    assert!(
        paths["/api/v1/hotlaps/{hotlap}/comments/{comment}"]
            .get("patch")
            .is_some()
    );
    assert!(
        paths["/api/v1/hotlaps/{hotlap}/comments/{comment}"]
            .get("get")
            .is_none()
    );
    assert!(
        paths["/api/v1/hotlaps"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter["name"] == "reported")
    );
}
// Read through the public collection, preserving the permalink's context.
async fn read_comment(app: &Router, hotlap: i64, id: i64) -> Value {
    fn find(entries: &[Value], id: i64) -> Option<Value> {
        entries.iter().find_map(|entry| {
            if entry["id"] == id {
                return Some(entry.clone());
            }
            entry["replies"]
                .as_array()
                .and_then(|replies| find(replies, id))
        })
    }
    let (status, response) = request(
        app,
        "GET",
        &format!("/api/v1/hotlaps/{hotlap}/comments?around={id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, 200);
    find(response["items"].as_array().unwrap(), id).expect("comment in its conversation page")
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
    if path.contains("/comments") {
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
        .merge(crate::api::v1::hotlaps::router())
        .split_for_parts()
        .0
        .with_state(state)
        .layer(tower_sessions::SessionManagerLayer::new(
            tower_sessions::MemoryStore::default(),
        ));
    let lap = hotlap::Entity::find().one(&database).await?.unwrap();
    let comments = format!("/api/v1/hotlaps/{}/comments", lap.id);
    let evidence = json!({"reason":"suspected_cheating","body":"Please inspect this replay."});
    assert_eq!(
        request(&app, "POST", &comments, Some(&tokens[0]), evidence.clone())
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
        request(&app, "POST", &comments, None, evidence.clone())
            .await
            .0,
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
            "POST",
            &comments,
            Some(&tokens[0]),
            json!({"reason":"invalid_lap","body":" "})
        )
        .await
        .0,
        400
    );
    let (first, duplicate) = tokio::join!(
        request(&app, "POST", &comments, Some(&tokens[0]), evidence.clone()),
        request(&app, "POST", &comments, Some(&tokens[0]), evidence.clone())
    );
    let (first, duplicate) = if first.0 == 201 {
        (first, duplicate)
    } else {
        (duplicate, first)
    };
    assert_eq!(first.0, 201);
    assert_eq!(duplicate.0, 409);
    let report_id = first.1["id"].as_i64().unwrap();
    let report_path = format!("/api/v1/hotlaps/{}/comments/{report_id}", lap.id);
    let detail = format!("/api/v1/hotlaps/{}/comments/{report_id}", lap.id);
    let replies = format!("{comments}?parent_id={report_id}");
    assert_eq!(
        request(&app, "GET", &detail, None, Value::Null).await.0,
        405
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/hotlaps/999999/comments",
            None,
            Value::Null
        )
        .await
        .0,
        404
    );
    let roots = request(&app, "GET", &comments, None, Value::Null).await.1;
    assert_eq!(roots["pagination"]["total_items"], 1);
    assert_eq!(roots["items"][0]["reason"], "suspected_cheating");
    let filtered = request(
        &app,
        "GET",
        &format!("{comments}?author_id={}&reported=true", author.id),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(filtered["items"][0]["id"], report_id);
    assert_eq!(filtered["pagination"]["total_items"], 1);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("{comments}?author_id={}&reported=true", visitor.id),
            None,
            Value::Null
        )
        .await
        .1["pagination"]["total_items"],
        0
    );
    let counts = request(
        &app,
        "GET",
        &format!("/api/v1/hotlaps/{}", lap.id),
        None,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(counts["reports"], json!({"total":1,"unresolved":1}));
    assert_eq!(
        request(
            &app,
            "POST",
            &comments,
            Some(&tokens[1]),
            json!({"body":"reply report", "reason":"invalid_lap", "parent_id":report_id})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &detail,
            Some(&tokens[1]),
            json!({"body":"someone else's text"})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(&app, "PATCH", &detail, Some(&tokens[0]), json!({}))
            .await
            .0,
        400
    );
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
        read_comment(&app, lap.id, report_id).await["reply_count"],
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
        request(
            &app,
            "GET",
            &format!("/api/v1/hotlaps/{}/comments?around={report_id}", other.id),
            None,
            Value::Null
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!(
                "/api/v1/hotlaps/{}/comments?parent_id={report_id}",
                other.id
            ),
            None,
            Value::Null
        )
        .await
        .0,
        404
    );
    request(&app, "DELETE", &wrong_scope, Some(&tokens[0]), Value::Null).await;
    assert_eq!(
        read_comment(&app, lap.id, report_id).await["deleted"],
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
        200
    );
    request(&app, "PATCH", &detail, Some(&tokens[0]), evidence).await;
    assert!(read_comment(&app, lap.id, report_id).await["resolved_at"].is_string());
    request(
        &app,
        "PATCH",
        &detail,
        Some(&tokens[0]),
        json!({"reason":"suspected_cheating","body":"New evidence."}),
    )
    .await;
    assert_eq!(
        read_comment(&app, lap.id, report_id).await["resolved_at"],
        Value::Null
    );
    // Withdraw only the flag. The text and the complete conversation remain.
    assert_eq!(
        request(
            &app,
            "PATCH",
            &report_path,
            Some(&tokens[1]),
            json!({"reason":null})
        )
        .await
        .0,
        403
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
        1
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &report_path,
            Some(&tokens[0]),
            json!({"reason":null})
        )
        .await
        .0,
        200
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
    let withdrawn = read_comment(&app, lap.id, report_id).await;
    assert_eq!(withdrawn["reason"], Value::Null);
    assert_eq!(withdrawn["body"], "New evidence.");
    assert_eq!(withdrawn["reply_count"], 1);
    // Deletion leaves a tombstone instead of orphaning or deleting descendants.
    request(&app, "DELETE", &detail, Some(&tokens[1]), Value::Null).await;
    assert_eq!(
        read_comment(&app, lap.id, report_id).await["deleted"],
        false
    );
    request(&app, "DELETE", &detail, Some(&tokens[0]), Value::Null).await;
    let deleted = read_comment(&app, lap.id, report_id).await;
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
        request(&app, "GET", &format!("{replies}&page=0"), None, Value::Null)
            .await
            .0,
        400
    );
    // A new report is allowed after withdrawal; archive remains readable after lap removal.
    let second = request(
        &app,
        "POST",
        &comments,
        Some(&tokens[0]),
        json!({"reason":"invalid_lap","body":"Different concern."}),
    )
    .await;
    assert_eq!(second.0, 201);
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
        "POST",
        &comments,
        Some(&tokens[1]),
        json!({"reason":"invalid_lap","body":"A newer report."}),
    )
    .await;
    assert_eq!(later_report.0, 201);
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
            &format!("/api/v1/hotlaps/{}/comments/{second_id}", lap.id),
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        200
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
            &format!("/api/v1/hotlaps/{}/comments/{second_id}", lap.id),
            Some(&tokens[1]),
            json!({"resolved":false})
        )
        .await
        .0,
        200
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
    assert_eq!(
        queue["items"][0]["reports"],
        json!({"total":2,"unresolved":2})
    );
    let wrong_moderation = format!("/api/v1/hotlaps/{}/comments/{second_id}", other.id);
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
                &format!("/api/v1/hotlaps/{}/comments/{id}", lap.id),
                Some(&tokens[1]),
                json!({"resolved":true})
            )
            .await
            .0,
            200
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
            &format!("/api/v1/hotlaps/{}/comments/{second_id}", lap.id),
            Some(&tokens[1]),
            json!({"resolved":false})
        )
        .await
        .0,
        200
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
    // Ordinary comments use the same update path; promotion keeps the uniqueness rule.
    let normal = request(
        &app,
        "POST",
        &comments,
        Some(&tokens[0]),
        json!({"body":"Ordinary comment"}),
    )
    .await
    .1;
    let normal_path = format!("{comments}/{}", normal["id"].as_i64().unwrap());
    assert_eq!(
        request(
            &app,
            "PATCH",
            &normal_path,
            Some(&tokens[0]),
            json!({"reason":"invalid_lap"})
        )
        .await
        .0,
        409
    );
    let edited = request(
        &app,
        "PATCH",
        &normal_path,
        Some(&tokens[0]),
        json!({"body":"Edited comment"}),
    )
    .await;
    assert_eq!(edited.0, 200);
    assert_eq!(edited.1["body"], "Edited comment");
    assert_eq!(edited.1["reason"], Value::Null);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &normal_path,
            Some(&tokens[1]),
            json!({"resolved":true})
        )
        .await
        .0,
        400
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &normal_path,
            Some(&tokens[1]),
            json!({"body":"Moderator editing someone else's words"})
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &format!("{comments}/{reply_id}"),
            Some(&tokens[1]),
            json!({"reason":"invalid_lap"})
        )
        .await
        .0,
        400
    );
    let second_path = format!("{comments}/{second_id}");
    let resolved = request(
        &app,
        "PATCH",
        &second_path,
        Some(&tokens[1]),
        json!({"resolved":true}),
    )
    .await
    .1;
    let repeated = request(
        &app,
        "PATCH",
        &second_path,
        Some(&tokens[1]),
        json!({"resolved":true}),
    )
    .await
    .1;
    assert_eq!(resolved["resolved_at"], repeated["resolved_at"]);
    let edited_report = request(
        &app,
        "PATCH",
        &second_path,
        Some(&tokens[0]),
        json!({"body":"Changed explanation, flag unchanged"}),
    )
    .await
    .1;
    assert_eq!(edited_report["reason"], "invalid_lap");
    assert_eq!(edited_report["resolved_at"], Value::Null);
    assert_eq!(
        request(
            &app,
            "PATCH",
            &second_path,
            Some(&tokens[0]),
            json!({"reason":null})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "PATCH",
            &normal_path,
            Some(&tokens[0]),
            json!({"reason":"invalid_lap"})
        )
        .await
        .0,
        200
    );
    hotlap::Entity::delete_by_id(lap.id).exec(&database).await?;
    let archive = request(&app, "GET", &comments, None, Value::Null).await;
    assert_eq!(archive.0, 200);
    assert!(archive.1["pagination"]["total_items"].as_u64().unwrap() > 0);
    assert_eq!(
        request(
            &app,
            "POST",
            &comments,
            Some(&tokens[0]),
            json!({"body":"Archived write"})
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/v1/hotlaps/{}/comments?around={report_id}", other.id),
            None,
            Value::Null
        )
        .await
        .0,
        404
    );

    assert_eq!(
        read_comment(&app, lap.id, report_id).await["hotlap_id"],
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
        read_comment(&app, lap.id, report_id).await["original_hotlap_id"],
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
    let archived = read_comment(&app, lap.id, reply_id).await;
    assert_eq!(archived["deleted"], true);
    assert!(!archived["replies"].as_array().unwrap().is_empty());
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
