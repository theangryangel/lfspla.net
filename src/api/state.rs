//! Dependencies shared by API request handlers.

use std::sync::Arc;

use object_store::ObjectStore;
use sea_orm::DatabaseConnection;

use crate::settings::HotlapSettings;
use lfsplanet_lfs_api::OAuthProvider;

/// Shared API dependencies.
#[derive(Clone)]
pub struct ApiState {
    pub(crate) database: DatabaseConnection,
    pub(crate) oauth: Option<OAuthProvider>,
    pub(crate) object_store: Arc<dyn ObjectStore>,
    pub(crate) hotlaps: HotlapSettings,
}
