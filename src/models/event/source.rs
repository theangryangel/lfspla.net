//! Provenance shared by leagues, seasons and advertised events.

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

/// Stored as ordinary text; adding a source does not require a PostgreSQL enum.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "lfs_net")]
    LfsNet,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Change {
    Created,
    Updated,
    Unchanged,
}
