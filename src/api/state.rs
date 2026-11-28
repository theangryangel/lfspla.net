//! Dependencies shared by API request handlers.

use std::sync::Arc;

use object_store::ObjectStore;
use sea_orm::DatabaseConnection;

use crate::settings::HotlapSettings;
use lfsplanet_lfs_net_api::OAuthProvider;

/// Shared API dependencies.
#[derive(Clone)]
pub struct ApiState {
    pub(crate) database: DatabaseConnection,
    pub(crate) oauth: Option<OAuthProvider>,
    pub(crate) object_store: Arc<dyn ObjectStore>,
    pub(crate) hotlaps: HotlapSettings,
    pub(crate) site_notice: Option<String>,
}

#[cfg(test)]
impl ApiState {
    /// Test state with an in-memory object store and default settings.
    pub(crate) fn for_tests(database: DatabaseConnection) -> Self {
        Self {
            database,
            oauth: None,
            object_store: Arc::new(object_store::memory::InMemory::new()),
            hotlaps: HotlapSettings::default(),
            site_notice: None,
        }
    }
}
