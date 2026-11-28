//! Artwork for one named badge presentation.
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "badge_variant")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub badge_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub image_attachment_id: Option<i64>,
}
impl ActiveModelBehavior for ActiveModel {}
