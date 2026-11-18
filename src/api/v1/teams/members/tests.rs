use super::*;
use sea_orm::SqlxPostgresConnector;

#[test]
fn membership_roles_use_only_admin_and_member() {
    assert!(serde_json::from_str::<ChangeRoleRequest>(r#"{"role":"owner"}"#).is_err());
    assert_eq!(
        serde_json::from_str::<ChangeRoleRequest>(r#"{"role":"admin"}"#)
            .unwrap()
            .role,
        Role::Admin
    );
}

async fn fixture(
    pool: &sqlx::PgPool,
) -> anyhow::Result<(ApiState, Vec<crate::models::Player>, i64)> {
    let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
    let mut players = Vec::new();
    for name in ["first-admin", "second-admin", "member", "outsider"] {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO player (lfs_username, display_name) VALUES ($1, $1) RETURNING id",
        )
        .bind(name)
        .fetch_one(pool)
        .await?;
        players.push(
            player::Entity::find_by_id(id)
                .one(&database)
                .await?
                .unwrap(),
        );
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO team (name) VALUES ('Roster team') RETURNING id")
        .fetch_one(pool)
        .await?;
    for (index, role) in [(0, "admin"), (1, "admin"), (2, "member")] {
        sqlx::query("INSERT INTO team_member (team_id, player_id, role) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(players[index].id)
            .bind(role)
            .execute(pool)
            .await?;
    }
    Ok((
        ApiState {
            database,
            object_store: std::sync::Arc::new(object_store::memory::InMemory::new()),
            oauth: None,
            hotlaps: Default::default(),
            site_notice: None,
        },
        players,
        id,
    ))
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn admins_have_equal_access_and_last_active_admin_is_protected(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players, id) = fixture(&pool).await?;
    assert_eq!(list(State(state.clone()), Path(id)).await?.0.items.len(), 3);
    for actor in [2, 3] {
        assert!(
            update(
                State(state.clone()),
                Path((id, players[0].id)),
                BrowserAuthenticatedPlayer(players[actor].clone()),
                Json(ChangeRoleRequest { role: Role::Member })
            )
            .await
            .is_err()
        );
        assert!(
            remove(
                State(state.clone()),
                Path((id, players[1].id)),
                BrowserAuthenticatedPlayer(players[actor].clone())
            )
            .await
            .is_err()
        );
        assert!(
            super::super::archive(
                State(state.clone()),
                Path(id),
                BrowserAuthenticatedPlayer(players[actor].clone())
            )
            .await
            .is_err()
        );
    }
    // Either admin can change another admin, including the original creator.
    update(
        State(state.clone()),
        Path((id, players[0].id)),
        BrowserAuthenticatedPlayer(players[1].clone()),
        Json(ChangeRoleRequest { role: Role::Member }),
    )
    .await?;
    assert_eq!(
        role(&state.database, id, players[0].id).await?,
        Some(Role::Member)
    );
    assert!(
        super::super::regenerate(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[0].clone())
        )
        .await
        .is_err()
    );
    assert_eq!(
        update(
            State(state.clone()),
            Path((id, players[1].id)),
            BrowserAuthenticatedPlayer(players[1].clone()),
            Json(ChangeRoleRequest { role: Role::Member })
        )
        .await
        .unwrap_err()
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        remove(
            State(state.clone()),
            Path((id, players[1].id)),
            BrowserAuthenticatedPlayer(players[1].clone())
        )
        .await
        .unwrap_err()
        .status,
        StatusCode::CONFLICT
    );
    update(
        State(state.clone()),
        Path((id, players[2].id)),
        BrowserAuthenticatedPlayer(players[1].clone()),
        Json(ChangeRoleRequest { role: Role::Admin }),
    )
    .await?;
    // Admins can remove other admins, and a departing admin leaves the other in control.
    remove(
        State(state.clone()),
        Path((id, players[1].id)),
        BrowserAuthenticatedPlayer(players[2].clone()),
    )
    .await?;
    assert_eq!(role(&state.database, id, players[1].id).await?, None);
    assert!(
        remove(
            State(state.clone()),
            Path((id, players[2].id)),
            BrowserAuthenticatedPlayer(players[2].clone())
        )
        .await
        .is_err()
    );
    let Json(archived) = super::super::archive(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[2].clone()),
    )
    .await?;
    assert!(!archived.is_active);
    let Json(restored) = super::super::restore(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[2].clone()),
    )
    .await?;
    assert!(restored.is_active);
    let _ = super::super::archive(
        State(state.clone()),
        Path(id),
        BrowserAuthenticatedPlayer(players[2].clone()),
    )
    .await?;
    // Disbanded teams may lose their last admin; this never strands an active team.
    remove(
        State(state.clone()),
        Path((id, players[2].id)),
        BrowserAuthenticatedPlayer(players[2].clone()),
    )
    .await?;
    assert!(
        super::super::restore(
            State(state.clone()),
            Path(id),
            BrowserAuthenticatedPlayer(players[0].clone())
        )
        .await
        .is_err()
    );
    remove(
        State(state.clone()),
        Path((id, players[0].id)),
        BrowserAuthenticatedPlayer(players[0].clone()),
    )
    .await?;
    assert!(list(State(state), Path(id)).await?.0.items.is_empty());
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn simultaneous_admin_demotions_and_departures_keep_one_admin(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let (state, players, id) = fixture(&pool).await?;
    let (first, second) = tokio::join!(
        update(
            State(state.clone()),
            Path((id, players[0].id)),
            BrowserAuthenticatedPlayer(players[0].clone()),
            Json(ChangeRoleRequest { role: Role::Member })
        ),
        update(
            State(state.clone()),
            Path((id, players[1].id)),
            BrowserAuthenticatedPlayer(players[1].clone()),
            Json(ChangeRoleRequest { role: Role::Member })
        ),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM team_member WHERE team_id = $1 AND role = 'admin'",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, 1);
    sqlx::query(
        "UPDATE team_member SET role = 'admin' WHERE team_id = $1 AND player_id IN ($2, $3)",
    )
    .bind(id)
    .bind(players[0].id)
    .bind(players[1].id)
    .execute(&pool)
    .await?;
    let (first, second) = tokio::join!(
        remove(
            State(state.clone()),
            Path((id, players[0].id)),
            BrowserAuthenticatedPlayer(players[0].clone())
        ),
        remove(
            State(state.clone()),
            Path((id, players[1].id)),
            BrowserAuthenticatedPlayer(players[1].clone())
        ),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM team_member WHERE team_id = $1 AND role = 'admin'",
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(remaining, 1);
    Ok(())
}

#[sqlx::test(migrations = false)]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn migration_preserves_former_owners_as_equal_admins(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let all = sqlx::migrate!();
    let before = sqlx::migrate::Migrator::with_migrations(
        all.iter()
            .filter(|migration| migration.version < 20261118010000)
            .cloned()
            .collect(),
    );
    before.run(&pool).await?;
    let first: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ('legacy-first', 'First') RETURNING id").fetch_one(&pool).await?;
    let second: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ('legacy-second', 'Second') RETURNING id").fetch_one(&pool).await?;
    let one: i64 = sqlx::query_scalar(
        "INSERT INTO team (owner_id, name) VALUES ($1, 'Existing membership') RETURNING id",
    )
    .bind(first)
    .fetch_one(&pool)
    .await?;
    let two: i64 = sqlx::query_scalar(
        "INSERT INTO team (owner_id, name) VALUES ($1, 'Missing membership') RETURNING id",
    )
    .bind(second)
    .fetch_one(&pool)
    .await?;
    sqlx::query("INSERT INTO team_member (team_id, player_id, role, joined_at) VALUES ($1, $2, 'member', '2020-01-01T00:00:00Z')").bind(one).bind(first).execute(&pool).await?;
    all.run(&pool).await?;
    for (team, player) in [(one, first), (two, second)] {
        let role: String = sqlx::query_scalar(
            "SELECT role FROM team_member WHERE team_id = $1 AND player_id = $2",
        )
        .bind(team)
        .bind(player)
        .fetch_one(&pool)
        .await?;
        assert_eq!(role, "admin");
    }
    let original_date: bool = sqlx::query_scalar("SELECT joined_at = '2020-01-01T00:00:00Z' FROM team_member WHERE team_id = $1 AND player_id = $2").bind(one).bind(first).fetch_one(&pool).await?;
    assert!(original_date);
    let owner_column: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'team' AND column_name = 'owner_id')").fetch_one(&pool).await?;
    assert!(!owner_column);
    Ok(())
}
