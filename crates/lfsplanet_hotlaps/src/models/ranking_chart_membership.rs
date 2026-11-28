//! Ordered membership of a chart in a ranking.
use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ranking_chart_membership")]
pub struct Model {
    pub era_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub ranking_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub position: i32,
    pub chart_id: i64,
    #[sea_orm(belongs_to, from = "ranking_id", to = "id", on_delete = "Cascade")]
    pub ranking: BelongsTo<crate::models::ranking::Entity>,
    #[sea_orm(belongs_to, from = "chart_id", to = "id", on_delete = "Cascade")]
    pub chart: BelongsTo<crate::models::chart::Entity>,
}
impl ActiveModelBehavior for ActiveModel {}
