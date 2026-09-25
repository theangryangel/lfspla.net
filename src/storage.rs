//! Shared object-store construction and record-backed object namespaces.

use std::{
    future::Future,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use futures::TryStreamExt;
use object_store::{
    ObjectMeta, ObjectStore, PutPayload, local::LocalFileSystem, path::Path as ObjectPath,
};
use sea_orm::{DatabaseConnection, DbErr};

use crate::settings::StorageSettings;

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

/// Object-store namespace owned by records of a single entity type.
///
/// Implementations identify which stored objects remain live. The shared
/// maintenance command chooses its reporting and deletion behavior.
pub(crate) trait Storage {
    /// Namespace containing this entity's objects.
    const PREFIX: &'static str;

    /// Resolves a namespace-relative suffix into a canonical object key.
    fn object_key(suffix: &str) -> String {
        format!("{}/{suffix}", Self::PREFIX)
    }

    /// Stores bytes at a namespace-relative suffix and returns its object key.
    async fn store(
        object_store: &dyn ObjectStore,
        suffix: &str,
        payload: PutPayload,
    ) -> Result<String, object_store::Error> {
        let key = Self::object_key(suffix);
        object_store
            .put(&ObjectPath::from(key.as_str()), payload)
            .await?;
        Ok(key)
    }

    /// Selects the supplied physical objects that no current record retains.
    ///
    /// `objects` is deliberately a bounded batch from the object-store
    /// listing, avoiding an application-sized set of all live keys.
    async fn should_gc(
        database: &DatabaseConnection,
        objects: &[ObjectMeta],
    ) -> Result<Vec<ObjectMeta>, DbErr>;

    /// Streams this namespace through record-backed retention checks.
    ///
    /// The visitor receives each old, unretained object immediately, so the
    /// caller can report or delete it without retaining a full GC plan.
    async fn gc<F, Fut>(
        database: &DatabaseConnection,
        object_store: &dyn ObjectStore,
        now: SystemTime,
        minimum_age: Duration,
        mut visit: F,
    ) -> anyhow::Result<GcSummary>
    where
        F: FnMut(ObjectMeta) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let cutoff = now
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the Unix epoch")?
            .as_secs()
            .saturating_sub(minimum_age.as_secs());
        let mut listing = object_store.list(Some(&ObjectPath::from(Self::PREFIX)));
        let mut summary = GcSummary {
            prefix: Self::PREFIX,
            referenced: 0,
            stored: 0,
            eligible: 0,
            protected: 0,
        };
        let mut batch = Vec::with_capacity(GC_BATCH_SIZE);

        while let Some(object) = listing
            .try_next()
            .await
            .with_context(|| format!("failed to list objects under {}", Self::PREFIX))?
        {
            batch.push(object);
            if batch.len() == GC_BATCH_SIZE {
                Self::gc_batch(database, &mut summary, &mut batch, cutoff, &mut visit).await?;
            }
        }
        if !batch.is_empty() {
            Self::gc_batch(database, &mut summary, &mut batch, cutoff, &mut visit).await?;
        }
        Ok(summary)
    }

    async fn gc_batch<F, Fut>(
        database: &DatabaseConnection,
        summary: &mut GcSummary,
        batch: &mut Vec<ObjectMeta>,
        cutoff: u64,
        visit: &mut F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(ObjectMeta) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let unreferenced = Self::should_gc(database, batch)
            .await
            .with_context(|| format!("failed to query object references under {}", Self::PREFIX))?;
        summary.stored += batch.len();
        summary.referenced += batch.len().saturating_sub(unreferenced.len());
        for object in unreferenced {
            if is_older_than(&object, cutoff) {
                visit(object).await?;
                summary.eligible += 1;
            } else {
                summary.protected += 1;
            }
        }
        batch.clear();
        Ok(())
    }
}

fn is_older_than(object: &ObjectMeta, cutoff: u64) -> bool {
    // Skip objects with unreadable modification times.
    u64::try_from(object.last_modified.timestamp()).unwrap_or(u64::MAX) <= cutoff
}

/// Builds the configured replay object store.
pub fn build(settings: &StorageSettings) -> anyhow::Result<Arc<dyn ObjectStore>> {
    let root = settings.object_store_root.path();
    std::fs::create_dir_all(root)
        .with_context(|| format!("failed to create object-store root {}", root.display()))?;
    let store = LocalFileSystem::new_with_prefix(root)
        .with_context(|| format!("failed to open object-store root {}", root.display()))?;
    Ok(Arc::new(store))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_only_old_objects_as_deletion_eligible() {
        let object = ObjectMeta {
            location: ObjectPath::from("test/object"),
            last_modified: (UNIX_EPOCH + Duration::from_secs(100)).into(),
            size: 123,
            e_tag: None,
            version: None,
        };
        assert!(is_older_than(&object, 900));
        assert!(!is_older_than(&object, 50));
    }
}
