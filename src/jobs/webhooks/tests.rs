use super::*;
use crate::models::{
    hotlaps::HotlapEntity,
    webhooks::{WebhookEvent, WebhookEventKind, enqueue_hotlap},
};
use sea_orm::{PaginatorTrait, SqlxPostgresConnector};

fn response(status: u16, headers: &[(&str, &str)], body: &str) -> reqwest::Response {
    let mut builder = axum::http::Response::builder().status(status);
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    builder.body(body.to_owned()).unwrap().into()
}

#[tokio::test]
async fn discord_responses_schedule_retries_and_stop_permanent_failures() {
    assert!(
        matches!(classify(response(200, &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset-after", "2.5")], "{}")).await,
        Outcome::Delivered { cooldown: Some(delay) } if delay == time::Duration::milliseconds(2500))
    );
    assert!(
        matches!(classify(response(429, &[], r#"{"retry_after": 4.5}"#)).await,
        Outcome::Retry { after: Some(delay), .. } if delay == time::Duration::milliseconds(4500))
    );
    assert!(
        matches!(classify(response(429, &[("retry-after", "10")], "{}")).await,
        Outcome::Retry { after: Some(delay), .. } if delay == time::Duration::seconds(10))
    );
    assert!(matches!(
        classify(response(502, &[], "secret")).await,
        Outcome::Retry { after: None, .. }
    ));
    assert!(
        matches!(classify(response(400, &[], "secret")).await, Outcome::Failed(detail) if !detail.contains("secret"))
    );
    assert!(matches!(
        classify(response(404, &[], "")).await,
        Outcome::Unavailable(_)
    ));
    assert!(matches!(
        classify(response(302, &[("location", "http://localhost")], "")).await,
        Outcome::Failed(_)
    ));
    assert!(seconds("NaN").is_none());
    assert!(seconds("-1").is_none());
}

async fn fixture(pool: &sqlx::PgPool) -> anyhow::Result<DatabaseConnection> {
    sqlx::raw_sql(include_str!("../hlvc/fixtures.sql"))
        .execute(pool)
        .await?;
    sqlx::raw_sql("INSERT INTO webhook (player_id, name, url, format, event_kind) SELECT id, 'Club', 'invalid-test-url', 'discord', 'hotlap_validated' FROM player;").execute(pool).await?;
    Ok(SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone()))
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn fanout_is_transactional_deduplicated_and_excludes_paused_subscriptions(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = fixture(&pool).await?;
    sqlx::raw_sql("INSERT INTO player (lfs_username, display_name) VALUES ('subscriber', 'Subscriber'); INSERT INTO webhook (player_id, name, url, format, event_kind, enabled) SELECT id, 'Other driver', 'other-url', 'discord', 'hotlap_validated', true FROM player WHERE lfs_username = 'subscriber'; INSERT INTO webhook (player_id, name, url, format, event_kind, enabled) SELECT id, 'Paused', 'paused-url', 'discord', 'hotlap_validated', false FROM player WHERE lfs_username = 'subscriber';").execute(&pool).await?;
    let hotlap = HotlapEntity::find().one(&database).await?.unwrap();
    let transaction = database.begin().await?;
    enqueue_hotlap(&transaction, &hotlap, false).await?;
    assert_eq!(notification::Entity::find().count(&transaction).await?, 2);
    transaction.rollback().await?;
    assert_eq!(notification::Entity::find().count(&database).await?, 0);
    let transaction = database.begin().await?;
    enqueue_hotlap(&transaction, &hotlap, false).await?;
    enqueue_hotlap(&transaction, &hotlap, false).await?;
    transaction.commit().await?;
    assert_eq!(notification::Entity::find().count(&database).await?, 2);
    let notification = notification::Entity::find().one(&database).await?.unwrap();
    assert_eq!(
        WebhookEventKind::from(&notification.event),
        WebhookEventKind::HotlapValidated
    );
    let (driver, lap_time_ms) = match notification.event {
        WebhookEvent::HotlapValidated {
            driver,
            lap_time_ms,
            ..
        } => (driver, lap_time_ms),
        WebhookEvent::WorldRecordSet { .. } => unreachable!(),
    };
    assert_eq!(driver, "worker-test");
    assert_eq!(lap_time_ms, 60000);
    HotlapEntity::delete_by_id(hotlap.id)
        .exec(&database)
        .await?;
    assert_eq!(notification::Entity::find().count(&database).await?, 0);
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn destination_lock_pause_and_cooldown_are_respected(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = fixture(&pool).await?;
    let transaction = database.begin().await?;
    let hotlap = HotlapEntity::find().one(&transaction).await?.unwrap();
    enqueue_hotlap(&transaction, &hotlap, false).await?;
    transaction.commit().await?;
    let worker = WebhookNotifications::new(database.clone(), "https://lfspla.net".parse()?)?;
    let mut lock = pool.begin().await?;
    sqlx::query("SELECT id FROM webhook FOR UPDATE")
        .fetch_one(&mut *lock)
        .await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), worker.process_next()).await??,
        Step::Idle
    );
    lock.rollback().await?;
    sqlx::query("UPDATE webhook SET enabled = false")
        .execute(&pool)
        .await?;
    assert_eq!(worker.process_next().await?, Step::Idle);
    sqlx::query("UPDATE webhook SET enabled = true, available_at = now() + interval '1 hour'")
        .execute(&pool)
        .await?;
    assert_eq!(worker.process_next().await?, Step::Idle);
    sqlx::query("UPDATE webhook SET available_at = now()")
        .execute(&pool)
        .await?;
    assert_eq!(worker.process_next().await?, Step::Processed);
    let notification = notification::Entity::find().one(&database).await?.unwrap();
    assert_eq!(notification.attempt_count, 1);
    assert!(notification.failed_at.is_some());
    assert!(notification.delivered_at.is_none());
    assert_eq!(worker.process_next().await?, Step::Idle);
    webhooks::Entity::delete_many().exec(&database).await?;
    assert_eq!(notification::Entity::find().count(&database).await?, 0);
    Ok(())
}

#[sqlx::test]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn delivery_outcomes_persist_retry_exhaustion_success_and_destination_pause(
    pool: sqlx::PgPool,
) -> anyhow::Result<()> {
    let database = fixture(&pool).await?;
    let transaction = database.begin().await?;
    let hotlap = HotlapEntity::find().one(&transaction).await?.unwrap();
    enqueue_hotlap(&transaction, &hotlap, false).await?;
    transaction.commit().await?;
    let worker = WebhookNotifications::new(database.clone(), "https://lfspla.net".parse()?)?;

    let save = async |outcome| -> anyhow::Result<notification::Model> {
        let transaction = database.begin().await?;
        let notification = notification::Entity::find()
            .one(&transaction)
            .await?
            .unwrap();
        let webhook = webhooks::Entity::find().one(&transaction).await?.unwrap();
        record_outcome(&transaction, notification, webhook, outcome).await?;
        transaction.commit().await?;
        Ok(notification::Entity::find().one(&database).await?.unwrap())
    };
    let before = time::OffsetDateTime::now_utc();
    let retry = save(Outcome::Retry {
        detail: "HTTP 503".into(),
        after: None,
    })
    .await?;
    assert_eq!(retry.attempt_count, 1);
    assert!(retry.next_attempt_at >= before + time::Duration::seconds(30));
    assert!(retry.failed_at.is_none());
    assert_eq!(worker.process_next().await?, Step::Idle);

    let before = time::OffsetDateTime::now_utc();
    let retry = save(Outcome::Retry {
        detail: "HTTP 429".into(),
        after: Some(time::Duration::seconds(90)),
    })
    .await?;
    let webhook = webhooks::Entity::find().one(&database).await?.unwrap();
    assert_eq!(webhook.available_at, retry.next_attempt_at);
    assert!(webhook.available_at >= before + time::Duration::seconds(90));

    sqlx::query("UPDATE webhook_notification SET attempt_count = $1")
        .bind(MAX_ATTEMPTS - 1)
        .execute(&pool)
        .await?;
    let exhausted = save(Outcome::Retry {
        detail: "HTTP 503".into(),
        after: None,
    })
    .await?;
    assert_eq!(exhausted.attempt_count, MAX_ATTEMPTS);
    assert!(exhausted.failed_at.is_some());
    assert!(exhausted.delivered_at.is_none());

    sqlx::query("UPDATE webhook_notification SET attempt_count = 0, failed_at = NULL")
        .execute(&pool)
        .await?;
    let delivered = save(Outcome::Delivered { cooldown: None }).await?;
    assert!(delivered.delivered_at.is_some());
    assert!(delivered.error_detail.is_none());
    assert_eq!(worker.process_next().await?, Step::Idle);

    sqlx::query("UPDATE webhook_notification SET delivered_at = NULL")
        .execute(&pool)
        .await?;
    let failed = save(Outcome::Unavailable("HTTP 404".into())).await?;
    assert!(failed.failed_at.is_some());
    assert!(
        !webhooks::Entity::find()
            .one(&database)
            .await?
            .unwrap()
            .enabled
    );
    Ok(())
}
