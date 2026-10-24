//! Synchronization and resolution of mutable Vehicle Mods catalogue metadata.

use crate::models::vehicle::{
    ActiveModel as VehicleMutation, Column as VehicleColumn, Entity as VehicleEntity,
};
use crate::vehicle_id::VehicleId;
use insim_core::vehicle::Vehicle;
use lfsplanet_lfs_api::RemoteVehicleMod;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use std::collections::HashMap;
const INSERT_CHUNK_SIZE: usize = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModCandidate {
    id: VehicleId,
    version: Option<i16>,
}

impl crate::models::Vehicle {
    pub(crate) async fn resolve(
        database: &DatabaseConnection,
        name: &str,
        version: Option<u16>,
    ) -> Result<Option<Vehicle>, sea_orm::DbErr> {
        let normalized = crate::models::Vehicle::normalize_name(name);
        if normalized.is_empty() {
            return Ok(None);
        }

        let candidates = VehicleEntity::find()
            .filter(VehicleColumn::Kind.eq("mod"))
            .filter(VehicleColumn::Available.eq(true))
            .filter(VehicleColumn::NormalizedName.eq(&normalized))
            .all(database)
            .await?
            .into_iter()
            .map(|model| ModCandidate {
                id: model.id,
                version: model.version,
            })
            .collect::<Vec<_>>();
        resolve_candidates(&candidates, version)
    }
}

/// Atomically replaces current availability while retaining all observed names.
impl crate::models::Vehicle {
    pub(crate) async fn refresh(
        database: &DatabaseConnection,
        remote: &[RemoteVehicleMod],
    ) -> Result<(), sea_orm::DbErr> {
        let transaction = database.begin().await?;
        crate::db::lock_catalogue_write(&transaction).await?;
        let observed_at = time::OffsetDateTime::now_utc();
        let previous = VehicleEntity::find()
            .filter(VehicleColumn::Kind.eq("mod"))
            .all(&transaction)
            .await?
            .into_iter()
            .map(|vehicle| (vehicle.id.to_string(), (vehicle.version, vehicle.metadata)))
            .collect::<HashMap<_, _>>();

        VehicleEntity::update_many()
            .filter(VehicleColumn::Kind.eq("mod"))
            .col_expr(VehicleColumn::Available, Expr::value(false))
            .exec(&transaction)
            .await?;

        for chunk in remote.chunks(INSERT_CHUNK_SIZE) {
            let mods = chunk
                .iter()
                .map(|entry| {
                    Ok(VehicleMutation {
                        id: Set(entry.id.parse::<VehicleId>()?),
                        kind: Set("mod".to_owned()),
                        sequence: NotSet,
                        name: Set(entry.name.clone()),
                        normalized_name: Set(crate::models::Vehicle::normalize_name(&entry.name)),
                        license: Set("s3".to_owned()),
                        version: Set(Some(i16::try_from(entry.version).map_err(|_| {
                            sea_orm::DbErr::Type(format!("mod {} has an invalid version", entry.id))
                        })?)),
                        class: Set(entry.class.map(i16::from)),
                        author_username: Set(entry.author_username.clone()),
                        work_in_progress: Set(entry.work_in_progress),
                        published_at: Set(entry
                            .published_at
                            .map(time::OffsetDateTime::from_unix_timestamp)
                            .transpose()
                            .map_err(|error| sea_orm::DbErr::Type(error.to_string()))?),
                        available: Set(true),
                        metadata: Set(Some(merged_metadata(entry, previous.get(&entry.id)))),
                        fetched_at: Set(Some(observed_at)),
                        last_seen_at: Set(Some(observed_at)),
                        image_attachment_id: Set(None),
                        image_fetched_at: Set(None),
                        image_version: Set(None),
                    })
                })
                .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;

            VehicleEntity::insert_many(mods)
                .on_conflict(
                    OnConflict::column(VehicleColumn::Id)
                        .update_columns([
                            VehicleColumn::Name,
                            VehicleColumn::NormalizedName,
                            VehicleColumn::Version,
                            VehicleColumn::Class,
                            VehicleColumn::AuthorUsername,
                            VehicleColumn::WorkInProgress,
                            VehicleColumn::PublishedAt,
                            VehicleColumn::Available,
                            VehicleColumn::Metadata,
                            VehicleColumn::FetchedAt,
                            VehicleColumn::LastSeenAt,
                        ])
                        .to_owned(),
                )
                .exec(&transaction)
                .await?;
        }

        transaction.commit().await
    }
}

/// Keep detail-only fields on unchanged revisions while refreshing list counters.
/// A new revision starts with the list payload, making missing details retryable.
fn merged_metadata(
    entry: &RemoteVehicleMod,
    previous: Option<&(Option<i16>, Option<serde_json::Value>)>,
) -> serde_json::Value {
    if let Some((Some(version), Some(serde_json::Value::Object(cached)))) = previous
        && u16::try_from(*version).ok() == Some(entry.version)
        && let Some(current) = entry.metadata.as_object()
    {
        let mut merged = cached.clone();
        merged.extend(current.clone());
        serde_json::Value::Object(merged)
    } else {
        entry.metadata.clone()
    }
}

/// Captures the cached revision before the mutable upstream catalogue updates.
impl crate::models::Vehicle {
    pub(crate) async fn image_cache_state(
        database: &DatabaseConnection,
    ) -> Result<HashMap<String, (Option<i16>, Option<i64>)>, sea_orm::DbErr> {
        VehicleEntity::find()
            .filter(VehicleColumn::Kind.eq("mod"))
            .all(database)
            .await
            .map(|vehicles| {
                vehicles
                    .into_iter()
                    .map(|vehicle| {
                        (
                            vehicle.id.to_string(),
                            (vehicle.image_version, vehicle.image_attachment_id),
                        )
                    })
                    .collect()
            })
    }
}

fn resolve_candidates(
    candidates: &[ModCandidate],
    version: Option<u16>,
) -> Result<Option<Vehicle>, sea_orm::DbErr> {
    if let Some(version) = version {
        let version = i16::try_from(version)
            .map_err(|_| sea_orm::DbErr::Type("mod version exceeds SMALLINT".to_owned()))?;
        let exact = candidates
            .iter()
            .filter(|candidate| candidate.version == Some(version))
            .cloned()
            .collect::<Vec<_>>();
        if !exact.is_empty() {
            return resolution_from_candidates(&exact);
        }
    }
    resolution_from_candidates(candidates)
}

fn resolution_from_candidates(
    candidates: &[ModCandidate],
) -> Result<Option<Vehicle>, sea_orm::DbErr> {
    if candidates.len() > 1 {
        return Ok(None);
    }
    let Some(candidate) = candidates.first() else {
        return Ok(None);
    };
    let id = candidate.id;
    let invalid = || sea_orm::DbErr::Type(format!("invalid mod ID stored in catalogue: {id}"));
    let vehicle = id.0;
    if !vehicle.is_mod() {
        return Err(invalid());
    }
    Ok(Some(vehicle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_refresh_preserves_details_only_for_the_same_revision() {
        let mut entry = RemoteVehicleMod {
            id: "ABCDEF".to_owned(),
            name: "Example".to_owned(),
            version: 7,
            class: None,
            author_username: None,
            work_in_progress: false,
            is_private: false,
            is_tweak_mod: false,
            published_at: None,
            cover_thumb_url: None,
            metadata: serde_json::json!({"version": 7, "numDownloads": 200}),
        };
        let previous = (
            Some(7),
            Some(serde_json::json!({
                "version": 7, "numDownloads": 100, "vehicle": {"power": 160}
            })),
        );
        let merged = merged_metadata(&entry, Some(&previous));
        assert_eq!(merged["numDownloads"], 200);
        assert_eq!(merged["vehicle"]["power"], 160);
        entry.version = 8;
        entry.metadata["version"] = 8.into();
        assert!(
            merged_metadata(&entry, Some(&previous))
                .get("vehicle")
                .is_none()
        );
        assert_eq!(merged_metadata(&entry, None), entry.metadata);
    }

    fn candidate(id: &str, version: Option<i16>) -> ModCandidate {
        ModCandidate {
            id: id.parse().expect("valid test vehicle ID"),
            version,
        }
    }

    #[test]
    fn an_exact_revision_match_beats_an_ambiguous_name() {
        let candidates = [candidate("41A2A0", Some(7)), candidate("41A2A1", Some(8))];

        assert!(resolve_candidates(&candidates, Some(7)).unwrap().is_some());
        assert_eq!(resolve_candidates(&candidates, None).unwrap(), None);
        assert_eq!(resolve_candidates(&candidates, Some(99)).unwrap(), None);
    }

    #[test]
    fn a_single_name_match_resolves_and_no_match_does_not() {
        assert!(
            resolve_candidates(&[candidate("41A2A0", Some(7))], None)
                .unwrap()
                .is_some()
        );
        assert_eq!(resolve_candidates(&[], None).unwrap(), None);
    }
}
