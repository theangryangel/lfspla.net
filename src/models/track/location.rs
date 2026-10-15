//! Shared location identity and authored presentation for track configurations.

use insim_core::track::Track;
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "track_location")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub sequence: i32,
    pub image_attachment_id: Option<i64>,
    #[sea_orm(has_many)]
    pub tracks: HasMany<super::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

/// Canonical location code, including Autocross under Other.
pub(crate) fn canonical_code(track: Track) -> &'static str {
    match track.code()[..2].to_ascii_uppercase().as_str() {
        "BL" => "BL",
        "SO" => "SO",
        "FE" => "FE",
        "KY" => "KY",
        "AS" => "AS",
        "WE" => "WE",
        "RO" => "RO",
        _ => "OTHER",
    }
}
