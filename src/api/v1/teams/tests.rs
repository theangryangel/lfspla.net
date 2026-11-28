use super::*;
use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt;

async fn request(state: &ApiState, path: &str) -> axum::response::Response {
    crate::api::router(state.hotlaps.max_spr_upload_bytes())
        .with_state(state.clone())
        .layer(Extension(tower_sessions::Session::new(
            None,
            Arc::new(tower_sessions::MemoryStore::default()),
            None,
        )))
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body(state: &ApiState, path: &str) -> serde_json::Value {
    let response = request(state, path).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn player_profile_lists_only_their_teams_in_name_order(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(
        "INSERT INTO player (lfs_username, display_name) VALUES
         ('profile-team-owner', 'Owner'), ('profile-team-member', 'Member'),
         ('profile-team-none', 'No teams');
         INSERT INTO team (name, tag, status) VALUES
         ('profile-team Zulu', '[Z]', 'active'),
         ('profile-team alpha', NULL, 'disbanded'),
         ('profile-team unrelated', NULL, 'active');
         INSERT INTO team_member (team_id, player_id, role)
         SELECT team.id, player.id,
             CASE WHEN lfs_username = 'profile-team-owner' THEN 'owner' ELSE 'member' END
         FROM team CROSS JOIN player
         WHERE team.name IN ('profile-team Zulu', 'profile-team alpha')
             AND lfs_username IN ('profile-team-owner', 'profile-team-member');",
    )
    .execute(&pool)
    .await?;
    let state = ApiState::for_tests(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
        pool,
    ));
    for username in ["profile-team-owner", "PROFILE-TEAM-MEMBER"] {
        let profile = body(&state, &format!("/api/v1/players/{username}")).await;
        let teams = profile["teams"].as_array().unwrap();
        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0]["name"], "profile-team alpha");
        assert_eq!(teams[0]["status"], "disbanded");
        assert!(teams[0]["tag"].is_null());
        assert_eq!(teams[1]["name"], "profile-team Zulu");
        assert_eq!(teams[1]["tag"], "[Z]");
        let team = body(&state, &format!("/api/v1/teams/{}", teams[1]["id"])).await;
        assert_eq!(team["team"]["name"], teams[1]["name"]);
    }
    let profile = body(&state, "/api/v1/players/profile-team-none").await;
    assert_eq!(profile["teams"], serde_json::json!([]));
    Ok(())
}

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn directory_filters_paginates_and_exposes_public_rosters(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    sqlx::raw_sql(
        "INSERT INTO team (name, tag, tags, accepts_applications) VALUES
         ('directory-v1 100%_ Racing', '[directory-v1]', '[\"race\",\"drift\"]', true),
         ('directory-v1 100XX Racing', '[D]', '[\"drag\"]', false),
         ('directory-v1 Zulu', NULL, '[]', false);
         INSERT INTO player (lfs_username, display_name) VALUES
         ('team-ui-z-owner', 'Owner'), ('team-ui-a-member', 'Member');
         INSERT INTO team_member (team_id, player_id, role)
         SELECT team.id, player.id,
             CASE WHEN lfs_username = 'team-ui-z-owner' THEN 'owner' ELSE 'member' END
         FROM team CROSS JOIN player
         WHERE team.tag = '[directory-v1]' AND team.name LIKE 'directory-v1%'
             AND lfs_username IN ('team-ui-z-owner', 'team-ui-a-member');",
    )
    .execute(&pool)
    .await?;
    let state = ApiState::for_tests(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
        pool,
    ));
    let first = body(&state, "/api/v1/teams?q=DIRECTORY-V1&per_page=1").await;
    assert_eq!(first["pagination"]["total_items"], 3);
    assert_eq!(first["pagination"]["total_pages"], 3);
    assert_eq!(first["items"][0]["member_count"], 2);
    let id = first["items"][0]["id"].as_i64().unwrap();
    let second = body(&state, "/api/v1/teams?q=directory-v1&per_page=1&page=2").await;
    assert_eq!(second["items"][0]["tag"], "[D]");
    assert_eq!(second["items"][0]["member_count"], 0);
    for query in [
        "q=100%25_",
        "q=%5BDIRECTORY-V1%5D",
        "q=directory-v1&discipline=drift",
        "q=directory-v1&accepts_applications=true",
    ] {
        let result = body(&state, &format!("/api/v1/teams?{query}")).await;
        assert_eq!(result["pagination"]["total_items"], 1, "{query}");
        assert_eq!(result["items"][0]["id"], id);
    }
    let closed = body(
        &state,
        "/api/v1/teams?q=directory-v1&accepts_applications=false",
    )
    .await;
    assert_eq!(closed["pagination"]["total_items"], 2);
    let beyond = body(&state, "/api/v1/teams?q=directory-v1&page=100").await;
    assert_eq!(beyond["pagination"]["total_items"], 3);
    assert_eq!(beyond["items"], serde_json::json!([]));
    let absent = body(
        &state,
        "/api/v1/teams?q=directory-v1&discipline=drag&accepts_applications=true",
    )
    .await;
    assert_eq!(absent["pagination"]["total_items"], 0);
    let detail = body(&state, &format!("/api/v1/teams/{id}")).await;
    assert_eq!(detail["team"]["id"], id);
    assert_eq!(
        detail["members"][0]["player"]["lfs_username"],
        "team-ui-z-owner"
    );
    assert_eq!(detail["members"][0]["role"], "owner");
    assert_eq!(detail["members"][1]["role"], "member");
    assert_eq!(
        request(&state, "/api/v1/teams/0").await.status(),
        StatusCode::NOT_FOUND
    );
    for query in [
        "discipline=invalid",
        "accepts_applications=invalid",
        "mine=invalid",
    ] {
        assert_eq!(
            request(&state, &format!("/api/v1/teams?{query}"))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    Ok(())
}
