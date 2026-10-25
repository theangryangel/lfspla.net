//! URL filters for the explorer; specifications remain in the upstream JSONB.
use crate::{
    api::ApiError,
    models::{
        player::{self, PlayerFilter},
        vehicle::{Column, Entity, VehicleOrder},
    },
};
use axum::http::StatusCode;
use lfsplanet_lfs_api::{Drivetrain, Transmission, VehicleClass};
use sea_orm::{
    ColumnTrait, EntityTrait, ExprTrait, Order, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
    Select,
    sea_query::{Expr, NullOrdering, SimpleExpr},
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Default, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VehicleSort {
    #[default]
    Catalogue,
    Newest,
    Downloads,
    Rating,
    Power,
    Mass,
    PowerWeight,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VehicleKind {
    Standard,
    Mod,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct VehicleQuery {
    /// Case-insensitive vehicle code or name search.
    pub q: Option<String>,
    #[param(inline)]
    pub kind: Option<VehicleKind>,
    /// Vehicle category. Vehicles without a class do not match.
    pub class: Option<VehicleClass>,
    /// Case-insensitive exact uploader username.
    pub author: Option<String>,
    pub approved: Option<bool>,
    pub electric: Option<bool>,
    pub drive: Option<Drivetrain>,
    pub transmission: Option<Transmission>,
    pub min_power_kw: Option<f64>,
    pub max_power_kw: Option<f64>,
    pub min_torque_nm: Option<f64>,
    pub max_torque_nm: Option<f64>,
    pub min_mass_kg: Option<f64>,
    pub max_mass_kg: Option<f64>,
    pub min_displacement_cc: Option<f64>,
    pub max_displacement_cc: Option<f64>,
    /// Power to weight in watts per kilogram.
    pub min_power_weight: Option<f64>,
    pub max_power_weight: Option<f64>,
    /// Descending except mass (lightest first); unknown values sort last.
    /// Rating blends the vehicle average with the catalogue average using 20 prior votes.
    #[serde(default)]
    #[param(inline)]
    pub sort: VehicleSort,
}

impl VehicleQuery {
    pub fn validate(&self) -> Result<(), ApiError> {
        let invalid = self.ranges().iter().any(|(_, min, max)| {
            min.is_some_and(|v| !v.is_finite() || v < 0.0)
                || max.is_some_and(|v| !v.is_finite() || v < 0.0)
                || matches!((min, max), (Some(min), Some(max)) if min > max)
        });
        if invalid {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_vehicle_filter",
                "Use finite non-negative ranges with minimum <= maximum",
            ));
        }
        Ok(())
    }

    fn ranges(&self) -> [(&'static str, Option<f64>, Option<f64>); 5] {
        [
            ("power", self.min_power_kw, self.max_power_kw),
            ("torque", self.min_torque_nm, self.max_torque_nm),
            ("mass", self.min_mass_kg, self.max_mass_kg),
            ("iceCc", self.min_displacement_cc, self.max_displacement_cc),
            (
                "powerWeightRatio",
                self.min_power_weight,
                self.max_power_weight,
            ),
        ]
    }

    pub fn filter(&self, mut vehicles: Select<Entity>) -> Select<Entity> {
        if let Some(kind) = self.kind {
            vehicles = vehicles.filter(Column::Kind.eq(match kind {
                VehicleKind::Standard => "standard",
                VehicleKind::Mod => "mod",
            }));
        }
        if let Some(class) = self.class {
            vehicles = vehicles.filter(Column::Class.eq(class));
        }
        if let Some(author) = self
            .author
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            vehicles = vehicles.filter(
                Column::AuthorPlayerId.in_subquery(
                    player::Entity::find()
                        .select_only()
                        .column(player::Column::Id)
                        .with_username(author)
                        .into_query(),
                ),
            );
        }
        for (key, value) in [("approved", self.approved), ("ev", self.electric)] {
            if let Some(value) = value {
                // Compare JSON booleans directly; absent fields remain unknown.
                vehicles = vehicles
                    .filter(Expr::cust(format!("metadata->'{key}'")).eq(serde_json::json!(value)));
            }
        }
        for (key, value) in [
            ("drive", self.drive.map(u8::from)),
            ("shiftType", self.transmission.map(u8::from)),
        ] {
            if let Some(value) = value {
                vehicles = vehicles.filter(spec_number(key).eq(f64::from(value)));
            }
        }
        for (key, min, max) in self.ranges() {
            if let Some(min) = min {
                vehicles = vehicles.filter(spec_number(key).gte(min));
            }
            if let Some(max) = max {
                vehicles = vehicles.filter(spec_number(key).lte(max));
            }
        }
        vehicles
    }

    pub fn order(&self, vehicles: Select<Entity>) -> Select<Entity> {
        let (expression, direction) = match self.sort {
            VehicleSort::Catalogue => {
                return vehicles.in_catalogue_search_order(self.q.as_deref().unwrap_or(""));
            }
            VehicleSort::Newest => (Expr::col(Column::PublishedAt), Order::Desc),
            VehicleSort::Downloads => (json_number("metadata", "numDownloads"), Order::Desc),
            VehicleSort::Rating => (weighted_rating(), Order::Desc),
            VehicleSort::Power => (spec_number("power"), Order::Desc),
            VehicleSort::Mass => (spec_number("mass"), Order::Asc),
            VehicleSort::PowerWeight => (spec_number("powerWeightRatio"), Order::Desc),
        };
        vehicles
            .order_by_with_nulls(expression, direction, NullOrdering::Last)
            .order_by_asc(Column::Id)
    }
}

fn spec_number(key: &'static str) -> SimpleExpr {
    json_number("metadata->'vehicle'", key)
}

/// Only internal constant paths reach this function. Malformed/missing upstream
/// values become SQL NULL instead of failing every filtered request.
fn json_number(object: &'static str, key: &'static str) -> SimpleExpr {
    Expr::cust(json_number_sql(object, key))
}

fn json_number_sql(object: &'static str, key: &'static str) -> String {
    format!(
        "CASE WHEN jsonb_typeof({object}->'{key}') = 'number' THEN ({object}->>'{key}')::double precision END"
    )
}

/// Twenty prior votes keep small samples close to the catalogue's vote-weighted
/// average. Use all available mods for the prior so filters do not change scores.
/// Missing, malformed and zero-vote ratings remain unranked (NULL, sorted last).
fn weighted_rating() -> SimpleExpr {
    let rating = json_number_sql("metadata", "rating");
    let votes = json_number_sql("metadata", "numRatings");
    Expr::cust(format!(
        "CASE WHEN ({rating}) >= 0 AND ({votes}) > 0 THEN \
         (({votes}) * ({rating}) + 20.0 * (\
             SELECT SUM(average * votes) / NULLIF(SUM(votes), 0) \
             FROM (SELECT ({rating}) AS average, ({votes}) AS votes \
                   FROM vehicle WHERE available = TRUE AND kind = 'mod') AS ratings \
             WHERE average >= 0 AND votes > 0\
         )) / (({votes}) + 20.0) END"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_codes_non_finite_and_inverted_ranges() {
        for query in [
            "min_power_kw=NaN",
            "max_mass_kg=inf",
            "min_mass_kg=-1",
            "min_power_kw=200&max_power_kw=100",
        ] {
            let query: VehicleQuery = serde_urlencoded::from_str(query).unwrap();
            assert!(query.validate().is_err());
        }
        for query in ["class=15", "drive=4", "transmission=8"] {
            assert!(serde_urlencoded::from_str::<VehicleQuery>(query).is_err());
        }
        let query: VehicleQuery =
            serde_urlencoded::from_str("drive=rear_wheel_drive&min_power_kw=100&max_power_kw=200")
                .unwrap();
        assert!(query.validate().is_ok());
    }
}
