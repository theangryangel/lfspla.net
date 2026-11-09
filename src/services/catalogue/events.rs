//! Catalogue-owned competitions and immutable layout attachments.
use crate::models::event::{game::GameConfig, source::Source};
use crate::models::{attachment, event, league, season};
use anyhow::{Context, ensure};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventDefinition {
    pub id: String,
    pub league: String,
    pub season: Option<String>,
    pub title: String,
    pub description: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub starts_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub ends_at: OffsetDateTime,
    pub game: GameConfig,
    pub layout: PathBuf,
    pub join_url: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}
impl EventDefinition {
    fn layout_bytes(&self, source: &Path) -> anyhow::Result<Vec<u8>> {
        let path = source.parent().unwrap_or(Path::new(".")).join(&self.layout);
        ensure!(
            path.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("lyt")),
            "layout must be a .lyt file"
        );
        let bytes =
            std::fs::read(&path).with_context(|| format!("reading layout {}", path.display()))?;
        ensure!(bytes.len() <= 2 * 1024 * 1024, "layout exceeds 2 MiB");
        let layout = insim_lyt::Lyt::read(bytes.as_slice()).context("invalid LYT file")?;
        ensure!(
            !layout.objects.is_empty(),
            "game layout must contain objects"
        );
        Ok(bytes)
    }
    pub(crate) fn validate(&self, source: &Path) -> anyhow::Result<()> {
        ensure!(
            !self.id.is_empty()
                && self
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
            "invalid event catalogue ID"
        );
        ensure!(
            !self.title.trim().is_empty() && !self.league.trim().is_empty(),
            "event title and league are required"
        );
        ensure!(
            self.season.as_ref().is_none_or(|s| !s.trim().is_empty()),
            "season must not be empty"
        );
        ensure!(self.ends_at > self.starts_at, "event end must follow start");
        if let Some(url) = &self.join_url {
            let parsed = url::Url::parse(url)?;
            ensure!(
                matches!(parsed.scheme(), "https" | "lfs"),
                "join URL must use https or lfs"
            );
        }
        self.game.validate()?;
        self.layout_bytes(source)?;
        Ok(())
    }
    pub(crate) async fn apply(
        &self,
        db: &DatabaseTransaction,
        store: Option<&dyn object_store::ObjectStore>,
        dry_run: bool,
        source: &Path,
    ) -> anyhow::Result<bool> {
        // PostgreSQL timestamps have microsecond precision. Normalize before
        // persistence and comparison so repeated sub-microsecond YAML times agree.
        let mut normalized = self.clone();
        normalized.starts_at = self
            .starts_at
            .replace_nanosecond(self.starts_at.nanosecond() / 1000 * 1000)?;
        normalized.ends_at = self
            .ends_at
            .replace_nanosecond(self.ends_at.nanosecond() / 1000 * 1000)?;
        normalized
            .apply_normalized(db, store, dry_run, source)
            .await
    }

    async fn apply_normalized(
        &self,
        db: &DatabaseTransaction,
        store: Option<&dyn object_store::ObjectStore>,
        dry_run: bool,
        source: &Path,
    ) -> anyhow::Result<bool> {
        self.validate(source)?;
        let bytes = self.layout_bytes(source)?;
        let digest = hex::encode(Sha256::digest(&bytes));
        let config = serde_json::to_value(&self.game)?;
        let old = event::Entity::find()
            .filter(event::Column::CatalogueId.eq(&self.id))
            .one(db)
            .await?;
        let old_attachment = if let Some(id) = old.as_ref().and_then(|e| e.layout_attachment_id) {
            Some(attachment::Model::required(db, id).await?)
        } else {
            None
        };
        let layout_same = old_attachment.as_ref().and_then(|a| a.sha256.as_ref()) == Some(&digest);
        if let Some(old) = &old {
            let executed = db
                .query_one_raw(sea_orm::Statement::from_sql_and_values(
                    sea_orm::DbBackend::Postgres,
                    "SELECT event_id FROM game_event_execution WHERE event_id=$1",
                    [old.id.into()],
                ))
                .await?
                .is_some();
            if old.starts_at <= OffsetDateTime::now_utc() || executed {
                ensure!(
                    old.starts_at == self.starts_at
                        && old.ends_at == Some(self.ends_at)
                        && old.game_config.as_ref() == Some(&config)
                        && layout_same
                        && old.hidden == self.hidden,
                    "event {} has started; schedule, gameplay, layout and visibility are immutable",
                    self.id
                );
            }
        }
        let league = match league::Entity::find()
            .filter(league::Column::Source.eq(Source::Local))
            .filter(league::Column::Name.eq(&self.league))
            .one(db)
            .await?
        {
            Some(row) => row,
            None => {
                league::ActiveModel {
                    name: Set(self.league.clone()),
                    ..Default::default()
                }
                .insert(db)
                .await?
            }
        };
        let season_id = if let Some(name) = &self.season {
            let row = season::Entity::find()
                .filter(season::Column::LeagueId.eq(league.id))
                .filter(season::Column::Source.eq(Source::Local))
                .filter(season::Column::Name.eq(name))
                .one(db)
                .await?;
            Some(match row {
                Some(row) => row.id,
                None => {
                    season::ActiveModel {
                        league_id: Set(league.id),
                        name: Set(name.clone()),
                        ..Default::default()
                    }
                    .insert(db)
                    .await?
                    .id
                }
            })
        } else {
            None
        };
        let attachment_id = if layout_same {
            old_attachment.context("missing current layout")?.id
        } else {
            let memory = object_store::memory::InMemory::new();
            let store = if dry_run {
                &memory as &dyn object_store::ObjectStore
            } else {
                store.context("object storage required for game layouts")?
            };
            attachment::Model::store(
                db,
                store,
                bytes,
                "lyt",
                "application/octet-stream",
                self.layout.file_name().and_then(|n| n.to_str()),
            )
            .await?
            .id
        };
        let cars = self
            .game
            .vehicles
            .iter()
            .map(|v| v.vehicle.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(old) = &old
            && old.league_id == league.id
            && old.season_id == season_id
            && old.title == self.title
            && old.description == self.description
            && old.starts_at == self.starts_at
            && old.ends_at == Some(self.ends_at)
            && old.game_config.as_ref() == Some(&config)
            && layout_same
            && old.signup_url == self.join_url
            && old.hidden == self.hidden
        {
            return Ok(false);
        }
        let now = OffsetDateTime::now_utc();
        let mut active = old.map_or_else(<event::ActiveModel as Default>::default, Into::into);
        active.catalogue_id = Set(Some(self.id.clone()));
        active.league_id = Set(league.id);
        active.season_id = Set(season_id);
        active.title = Set(self.title.clone());
        active.description = Set(self.description.clone());
        active.starts_at = Set(self.starts_at);
        active.ends_at = Set(Some(self.ends_at));
        active.source = Set(self.game.source.kind());
        active.game_config = Set(Some(config));
        active.layout_attachment_id = Set(Some(attachment_id));
        active.signup_url = Set(self.join_url.clone());
        active.track_label = Set(Some(self.game.track.clone()));
        active.cars_label = Set(Some(if cars.is_empty() {
            "Unrestricted".into()
        } else {
            cars
        }));
        active.hidden = Set(self.hidden);
        active.updated_at = Set(now);
        active.save(db).await?;
        Ok(true)
    }
}

pub(crate) async fn validate_schedule(db: &DatabaseTransaction) -> anyhow::Result<()> {
    let overlap = db.query_one_raw(sea_orm::Statement::from_string(sea_orm::DbBackend::Postgres,
        "SELECT a.catalogue_id AS first, b.catalogue_id AS second FROM event a JOIN event b ON a.id < b.id AND tstzrange(a.starts_at,a.ends_at,'[)') && tstzrange(b.starts_at,b.ends_at,'[)') WHERE a.game_config IS NOT NULL AND b.game_config IS NOT NULL AND NOT a.hidden AND NOT b.hidden LIMIT 1")).await?;
    if let Some(row) = overlap {
        anyhow::bail!(
            "game events {} and {} overlap",
            row.try_get::<String>("", "first")?,
            row.try_get::<String>("", "second")?
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::catalogue::{self, CatalogueDefinition, load::SourceDefinition};
    use sea_orm::SqlxPostgresConnector;
    use std::sync::Arc;
    fn layout(path: &Path) {
        // One raw layout object, LYT 0 / mini revision 9.
        std::fs::write(
            path,
            [
                b'L', b'F', b'S', b'L', b'Y', b'T', 0, 0, 1, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        )
        .unwrap();
    }
    fn definition() -> EventDefinition {
        EventDefinition {
            id: "test-shortcut".into(),
            league: "LFS Planet".into(),
            season: Some("Specials".into()),
            title: "Shortcut".into(),
            description: None,
            starts_at: OffsetDateTime::now_utc() + time::Duration::days(1),
            ends_at: OffsetDateTime::now_utc() + time::Duration::days(2),
            game: GameConfig {
                source: crate::models::event::game::GameSource::Shortcut,
                track: "BL1".into(),
                vehicles: Vec::new(),
            },
            layout: "test.lyt".into(),
            join_url: None,
            hidden: false,
        }
    }
    #[test]
    fn event_definitions_reject_invalid_assets_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("event.yaml");
        let mut event = definition();
        assert!(event.validate(&source).is_err());
        layout(&dir.path().join("test.lyt"));
        event.validate(&source).unwrap();
        event.game.source = crate::models::event::game::GameSource::Metronome { target_ms: 0 };
        assert!(event.validate(&source).is_err());
        event.game.source = crate::models::event::game::GameSource::Shortcut;
        event.ends_at = event.starts_at;
        assert!(event.validate(&source).is_err());
    }
    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn game_catalogue_is_repeatable_atomic_and_freezes_started_events(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let store = Arc::new(object_store::memory::InMemory::new());
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("event.yaml");
        layout(&dir.path().join("test.lyt"));
        let mut event = definition();
        let docs = |event: &EventDefinition| {
            vec![SourceDefinition {
                source: source.clone(),
                definition: CatalogueDefinition::Event(event.clone()),
            }]
        };
        catalogue::seed(&db, &docs(&event), None, true, |_| Ok(())).await?;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event")
                .fetch_one(&pool)
                .await?,
            0
        );
        catalogue::seed(&db, &docs(&event), Some(&*store), false, |_| Ok(())).await?;
        let (id, attachment): (i64, i64) = sqlx::query_as(
            "SELECT id,layout_attachment_id FROM event WHERE catalogue_id='test-shortcut'",
        )
        .fetch_one(&pool)
        .await?;
        let again = catalogue::seed(&db, &docs(&event), Some(&*store), false, |_| Ok(())).await?;
        assert!(again.events.is_empty());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachment")
                .fetch_one(&pool)
                .await?,
            1
        );
        let mut other = event.clone();
        other.id = "overlap".into();
        assert!(
            catalogue::seed(&db, &docs(&other), Some(&*store), false, |_| Ok(()))
                .await
                .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event")
                .fetch_one(&pool)
                .await?,
            1
        );
        event.starts_at = OffsetDateTime::now_utc() - time::Duration::hours(1);
        event.ends_at = OffsetDateTime::now_utc() + time::Duration::hours(1);
        catalogue::seed(&db, &docs(&event), Some(&*store), false, |_| Ok(())).await?;
        event.title = "Updated description of event".into();
        catalogue::seed(&db, &docs(&event), Some(&*store), false, |_| Ok(())).await?;
        event.game.track = "BL2".into();
        assert!(
            catalogue::seed(&db, &docs(&event), Some(&*store), false, |_| Ok(()))
                .await
                .is_err()
        );
        let row = event::Entity::find_by_id(id).one(&db).await?.unwrap();
        assert_eq!(row.layout_attachment_id, Some(attachment));
        assert_eq!(row.title, "Updated description of event");
        assert_eq!(row.game()?.unwrap().track, "BL1");
        Ok(())
    }
}
