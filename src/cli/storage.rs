//! Object-storage garbage collection command orchestration.

use std::time::{Duration, SystemTime};

use anyhow::Context;
use object_store::ObjectStore;
use sea_orm::DatabaseConnection;

use crate::{
    models::{hotlaps::HotlapEntity, vehicles::VehicleEntity},
    settings::StorageSettings,
    storage::{self, GcSummary, Storage},
};

/// Inspects or removes old objects no longer retained by their database rows.
pub(super) async fn gc(
    settings: &StorageSettings,
    database: &DatabaseConnection,
    delete: bool,
    older_than_hours: u64,
) -> anyhow::Result<()> {
    let store = storage::build(settings)?;
    let minimum_age = Duration::from_secs(
        older_than_hours
            .checked_mul(60 * 60)
            .context("object minimum age is too large")?,
    );
    let now = SystemTime::now();
    let summaries = [
        gc_source::<HotlapEntity>(&*store, database, now, minimum_age, delete).await?,
        gc_source::<VehicleEntity>(&*store, database, now, minimum_age, delete).await?,
    ];

    for summary in summaries {
        println!(
            "summary\tprefix={}\treferenced={}\tstored={}\teligible={}\tprotected={}\tmode={}",
            summary.prefix,
            summary.referenced,
            summary.stored,
            summary.eligible,
            summary.protected,
            if delete { "delete" } else { "dry-run" },
        );
    }
    Ok(())
}

async fn gc_source<S: Storage>(
    store: &dyn ObjectStore,
    database: &DatabaseConnection,
    now: SystemTime,
    minimum_age: Duration,
    delete: bool,
) -> anyhow::Result<GcSummary> {
    S::gc(database, store, now, minimum_age, |object| async move {
        if delete {
            match store.delete(&object.location).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => {
                    println!("deleted\t{}\t{}", object.location, object.size);
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to delete object {}", object.location));
                }
            }
        } else {
            println!("would-delete\t{}\t{}", object.location, object.size);
        }
        Ok(())
    })
    .await
}
