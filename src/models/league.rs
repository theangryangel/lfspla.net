//! Organising groups: the future boundary for event ownership.

use super::event::source::{Change, Source};
use sea_orm::{Set, entity::prelude::*};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "league")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub forum_url: Option<String>,
    pub image_attachment_id: Option<i64>,
    pub source: Source,
    pub external_id: Option<String>,
    pub source_url: Option<String>,
    pub last_seen_at: Option<TimeDateTimeWithTimeZone>,
    pub last_synced_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
}
impl ActiveModelBehavior for ActiveModel {}

/// Validated raster bytes obtained from the league logo endpoint.
pub(crate) struct Logo {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
    pub media_type: &'static str,
}

impl Model {
    /// Reuse identical bytes. Without storage, report a proposed change only.
    pub(crate) async fn cache_logo(
        database: &sea_orm::DatabaseConnection,
        league_id: i64,
        logo: &Logo,
        store: Option<&dyn object_store::ObjectStore>,
        now: TimeDateTimeWithTimeZone,
    ) -> anyhow::Result<bool> {
        use anyhow::Context;
        use sea_orm::{QuerySelect, TransactionTrait};
        use sha2::{Digest, Sha256};
        let transaction = database.begin().await?;
        let old = Entity::find_by_id(league_id)
            .lock_exclusive()
            .one(&transaction)
            .await?
            .context("league missing during logo refresh")?;
        let digest = hex::encode(Sha256::digest(&logo.bytes));
        if let Some(id) = old.image_attachment_id {
            let cached = super::attachment::Model::required(&transaction, id).await?;
            if cached.sha256.as_deref() == Some(digest.as_str())
                && cached.media_type == logo.media_type
            {
                transaction.rollback().await?;
                return Ok(false);
            }
        }
        let Some(store) = store else {
            transaction.rollback().await?;
            return Ok(true);
        };
        let attachment = super::attachment::Model::store(
            &transaction,
            store,
            logo.bytes.clone(),
            logo.extension,
            logo.media_type,
            None,
        )
        .await?;
        let mut active: ActiveModel = old.into();
        active.image_attachment_id = Set(Some(attachment.id));
        active.updated_at = Set(now);
        active.update(&transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }

    /// Revisit imported organisers even after they disappear from the active directory.
    pub(crate) async fn known_with_future_events(
        database: &sea_orm::DatabaseConnection,
        now: TimeDateTimeWithTimeZone,
    ) -> Result<Vec<u64>, DbErr> {
        #[derive(sea_orm::FromQueryResult)]
        struct Key {
            external_id: String,
        }
        use sea_orm::{DbBackend, FromQueryResult, Statement};
        Key::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT external_id FROM league WHERE source = 'lfs_net' AND EXISTS (
                SELECT 1 FROM event WHERE event.league_id = league.id AND event.starts_at >= $1)",
            [now.into()],
        ))
        .all(database)
        .await?
        .into_iter()
        .map(|row| {
            row.external_id
                .parse()
                .map_err(|_| DbErr::Custom("invalid LFS league source ID".into()))
        })
        .collect()
    }

    pub(crate) async fn import(
        database: &impl ConnectionTrait,
        key: &str,
        name: &str,
        url: &str,
        now: TimeDateTimeWithTimeZone,
    ) -> Result<(Self, Change), DbErr> {
        use sea_orm::QueryFilter;
        let existing = Entity::find()
            .filter(Column::Source.eq(Source::LfsNet))
            .filter(Column::ExternalId.eq(key))
            .one(database)
            .await?;
        let change = match &existing {
            None => Change::Created,
            Some(old) if old.name == name => Change::Unchanged,
            Some(_) => Change::Updated,
        };
        let mut active = existing.map_or_else(
            || ActiveModel {
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            },
            Into::into,
        );
        active.name = Set(name.to_owned());
        active.source = Set(Source::LfsNet);
        active.external_id = Set(Some(key.to_owned()));
        active.source_url = Set(Some(url.to_owned()));
        active.last_seen_at = Set(Some(now));
        active.last_synced_at = Set(Some(now));
        if !matches!(change, Change::Unchanged) {
            active.updated_at = Set(now);
        }
        let row = match change {
            Change::Created => active.insert(database).await?,
            _ => active.update(database).await?,
        };
        Ok((row, change))
    }
}
