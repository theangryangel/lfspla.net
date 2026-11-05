//! Public threaded conversation; a reason marks an entry as a report.
use super::report_reason::ReportReason;
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, ExprTrait, QueryFilter, QueryOrder, Select, UpdateMany};

mod reports;
mod thread;
pub(crate) use thread::CommentThread;
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "hotlap_comment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub hotlap_id: Option<i64>,
    pub original_hotlap_id: i64,
    pub parent_id: Option<i64>,
    pub root_id: Option<i64>,
    pub depth: i16,
    pub author_id: i64,
    pub body: Option<String>,
    pub reason: Option<ReportReason>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub deleted_at: Option<TimeDateTimeWithTimeZone>,
    pub resolved_at: Option<TimeDateTimeWithTimeZone>,
    pub resolved_by: Option<i64>,
    #[sea_orm(belongs_to, from = "author_id", to = "id")]
    pub author: BelongsTo<crate::models::player::Entity>,
}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub(crate) fn is_open_report(&self) -> bool {
        self.reason.is_some() && self.resolved_at.is_none() && self.deleted_at.is_none()
    }

    /// Shared by root ordering and the around-comment position lookup.
    pub(crate) fn open_report_priority() -> sea_orm::sea_query::SimpleExpr {
        sea_orm::sea_query::Expr::col((Entity, Column::Reason))
            .is_not_null()
            .and(sea_orm::sea_query::Expr::col((Entity, Column::ResolvedAt)).is_null())
            .and(sea_orm::sea_query::Expr::col((Entity, Column::DeletedAt)).is_null())
    }

    pub(crate) fn ordered_roots(hotlap_id: i64) -> Select<Entity> {
        Entity::find()
            .on_hotlap(hotlap_id)
            .filter(Column::ParentId.is_null())
            .order_by_desc(Self::open_report_priority())
    }

    pub(crate) async fn report_exists(
        database: &impl ConnectionTrait,
        hotlap_id: i64,
        author_id: i64,
    ) -> Result<bool, DbErr> {
        Ok(Entity::find()
            .filter(Column::HotlapId.eq(hotlap_id))
            .by_author(author_id)
            .reported(true)
            .one(database)
            .await?
            .is_some())
    }

    /// Remove content while preserving the conversation and its replies.
    pub(crate) async fn remove_owned(
        database: &impl ConnectionTrait,
        hotlap_id: i64,
        id: i64,
        author_id: i64,
    ) -> Result<(), DbErr> {
        use sea_orm::sea_query::Expr;
        Entity::update_many()
            .filter(Column::Id.eq(id))
            .on_hotlap(hotlap_id)
            .by_author(author_id)
            .not_deleted()
            .col_expr(Column::Body, Expr::value(Option::<String>::None))
            .col_expr(Column::Reason, Expr::value(Option::<String>::None))
            .col_expr(
                Column::ResolvedAt,
                Expr::value(Option::<time::OffsetDateTime>::None),
            )
            .col_expr(Column::ResolvedBy, Expr::value(Option::<i64>::None))
            .col_expr(
                Column::DeletedAt,
                Expr::value(time::OffsetDateTime::now_utc()),
            )
            .exec(database)
            .await?;
        Ok(())
    }
}

pub(crate) trait CommentFilter: QueryFilter + Sized {
    /// Include archived conversations through their stable hotlap identity.
    fn on_hotlap(self, hotlap_id: i64) -> Self {
        self.filter(Column::OriginalHotlapId.eq(hotlap_id))
    }

    fn by_author(self, author_id: i64) -> Self {
        self.filter(Column::AuthorId.eq(author_id))
    }

    fn not_deleted(self) -> Self {
        self.filter(Column::DeletedAt.is_null())
    }

    /// Reports include resolved entries, but exclude deleted entries.
    fn reported(self, reported: bool) -> Self {
        let report = Condition::all()
            .add(Column::Reason.is_not_null())
            .add(Column::DeletedAt.is_null());
        self.filter(if reported { report } else { report.not() })
    }

    fn open_reports(self) -> Self {
        self.reported(true).filter(Column::ResolvedAt.is_null())
    }
}

impl CommentFilter for Select<Entity> {}
impl CommentFilter for UpdateMany<Entity> {}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{PaginatorTrait, Set};

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn report_filters_include_resolved_reports_and_preserve_archived_scope(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!("../services/validate_hotlap/fixtures.sql"))
            .execute(&pool)
            .await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let lap = crate::models::hotlap::Entity::find()
            .one(&database)
            .await?
            .unwrap();
        let entry = ActiveModel {
            hotlap_id: Set(Some(lap.id)),
            original_hotlap_id: Set(lap.id),
            author_id: Set(lap.player_id),
            body: Set(Some("Comment".into())),
            ..Default::default()
        }
        .insert(&database)
        .await?;
        let query = || Entity::find().on_hotlap(lap.id).by_author(lap.player_id);
        for (report, resolved, deleted) in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
            (true, true, true),
            (false, false, true),
        ] {
            sqlx::query("UPDATE hotlap_comment SET reason = CASE WHEN $2 THEN 'other_problem' END,
                resolved_at = CASE WHEN $3 THEN now() END, deleted_at = CASE WHEN $4 THEN now() END WHERE id = $1")
                .bind(entry.id).bind(report).bind(resolved).bind(deleted).execute(&pool).await?;
            assert_eq!(
                query().reported(true).count(&database).await?,
                u64::from(report && !deleted)
            );
            assert_eq!(
                query().reported(false).count(&database).await?,
                u64::from(!report || deleted)
            );
            assert_eq!(
                query().open_reports().count(&database).await?,
                u64::from(report && !resolved && !deleted)
            );
            assert_eq!(
                query().not_deleted().count(&database).await?,
                u64::from(!deleted)
            );
            assert_eq!(
                Model::report_exists(&database, lap.id, lap.player_id).await?,
                report && !deleted
            );
            let counts = Model::report_counts(&database, vec![lap.id]).await?;
            assert_eq!(counts.len(), usize::from(report && !deleted));
            if let Some(counts) = counts.first() {
                assert_eq!(counts.total, 1);
                assert_eq!(counts.unresolved, i64::from(!resolved));
            }
        }
        assert_eq!(
            Model::root_position(&database, query(), lap.id, entry.id).await?,
            Some(0)
        );
        assert_eq!(
            Model::root_position(&database, query(), lap.id, entry.id + 1).await?,
            None
        );
        sqlx::query("UPDATE hotlap_comment SET hotlap_id = NULL WHERE id = $1")
            .bind(entry.id)
            .execute(&pool)
            .await?;
        assert_eq!(query().count(&database).await?, 1);
        let archived = Model::thread(&database, entry.id).await?.unwrap();
        assert_eq!(archived.entry.comment.original_hotlap_id, lap.id);
        assert_eq!(archived.entry.comment.hotlap_id, None);
        Model::remove_owned(&database, lap.id, entry.id, lap.player_id).await?;
        assert_eq!(query().count(&database).await?, 1);
        assert_eq!(
            Entity::find()
                .on_hotlap(lap.id + 1)
                .count(&database)
                .await?,
            0
        );
        Ok(())
    }
}
