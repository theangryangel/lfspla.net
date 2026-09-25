//! SeaORM entity for the `player` table.

use sea_orm::entity::prelude::*;

use crate::models::country::CountryCode;

/// A Live for Speed player and their current profile.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "player")]
pub struct Model {
    /// Database identity.
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Stable LFS account name.
    #[sea_orm(unique)]
    pub lfs_username: String,
    /// Current display name returned by LFS.
    pub display_name: String,
    /// Whether every authentication path should reject this player.
    pub deny_auth: bool,
    /// Whether the hotlap upload route should reject this player.
    pub deny_uploads: bool,
    /// Nation selected for rankings, when present.
    ///
    /// Validated on read by [`CountryCode`], so no row loaded through this
    /// entity can carry a code that names no country.
    pub country_code: Option<CountryCode>,
    /// Time at which this player was first created.
    pub created_at: TimeDateTimeWithTimeZone,
    /// Time at which LFS most recently authenticated this player.
    pub last_authenticated_at: Option<TimeDateTimeWithTimeZone>,
    /// Numeric identity from the old LFSWorld database, when imported.
    #[sea_orm(unique)]
    pub lfsworld_id: Option<i64>,
    #[sea_orm(has_many)]
    pub era_badges: HasMany<crate::models::badges::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlaps::Entity>,
    #[sea_orm(has_many)]
    pub personal_access_tokens: HasMany<crate::models::personal_access_tokens::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
