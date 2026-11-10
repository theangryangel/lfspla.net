//! Mode-specific result recording and canonical best-result reads.
use super::game::EventKind;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Standing {
    pub position: i64,
    pub player_id: i64,
    pub username: String,
    pub display_name: String,
    pub country_code: Option<String>,
    pub flag_code: Option<String>,
    pub vehicle: Option<String>,
    pub checkpoint_count: Option<i64>,
    pub survival_ms: Option<i64>,
    pub time_ms: Option<i64>,
    pub delta_ms: Option<i64>,
}
pub(crate) async fn standings(
    pool: &PgPool,
    event_id: i64,
    kind: EventKind,
    limit: i64,
    offset: i64,
) -> anyhow::Result<(Vec<Standing>, i64)> {
    if matches!(kind, EventKind::Local | EventKind::LfsNet) {
        return Ok((Vec::new(), 0));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_event_standing WHERE event_id=$1")
            .bind(event_id)
            .fetch_one(&mut *tx)
            .await?;
    let rows = sqlx::query_as::<_, Standing>("SELECT r.position, r.player_id, p.lfs_username AS username, p.display_name, p.country_code, p.flag_code, r.vehicle, r.checkpoint_count, r.survival_ms, r.time_ms, r.delta_ms FROM game_event_standing r JOIN player p ON p.id=r.player_id WHERE event_id=$1 ORDER BY r.position LIMIT $2 OFFSET $3")
        .bind(event_id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok((rows, total))
}

pub(crate) enum Submission {
    Bomb(lfsplanet_bomb::BombResult),
    Shortcut(lfsplanet_shortcut::ShortcutResult),
    Metronome(lfsplanet_metronome::MetronomeResult),
}
pub(crate) async fn record(
    pool: &PgPool,
    event_id: i64,
    submission: Submission,
) -> anyhow::Result<()> {
    let (kind, uname, pname) = match &submission {
        Submission::Bomb(r) => ("bomb", &r.uname, &r.pname),
        Submission::Shortcut(r) => ("shortcut", &r.uname, &r.pname),
        Submission::Metronome(r) => ("metronome", &r.uname, &r.pname),
    };
    anyhow::ensure!(
        !uname.trim().is_empty(),
        "result requires an authenticated LFS username"
    );
    let mut tx = pool.begin().await?;
    let active = sqlx::query("SELECT id FROM event WHERE id=$1 AND source=$2 AND NOT hidden AND starts_at <= clock_timestamp() AND ends_at > clock_timestamp() FOR SHARE")
        .bind(event_id).bind(kind).fetch_optional(&mut *tx).await?;
    anyhow::ensure!(active.is_some(), "event is no longer accepting results");
    let player_id: i64 = sqlx::query_scalar("INSERT INTO player (lfs_username, display_name) VALUES ($1,$2) ON CONFLICT (lower(lfs_username)) DO UPDATE SET display_name=excluded.display_name RETURNING id")
        .bind(uname).bind(pname).fetch_one(&mut *tx).await?;
    match submission {
        Submission::Bomb(r) => {
            sqlx::query("INSERT INTO bomb_run (event_id,player_id,vehicle,checkpoint_count,survival_ms) VALUES ($1,$2,$3,$4,$5)").bind(event_id).bind(player_id).bind(r.vehicle.to_string()).bind(r.checkpoints).bind(r.survival_ms).execute(&mut *tx).await?;
        }
        Submission::Shortcut(r) => {
            sqlx::query("INSERT INTO shortcut_time (event_id,player_id,vehicle,time_ms) VALUES ($1,$2,$3,$4)").bind(event_id).bind(player_id).bind(r.vehicle).bind(r.time_ms).execute(&mut *tx).await?;
        }
        Submission::Metronome(r) => {
            sqlx::query(
                "INSERT INTO metronome_result (event_id,player_id,delta_ms) VALUES ($1,$2,$3)",
            )
            .bind(event_id)
            .bind(player_id)
            .bind(r.delta_ms)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn game_results_preserve_mode_ordering_and_case_insensitive_players(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let league: i64 =
            sqlx::query_scalar("INSERT INTO league (name) VALUES ('Results') RETURNING id")
                .fetch_one(&pool)
                .await?;
        let attachment: i64 = sqlx::query_scalar("INSERT INTO attachment (object_key,media_type,extension) VALUES ('test.lyt','application/octet-stream','lyt') RETURNING id").fetch_one(&pool).await?;
        for (source, payload) in [
            (
                "bomb",
                serde_json::json!({"type":"bomb","checkpoint_timeout_secs":30,"checkpoint_penalty_ms":250,"collision_max_penalty_ms":500}),
            ),
            (
                "metronome",
                serde_json::json!({"type":"metronome","target_ms":30000}),
            ),
            ("shortcut", serde_json::json!({"type":"shortcut"})),
        ] {
            sqlx::query("UPDATE event SET ends_at=clock_timestamp() WHERE game_config IS NOT NULL AND ends_at>clock_timestamp()").execute(&pool).await?;
            let event: i64 = sqlx::query_scalar("INSERT INTO event (league_id,title,starts_at,ends_at,source,catalogue_id,game_config,layout_attachment_id) VALUES ($1,$2,clock_timestamp(),clock_timestamp()+interval '1 hour',$2,$2,$3,$4) RETURNING id")
                .bind(league).bind(source).bind(serde_json::json!({"source":payload,"track":"BL1","vehicles":[]})).bind(attachment).fetch_one(&pool).await?;
            let kind = match source {
                "bomb" => {
                    for (uname, cp, ms) in [
                        ("Alpha", 5, 1000),
                        ("alpha", 4, 2000),
                        ("Beta", 5, 2000),
                        ("Charlie", 5, 2000),
                    ] {
                        record(
                            &pool,
                            event,
                            Submission::Bomb(lfsplanet_bomb::BombResult {
                                uname: uname.into(),
                                pname: uname.into(),
                                vehicle: "XFG".parse()?,
                                checkpoints: cp,
                                survival_ms: ms,
                            }),
                        )
                        .await?;
                    }
                    EventKind::Bomb
                }
                "metronome" => {
                    for (uname, delta) in [
                        ("Alpha", 100),
                        ("alpha", -50),
                        ("Beta", 50),
                        ("Charlie", -50),
                    ] {
                        record(
                            &pool,
                            event,
                            Submission::Metronome(lfsplanet_metronome::MetronomeResult {
                                uname: uname.into(),
                                pname: uname.into(),
                                delta_ms: delta,
                            }),
                        )
                        .await?;
                    }
                    EventKind::Metronome
                }
                _ => {
                    for (uname, ms) in [
                        ("Alpha", 1000),
                        ("alpha", 2000),
                        ("Beta", 900),
                        ("Charlie", 900),
                    ] {
                        record(
                            &pool,
                            event,
                            Submission::Shortcut(lfsplanet_shortcut::ShortcutResult {
                                uname: uname.into(),
                                pname: uname.into(),
                                vehicle: "XFG".into(),
                                time_ms: ms,
                            }),
                        )
                        .await?;
                    }
                    EventKind::Shortcut
                }
            };
            sqlx::query("UPDATE player SET country_code='GB', flag_code='GB-ENG' WHERE lower(lfs_username)='alpha'")
                .execute(&pool)
                .await?;
            let (rows, total) = standings(&pool, event, kind, 10, 0).await?;
            assert_eq!(total, 3);
            let alpha = rows
                .iter()
                .find(|row| row.username.eq_ignore_ascii_case("alpha"))
                .unwrap();
            assert_eq!(alpha.country_code.as_deref(), Some("GB"));
            assert_eq!(alpha.flag_code.as_deref(), Some("GB-ENG"));
            assert!(
                rows.iter()
                    .filter(|row| row.username != alpha.username)
                    .all(|row| row.country_code.is_none() && row.flag_code.is_none())
            );
            let expected = if source == "metronome" {
                ["alpha", "Beta", "Charlie"]
            } else {
                ["Beta", "Charlie", "alpha"]
            };
            assert_eq!(
                rows.iter()
                    .map(|r| r.username.to_ascii_lowercase())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect::<Vec<_>>()
            );
            assert_eq!(rows[0].position, 1);
            let (second, _) = standings(&pool, event, kind, 1, 1).await?;
            assert_eq!(second[0].position, 2);
            assert!(
                record(
                    &pool,
                    event,
                    Submission::Shortcut(lfsplanet_shortcut::ShortcutResult {
                        uname: "x".into(),
                        pname: "x".into(),
                        vehicle: "XFG".into(),
                        time_ms: 0
                    })
                )
                .await
                .is_err()
            );
            sqlx::query("UPDATE event SET ends_at=clock_timestamp() WHERE id=$1")
                .bind(event)
                .execute(&pool)
                .await?;
            assert!(
                record(
                    &pool,
                    event,
                    Submission::Metronome(lfsplanet_metronome::MetronomeResult {
                        uname: "late".into(),
                        pname: "late".into(),
                        delta_ms: 1
                    })
                )
                .await
                .is_err()
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM player")
                .fetch_one(&pool)
                .await?,
            3
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM player WHERE last_authenticated_at IS NOT NULL"
            )
            .fetch_one(&pool)
            .await?,
            0
        );
        Ok(())
    }
}
