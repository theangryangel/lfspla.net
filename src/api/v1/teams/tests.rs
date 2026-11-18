use super::*;
use sea_orm::SqlxPostgresConnector;

#[test]
fn team_details_reject_unsafe_links_and_blank_names() {
    for (name, url) in [
        (" ", None),
        ("Team", Some("javascript:alert(1)")),
        ("Team", Some("https://user:password@example.com")),
    ] {
        assert!(
            CreateTeamRequest {
                name: name.into(),
                description: None,
                website_url: url.map(str::to_owned),
                discord_url: None
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn invite_tokens_are_bounded_and_not_stored_verbatim() {
    let token = hex::encode([42_u8; 32]);
    let hash = token_hash(&token).unwrap();
    assert_eq!(hash.len(), 32);
    assert_ne!(hash, token.as_bytes());
    for invalid in [
        "".to_owned(),
        "a".repeat(63),
        "a".repeat(65),
        "G".repeat(64),
        "é".repeat(32),
    ] {
        assert!(token_hash(&invalid).is_err());
    }
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn shared_links_require_admin_and_preserve_existing_memberships(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let players: Vec<crate::models::Player> = {
        let mut players = Vec::new();
        for name in ["creator", "admin", "member", "newcomer"] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO player (lfs_username, display_name) VALUES ($1, $1) RETURNING id",
            )
            .bind(name)
            .fetch_one(&pool)
            .await?;
            players.push(
                crate::models::player::Entity::find_by_id(id)
                    .one(&database)
                    .await?
                    .unwrap(),
            );
        }
        players
    };
    let id: i64 = sqlx::query_scalar("INSERT INTO team (name) VALUES ('Test team') RETURNING id")
        .fetch_one(&pool)
        .await?;
    for (player, role) in [
        (&players[0], "admin"),
        (&players[1], "admin"),
        (&players[2], "member"),
    ] {
        sqlx::query("INSERT INTO team_member (team_id, player_id, role) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(player.id)
            .bind(role)
            .execute(&pool)
            .await?;
    }
    let state = ApiState {
        database,
        object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
        oauth: None,
        hotlaps: Default::default(),
        site_notice: None,
    };
    assert!(
        regenerate(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    let Json(link) = regenerate(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    let Json(public) = preview(State(state.clone()), Path(link.token.clone())).await?;
    assert_eq!(public.id, id);
    let public_json = serde_json::to_value(&public)?;
    assert!(public_json.get("invite_token_hash").is_none());

    // Exercise the actual HTTP authentication/CSRF boundary as well as handlers.
    use tower::ServiceExt;
    let session = tower_sessions::Session::new(
        None,
        std::sync::Arc::new(tower_sessions::MemoryStore::default()),
        None,
    );
    let app = router().with_state(state.clone()).split_for_parts().0;
    let request = || {
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/v1/team-invites/{}", link.token))
            .extension(session.clone())
    };
    assert_eq!(
        app.clone()
            .oneshot(request().body(axum::body::Body::empty())?)
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    crate::api::extractors::login(&session, players[2].id).await?;
    let csrf = crate::api::extractors::csrf_token(&session).await?;
    assert_eq!(
        app.clone()
            .oneshot(request().body(axum::body::Body::empty())?)
            .await?
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.oneshot(
            request()
                .header("X-CSRF-Token", csrf)
                .body(axum::body::Body::empty())?
        )
        .await?
        .status(),
        StatusCode::NO_CONTENT
    );

    let (status, Json(created)) = create(
        State(state.clone()),
        BrowserAuthenticatedPlayer(players[3].clone()),
        Json(CreateTeamRequest {
            name: "New team".into(),
            description: None,
            website_url: None,
            discord_url: None,
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!created.invite_enabled);
    assert_eq!(
        role(&state.database, created.id, players[3].id).await?,
        Some(Role::Admin)
    );
    let Json(directory) = list(
        State(state.clone()),
        Query(TeamListQuery::default()),
        Query(PaginationQuery::default()),
    )
    .await?;
    assert_eq!(directory.items.len(), 2);
    assert_eq!(directory.pagination.total_items, 2);
    let Json(second_page) = list(
        State(state.clone()),
        Query(TeamListQuery::default()),
        Query(PaginationQuery {
            page: 2,
            per_page: 1,
        }),
    )
    .await?;
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].team.id, id);
    assert_eq!(second_page.pagination.total_pages, 2);
    let Json(filtered) = list(
        State(state.clone()),
        Query(TeamListQuery {
            q: Some("NEW".into()),
            ..Default::default()
        }),
        Query(PaginationQuery {
            page: 1,
            per_page: 1,
        }),
    )
    .await?;
    assert_eq!(filtered.items[0].team.id, created.id);
    assert_eq!(filtered.pagination.total_items, 1);
    let Json(literal) = list(
        State(state.clone()),
        Query(TeamListQuery {
            q: Some("%".into()),
            ..Default::default()
        }),
        Query(PaginationQuery::default()),
    )
    .await?;
    assert_eq!(literal.pagination.total_items, 0);
    assert!(
        team_member::Entity::find_by_id((id, players[3].id))
            .one(&state.database)
            .await?
            .is_none()
    );
    for player in [&players[1], &players[3], &players[3]] {
        assert_eq!(
            join(
                State(state.clone()),
                Path(link.token.clone()),
                BrowserAuthenticatedPlayer(player.clone())
            )
            .await?,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        role(&state.database, id, players[1].id).await?,
        Some(Role::Admin)
    );
    assert_eq!(
        role(&state.database, id, players[3].id).await?,
        Some(Role::Member)
    );
    let joined = team_member::Entity::find_by_id((id, players[3].id))
        .one(&state.database)
        .await?
        .unwrap()
        .joined_at;
    join(
        State(state.clone()),
        Path(link.token.clone()),
        BrowserAuthenticatedPlayer(players[3].clone()),
    )
    .await?;
    assert_eq!(
        team_member::Entity::find_by_id((id, players[3].id))
            .one(&state.database)
            .await?
            .unwrap()
            .joined_at,
        joined
    );
    let Json(replacement) = regenerate(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[1].clone()),
    )
    .await?;
    assert!(
        preview(State(state.clone()), Path(link.token.clone()))
            .await
            .is_err()
    );
    assert!(
        join(
            State(state.clone()),
            Path(link.token),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    assert!(
        disable(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    disable(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    assert!(
        preview(State(state.clone()), Path(replacement.token))
            .await
            .is_err()
    );

    let edit_request = || UpdateTeamRequest {
        name: "Updated team".into(),
        description: Some("Welcome to our team\nRace fairly".into()),
        website_url: Some("https://example.com".into()),
        discord_url: None,
    };
    assert!(
        update(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[2].clone()),
            Json(edit_request())
        )
        .await
        .is_err()
    );
    let Json(updated) = update(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[1].clone()),
        Json(edit_request()),
    )
    .await?;
    assert_eq!(updated.name, "Updated team");
    assert_eq!(
        updated.description.as_deref(),
        Some("Welcome to our team\nRace fairly")
    );
    let mut malicious = edit_request();
    malicious.description = Some("x".repeat(5001));
    assert!(
        update(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[0].clone()),
            Json(malicious)
        )
        .await
        .is_err()
    );
    assert!(
        archive(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    let Json(live_link) = regenerate(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    let Json(archived) = archive(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    assert!(!archived.is_active);
    assert!(!archived.invite_enabled);
    assert!(get(State(state.clone()), Path(id)).await.is_ok());
    assert!(role(&state.database, id, players[3].id).await?.is_some());
    assert!(
        preview(State(state.clone()), Path(live_link.token.clone()))
            .await
            .is_err()
    );
    assert!(
        join(
            State(state.clone()),
            Path(live_link.token.clone()),
            BrowserAuthenticatedPlayer(players[3].clone())
        )
        .await
        .is_err()
    );
    assert!(
        regenerate(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[0].clone())
        )
        .await
        .is_err()
    );
    assert!(
        update(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[0].clone()),
            Json(edit_request())
        )
        .await
        .is_err()
    );
    assert_eq!(
        list(
            State(state.clone()),
            Query(TeamListQuery::default()),
            Query(PaginationQuery::default())
        )
        .await?
        .0
        .items
        .len(),
        1
    );
    assert_eq!(
        list(
            State(state.clone()),
            Query(TeamListQuery {
                include_archived: true,
                ..Default::default()
            }),
            Query(PaginationQuery::default())
        )
        .await?
        .0
        .items
        .len(),
        2
    );
    assert!(
        restore(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    let Json(restored) = restore(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    assert!(restored.is_active);
    assert!(!restored.invite_enabled);
    assert!(preview(State(state), Path(live_link.token)).await.is_err());
    Ok(())
}
