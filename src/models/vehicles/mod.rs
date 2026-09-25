//! The unified vehicle catalogue and explicit era policy.

mod entity;
mod filter;
pub(crate) mod mods;

pub(crate) use filter::{KIND_STANDARD, VehicleFilter, VehicleOrder};

pub(crate) use entity::*;
#[allow(unused_imports, reason = "part of the standard entity re-export set")]
pub use entity::{
    ActiveModel as VehicleMutation, Column as VehicleColumn, Entity as VehicleEntity,
    Model as VehicleModel,
};

use std::{path::Path, sync::Arc};

use anyhow::Context;
use insim_core::vehicle::Vehicle;
use object_store::{ObjectStore, PutPayload, path::Path as ObjectPath};
use sea_orm::{
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    sea_query::{Expr, OnConflict},
};

use crate::models::hotlaps::HotlapRankable;

const STANDARD_VEHICLES: &[(Vehicle, &str)] = &[
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

/// Synchronizes every built-in vehicle known by this binary without touching
/// mutable Vehicle Mods metadata.
pub(crate) async fn sync_builtin(database: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    VehicleEntity::insert_many(STANDARD_VEHICLES.iter().enumerate().map(
        |(sequence, (vehicle, name))| {
            VehicleMutation {
                id: Set(vehicle.to_string()),
                kind: Set(KIND_STANDARD.to_owned()),
                sequence: Set(i32::try_from(sequence)
                    .expect("the compiled vehicle catalogue is far smaller than i32::MAX")),
                name: Set((*name).to_owned()),
                normalized_name: Set(normalize_name(name)),
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
        OnConflict::column(VehicleColumn::Id)
            .update_columns([
                VehicleColumn::Kind,
                VehicleColumn::Sequence,
                VehicleColumn::Name,
                VehicleColumn::NormalizedName,
                VehicleColumn::License,
                VehicleColumn::Available,
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

/// Seeds version-controlled built-in vehicle images into object storage.
///
/// Each known built-in vehicle must have a PNG named after its canonical code.
/// Their database rows point at the deterministic object keys after this
/// succeeds, so API image delivery does not need a special built-in branch.
pub(crate) async fn seed_builtin_images(
    database: &DatabaseConnection,
    object_store: Arc<dyn ObjectStore>,
    directory: &Path,
) -> anyhow::Result<usize> {
    let mut seeded = 0;
    for (vehicle, _) in STANDARD_VEHICLES {
        let id = vehicle.to_string();
        let source = directory.join(format!("{id}.png"));
        let bytes = std::fs::read(&source).with_context(|| {
            format!("failed to read built-in vehicle image {}", source.display())
        })?;
        let object_key = format!("builtin-vehicles/{id}.png");
        object_store
            .put(
                &ObjectPath::from(object_key.as_str()),
                PutPayload::from(bytes),
            )
            .await
            .with_context(|| format!("failed to store built-in vehicle image {id}"))?;
        VehicleEntity::update_many()
            .filter(VehicleColumn::Id.eq(&id))
            .col_expr(VehicleColumn::ImageObjectKey, Expr::value(object_key))
            .col_expr(VehicleColumn::ImageContentType, Expr::value("image/png"))
            .col_expr(
                VehicleColumn::ImageFetchedAt,
                Expr::value(time::OffsetDateTime::now_utc()),
            )
            .exec(database)
            .await?;
        seeded += 1;
    }
    Ok(seeded)
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
    let model = VehicleEntity::find()
        .standard()
        .with_normalized_name(&normalize_name(name))
        .order_by_asc(VehicleColumn::Id)
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

#[cfg(test)]
mod tests {
    use super::normalize_name;

    #[test]
    fn normalization_is_conservative_but_whitespace_insensitive() {
        assert_eq!(normalize_name(" PIRAN  FIREFLY 200 "), "piran firefly 200");
        assert_ne!(normalize_name("GT-V34"), normalize_name("GT V34"));
    }
}
