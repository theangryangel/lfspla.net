//! The unified vehicle catalogue and explicit era policy.

use crate::models::hotlap::HotlapRankable;

use insim_core::vehicle::Vehicle;

use sea_orm::{
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    sea_query::{Expr, OnConflict},
};

use crate::storage::Storage;

use object_store::ObjectMeta;

use sea_orm::{QuerySelect, entity::prelude::*};

use std::collections::HashSet;

use crate::db::query::escape_like;

use sea_orm::{
    Condition, ExprTrait, Order, Select,
    sea_query::{CaseStatement, Func, SimpleExpr, extension::postgres::PgExpr},
};

mod id;
pub(crate) use id::VehicleId;
mod mods;

/// Canonical vehicle identity and presentation/upstream metadata.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub kind: String,
    pub sequence: i32,
    pub name: String,
    pub normalized_name: String,
    pub license: String,
    pub version: Option<i16>,
    pub class: Option<i16>,
    pub author_username: Option<String>,
    pub work_in_progress: bool,
    pub published_at: Option<TimeDateTimeWithTimeZone>,
    pub available: bool,
    pub metadata: Option<Json>,
    pub fetched_at: Option<TimeDateTimeWithTimeZone>,
    pub last_seen_at: Option<TimeDateTimeWithTimeZone>,
    pub image_object_key: Option<String>,
    pub image_content_type: Option<String>,
    pub image_fetched_at: Option<TimeDateTimeWithTimeZone>,
    /// Mod revision whose cover was successfully cached; independent of metadata.
    pub image_version: Option<i16>,
    #[sea_orm(has_many)]
    pub ranking_charts: HasMany<crate::models::ranking_chart::Entity>,
    #[sea_orm(has_many)]
    pub hotlaps: HasMany<crate::models::hotlap::Entity>,
}

pub(crate) const STANDARD_VEHICLES: &[(Vehicle, &str)] = &[
    (Vehicle::Xfg, "XF GTI"),
    (Vehicle::Xrg, "XR GT"),
    (Vehicle::Fbm, "Formula BMW FB02"),
    (Vehicle::Xrt, "XR GT Turbo"),
    (Vehicle::Rb4, "RB4 GT"),
    (Vehicle::Fxo, "FXO Turbo"),
    (Vehicle::Lx4, "LX4"),
    (Vehicle::Lx6, "LX6"),
    (Vehicle::Mrt, "MRT5"),
    (Vehicle::Uf1, "UF 1000"),
    (Vehicle::Rac, "RaceAbout"),
    (Vehicle::Fz5, "FZ50"),
    (Vehicle::Fox, "Formula XR"),
    (Vehicle::Xfr, "XF GTR"),
    (Vehicle::Ufr, "UF GTR"),
    (Vehicle::Fo8, "Formula V8"),
    (Vehicle::Fxr, "FXO GTR"),
    (Vehicle::Xrr, "XR GTR"),
    (Vehicle::Fzr, "FZ50 GTR"),
    (Vehicle::Bf1, "BMW Sauber F1.06"),
];

impl Model {
    /// Synchronizes every built-in vehicle known by this binary without touching
    /// mutable Vehicle Mods metadata.
    pub(crate) async fn sync_builtin(
        database: &impl sea_orm::ConnectionTrait,
    ) -> Result<(), sea_orm::DbErr> {
        Entity::insert_many(STANDARD_VEHICLES.iter().enumerate().map(
            |(sequence, (vehicle, name))| {
                ActiveModel {
                    id: Set(vehicle.to_string()),
                    kind: Set(KIND_STANDARD.to_owned()),
                    sequence: Set(i32::try_from(sequence)
                        .expect("the compiled vehicle catalogue is far smaller than i32::MAX")),
                    name: Set((*name).to_owned()),
                    normalized_name: Set(Self::normalize_name(name)),
                    license: Set(vehicle.license().to_string().to_ascii_lowercase()),
                    version: Set(None),
                    class: Set(None),
                    author_username: Set(None),
                    work_in_progress: Set(false),
                    published_at: Set(None),
                    available: Set(true),
                    metadata: Set(None),
                    fetched_at: Set(None),
                    last_seen_at: Set(None),
                    image_object_key: Set(None),
                    image_content_type: Set(None),
                    image_fetched_at: Set(None),
                    image_version: Set(None),
                }
            },
        ))
        .on_conflict(
            OnConflict::column(Column::Id)
                .update_columns([
                    Column::Kind,
                    Column::Sequence,
                    Column::Name,
                    Column::NormalizedName,
                    Column::License,
                    Column::Available,
                ])
                .to_owned(),
        )
        .exec(database)
        .await?;

        tracing::info!(
            vehicles = STANDARD_VEHICLES.len(),
            "built-in vehicles synchronized"
        );
        Ok(())
    }

    pub(crate) fn normalize_name(value: &str) -> String {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }

    /// Resolves an SPR display name against synchronized standard vehicle metadata.
    pub(crate) async fn standard_by_name(
        database: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<Vehicle>, sea_orm::DbErr> {
        let model = Entity::find()
            .standard()
            .with_normalized_name(&Self::normalize_name(name))
            .order_by_asc(Column::Id)
            .one(database)
            .await?;
        model
            .map(|model| {
                model
                    .id
                    .parse::<Vehicle>()
                    .expect("insim_core vehicle parsing is infallible")
                    .ensure_hotlap_rankable()
                    .map_err(|error| {
                        sea_orm::DbErr::Type(format!(
                            "invalid standard vehicle ID stored in catalogue: {error}"
                        ))
                    })
            })
            .transpose()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Storage for Entity {
    const PREFIX: &'static str = "vehicle-images";

    async fn should_gc(
        database: &DatabaseConnection,
        objects: &[ObjectMeta],
    ) -> Result<Vec<ObjectMeta>, DbErr> {
        let candidates = objects
            .iter()
            .map(|object| object.location.to_string())
            .collect::<Vec<_>>();
        let retained = Entity::find()
            .select_only()
            .column(Column::ImageObjectKey)
            .filter(Column::Available.eq(true))
            .filter(Column::ImageObjectKey.is_in(candidates))
            .into_tuple::<String>()
            .all(database)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        Ok(objects
            .iter()
            .filter(|object| !retained.contains(object.location.as_ref()))
            .cloned()
            .collect())
    }
}

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
                .add(Expr::col(Column::Id).ilike(pattern.clone()))
                .add(Expr::col(Column::Name).ilike(pattern)),
        )
    }

    fn standard(self) -> Self {
        self.filter(Column::Kind.eq(KIND_STANDARD))
    }

    fn with_normalized_name(self, normalized: &str) -> Self {
        self.filter(Column::NormalizedName.eq(normalized))
    }
}

impl VehicleFilter for Select<Entity> {}

/// The order a catalogue search returns its matches in.
pub(crate) trait VehicleOrder: QueryOrder + Sized {
    /// Sorts exact codes first, then standard vehicles before mods, then by
    /// catalogue order. Apply before the page limit.
    fn in_catalogue_search_order(self, query: &str) -> Self {
        let query = query.trim().to_lowercase();
        let exact_id_first = SimpleExpr::Case(Box::new(
            CaseStatement::new()
                .case(Expr::expr(Func::lower(Expr::col(Column::Id))).eq(query), 0)
                .finally(1),
        ));
        let standard_first = SimpleExpr::Case(Box::new(
            CaseStatement::new()
                .case(Expr::col(Column::Kind).eq(KIND_STANDARD), 0)
                .finally(1),
        ));

        self.order_by(exact_id_first, Order::Asc)
            .order_by(standard_first, Order::Asc)
            .order_by_asc(Column::Sequence)
            .order_by_asc(Column::Name)
            .order_by_asc(Column::Id)
    }
}

impl VehicleOrder for Select<Entity> {}

#[cfg(test)]
mod tests {
    use super::Model;

    #[test]
    fn normalization_is_conservative_but_whitespace_insensitive() {
        assert_eq!(
            Model::normalize_name(" PIRAN  FIREFLY 200 "),
            "piran firefly 200"
        );
        assert_ne!(
            Model::normalize_name("GT-V34"),
            Model::normalize_name("GT V34")
        );
    }
}
