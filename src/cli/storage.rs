//! Object-storage garbage collection output.

use crate::{services::storage_gc, settings::StorageSettings};
use sea_orm::DatabaseConnection;

pub(super) async fn gc(
    settings: &StorageSettings,
    database: &DatabaseConnection,
    delete: bool,
    older_than_hours: u64,
) -> anyhow::Result<()> {
    let summaries = storage_gc::collect(settings, database, delete, older_than_hours, |object| {
        println!(
            "{}\t{}\t{}",
            if delete { "deleted" } else { "would-delete" },
            object.location,
            object.size
        );
    })
    .await?;
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
