//! Database ordering for paginated hotlap collections.
use super::{HotlapColumn, HotlapEntity};
use crate::models::Ordering;
use sea_orm::{
    ExprTrait, Order, QueryOrder, QueryTrait, Select,
    sea_query::{Alias, Expr, JoinType, NullOrdering},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HotlapListColumn {
    #[default]
    Submitted,
    Driver,
    Rank,
    LapTime,
}

/// Collection ordering supported by hotlap select queries.
pub(crate) trait HotlapOrder: Sized {
    fn ordered(self, column: HotlapListColumn, order: Ordering) -> Self;
}

impl HotlapOrder for Select<HotlapEntity> {
    fn ordered(mut self, column: HotlapListColumn, order: Ordering) -> Self {
        let order = match order {
            Ordering::Asc => Order::Asc,
            Ordering::Desc => Order::Desc,
        };
        // Join sort keys once across the candidate set, rather than running a scalar
        // subquery for every upload. Aliases avoid the related-player loading join.
        match column {
            HotlapListColumn::Rank => {
                QueryTrait::query(&mut self).join_as(
                    JoinType::LeftJoin,
                    Alias::new("hotlap_personal_best"),
                    Alias::new("sort_rank"),
                    Expr::col((Alias::new("sort_rank"), Alias::new("hotlap_id")))
                        .equals((HotlapEntity, HotlapColumn::Id))
                        .and(
                            Expr::col((Alias::new("sort_rank"), Alias::new("era_id")))
                                .equals((HotlapEntity, HotlapColumn::EraId)),
                        ),
                );
            }
            HotlapListColumn::Driver => {
                QueryTrait::query(&mut self).join_as(
                    JoinType::LeftJoin,
                    crate::models::players::PlayerEntity,
                    Alias::new("sort_player"),
                    Expr::col((Alias::new("sort_player"), Alias::new("id")))
                        .equals((HotlapEntity, HotlapColumn::PlayerId)),
                );
            }
            _ => {}
        }
        let query = match column {
            HotlapListColumn::Submitted => self.order_by(HotlapColumn::CreatedAt, order),
            HotlapListColumn::LapTime => self.order_by(HotlapColumn::LapTimeMs, order),
            HotlapListColumn::Driver => self.order_by(
                Expr::cust("LOWER(sort_player.display_name) COLLATE \"C\""),
                order,
            ),
            HotlapListColumn::Rank => self.order_by_with_nulls(
                Expr::cust("sort_rank.position"),
                order,
                NullOrdering::Last,
            ),
        };
        query
            .order_by_desc(HotlapColumn::CreatedAt)
            .order_by_desc(HotlapColumn::Id)
    }
}
