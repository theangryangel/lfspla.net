//! Catalogue-backed badge awards. Automatic rebuilds never alter manual grants.

use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue::Set, DatabaseTransaction, FromQueryResult, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

pub(crate) mod definition;
mod refresh;
pub(crate) mod variant;
pub(crate) use refresh::{
    qualifies_for_ranking_badge, rebuild_published_badges, request_badge_refresh,
    retry_badge_refreshes,
};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "badge_award")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub era_id: Option<i64>,
    pub player_id: i64,
    pub badge_id: String,
    pub variant: String,
    pub origin: String,
    pub source: String,
    pub detail: Option<String>,
    pub created_at: time::OffsetDateTime,
    #[sea_orm(
        belongs_to,
        from = "player_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub player: BelongsTo<crate::models::player::Entity>,
    #[sea_orm(
        belongs_to,
        from = "era_id",
        to = "id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    pub era: HasOne<crate::models::era::Entity>,
}
impl ActiveModelBehavior for ActiveModel {}

/// Uniform, resolved presentation; clients do not interpret award categories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct PlayerBadge {
    pub id: i64,
    pub badge_id: String,
    pub variant: String,
    pub name: String,
    pub description: String,
    #[schema(required)]
    pub image: Option<String>,
}

/// One calculated award. The source identifies the Rust calculation that owns it.
#[derive(Debug)]
pub(crate) struct CalculatedAward {
    pub badge_id: String,
    pub variant: String,
    pub source: String,
    pub detail: Option<String>,
}

impl CalculatedAward {
    pub(crate) fn new(
        badge_id: impl Into<String>,
        variant: &str,
        source: impl Into<String>,
        detail: impl Into<Option<String>>,
    ) -> Self {
        Self {
            badge_id: badge_id.into(),
            variant: variant.to_owned(),
            source: source.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, FromQueryResult)]
pub(crate) struct ResolvedAward {
    pub player_id: i64,
    pub era_id: Option<i64>,
    pub id: i64,
    pub badge_id: String,
    pub variant: String,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
}
impl ResolvedAward {
    pub(crate) fn presentation(self) -> PlayerBadge {
        PlayerBadge {
            id: self.id,
            badge_id: self.badge_id,
            variant: self.variant,
            name: self.name,
            description: self.description,
            image: self.image,
        }
    }
}

impl Model {
    /// Resolve definitions at read time, so reseeding presentation needs no rebadge.
    pub(crate) async fn resolved(
        database: &impl ConnectionTrait,
        era_id: Option<i64>,
        player_ids: Vec<i64>,
        all_eras: bool,
    ) -> Result<Vec<ResolvedAward>, DbErr> {
        if player_ids.is_empty() {
            return Ok(Vec::new());
        }
        ResolvedAward::find_by_statement(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            r"SELECT a.player_id, a.era_id, a.id, a.badge_id, a.variant, b.name,
                COALESCE(a.detail, b.description) AS description,
                CASE WHEN f.public_id IS NOT NULL THEN '/api/v1/attachments/' || f.public_id::text END AS image
              FROM badge_award a JOIN badge b ON b.id = a.badge_id
              JOIN badge_variant v ON v.badge_id = a.badge_id AND v.id = a.variant
              LEFT JOIN attachment f ON f.id = v.image_attachment_id
              WHERE a.player_id = ANY($1::bigint[])
                AND ($3 OR a.era_id = $2 OR a.era_id IS NULL)
              ORDER BY b.sequence, b.id, a.era_id NULLS FIRST, a.source, a.id",
            [player_ids.into(), era_id.into(), all_eras.into()],
        )).all(database).await
    }

    /// Era responses include lifetime grants as well as that era's awards.
    pub(crate) async fn list_for_players(
        database: &impl ConnectionTrait,
        era_id: i64,
        player_ids: impl IntoIterator<Item = i64>,
    ) -> Result<HashMap<i64, Vec<PlayerBadge>>, DbErr> {
        let mut result = HashMap::<_, Vec<_>>::new();
        for award in Self::resolved(
            database,
            Some(era_id),
            player_ids.into_iter().collect(),
            false,
        )
        .await?
        {
            result
                .entry(award.player_id)
                .or_default()
                .push(award.presentation());
        }
        Ok(result)
    }

    /// Replace only the computed projection, under the existing catalogue lock.
    /// Missing definitions/variants fail atomically via the foreign key.
    pub(crate) async fn replace_era(
        transaction: &DatabaseTransaction,
        era_id: i64,
        badges: HashMap<i64, Vec<CalculatedAward>>,
    ) -> Result<usize, DbErr> {
        Entity::delete_many()
            .filter(Column::EraId.eq(era_id))
            .filter(Column::Origin.eq("automatic"))
            .exec(transaction)
            .await?;
        let count = badges.len();
        let models: Vec<_> = badges
            .into_iter()
            .flat_map(|(player_id, awards)| {
                awards.into_iter().map(move |award| ActiveModel {
                    era_id: Set(Some(era_id)),
                    player_id: Set(player_id),
                    badge_id: Set(award.badge_id),
                    variant: Set(award.variant),
                    origin: Set("automatic".to_owned()),
                    source: Set(award.source),
                    detail: Set(award.detail),
                    created_at: Set(time::OffsetDateTime::now_utc()),
                    ..Default::default()
                })
            })
            .collect();
        // Bound statement parameters when rebuilding large historical eras.
        for batch in models.chunks(1_000) {
            Entity::insert_many(batch.iter().cloned())
                .exec(transaction)
                .await?;
        }
        Ok(count)
    }

    pub(crate) async fn list_manual(
        database: &impl ConnectionTrait,
        player_id: i64,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::PlayerId.eq(player_id))
            .filter(Column::Origin.eq("manual"))
            .order_by_asc(Column::BadgeId)
            .all(database)
            .await
    }
}

#[cfg(test)]
mod tests;

/// Caller holds the catalogue write lock. Repeated grants update one lifetime award.
pub(crate) async fn grant_manual(
    database: &impl ConnectionTrait,
    player_id: i64,
    badge_id: &str,
    variant: &str,
    detail: Option<&str>,
) -> Result<(), DbErr> {
    database
        .execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "INSERT INTO badge_award (player_id, badge_id, variant, origin, source, detail)
         VALUES ($1, $2, $3, 'manual', 'operator', $4)
         ON CONFLICT (player_id, era_id, badge_id, origin, source)
         DO UPDATE SET variant = EXCLUDED.variant, detail = EXCLUDED.detail",
            [
                player_id.into(),
                badge_id.into(),
                variant.into(),
                detail.map(str::to_owned).into(),
            ],
        ))
        .await?;
    Ok(())
}

pub(crate) async fn revoke_manual(
    database: &impl ConnectionTrait,
    player_id: i64,
    badge_id: &str,
) -> Result<u64, DbErr> {
    Ok(Entity::delete_many()
        .filter(Column::PlayerId.eq(player_id))
        .filter(Column::BadgeId.eq(badge_id))
        .filter(Column::Origin.eq("manual"))
        .filter(Column::EraId.is_null())
        .exec(database)
        .await?
        .rows_affected)
}
