//! Public event reads and reports of listings missing from a successful sync.

use super::{Column, Entity, Model, source::Source};
use crate::models::{league, season};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult, JoinType, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Select,
};
use time::OffsetDateTime;

#[derive(Debug, FromQueryResult)]
pub(crate) struct EventListEntry {
    pub id: i64,
    pub league_id: i64,
    pub league_name: String,
    pub league_image_attachment_id: Option<i64>,
    pub season_id: Option<i64>,
    pub season_name: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: OffsetDateTime,
    pub details_url: Option<String>,
    pub signup_url: Option<String>,
    pub track_label: Option<String>,
    pub cars_label: Option<String>,
}

pub(crate) struct EventListFilters {
    pub starts_from: OffsetDateTime,
    pub starts_before: Option<OffsetDateTime>,
    pub league_id: Option<i64>,
    pub season_id: Option<i64>,
}

impl EventListFilters {
    pub(crate) fn select(&self) -> Select<Entity> {
        let mut events = Entity::find()
            .filter(Column::Hidden.eq(false))
            .filter(Column::StartsAt.gte(self.starts_from));
        if let Some(before) = self.starts_before {
            events = events.filter(Column::StartsAt.lt(before));
        }
        if let Some(league_id) = self.league_id {
            events = events.filter(Column::LeagueId.eq(league_id));
        }
        if let Some(season_id) = self.season_id {
            events = events.filter(Column::SeasonId.eq(season_id));
        }
        events
    }
}

impl Model {
    pub(crate) async fn list(
        database: &impl ConnectionTrait,
        filters: &EventListFilters,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<super::EventListEntry>, u64), DbErr> {
        let events = filters.select();
        let total = events.clone().count(database).await?;
        let items = events
            .join(
                JoinType::InnerJoin,
                Entity::belongs_to(league::Entity)
                    .from(Column::LeagueId)
                    .to(league::Column::Id)
                    .into(),
            )
            .join(
                JoinType::LeftJoin,
                Entity::belongs_to(season::Entity)
                    .from(Column::SeasonId)
                    .to(season::Column::Id)
                    .into(),
            )
            .column_as(league::Column::Name, "league_name")
            .column_as(
                league::Column::ImageAttachmentId,
                "league_image_attachment_id",
            )
            .column_as(season::Column::Name, "season_name")
            .order_by_asc(Column::StartsAt)
            .order_by_asc(Column::Id)
            .offset(offset)
            .limit(limit)
            .into_model::<EventListEntry>()
            .all(database)
            .await?;
        Ok((items, total))
    }

    pub(crate) async fn stale(
        database: &impl ConnectionTrait,
        league_id: i64,
        now: OffsetDateTime,
    ) -> Result<Vec<i64>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::LeagueId.eq(league_id))
            .filter(Column::StartsAt.gte(now))
            .filter(Column::Source.eq(Source::LfsNet))
            .filter(Column::LastSyncedAt.lt(now))
            .order_by_asc(Column::Id)
            .into_tuple::<i64>()
            .all(database)
            .await
    }
}
