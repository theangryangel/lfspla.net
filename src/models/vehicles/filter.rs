//! Named predicates and orderings over the `vehicle` table.

use sea_orm::{
    ColumnTrait, Condition, ExprTrait, Order, QueryFilter, QueryOrder, Select,
    sea_query::{CaseStatement, Expr, Func, SimpleExpr, extension::postgres::PgExpr},
};

use super::{VehicleColumn, VehicleEntity};
use crate::models::escape_like;

/// The `kind` value carried by vehicles that ship with the game.
pub(crate) const KIND_STANDARD: &str = "standard";

/// Database filters for vehicle queries.
pub(crate) trait VehicleFilter: QueryFilter + Sized {
    /// Searches vehicle codes and names. Blank searches match all vehicles.
    fn name_or_id_contains(self, query: &str) -> Self {
        let query = query.trim();
        if query.is_empty() {
            return self;
        }

        let pattern = format!("%{}%", escape_like(query));
        self.filter(
            Condition::any()
                .add(Expr::col(VehicleColumn::Id).ilike(pattern.clone()))
                .add(Expr::col(VehicleColumn::Name).ilike(pattern)),
        )
    }

    fn standard(self) -> Self {
        self.filter(VehicleColumn::Kind.eq(KIND_STANDARD))
    }

    fn with_normalized_name(self, normalized: &str) -> Self {
        self.filter(VehicleColumn::NormalizedName.eq(normalized))
    }
}

impl VehicleFilter for Select<VehicleEntity> {}

/// The order a catalogue search returns its matches in.
pub(crate) trait VehicleOrder: QueryOrder + Sized {
    /// Sorts exact codes first, then standard vehicles before mods, then by
    /// catalogue order. Apply before the page limit.
    fn in_catalogue_search_order(self, query: &str) -> Self {
        let query = query.trim().to_lowercase();
        let exact_id_first = SimpleExpr::Case(Box::new(
            CaseStatement::new()
                .case(
                    Expr::expr(Func::lower(Expr::col(VehicleColumn::Id))).eq(query),
                    0,
                )
                .finally(1),
        ));
        let standard_first = SimpleExpr::Case(Box::new(
            CaseStatement::new()
                .case(Expr::col(VehicleColumn::Kind).eq(KIND_STANDARD), 0)
                .finally(1),
        ));

        self.order_by(exact_id_first, Order::Asc)
            .order_by(standard_first, Order::Asc)
            .order_by_asc(VehicleColumn::Sequence)
            .order_by_asc(VehicleColumn::Name)
            .order_by_asc(VehicleColumn::Id)
    }
}

impl VehicleOrder for Select<VehicleEntity> {}
