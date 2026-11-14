//! Single-connection reconciliation, adapted from Anomalous's runtime owner.
mod runtime;
use crate::{models::event, settings::GameServerSettings};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

const OWNER_LOCK: i64 = 0x4c46_5350_4741;

struct Running {
    event_id: i64,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}
async fn stop(
    running: &mut Option<Running>,
    database: &sea_orm::DatabaseConnection,
) -> anyhow::Result<()> {
    if let Some(mut old) = running.take() {
        old.cancel.cancel();
        if tokio::time::timeout(Duration::from_secs(5), &mut old.task)
            .await
            .is_err()
        {
            old.task.abort();
            let _ = (&mut old.task).await;
        }
        sqlx::query(
            "UPDATE game_event_execution SET stopped_at=clock_timestamp() WHERE event_id=$1",
        )
        .bind(old.event_id)
        .execute(database.get_postgres_connection_pool())
        .await?;
    }
    Ok(())
}
async fn desired(database: &sea_orm::DatabaseConnection) -> anyhow::Result<Option<event::Model>> {
    let now = time::OffsetDateTime::now_utc();
    let mut events = event::Entity::find()
        .filter(event::Column::Hidden.eq(false))
        .filter(event::Column::GameConfig.is_not_null())
        .filter(event::Column::StartsAt.lte(now))
        .filter(event::Column::EndsAt.gt(now))
        .all(database)
        .await?;
    anyhow::ensure!(
        events.len() <= 1,
        "multiple scheduled game events are active"
    );
    Ok(events.pop())
}

async fn acquire_lock(
    database: &sea_orm::DatabaseConnection,
) -> anyhow::Result<sqlx::PgConnection> {
    // Detached from the pool: dropping this owner always closes the lock session.
    let mut lock = database
        .get_postgres_connection_pool()
        .acquire()
        .await?
        .detach();
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(OWNER_LOCK)
        .fetch_one(&mut lock)
        .await?;
    anyhow::ensure!(acquired, "another game-server process owns this database");
    Ok(lock)
}

async fn ownership_held(connection: &mut sqlx::PgConnection) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype='advisory' AND pid=pg_backend_pid() AND classid::bigint=($1::bigint >> 32) AND objid::bigint=($1::bigint & 4294967295) AND objsubid=1)")
        .bind(OWNER_LOCK).fetch_one(connection).await
}

pub(crate) async fn run(
    database: sea_orm::DatabaseConnection,
    store: Arc<dyn object_store::ObjectStore>,
    config: GameServerSettings,
) -> anyhow::Result<()> {
    let mut lock = acquire_lock(&database).await?;
    let mut running: Option<Running> = None;
    let mut ticker = tokio::time::interval(Duration::from_secs(config.poll_interval_seconds.get()));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let shutdown = async {
        #[cfg(unix)]
        tokio::select! { result = tokio::signal::ctrl_c() => result, _ = terminate.recv() => Ok(()) }
        #[cfg(not(unix))]
        tokio::signal::ctrl_c().await
    };
    tokio::pin!(shutdown);
    tracing::info!(address = %config.insim_address, "game server started");
    let result: anyhow::Result<()> = async {
        loop {
            tokio::select! {
                result = &mut shutdown => { result?; break; }
                _ = ticker.tick() => {}
            }
            // Fail closed if the dedicated ownership session is lost.
            let owned = tokio::time::timeout(Duration::from_secs(5), ownership_held(&mut lock)).await??;
            anyhow::ensure!(owned, "game-server ownership lock was lost");
            let event = match desired(&database).await {
                Ok(event) => event,
                Err(error) => { stop(&mut running, &database).await?; tracing::error!(%error, "cannot determine scheduled event"); continue; }
            };
            let id = event.as_ref().map(|e| e.id);
            if running.as_ref().is_some_and(|r| Some(r.event_id) != id || r.task.is_finished()) {
                stop(&mut running, &database).await?;
            }
            if running.is_none() && let Some(event) = event {
                let event_id = event.id;
                let prepared = runtime::EventRuntime::new(database.get_postgres_connection_pool().clone(), &database, event, config.clone(), &*store).await;
                match prepared {
                    Ok(runtime) => {
                        sqlx::query("INSERT INTO game_event_execution (event_id) VALUES ($1) ON CONFLICT (event_id) DO UPDATE SET stopped_at=NULL, last_error=NULL")
                            .bind(event_id).execute(database.get_postgres_connection_pool()).await?;
                        let cancel = CancellationToken::new();
                        let task = tokio::spawn(runtime.run(cancel.clone()));
                        running = Some(Running { event_id, cancel, task });
                        tracing::info!(event_id, "started scheduled game");
                    }
                    Err(error) => {
                        tracing::error!(event_id, %error, "game preparation failed");
                        // Persist the failure only after the definition has been selected for execution.
                        sqlx::query("INSERT INTO game_event_execution (event_id,last_error,stopped_at) VALUES ($1,$2,clock_timestamp()) ON CONFLICT (event_id) DO UPDATE SET last_error=excluded.last_error, stopped_at=excluded.stopped_at")
                            .bind(event_id).bind(error.to_string()).execute(database.get_postgres_connection_pool()).await?;
                    }
                }
            }
        }
        Ok(())
    }.await;
    let stopped = stop(&mut running, &database).await;
    result?;
    stopped?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::SqlxPostgresConnector;
    use sqlx::Connection;

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn game_server_owns_one_lock_and_awaits_cancellation(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let mut lock = acquire_lock(&database).await?;
        assert!(ownership_held(&mut lock).await?);
        assert!(acquire_lock(&database).await.is_err());
        lock.close().await?;
        let mut lock = acquire_lock(&database).await?;
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(OWNER_LOCK)
            .execute(&mut lock)
            .await?;
        assert!(!ownership_held(&mut lock).await?);
        let _lock = acquire_lock(&database).await?;
        let league: i64 =
            sqlx::query_scalar("INSERT INTO league(name) VALUES ('Runtime') RETURNING id")
                .fetch_one(&pool)
                .await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO event(league_id,title,starts_at) VALUES ($1,'Runtime',now()) RETURNING id",
        )
        .bind(league)
        .fetch_one(&pool)
        .await?;
        sqlx::query("INSERT INTO game_event_execution(event_id) VALUES ($1)")
            .bind(id)
            .execute(&pool)
            .await?;
        let cancel = CancellationToken::new();
        let signal = cancel.clone();
        let (sent, received) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            signal.cancelled().await;
            let _ = sent.send(());
        });
        let mut running = Some(Running {
            event_id: id,
            cancel,
            task,
        });
        stop(&mut running, &database).await?;
        assert!(running.is_none());
        received.await?;
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT stopped_at IS NOT NULL FROM game_event_execution WHERE event_id=$1"
            )
            .bind(id)
            .fetch_one(&pool)
            .await?
        );
        // Ordinary calendar listings must never be selected as games.
        assert!(desired(&database).await?.is_none());
        let attachment: i64 = sqlx::query_scalar("INSERT INTO attachment(object_key,media_type,extension) VALUES ('scheduled.lyt','application/octet-stream','lyt') RETURNING id").fetch_one(&pool).await?;
        let game_id: i64 = sqlx::query_scalar("INSERT INTO event(league_id,title,starts_at,ends_at,source,catalogue_id,game_config,layout_attachment_id) VALUES ($1,'Scheduled',now()-interval '1 minute',now()+interval '1 minute','shortcut','scheduled','{\"source\":{\"type\":\"shortcut\"},\"track\":\"BL1\",\"vehicles\":[]}',$2) RETURNING id").bind(league).bind(attachment).fetch_one(&pool).await?;
        assert_eq!(desired(&database).await?.unwrap().id, game_id);
        sqlx::query("UPDATE event SET hidden=true WHERE id=$1")
            .bind(game_id)
            .execute(&pool)
            .await?;
        assert!(desired(&database).await?.is_none());
        sqlx::query("UPDATE event SET hidden=false,ends_at=now()-interval '1 second' WHERE id=$1")
            .bind(game_id)
            .execute(&pool)
            .await?;
        assert!(desired(&database).await?.is_none());
        Ok(())
    }
}
