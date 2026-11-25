use super::*;
use crate::models::player;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use tower_sessions::{MemoryStore, Session};

async fn call(
    state: &ApiState,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    call_session(state, method, path, token, body, session, None).await
}

async fn call_session(
    state: &ApiState,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
    session: Session,
    csrf: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(csrf) = csrf {
        request = request.header("X-CSRF-Token", csrf);
    }
    let response = crate::api::router(state.hotlaps.max_spr_upload_bytes())
        .with_state(state.clone())
        .layer(Extension(session))
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"text": String::from_utf8_lossy(&bytes)}));
    (status, value)
}

fn details() -> Value {
    json!({"name":"Management test", "tag":"[T]", "description":"About", "website_url":"example.com", "tags":["race","drift"], "accepts_applications":false})
}

async fn fixture(pool: sqlx::PgPool) -> anyhow::Result<(ApiState, Vec<(i64, String)>)> {
    let state = ApiState {
        database: sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool),
        oauth: None,
        object_store: Arc::new(object_store::memory::InMemory::new()),
        hotlaps: Default::default(),
        site_notice: None,
    };
    let mut players = Vec::new();
    for name in ["manage-owner", "manage-member", "manage-other"] {
        let player = player::ActiveModel {
            lfs_username: sea_orm::Set(name.to_owned()),
            display_name: sea_orm::Set(name.to_owned()),
            ..Default::default()
        }
        .insert(&state.database)
        .await?;
        let token = crate::models::PersonalAccessToken::create(
            &state.database,
            player.id,
            "test",
            time::OffsetDateTime::now_utc() + time::Duration::days(1),
        )
        .await?;
        players.push((player.id, token.token));
    }
    Ok((state, players))
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn team_management_enforces_roles_and_preserves_ownership(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players) = fixture(pool).await?;
    let (owner, token) = &players[0];
    let (member, member_token) = &players[1];
    assert_eq!(
        call(&state, "POST", "/api/v1/teams", None, details())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut invalid = details();
    invalid["name"] = json!(" ");
    assert_eq!(
        call(&state, "POST", "/api/v1/teams", Some(token), invalid)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, created) = call(&state, "POST", "/api/v1/teams", Some(token), details()).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_i64().unwrap();
    let base = format!("/api/v1/teams/{id}");
    assert_eq!(created["website_url"], "https://example.com/");
    assert_eq!(
        call(&state, "PUT", &base, Some(member_token), details())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &state,
            "DELETE",
            &format!("{base}/members/{owner}"),
            Some(token),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{base}/invites"),
            Some(member_token),
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{base}/invites"),
            Some(token),
            json!({"max_uses":0})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, invite) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(token),
        json!({"expires_in_days":7}),
    )
    .await;
    let secret = invite["token"].as_str().unwrap();
    let invite_path = format!("/api/v1/team-invites/{secret}");
    assert_eq!(
        call(&state, "GET", &invite_path, None, Value::Null).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&state, "GET", &base, None, Value::Null).await.1["team"]["member_count"],
        1
    );
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{invite_path}/accept"),
            Some(member_token),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{invite_path}/accept"),
            Some(member_token),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, invites) = call(
        &state,
        "GET",
        &format!("{base}/invites"),
        Some(token),
        Value::Null,
    )
    .await;
    assert_eq!(invites["items"][0]["uses"], 1);
    assert!(invites["items"][0].get("token").is_none());
    let detail = call(&state, "GET", &base, None, Value::Null).await.1;
    assert_eq!(detail["members"][0]["role"], "owner");
    assert_eq!(detail["members"][1]["role"], "member");
    let (_, other_team) = call(&state, "POST", "/api/v1/teams", Some(token), details()).await;
    let foreign = format!(
        "/api/v1/teams/{}/invites/{}",
        other_team["id"], invite["invite"]["id"]
    );
    assert_eq!(
        call(&state, "DELETE", &foreign, Some(token), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, spare) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(token),
        json!({}),
    )
    .await;
    let spare_id = spare["invite"]["id"].clone();
    let spare_path = format!("{base}/invites/{spare_id}");
    let invites_path = format!("{base}/invites");
    let (state_ref, invites_ref, spare_ref) = (&state, &invites_path, &spare_id);
    let revoked_at = move || async move {
        let (_, invites) = call(state_ref, "GET", invites_ref, Some(token), Value::Null).await;
        invites["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == *spare_ref)
            .unwrap()["revoked_at"]
            .clone()
    };
    assert_eq!(
        call(&state, "DELETE", &spare_path, Some(token), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let first_revoked_at = revoked_at().await;
    assert!(!first_revoked_at.is_null());
    assert_eq!(
        call(&state, "DELETE", &spare_path, Some(token), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(revoked_at().await, first_revoked_at);
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{base}/transfer-ownership"),
            Some(token),
            json!({"player_id":member})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&state, "GET", &invite_path, None, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&state, "PUT", &base, Some(token), details()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&state, "PUT", &base, Some(member_token), details())
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &state,
            "DELETE",
            &format!("{base}/members/{owner}"),
            Some(token),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &state,
            "DELETE",
            &format!("{base}/members/{member}"),
            Some(member_token),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, invite) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(member_token),
        json!({}),
    )
    .await;
    assert_eq!(
        call(
            &state,
            "PUT",
            &format!("{base}/status"),
            Some(member_token),
            json!({"status":"disbanded"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &state,
            "POST",
            &format!("{base}/invites"),
            Some(member_token),
            json!({})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let detail = call(&state, "GET", &base, None, Value::Null).await.1;
    assert_eq!(detail["team"]["status"], "disbanded");
    assert_eq!(detail["team"]["accepts_applications"], false);
    let mut open = details();
    open["accepts_applications"] = json!(true);
    assert_eq!(
        call(&state, "PUT", &base, Some(member_token), open).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &state,
            "PUT",
            &format!("{base}/status"),
            Some(member_token),
            json!({"status":"active"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let path = format!("/api/v1/team-invites/{}", invite["token"].as_str().unwrap());
    assert_eq!(
        call(&state, "GET", &path, None, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn limited_invites_are_atomic_and_browser_writes_require_csrf(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players) = fixture(pool).await?;
    let (owner, token) = &players[0];
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    crate::api::extractors::login(&session, *owner).await?;
    let csrf = crate::api::extractors::csrf_token(&session).await?;
    assert_eq!(
        call_session(
            &state,
            "POST",
            "/api/v1/teams",
            None,
            details(),
            session.clone(),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, created) = call_session(
        &state,
        "POST",
        "/api/v1/teams",
        None,
        details(),
        session,
        Some(&csrf),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let base = format!("/api/v1/teams/{}", created["id"]);
    let (_, invite) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(token),
        json!({"max_uses":1}),
    )
    .await;
    let accept = format!(
        "/api/v1/team-invites/{}/accept",
        invite["token"].as_str().unwrap()
    );
    let (first, second) = tokio::join!(
        call(&state, "POST", &accept, Some(&players[1].1), Value::Null),
        call(&state, "POST", &accept, Some(&players[2].1), Value::Null)
    );
    let mut statuses = [first.0.as_u16(), second.0.as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [200, 404]);
    assert_eq!(
        call(&state, "GET", &base, None, Value::Null).await.1["team"]["member_count"],
        2
    );
    let (_, invite) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(token),
        json!({"expires_in_days":1}),
    )
    .await;
    let invite_id = invite["invite"]["id"].as_i64().unwrap();
    crate::models::team::invite::Entity::update_many()
        .col_expr(
            crate::models::team::invite::Column::ExpiresAt,
            Expr::value(time::OffsetDateTime::UNIX_EPOCH),
        )
        .filter(crate::models::team::invite::Column::Id.eq(invite_id))
        .exec(&state.database)
        .await?;
    let path = format!("/api/v1/team-invites/{}", invite["token"].as_str().unwrap());
    assert_eq!(
        call(&state, "GET", &path, None, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn mine_filters_owner_and_member_teams_before_pagination(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players) = fixture(pool).await?;
    let (_, owner_token) = &players[0];
    let (_, member_token) = &players[1];
    let (_, other_token) = &players[2];
    assert_eq!(
        call(&state, "GET", "/api/v1/teams?mine=true", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, empty) = call(
        &state,
        "GET",
        "/api/v1/teams?mine=true",
        Some(owner_token),
        Value::Null,
    )
    .await;
    assert_eq!(empty["pagination"]["total_items"], 0);
    let mut owned = details();
    owned["name"] = json!("mine-test A");
    owned["tags"] = json!(["race"]);
    let (_, owned) = call(&state, "POST", "/api/v1/teams", Some(owner_token), owned).await;
    let mut joined = details();
    joined["name"] = json!("mine-test B");
    joined["tags"] = json!(["drift"]);
    joined["accepts_applications"] = json!(true);
    let (_, joined) = call(&state, "POST", "/api/v1/teams", Some(member_token), joined).await;
    let mut unrelated = details();
    unrelated["name"] = json!("mine-test C");
    call(
        &state,
        "POST",
        "/api/v1/teams",
        Some(other_token),
        unrelated,
    )
    .await;
    let (_, invite) = call(
        &state,
        "POST",
        &format!("/api/v1/teams/{}/invites", joined["id"]),
        Some(member_token),
        json!({}),
    )
    .await;
    let path = format!(
        "/api/v1/team-invites/{}/accept",
        invite["token"].as_str().unwrap()
    );
    assert_eq!(
        call(&state, "POST", &path, Some(owner_token), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let (_, first) = call(
        &state,
        "GET",
        "/api/v1/teams?mine=true&q=mine-test&per_page=1",
        Some(owner_token),
        Value::Null,
    )
    .await;
    assert_eq!(first["pagination"]["total_items"], 2);
    assert_eq!(first["pagination"]["total_pages"], 2);
    assert_eq!(first["items"][0]["id"], owned["id"]);
    let (_, second) = call(
        &state,
        "GET",
        "/api/v1/teams?mine=true&q=mine-test&per_page=1&page=2",
        Some(owner_token),
        Value::Null,
    )
    .await;
    assert_eq!(second["items"][0]["id"], joined["id"]);
    assert_eq!(second["items"][0]["member_count"], 2);
    let (_, filtered) = call(
        &state,
        "GET",
        "/api/v1/teams?mine=true&discipline=drift&accepts_applications=true",
        Some(owner_token),
        Value::Null,
    )
    .await;
    assert_eq!(filtered["pagination"]["total_items"], 1);
    assert_eq!(filtered["items"][0]["id"], joined["id"]);
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    crate::api::extractors::login(&session, players[0].0).await?;
    let (_, cookie) = call_session(
        &state,
        "GET",
        "/api/v1/teams?mine=true",
        None,
        Value::Null,
        session,
        None,
    )
    .await;
    assert_eq!(cookie["pagination"]["total_items"], 2);
    for query in ["q=mine-test", "q=mine-test&mine=false"] {
        let (status, all) = call(
            &state,
            "GET",
            &format!("/api/v1/teams?{query}"),
            None,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(all["pagination"]["total_items"], 3);
    }
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn multiple_owners_can_manage_roles_without_losing_the_last_owner(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players) = fixture(pool).await?;
    let (first, first_token) = &players[0];
    let (second, second_token) = &players[1];
    let (_, created) = call(
        &state,
        "POST",
        "/api/v1/teams",
        Some(first_token),
        details(),
    )
    .await;
    let base = format!("/api/v1/teams/{}", created["id"]);
    let first_role = format!("{base}/members/{first}/role");
    let second_role = format!("{base}/members/{second}/role");
    assert_eq!(
        call(
            &state,
            "PUT",
            &first_role,
            Some(first_token),
            json!({"role":"owner"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &state,
            "PUT",
            &first_role,
            Some(first_token),
            json!({"role":"member"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, invite) = call(
        &state,
        "POST",
        &format!("{base}/invites"),
        Some(first_token),
        json!({}),
    )
    .await;
    let accept = format!(
        "/api/v1/team-invites/{}/accept",
        invite["token"].as_str().unwrap()
    );
    call(&state, "POST", &accept, Some(second_token), Value::Null).await;
    assert_eq!(
        call(
            &state,
            "PUT",
            &second_role,
            Some(second_token),
            json!({"role":"owner"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &state,
            "PUT",
            &second_role,
            Some(first_token),
            json!({"role":"owner"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, detail) = call(&state, "GET", &base, None, Value::Null).await;
    assert_eq!(
        detail["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|member| member["role"] == "owner")
            .count(),
        2
    );
    for token in [first_token, second_token] {
        assert_eq!(
            call(&state, "PUT", &base, Some(token), details()).await.0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        call(
            &state,
            "PUT",
            &first_role,
            Some(second_token),
            json!({"role":"member"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&state, "PUT", &base, Some(first_token), details())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &state,
            "PUT",
            &second_role,
            Some(second_token),
            json!({"role":"member"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &state,
            "PUT",
            &first_role,
            Some(second_token),
            json!({"role":"owner"})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (a, b) = tokio::join!(
        call(
            &state,
            "PUT",
            &first_role,
            Some(first_token),
            json!({"role":"member"})
        ),
        call(
            &state,
            "PUT",
            &second_role,
            Some(second_token),
            json!({"role":"member"})
        )
    );
    let mut statuses = [a.0.as_u16(), b.0.as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [204, 409]);
    let (_, detail) = call(&state, "GET", &base, None, Value::Null).await;
    assert_eq!(
        detail["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|member| member["role"] == "owner")
            .count(),
        1
    );
    Ok(())
}
