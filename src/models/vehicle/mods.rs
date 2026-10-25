//! Synchronization and resolution of mutable Vehicle Mods catalogue metadata.

use crate::models::vehicle::{
    ActiveModel as VehicleMutation, Column as VehicleColumn, Entity as VehicleEntity,
};
use crate::vehicle_id::VehicleId;
use insim_core::vehicle::Vehicle;
use lfsplanet_lfs_api::RemoteVehicleMod;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, ExprTrait, QueryFilter,
    TransactionTrait,
    sea_query::{Expr, Func, OnConflict},
};
use std::collections::{BTreeMap, HashMap};
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
        let author_ids = resolve_authors(&transaction, remote).await?;
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
                        class: Set(entry.class),
                        author_username: Set(entry
                            .author_username
                            .as_deref()
                            .map(str::trim)
                            .filter(|username| !username.is_empty())
                            .map(str::to_owned)),
                        author_player_id: Set(entry
                            .author_username
                            .as_deref()
                            .and_then(author_key)
                            .and_then(|key| author_ids.get(&key).copied())),
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
                            VehicleColumn::AuthorPlayerId,
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

/// Reuse player identities and create ordinary profiles for newly observed authors.
/// Conflict handling deliberately leaves existing profile fields untouched.
async fn resolve_authors(
    transaction: &DatabaseTransaction,
    remote: &[RemoteVehicleMod],
) -> Result<HashMap<String, i64>, sea_orm::DbErr> {
    use crate::models::player::{ActiveModel, Column, Entity};
    let mut usernames = BTreeMap::new();
    for username in remote
        .iter()
        .filter_map(|entry| entry.author_username.as_deref())
    {
        if let Some(key) = author_key(username) {
            usernames
                .entry(key)
                .or_insert_with(|| username.trim().to_owned());
        }
    }
    let mut ids = HashMap::new();
    let usernames = usernames.into_values().collect::<Vec<_>>();
    for chunk in usernames.chunks(INSERT_CHUNK_SIZE) {
        Entity::insert_many(chunk.iter().map(|username| ActiveModel {
            lfs_username: Set(username.clone()),
            display_name: Set(username.clone()),
            ..Default::default()
        }))
        .on_conflict(
            OnConflict::new()
                .expr(Func::lower(Expr::col(Column::LfsUsername)))
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(transaction)
        .await?;
        let players = Entity::find()
            .filter(
                Expr::expr(Func::lower(Expr::col(Column::LfsUsername)))
                    .is_in(chunk.iter().map(|username| username.to_ascii_lowercase())),
            )
            .all(transaction)
            .await?;
        ids.extend(
            players
                .into_iter()
                .map(|player| (player.lfs_username.to_ascii_lowercase(), player.id)),
        );
    }
    Ok(ids)
}

fn author_key(username: &str) -> Option<String> {
    let username = username.trim();
    (!username.is_empty()).then(|| username.to_ascii_lowercase())
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

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn sync_links_and_creates_ordinary_author_profiles(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        use crate::models::player::{ActiveModel as PlayerMutation, Entity as PlayerEntity};
        use sea_orm::{ActiveModelTrait, PaginatorTrait};
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let existing = PlayerMutation {
            lfs_username: Set("KnownAuthor".to_owned()),
            display_name: Set("Chosen display name".to_owned()),
            country_code: Set(Some(celes::Country::germany().into())),
            deny_auth: Set(true),
            deny_uploads: Set(true),
            last_authenticated_at: Set(Some(time::OffsetDateTime::UNIX_EPOCH)),
            ..Default::default()
        }
        .insert(&database)
        .await?;
        let entry = |id: &str, author: Option<&str>| RemoteVehicleMod {
            id: id.to_owned(),
            name: id.to_owned(),
            version: 1,
            class: None,
            author_username: author.map(str::to_owned),
            work_in_progress: false,
            is_private: false,
            is_tweak_mod: false,
            published_at: None,
            cover_thumb_url: None,
            metadata: serde_json::json!({}),
        };
        let mut remote = vec![
            entry("AAAA01", Some(" knownAUTHOR ")),
            entry("AAAA02", Some("NewAuthor")),
            entry("AAAA03", Some("newauthor")),
            entry("AAAA04", None),
            entry("AAAA05", Some("   ")),
        ];
        crate::models::Vehicle::refresh(&database, &remote).await?;
        crate::models::Vehicle::refresh(&database, &remote).await?;
        assert_eq!(PlayerEntity::find().count(&database).await?, 2);
        assert_eq!(
            PlayerEntity::find_by_id(existing.id)
                .one(&database)
                .await?
                .unwrap(),
            existing
        );
        let vehicles = VehicleEntity::find().all(&database).await?;
        let linked = |id: &str| {
            vehicles
                .iter()
                .find(|vehicle| vehicle.id.to_string() == id)
                .unwrap()
                .author_player_id
        };
        assert_eq!(linked("AAAA01"), Some(existing.id));
        let author_id = linked("AAAA02").unwrap();
        assert_eq!(linked("AAAA03"), Some(author_id));
        assert_eq!(linked("AAAA04"), None);
        assert_eq!(linked("AAAA05"), None);
        let author = PlayerEntity::find_by_id(author_id)
            .one(&database)
            .await?
            .unwrap();
        assert_eq!(author.display_name, "NewAuthor");
        assert!(!author.deny_auth);
        assert!(!author.deny_uploads);
        let profile = author.profile(&database).await?;
        assert_eq!(profile.stats.hotlaps, 0);
        let authenticated = crate::models::Player::record_authentication(
            &database,
            &lfsplanet_lfs_api::LfsAccount {
                lfsworld_id: 12345,
                username: "NEWAUTHOR".to_owned(),
                display_name: "Author's chosen name".to_owned(),
            },
        )
        .await?;
        assert_eq!(authenticated.id, author_id);
        assert_eq!(authenticated.display_name, "Author's chosen name");
        crate::models::Vehicle::refresh(&database, &remote).await?;
        assert_eq!(
            PlayerEntity::find_by_id(author_id)
                .one(&database)
                .await?
                .unwrap(),
            authenticated
        );
        remote[1].author_username = Some("KnownAuthor".to_owned());
        remote[2].author_username = None;
        crate::models::Vehicle::refresh(&database, &remote).await?;
        assert_eq!(
            VehicleEntity::find_by_id("AAAA02".parse::<VehicleId>()?)
                .one(&database)
                .await?
                .unwrap()
                .author_player_id,
            Some(existing.id)
        );
        assert_eq!(
            VehicleEntity::find_by_id("AAAA03".parse::<VehicleId>()?)
                .one(&database)
                .await?
                .unwrap()
                .author_player_id,
            None
        );
        Ok(())
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn migration_backfills_cached_author_profiles(pool: sqlx::PgPool) -> anyhow::Result<()> {
        // Recreate the pre-migration state in this isolated test database.
        sqlx::query("ALTER TABLE vehicle DROP COLUMN author_player_id")
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO player (lfs_username, display_name) VALUES ('KnownAuthor', 'Chosen name')",
        )
        .execute(&pool)
        .await?;
        sqlx::raw_sql(
            "INSERT INTO vehicle (id, kind, name, normalized_name, license, author_username) VALUES
            ('AAAA01', 'mod', 'One', 'one', 's3', 'knownauthor'),
            ('AAAA02', 'mod', 'Two', 'two', 's3', ' NewAuthor '),
            ('AAAA03', 'mod', 'Three', 'three', 's3', 'newauthor'),
            ('AAAA04', 'mod', 'Four', 'four', 's3', NULL),
            ('AAAA05', 'mod', 'Five', 'five', 's3', '   '),
            ('XFG', 'standard', 'XF GTI', 'xf gti', 'demo', NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::raw_sql(include_str!(
            "../../../migrations/20261025120000_vehicle_authors.sql"
        ))
        .execute(&pool)
        .await?;
        let players: i64 = sqlx::query_scalar("SELECT count(*) FROM player")
            .fetch_one(&pool)
            .await?;
        assert_eq!(players, 2);
        let linked: i64 =
            sqlx::query_scalar("SELECT count(*) FROM vehicle WHERE author_player_id IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(linked, 3);
        let preserved: String = sqlx::query_scalar(
            "SELECT display_name FROM player WHERE lfs_username = 'KnownAuthor'",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(preserved, "Chosen name");
        // Deleting a player leaves their vehicle records intact and unlinked.
        sqlx::query("DELETE FROM player WHERE lower(lfs_username) = 'newauthor'")
            .execute(&pool)
            .await?;
        let linked: i64 =
            sqlx::query_scalar("SELECT count(*) FROM vehicle WHERE author_player_id IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(linked, 1);
        let vehicles: i64 = sqlx::query_scalar("SELECT count(*) FROM vehicle")
            .fetch_one(&pool)
            .await?;
        assert_eq!(vehicles, 6);
        Ok(())
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
