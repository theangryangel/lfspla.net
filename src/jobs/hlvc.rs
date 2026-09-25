//! Validate pending hotlaps while holding their row lock.

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use lfsplanet_jobs::{Processor, Step};
use object_store::{ObjectStore, path::Path};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
    sea_query::{LockBehavior, LockType},
};

use crate::{
    hlvc::{self, HlvcResult},
    models::{
        eras::{self, EraEntity},
        hotlaps::{
            HotlapColumn, HotlapEntity, HotlapFilter, HotlapModel, HotlapMutation, HotlapState,
            personal_bests,
        },
    },
    settings::{HlvcSettings, LfsRuntimeSettings},
};

const MAX_ATTEMPTS: i32 = 5;
const RETRY_DELAY: time::Duration = time::Duration::seconds(30);

pub(crate) struct HotlapValidation {
    pub database: DatabaseConnection,
    pub object_store: Arc<dyn ObjectStore>,
    pub runtime: LfsRuntimeSettings,
    pub settings: HlvcSettings,
}

impl Processor for HotlapValidation {
    async fn process_next(&self) -> anyhow::Result<Step> {
        let transaction = self.database.begin().await?;
        // Preserve the application's lock order: catalogue, hotlap, chart.
        // This shared lock allows other admissions/validators but delays bulk repairs.
        eras::lock_admission(&transaction).await?;
        let now = time::OffsetDateTime::now_utc();
        let Some(hotlap) = HotlapEntity::find()
            .uploads()
            .in_state(HotlapState::Pending)
            .filter(HotlapColumn::AttemptCount.lt(MAX_ATTEMPTS))
            .filter(
                Condition::any()
                    .add(HotlapColumn::NextAttemptAt.is_null())
                    .add(HotlapColumn::NextAttemptAt.lte(now)),
            )
            .order_by_asc(HotlapColumn::CreatedAt)
            .order_by_asc(HotlapColumn::Id)
            .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
            .one(&transaction)
            .await?
        else {
            transaction.rollback().await?;
            return Ok(Step::Idle);
        };

        let hotlap_id = hotlap.id;
        let era_id = hotlap.era_id;
        let attempt = hotlap.attempt_count + 1;
        tracing::info!(hotlap_id, attempt, "validating hotlap");
        // Bound replay download as well as LFS execution. Convert timeout into
        // a committed retry outcome, rather than cancelling the transaction.
        let deadline = self
            .settings
            .timeout
            .duration()
            .checked_add(Duration::from_secs(30))
            .context("HLVC deadline is too large")?;
        let checked =
            match tokio::time::timeout(deadline, self.validate(&transaction, &hotlap)).await {
                Ok(result) => result,
                Err(error) => Err(anyhow::Error::new(error).context("hotlap validation timed out")),
            };
        let mut active: HotlapMutation = hotlap.clone().into();
        active.attempt_count = Set(attempt);
        active.started_at = Set(Some(now));
        active.next_attempt_at = Set(None);
        active.finished_at = Set(None);
        active.hlvc_result_code = Set(None);
        active.error_detail = Set(None);
        let published = match checked {
            Ok(result) => {
                // Recheck eligibility after external validation.
                let eligible = if result == HlvcResult::Ok {
                    let vehicle = hotlap
                        .vehicle
                        .as_ref()
                        .context("pending hotlap has no vehicle")?;
                    match EraEntity::find_by_id(era_id).one(&transaction).await? {
                        Some(era) => {
                            era.admits_combination(&transaction, &hotlap.track.0, &vehicle.0)
                                .await?
                        }
                        None => false,
                    }
                } else {
                    false
                };
                let published = result == HlvcResult::Ok && eligible;
                active.state = Set(if published {
                    HotlapState::Valid
                } else {
                    HotlapState::Invalid
                });
                active.hlvc_result_code = Set(Some(i16::from(result.code())));
                active.finished_at = Set(Some(time::OffsetDateTime::now_utc()));
                if !published {
                    active.error_detail = Set(Some(if result == HlvcResult::Ok {
                        "The track and vehicle combination is not eligible for this era".to_owned()
                    } else {
                        result.description().to_owned()
                    }));
                }
                published
            }
            Err(error) => {
                let exhausted = attempt >= MAX_ATTEMPTS;
                active.error_detail = Set(Some(format!("{error:#}")));
                if !exhausted {
                    active.next_attempt_at =
                        Set(Some(time::OffsetDateTime::now_utc() + RETRY_DELAY));
                }
                tracing::error!(
                    hotlap_id,
                    attempt,
                    exhausted,
                    ?error,
                    "hotlap validation failed"
                );
                // Keep failed attempts pending, even when exhausted.
                false
            }
        };
        active.update(&transaction).await?;
        if published {
            let world_record = personal_bests::consider(&transaction, hotlap_id).await?;
            crate::models::webhooks::enqueue_hotlap(&transaction, &hotlap, world_record).await?;
        }
        transaction.commit().await?;
        if published {
            eras::rebuild_published_badges(&self.database, era_id).await;
        }
        Ok(Step::Processed)
    }
}

impl HotlapValidation {
    async fn validate(
        &self,
        transaction: &DatabaseTransaction,
        hotlap: &HotlapModel,
    ) -> anyhow::Result<HlvcResult> {
        let key = hotlap
            .spr_object_key
            .as_deref()
            .context("uploaded hotlap has no replay object")?;
        let replay = self
            .object_store
            .get(&Path::from(key))
            .await?
            .bytes()
            .await
            .with_context(|| format!("failed to download replay {key}"))?;
        let era = EraEntity::find_by_id(hotlap.era_id)
            .one(transaction)
            .await?
            .context("hotlap references a missing era")?;
        Ok(
            hlvc::validate(&self.runtime, &self.settings, &era.installation_id, &replay)
                .await?
                .lfs,
        )
    }
}

#[cfg(test)]
mod tests;
