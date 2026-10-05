//! Collect unreferenced replay and vehicle image objects.

use crate::{
    models::{hotlap::Entity as HotlapEntity, vehicle::Entity as VehicleEntity},
    settings::StorageSettings,
    storage::{self, GcSummary, Storage},
};
use anyhow::Context;
use object_store::{ObjectStore, ObjectStoreExt};
use sea_orm::DatabaseConnection;
use std::time::{Duration, SystemTime};

/// Inspects or removes old objects no longer retained by their database rows.
pub(crate) async fn collect(
    settings: &StorageSettings,
    database: &DatabaseConnection,
    delete: bool,
    older_than_hours: u64,
    report: impl Fn(&object_store::ObjectMeta),
) -> anyhow::Result<[GcSummary; 2]> {
    let store = storage::build(settings)?;
    let minimum_age = Duration::from_secs(
        older_than_hours
            .checked_mul(60 * 60)
            .context("object minimum age is too large")?,
    );
    let now = SystemTime::now();
    let summaries = [
        gc_source::<HotlapEntity>(&*store, database, now, minimum_age, delete, &report).await?,
        gc_source::<VehicleEntity>(&*store, database, now, minimum_age, delete, &report).await?,
    ];

    Ok(summaries)
}

async fn gc_source<S: Storage>(
    store: &dyn ObjectStore,
    database: &DatabaseConnection,
    now: SystemTime,
    minimum_age: Duration,
    delete: bool,
    report: &impl Fn(&object_store::ObjectMeta),
) -> anyhow::Result<GcSummary> {
    S::gc(database, store, now, minimum_age, |object| async move {
        if delete {
            match store.delete(&object.location).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to delete object {}", object.location));
                }
            }
        }
        report(&object);
        Ok(())
    })
    .await
}
