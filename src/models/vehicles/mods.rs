//! Synchronization and resolution of mutable Vehicle Mods catalogue metadata.

use std::{collections::HashMap, sync::Arc};

use futures::stream::{self, StreamExt};
use insim_core::vehicle::Vehicle;
use object_store::{ObjectStore, PutPayload};
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
    sea_query::{Expr, OnConflict},
};

use crate::models::{
    hotlaps::HotlapRankable,
    vehicles::{VehicleColumn, VehicleEntity, VehicleMutation},
};
use crate::storage::Storage;
use lfsplanet_lfs_api::{LfsClient, RemoteVehicleMod};
use sha2::{Digest, Sha256};

use super::normalize_name;

const INSERT_CHUNK_SIZE: usize = 250;
const IMAGE_DOWNLOAD_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModCandidate {
    id: String,
    version: Option<i16>,
}

pub(crate) async fn resolve(
    database: &DatabaseConnection,
    name: &str,
    version: Option<u16>,
) -> Result<Option<Vehicle>, sea_orm::DbErr> {
    let normalized = normalize_name(name);
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

/// Atomically replaces current availability while retaining all observed names.
pub(crate) async fn refresh(
    database: &DatabaseConnection,
    remote: &[RemoteVehicleMod],
) -> Result<(), sea_orm::DbErr> {
    let transaction = database.begin().await?;
    let observed_at = time::OffsetDateTime::now_utc();

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
                    id: Set(entry.id.clone()),
                    kind: Set("mod".to_owned()),
                    sequence: NotSet,
                    name: Set(entry.name.clone()),
                    normalized_name: Set(normalize_name(&entry.name)),
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
                    metadata: Set(Some(entry.metadata.clone())),
                    fetched_at: Set(Some(observed_at)),
                    last_seen_at: Set(Some(observed_at)),
                    image_object_key: Set(None),
                    image_content_type: Set(None),
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

/// Captures the cached revision before the mutable upstream catalogue updates.
pub(crate) async fn image_cache_state(
    database: &DatabaseConnection,
) -> Result<HashMap<String, (Option<i16>, Option<String>)>, sea_orm::DbErr> {
    VehicleEntity::find()
        .filter(VehicleColumn::Kind.eq("mod"))
        .all(database)
        .await
        .map(|vehicles| {
            vehicles
                .into_iter()
                .map(|vehicle| {
                    (
                        vehicle.id,
                        (vehicle.image_version, vehicle.image_object_key),
                    )
                })
                .collect()
        })
}

/// Downloads covers for new Mods and when their LFS revision changes.
///
/// A failed refresh leaves a previously cached image intact. This keeps
/// catalogue synchronization resilient to a temporary CDN failure.
pub(crate) async fn cache_images(
    database: &DatabaseConnection,
    object_store: Arc<dyn ObjectStore>,
    client: LfsClient,
    remote: &[RemoteVehicleMod],
    previous: &HashMap<String, (Option<i16>, Option<String>)>,
) -> Result<usize, sea_orm::DbErr> {
    let pending = remote
        .iter()
        .filter_map(|modification| {
            let source = modification.cover_thumb_url.as_ref()?;
            let cached = previous.get(&modification.id);
            let current = cached.is_some_and(|(version, object_key)| {
                *version == Some(i16::try_from(modification.version).unwrap_or_default())
                    && object_key.is_some()
            });
            (!current).then(|| {
                (
                    modification.id.clone(),
                    modification.version,
                    source.clone(),
                )
            })
        })
        .collect::<Vec<_>>();

    let cached = stream::iter(pending)
        .map(|(id, version, source)| {
            let database = database.clone();
            let object_store = Arc::clone(&object_store);
            let client = client.clone();
            async move {
                let version = i16::try_from(version)?;
                let cover = client
                    .vehicle_mod_cover(&source)
                    .await
                    .map_err(anyhow::Error::from)?;
                let digest = hex::encode(Sha256::digest(&cover.bytes));
                let object_key = VehicleEntity::store(
                    object_store.as_ref(),
                    &format!("{id}/{digest}"),
                    PutPayload::from(cover.bytes),
                )
                .await?;
                let updated = VehicleEntity::update_many()
                    .filter(VehicleColumn::Id.eq(&id))
                    // A slower sync must not replace an image for a newer revision.
                    .filter(VehicleColumn::Version.eq(version))
                    .col_expr(VehicleColumn::ImageObjectKey, Expr::value(object_key))
                    .col_expr(VehicleColumn::ImageVersion, Expr::value(version))
                    .col_expr(
                        VehicleColumn::ImageContentType,
                        Expr::value(cover.content_type),
                    )
                    .col_expr(
                        VehicleColumn::ImageFetchedAt,
                        Expr::value(time::OffsetDateTime::now_utc()),
                    )
                    .exec(&database)
                    .await?;
                Ok::<bool, anyhow::Error>(updated.rows_affected == 1)
            }
        })
        .buffer_unordered(IMAGE_DOWNLOAD_CONCURRENCY)
        .fold(0usize, |cached, result| async move {
            match result {
                Ok(updated) => cached + usize::from(updated),
                Err(error) => {
                    tracing::warn!(?error, "Vehicle Mod cover download failed");
                    cached
                }
            }
        })
        .await;
    Ok(cached)
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
    let id = &candidate.id;
    let invalid = || sea_orm::DbErr::Type(format!("invalid mod ID stored in catalogue: {id}"));
    let vehicle = id
        .parse::<Vehicle>()
        .expect("insim_core vehicle parsing is infallible")
        .ensure_hotlap_rankable()
        .map_err(|_| invalid())?;
    if !vehicle.is_mod() {
        return Err(invalid());
    }
    Ok(Some(vehicle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, version: Option<i16>) -> ModCandidate {
        ModCandidate {
            id: id.to_owned(),
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
