//! Send queued webhook notifications while locking the notification and destination.

use crate::models::webhooks::{
    self, WebhookFormat,
    discord::{self, webhook_url},
    notification,
};
use lfsplanet_jobs::{Processor, Step};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, TransactionTrait,
    sea_query::{LockBehavior, LockType},
};
use std::time::Duration;

const MAX_ATTEMPTS: i32 = 8;

pub struct WebhookNotifications {
    pub database: DatabaseConnection,
    client: reqwest::Client,
    public_base_url: url::Url,
}

impl WebhookNotifications {
    pub fn new(database: DatabaseConnection, public_base_url: url::Url) -> anyhow::Result<Self> {
        Ok(Self {
            database,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            public_base_url,
        })
    }

    async fn send(&self, webhook: &webhooks::Model, event: &webhooks::WebhookEvent) -> Outcome {
        match webhook.format {
            WebhookFormat::Discord => {
                let Ok(mut url) = webhook_url(&webhook.url) else {
                    return Outcome::Failed("Invalid Discord webhook URL".into());
                };
                // Discord only confirms persistence with wait=true.
                url.query_pairs_mut().append_pair("wait", "true");
                let response = self
                    .client
                    .post(url)
                    .json(&discord::render(event, &self.public_base_url))
                    .send()
                    .await;
                match response {
                    Ok(response) => classify(response).await,
                    // reqwest errors can contain the secret URL. Never persist/log them.
                    Err(_) => Outcome::Retry {
                        detail: "Webhook request failed or timed out".into(),
                        after: None,
                    },
                }
            }
        }
    }
}

impl Processor for WebhookNotifications {
    async fn process_next(&self) -> anyhow::Result<Step> {
        let transaction = self.database.begin().await?;
        let now = time::OffsetDateTime::now_utc();
        // Lock the destination too, to serialize sends and share cooldowns.
        let Some(notification) = notification::Entity::find()
            .join(JoinType::InnerJoin, notification::Relation::Webhook.def())
            .filter(notification::Column::DeliveredAt.is_null())
            .filter(notification::Column::FailedAt.is_null())
            .filter(notification::Column::NextAttemptAt.lte(now))
            .filter(webhooks::Column::Enabled.eq(true))
            .filter(webhooks::Column::AvailableAt.lte(now))
            .order_by_asc(notification::Column::NextAttemptAt)
            .order_by_asc(notification::Column::Id)
            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
            .one(&transaction)
            .await?
        else {
            transaction.rollback().await?;
            return Ok(Step::Idle);
        };
        let webhook = webhooks::Entity::find_by_id(notification.webhook_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| anyhow::anyhow!("notification has no webhook"))?;
        let outcome = self.send(&webhook, &notification.event).await;
        let notification_id = notification.id;
        let attempt = notification.attempt_count + 1;
        record_outcome(&transaction, notification, webhook, outcome).await?;
        transaction.commit().await?;
        tracing::debug!(
            notification_id,
            attempt,
            "webhook delivery attempt recorded"
        );
        Ok(Step::Processed)
    }
}

async fn record_outcome(
    transaction: &sea_orm::DatabaseTransaction,
    notification: notification::Model,
    webhook: webhooks::Model,
    outcome: Outcome,
) -> anyhow::Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let attempt = notification.attempt_count + 1;
    let mut active: notification::ActiveModel = notification.into();
    let mut destination: webhooks::ActiveModel = webhook.into();
    active.attempt_count = Set(attempt);
    match outcome {
        Outcome::Delivered { cooldown } => {
            active.delivered_at = Set(Some(now));
            active.error_detail = Set(None);
            if let Some(delay) = cooldown {
                destination.available_at = Set(now + delay);
            }
        }
        Outcome::Unavailable(detail) => {
            active.failed_at = Set(Some(now));
            active.error_detail = Set(Some(detail));
            destination.enabled = Set(false);
        }
        Outcome::Failed(detail) => {
            active.failed_at = Set(Some(now));
            active.error_detail = Set(Some(detail));
        }
        Outcome::Retry { detail, after } => {
            active.error_detail = Set(Some(detail));
            let delay = after.unwrap_or_else(|| {
                time::Duration::seconds(
                    30 * 2_i64.pow(u32::try_from(attempt - 1).unwrap_or(0).min(7)),
                )
            });
            if after.is_some() {
                destination.available_at = Set(now + delay);
            }
            if attempt >= MAX_ATTEMPTS {
                active.failed_at = Set(Some(now));
            } else {
                active.next_attempt_at = Set(now + delay);
            }
        }
    }
    destination.update(transaction).await?;
    active.update(transaction).await?;
    Ok(())
}

enum Outcome {
    Delivered {
        cooldown: Option<time::Duration>,
    },
    Retry {
        detail: String,
        after: Option<time::Duration>,
    },
    Failed(String),
    Unavailable(String),
}

fn seconds(value: &str) -> Option<time::Duration> {
    let value: f64 = value.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(time::Duration::seconds_f64(value.clamp(1.0, 86400.0)))
}

async fn classify(response: reqwest::Response) -> Outcome {
    let status = response.status();
    if status.is_success() {
        let cooldown = (response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|h| h.to_str().ok())
            == Some("0"))
        .then(|| {
            response
                .headers()
                .get("x-ratelimit-reset-after")
                .and_then(|h| h.to_str().ok())
                .and_then(seconds)
        })
        .flatten();
        return Outcome::Delivered { cooldown };
    }
    let detail = format!("Discord returned HTTP {}", status.as_u16());
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let header_delay = response
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(seconds);
        // Discord's retry delays may be fractional.
        let body_delay = match response.json::<serde_json::Value>().await {
            Ok(body) => body
                .get("retry_after")
                .and_then(serde_json::Value::as_f64)
                .and_then(|n| seconds(&n.to_string())),
            Err(_) => None,
        };
        return Outcome::Retry {
            detail,
            after: Some(
                header_delay
                    .or(body_delay)
                    .unwrap_or(time::Duration::seconds(30)),
            ),
        };
    }
    if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
            | reqwest::StatusCode::NOT_FOUND
    ) {
        return Outcome::Unavailable(detail);
    }
    if status.is_server_error() || status == reqwest::StatusCode::REQUEST_TIMEOUT {
        Outcome::Retry {
            detail,
            after: None,
        }
    } else {
        Outcome::Failed(detail)
    }
}

#[cfg(test)]
mod tests;
