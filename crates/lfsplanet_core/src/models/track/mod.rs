//! Stable track metadata and eligibility.

use sea_orm::entity::prelude::*;

use crate::db::escaped_like::EscapedLike;
use sea_orm::{
    Condition, QueryFilter, QueryOrder, Select,
    sea_query::{Expr, extension::postgres::PgExpr},
};

pub mod location;

pub use location::Model as TrackLocation;

/// One canonical LFS track configuration.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "track")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: crate::track_id::TrackId,
    pub name: String,
    pub license: String,
    pub location: String,
    #[sea_orm(belongs_to, from = "location", to = "id")]
    pub location_record: HasOne<location::Entity>,
    pub reverse: bool,
    pub open_configuration: bool,
    pub sequence: i32,
    pub outline_attachment_id: Option<i64>,
    pub image_attachment_id: Option<i64>,
}

impl ActiveModelBehavior for ActiveModel {}

/// Literal, case-insensitive code and name search for the track catalogue.
pub trait TrackFilter: QueryFilter + Sized {
    fn name_or_id_contains(self, query: &str) -> Self {
        let query = query.trim();
        if query.is_empty() {
            return self;
        }
        let pattern = EscapedLike::contains(query);
        self.filter(
            Condition::any()
                .add(Expr::col(Column::Id).ilike(pattern.clone()))
                .add(Expr::col(Column::Name).ilike(pattern)),
        )
    }
}

impl TrackFilter for Select<Entity> {}

/// Canonical presentation ordering for track catalogue queries.
pub trait TrackOrder: QueryOrder + Sized {
    fn in_catalogue_order(self) -> Self {
        self.order_by_asc(Column::Sequence).order_by_asc(Column::Id)
    }
}

impl TrackOrder for Select<Entity> {}
