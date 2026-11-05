//! Public threaded conversation; a reason marks an entry as a report.
use super::report_reason::ReportReason;
use sea_orm::entity::prelude::*;
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "hotlap_comment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub hotlap_id: Option<i64>,
    pub original_hotlap_id: i64,
    pub parent_id: Option<i64>,
    pub root_id: Option<i64>,
    pub depth: i16,
    pub author_id: i64,
    pub body: Option<String>,
    pub reason: Option<ReportReason>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub deleted_at: Option<TimeDateTimeWithTimeZone>,
    pub resolved_at: Option<TimeDateTimeWithTimeZone>,
    pub resolved_by: Option<i64>,
    #[sea_orm(belongs_to, from = "author_id", to = "id")]
    pub author: BelongsTo<crate::models::player::Entity>,
}
impl ActiveModelBehavior for ActiveModel {}
