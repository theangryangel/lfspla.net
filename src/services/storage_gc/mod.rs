//! Collect unreferenced objects from the configured attachment store.

use crate::{models::attachment, settings::StorageSettings, storage};
use anyhow::Context;
use futures::TryStreamExt;
use object_store::{ObjectMeta, ObjectStore, ObjectStoreExt};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use std::{
    collections::HashSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Limits memory and SQL parameter counts while walking an object namespace.
const GC_BATCH_SIZE: usize = 1_000;

/// The outcome of one record-backed object namespace sweep.
#[derive(Debug)]
pub(crate) struct GcSummary {
    pub prefix: &'static str,
    pub referenced: usize,
    pub stored: usize,
    pub eligible: usize,
    pub protected: usize,
}

pub(crate) async fn collect(
    settings: &StorageSettings,
    database: &DatabaseConnection,
    delete: bool,
    older_than_hours: u64,
    report: impl Fn(&ObjectMeta),
) -> anyhow::Result<[GcSummary; 1]> {
    let store = storage::build(settings)?;
    let minimum_age = Duration::from_secs(
        older_than_hours
            .checked_mul(3600)
            .context("object minimum age is too large")?,
    );
    Ok([sweep(
        &*store,
        database,
        SystemTime::now(),
        minimum_age,
        delete,
        report,
    )
    .await?])
}

pub(crate) async fn sweep(
    store: &dyn ObjectStore,
    database: &DatabaseConnection,
    now: SystemTime,
    minimum_age: Duration,
    delete: bool,
    report: impl Fn(&ObjectMeta),
) -> anyhow::Result<GcSummary> {
    let cutoff = now
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .saturating_sub(minimum_age.as_secs());
    let mut summary = GcSummary {
        prefix: "attachments",
        referenced: 0,
        stored: 0,
        eligible: 0,
        protected: 0,
    };
    let mut objects = store.list(None);
    let mut batch = Vec::with_capacity(GC_BATCH_SIZE);
    while let Some(object) = objects.try_next().await? {
        batch.push(object);
        if batch.len() == GC_BATCH_SIZE {
            process_batch(
                store,
                database,
                &mut batch,
                cutoff,
                delete,
                &report,
                &mut summary,
            )
            .await?;
        }
    }
    if !batch.is_empty() {
        process_batch(
            store,
            database,
            &mut batch,
            cutoff,
            delete,
            &report,
            &mut summary,
        )
        .await?;
    }
    Ok(summary)
}

async fn process_batch(
    store: &dyn ObjectStore,
    database: &DatabaseConnection,
    batch: &mut Vec<ObjectMeta>,
    cutoff: u64,
    delete: bool,
    report: &impl Fn(&ObjectMeta),
    summary: &mut GcSummary,
) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    // Uploaders hold a shared lock until their parent reference is committed.
    // Under this exclusive lock, references and physical bytes cannot race GC.
    attachment::lock(&transaction, true).await?;
    let keys: Vec<String> = batch
        .iter()
        .map(|object| object.location.to_string())
        .collect();
    let rows = transaction
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT object_key FROM attachment a WHERE object_key = ANY($1::text[]) AND (
            EXISTS (SELECT 1 FROM hotlap WHERE spr_attachment_id = a.id OR raf_attachment_id = a.id)
            OR EXISTS (SELECT 1 FROM vehicle WHERE image_attachment_id = a.id))",
            [keys.clone().into()],
        ))
        .await?;
    let retained = rows
        .iter()
        .map(|row| row.try_get::<String>("", "object_key"))
        .collect::<Result<HashSet<_>, _>>()?;
    if delete {
        transaction.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "DELETE FROM attachment a WHERE object_key = ANY($1::text[]) AND created_at < to_timestamp($2::bigint)
             AND NOT EXISTS (SELECT 1 FROM hotlap WHERE spr_attachment_id = a.id OR raf_attachment_id = a.id)
             AND NOT EXISTS (SELECT 1 FROM vehicle WHERE image_attachment_id = a.id)",
            [keys.into(), i64::try_from(cutoff)?.into()])).await?;
    }
    summary.stored += batch.len();
    for listed in batch.drain(..) {
        if retained.contains(listed.location.as_ref()) {
            summary.referenced += 1;
            continue;
        }
        // Refresh age after taking the lock: an upload may have replaced bytes
        // since the initial listing, and must receive the minimum-age protection.
        let object = match store.head(&listed.location).await {
            Ok(object) => object,
            Err(object_store::Error::NotFound { .. }) => continue,
            Err(error) => return Err(error.into()),
        };
        if u64::try_from(object.last_modified.timestamp()).unwrap_or(u64::MAX) > cutoff {
            summary.protected += 1;
            continue;
        }
        if delete {
            match store.delete(&object.location).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => (),
                Err(error) => return Err(error.into()),
            }
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM attachment WHERE object_key = $1",
                    [object.location.to_string().into()],
                ))
                .await?;
        }
        report(&object);
        summary.eligible += 1;
    }
    transaction.commit().await?;
    Ok(())
}
