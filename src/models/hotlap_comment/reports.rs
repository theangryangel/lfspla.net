//! Report counts for published hotlaps.
use super::{Column, CommentFilter, Entity, Model};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect};

#[derive(sea_orm::FromQueryResult)]
pub(crate) struct ReportCounts {
    pub(crate) hotlap_id: i64,
    pub(crate) total: i64,
    pub(crate) unresolved: i64,
}

impl Model {
    pub(crate) async fn report_counts(
        database: &impl ConnectionTrait,
        hotlap_ids: Vec<i64>,
    ) -> Result<Vec<ReportCounts>, DbErr> {
        if hotlap_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .select_only()
            .column(Column::HotlapId)
            .column_as(sea_orm::sea_query::Expr::cust("COUNT(*)"), "total")
            .column_as(
                sea_orm::sea_query::Expr::cust("COUNT(*) FILTER (WHERE resolved_at IS NULL)"),
                "unresolved",
            )
            .filter(Column::HotlapId.is_in(hotlap_ids))
            .reported(true)
            .group_by(Column::HotlapId)
            .into_model::<ReportCounts>()
            .all(database)
            .await
    }
}
