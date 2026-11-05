//! Public threaded comments, including reports as flagged entries.
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::{
        hotlap,
        hotlap_comment::{self as comment, CommentFilter},
        report_reason::ReportReason,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use validator::Validate;

#[derive(Serialize, ToSchema)]
pub(crate) struct HotlapCommentResponse {
    pub(crate) id: i64,
    #[schema(required)]
    pub(crate) hotlap_id: Option<i64>,
    /// Stable hotlap identity for scoped access, including archived conversations.
    original_hotlap_id: i64,
    #[schema(required)]
    parent_id: Option<i64>,
    #[schema(required)]
    root_id: Option<i64>,
    depth: i16,
    author: PlayerSummary,
    /// Plain text; null for deleted entries, whose replies remain readable.
    #[schema(required)]
    body: Option<String>,
    #[schema(required)]
    reason: Option<ReportReason>,
    reply_count: u64,
    /// Included descendants, oldest first. If fewer than reply_count, load more direct replies.
    #[schema(no_recursion)]
    replies: Vec<HotlapCommentResponse>,
    deleted: bool,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(required, value_type = Option<String>, format = DateTime)]
    resolved_at: Option<OffsetDateTime>,
    #[schema(required)]
    resolved_by: Option<i64>,
}
#[derive(Deserialize, ToSchema, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateHotlapComment {
    #[validate(length(min = 1, max = 4000))]
    body: String,
    /// Omit for a new top-level comment. Must belong to the same published lap.
    parent_id: Option<i64>,
    /// A reason flags a top-level comment as a report.
    reason: Option<ReportReason>,
}

#[derive(Deserialize, ToSchema, Validate)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateHotlapComment {
    #[validate(length(min = 1, max = 4000))]
    body: Option<String>,
    /// Omit to leave unchanged; null withdraws the flag and preserves the conversation.
    #[serde(default, deserialize_with = "optional_reason")]
    #[schema(value_type = Option<ReportReason>, nullable = true)]
    reason: Option<Option<ReportReason>>,
    /// Only moderators can resolve or reopen a report.
    resolved: Option<bool>,
}

fn optional_reason<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<ReportReason>>, D::Error> {
    Option::<ReportReason>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CommentLocationQuery {
    /// Open the root page containing this comment, including replies.
    around: Option<i64>,
    /// List direct replies instead of root conversations.
    parent_id: Option<i64>,
    author_id: Option<i64>,
    /// Filter roots by whether they are reports (including resolved reports).
    reported: Option<bool>,
}
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update, remove))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        ))
}
impl From<comment::CommentThread> for HotlapCommentResponse {
    fn from(thread: comment::CommentThread) -> Self {
        let entry = thread.entry;
        let row = entry.comment;
        Self {
            id: row.id,
            hotlap_id: row.hotlap_id,
            original_hotlap_id: row.original_hotlap_id,
            parent_id: row.parent_id,
            root_id: row.root_id,
            depth: row.depth,
            body: row.body,
            reason: row.reason,
            reply_count: entry.reply_count as u64,
            replies: thread.replies.into_iter().map(Into::into).collect(),
            deleted: row.deleted_at.is_some(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            resolved_at: row.resolved_at,
            resolved_by: row.resolved_by,
            author: PlayerSummary {
                id: row.author_id,
                lfs_username: entry.lfs_username,
                display_name: entry.display_name,
                country_code: entry.country_code,
                flag_code: entry.flag_code,
            },
        }
    }
}

impl HotlapCommentResponse {
    async fn find(database: &impl ConnectionTrait, id: i64) -> Result<Self, ApiError> {
        comment::Model::thread(database, id)
            .await
            .map_err(ApiError::database)?
            .map(Into::into)
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment"))
    }
}
async fn collection(
    database: &impl ConnectionTrait,
    query: sea_orm::Select<comment::Entity>,
    page: PaginationQuery,
) -> Result<Json<PaginatedResponse<HotlapCommentResponse>>, ApiError> {
    let (items, total) = comment::Model::list(database, query, page.offset()?, page.per_page)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items: items.into_iter().map(Into::into).collect(),
        pagination: page.metadata(total),
    }))
}
#[utoipa::path(get, path = "/api/v1/hotlaps/{hotlap}/comments", operation_id = "list_hotlap_comments", tag = "comments",
    params(("hotlap" = i64, Path), PaginationQuery, CommentLocationQuery), responses((status = 200, body = PaginatedResponse<HotlapCommentResponse>, description = "Paginated public or archived conversations: open reports first, then other roots chronologically, with nested chronological replies"),
    (status = 400, body = ErrorResponse, description = "Invalid pagination"), (status = 404, body = ErrorResponse, description = "Published hotlap or archived conversation not found")))]
pub(crate) async fn list(
    State(state): State<ApiState>,
    Path(id): Path<i64>,
    Query(mut page): Query<PaginationQuery>,
    Query(location): Query<CommentLocationQuery>,
) -> Result<Json<PaginatedResponse<HotlapCommentResponse>>, ApiError> {
    page.offset()?;
    let mut query = comment::Model::ordered_roots(id);
    if let Some(parent_id) = location.parent_id {
        if location.around.is_some() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_comment_filters",
                "around cannot be combined with parent_id.",
            ));
        }
        comment::Entity::find_by_id(parent_id)
            .on_hotlap(id)
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Parent on this hotlap"))?;
        query = comment::Entity::find()
            .on_hotlap(id)
            .filter(comment::Column::ParentId.eq(parent_id));
    } else {
        let published = hotlap::Entity::find_by_id(id)
            .filter(hotlap::Column::State.eq(hotlap::HotlapState::Valid))
            .count(&state.database)
            .await
            .map_err(ApiError::database)?
            > 0;
        // A removed lap's stable identity keeps its conversation accessible.
        // Only detached comments qualify, so unpublished live laps stay private.
        let archived = !published
            && comment::Entity::find()
                .on_hotlap(id)
                .filter(comment::Column::HotlapId.is_null())
                .count(&state.database)
                .await
                .map_err(ApiError::database)?
                > 0;
        if !published && !archived {
            return Err(ApiError::not_found(
                "hotlap_not_found",
                "Published hotlap or archived conversation",
            ));
        }
    }
    if let Some(author_id) = location.author_id {
        query = query.by_author(author_id);
    }
    if let Some(reported) = location.reported {
        query = query.reported(reported);
    }
    if let Some(comment_id) = location.around {
        let before = comment::Model::root_position(&state.database, query.clone(), id, comment_id)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment on this hotlap"))?;
        page.page = before / page.per_page + 1;
    }
    collection(&state.database, query, page).await
}
#[utoipa::path(post, path = "/api/v1/hotlaps/{hotlap}/comments", operation_id = "create_hotlap_comment", tag = "comments",
    security(("cookie_session" = []), ("personal_access_token" = [])), params(("hotlap" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")), request_body = CreateHotlapComment,
    responses((status = 201, body = HotlapCommentResponse, description = "Comment, reply, or report published"), (status = 409, body = ErrorResponse, description = "You already have a report on this hotlap"), (status = 400, body = ErrorResponse, description = "Invalid text or reply depth above 8"),
    (status = 401, body = ErrorResponse, description = "Authentication required"), (status = 403, body = ErrorResponse, description = "Invalid CSRF token"), (status = 404, body = ErrorResponse, description = "Published lap or matching parent not found")))]
pub(crate) async fn create(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path(id): Path<i64>,
    Json(mut request): Json<CreateHotlapComment>,
) -> Result<(StatusCode, Json<HotlapCommentResponse>), ApiError> {
    request.body = request.body.trim().to_owned();
    request.validate()?;
    let body = request.body;
    let txn = state.database.begin().await.map_err(ApiError::database)?;
    hotlap::Entity::find_by_id(id)
        .filter(hotlap::Column::State.eq(hotlap::HotlapState::Valid))
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Published hotlap"))?;
    if request.reason.is_some() {
        if request.parent_id.is_some() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_report_parent",
                "Reports must be top-level comments.",
            ));
        }
        ensure_report_available(&txn, id, player.id).await?;
    }
    let parent = if let Some(parent_id) = request.parent_id {
        let parent = comment::Entity::find_by_id(parent_id)
            .filter(comment::Column::HotlapId.eq(id))
            .lock_shared()
            .one(&txn)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Parent on this hotlap"))?;
        if parent.depth >= 8 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "reply_depth_exceeded",
                "Reply to an earlier comment; the maximum depth is 8.",
            ));
        }
        Some(parent)
    } else {
        None
    };
    let row = comment::ActiveModel {
        hotlap_id: Set(Some(id)),
        original_hotlap_id: Set(id),
        parent_id: Set(parent.as_ref().map(|row| row.id)),
        root_id: Set(parent.as_ref().map(|row| row.root_id.unwrap_or(row.id))),
        depth: Set(parent.as_ref().map_or(0, |row| row.depth + 1)),
        author_id: Set(player.id),
        body: Set(Some(body)),
        reason: Set(request.reason),
        ..Default::default()
    }
    .insert(&txn)
    .await
    .map_err(ApiError::database)?;
    let response = HotlapCommentResponse::find(&txn, row.id).await?;
    txn.commit().await.map_err(ApiError::database)?;
    Ok((StatusCode::CREATED, Json(response)))
}
async fn ensure_report_available(
    database: &impl ConnectionTrait,
    hotlap_id: i64,
    author_id: i64,
) -> Result<(), ApiError> {
    if comment::Model::report_exists(database, hotlap_id, author_id)
        .await
        .map_err(ApiError::database)?
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "report_exists",
            "You already have a report on this hotlap. Edit or withdraw it first.",
        ));
    }
    Ok(())
}

#[utoipa::path(patch, path = "/api/v1/hotlaps/{hotlap}/comments/{comment}", operation_id = "update_hotlap_comment", tag = "comments",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("hotlap" = i64, Path), ("comment" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")), request_body = UpdateHotlapComment,
    responses((status = 200, body = HotlapCommentResponse, description = "Updated comment; changed report text or reason reopens it"),
    (status = 400, body = ErrorResponse, description = "Invalid update"), (status = 401, body = ErrorResponse, description = "Authentication required"),
    (status = 403, body = ErrorResponse, description = "Author or moderator permission required"), (status = 404, body = ErrorResponse, description = "Comment not found"),
    (status = 409, body = ErrorResponse, description = "You already have a report on this hotlap")))]
pub(crate) async fn update(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path((hotlap_id, id)): Path<(i64, i64)>,
    Json(mut request): Json<UpdateHotlapComment>,
) -> Result<Json<HotlapCommentResponse>, ApiError> {
    request.body = request.body.map(|body| body.trim().to_owned());
    request.validate()?;
    let body = request.body;
    let content = body.is_some() || request.reason.is_some();
    if !content && request.resolved.is_none() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "empty_comment_update",
            "Supply body, reason, or resolved.",
        ));
    }
    let moderator_id = if request.resolved.is_some() {
        crate::api::extractors::require_moderator(&player)?;
        Some(player.id)
    } else {
        None
    };
    let txn = state.database.begin().await.map_err(ApiError::database)?;
    // Match creation's lock order so promoting comments and report submissions serialize.
    let lap = hotlap::Entity::find_by_id(hotlap_id)
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?;
    let row = comment::Entity::find_by_id(id)
        .on_hotlap(hotlap_id)
        .not_deleted()
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment on this hotlap"))?;
    if content && row.author_id != player.id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "comment_author_required",
            "Only the author can edit this comment.",
        ));
    }
    let reason = request.reason.unwrap_or(row.reason);
    let changed = body
        .as_ref()
        .is_some_and(|body| row.body.as_ref() != Some(body))
        || reason != row.reason;
    if content
        && (body.is_some() || reason.is_some())
        && !lap.is_some_and(|lap| lap.state == hotlap::HotlapState::Valid)
    {
        return Err(ApiError::not_found("hotlap_not_found", "Published hotlap"));
    }
    if reason.is_some() && row.parent_id.is_some() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_report_parent",
            "Reports must be top-level comments.",
        ));
    }
    if reason.is_some() && row.reason.is_none() {
        ensure_report_available(&txn, hotlap_id, row.author_id).await?;
    }
    if request.resolved.is_some() && (reason.is_none() || changed) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_report_resolution",
            "Resolve an existing report separately from changing its content.",
        ));
    }
    let mut active = row.into_active_model();
    if let Some(body) = body {
        active.body = Set(Some(body));
    }
    if let Some(reason) = request.reason {
        active.reason = Set(reason);
    }
    if changed {
        active.updated_at = Set(OffsetDateTime::now_utc());
        active.resolved_at = Set(None);
        active.resolved_by = Set(None);
    }
    if let Some(resolved) = request.resolved {
        // Preserve the original resolver and time on repeated resolution requests.
        if active.resolved_at.as_ref().is_some() != resolved {
            active.resolved_at = Set(resolved.then(OffsetDateTime::now_utc));
            active.resolved_by = Set(if resolved { moderator_id } else { None });
        }
    }
    active.update(&txn).await.map_err(ApiError::database)?;
    let response = HotlapCommentResponse::find(&txn, id).await?;
    txn.commit().await.map_err(ApiError::database)?;
    Ok(Json(response))
}

#[utoipa::path(delete, path = "/api/v1/hotlaps/{hotlap}/comments/{comment}", operation_id = "remove_hotlap_comment", tag = "comments",
    security(("cookie_session" = []), ("personal_access_token" = [])), params(("hotlap" = i64, Path), ("comment" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")),
    responses((status = 204, description = "Your content removed; a placeholder preserves replies"), (status = 401, body = ErrorResponse, description = "Authentication required"), (status = 403, body = ErrorResponse, description = "Invalid CSRF token")))]
pub(crate) async fn remove(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path((hotlap_id, id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    comment::Model::remove_owned(&state.database, hotlap_id, id, player.id)
        .await
        .map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comment_requests_require_bounded_text() {
        for (value, valid) in [
            ("", false),
            ("  ", false),
            ("<b>plain</b>\ntext\t", true),
            (&"x".repeat(4001), false),
            (&"é".repeat(4000), true),
        ] {
            let create = CreateHotlapComment {
                body: value.trim().to_owned(),
                parent_id: None,
                reason: None,
            };
            let update = UpdateHotlapComment {
                body: Some(value.trim().to_owned()),
                reason: None,
                resolved: None,
            };
            assert_eq!(create.validate().is_ok(), valid);
            assert_eq!(update.validate().is_ok(), valid);
        }
        let resolution = UpdateHotlapComment {
            body: None,
            reason: None,
            resolved: Some(true),
        };
        assert!(resolution.validate().is_ok());
    }
    struct Counted<'a> {
        database: &'a sea_orm::DatabaseConnection,
        queries: std::sync::atomic::AtomicUsize,
    }
    impl Counted<'_> {
        fn count(&self) -> usize {
            self.queries.swap(0, std::sync::atomic::Ordering::SeqCst)
        }
    }
    macro_rules! counted_method {
        ($method:ident, $output:ty) => {
            fn $method<'life0, 'async_trait>(
                &'life0 self,
                statement: sea_orm::Statement,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<$output, sea_orm::DbErr>>
                        + Send
                        + 'async_trait,
                >,
            >
            where
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async move {
                    self.queries
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    self.database.$method(statement).await
                })
            }
        };
    }
    impl sea_orm::ConnectionTrait for Counted<'_> {
        fn get_database_backend(&self) -> sea_orm::DbBackend {
            self.database.get_database_backend()
        }
        counted_method!(execute_raw, sea_orm::ExecResult);
        counted_method!(query_one_raw, Option<sea_orm::QueryResult>);
        counted_method!(query_all_raw, Vec<sea_orm::QueryResult>);
        fn execute_unprepared<'life0, 'life1, 'async_trait>(
            &'life0 self,
            sql: &'life1 str,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<sea_orm::ExecResult, sea_orm::DbErr>>
                    + Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                self.queries
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.database.execute_unprepared(sql).await
            })
        }
    }

    #[sqlx::test]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn database_tests_nested_pages_have_constant_query_count(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        sqlx::raw_sql(include_str!(
            "../../../services/validate_hotlap/fixtures.sql"
        ))
        .execute(&pool)
        .await?;
        // Twenty roots, each with two replies and one grandchild per reply.
        sqlx::raw_sql("WITH roots AS (
            INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body)
            SELECT h.id,h.id,h.player_id,'root' FROM hotlap h CROSS JOIN generate_series(1,20) RETURNING *
        ), children AS (
            INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT r.original_hotlap_id,r.hotlap_id,r.author_id,'child',r.id,r.id,1 FROM roots r CROSS JOIN generate_series(1,2) RETURNING *
        ) INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
        SELECT c.original_hotlap_id,c.hotlap_id,c.author_id,'grandchild',c.id,c.root_id,2 FROM children c").execute(&pool).await?;
        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let counted = Counted {
            database: &database,
            queries: Default::default(),
        };
        let query = || comment::Entity::find().filter(comment::Column::ParentId.is_null());
        let Json(first) = collection(
            &counted,
            query(),
            PaginationQuery {
                page: 1,
                per_page: 1,
            },
        )
        .await?;
        assert_eq!(
            counted.count(),
            3,
            "count, selected comments, recursive thread with authors and reply counts"
        );
        assert_eq!(first.pagination.total_items, 20);
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0].replies.len(), 2);
        assert_eq!(
            first.items[0].replies[0].replies[0].body.as_deref(),
            Some("grandchild")
        );
        let Json(second) = collection(
            &counted,
            query(),
            PaginationQuery {
                page: 2,
                per_page: 1,
            },
        )
        .await?;
        assert_eq!(counted.count(), 3);
        assert_ne!(first.items[0].id, second.items[0].id);
        assert!(
            second.items[0]
                .replies
                .iter()
                .all(|reply| reply.parent_id == Some(second.items[0].id))
        );
        let Json(all) = collection(
            &counted,
            query(),
            PaginationQuery {
                page: 1,
                per_page: 20,
            },
        )
        .await?;
        assert_eq!(
            counted.count(),
            3,
            "query count must not grow with roots or depth"
        );
        assert_eq!(all.items.len(), 20);
        assert!(all.items.iter().all(|root| root.replies.len() == 2
            && root.replies.iter().all(|child| child.replies.len() == 1)));
        let root = all.items[0].id;
        // A huge branch is explicitly incomplete, and the rest can be paged.
        sqlx::query("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT original_hotlap_id,hotlap_id,author_id,'extra',id,id,1 FROM hotlap_comment CROSS JOIN generate_series(1,1001) WHERE id=$1")
            .bind(root).execute(&pool).await?;
        let Json(large) = collection(
            &counted,
            query(),
            PaginationQuery {
                page: 1,
                per_page: 1,
            },
        )
        .await?;
        assert_eq!(counted.count(), 3);
        assert_eq!(large.items[0].reply_count, 1003);
        assert!(large.items[0].replies.len() < large.items[0].reply_count as usize);
        let Json(last) = collection(
            &counted,
            comment::Entity::find().filter(comment::Column::ParentId.eq(root)),
            PaginationQuery {
                page: 21,
                per_page: 50,
            },
        )
        .await?;
        assert_eq!(counted.count(), 3);
        assert_eq!(last.pagination.total_items, 1003);
        assert_eq!(last.items.len(), 3);

        // A busy sibling branch must not consume a selected comment's budget.
        // Create its descendants after the siblings so a root-wide ID prefix
        // would miss them entirely.
        let selected = sqlx::query_scalar::<_, i64>("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT original_hotlap_id,hotlap_id,author_id,'selected',id,id,1 FROM hotlap_comment WHERE id=$1 RETURNING id")
            .bind(root).fetch_one(&pool).await?;
        sqlx::query("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT original_hotlap_id,hotlap_id,author_id,'unrelated sibling',id,id,1 FROM hotlap_comment CROSS JOIN generate_series(1,1001) WHERE id=$1")
            .bind(root).execute(&pool).await?;
        let descendant = sqlx::query_scalar::<_, i64>("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT original_hotlap_id,hotlap_id,author_id,'selected descendant',id,root_id,2 FROM hotlap_comment WHERE id=$1 RETURNING id")
            .bind(selected).fetch_one(&pool).await?;
        let nested = sqlx::query_scalar::<_, i64>("INSERT INTO hotlap_comment (original_hotlap_id,hotlap_id,author_id,body,parent_id,root_id,depth)
            SELECT original_hotlap_id,hotlap_id,author_id,'nested descendant',id,root_id,3 FROM hotlap_comment WHERE id=$1 RETURNING id")
            .bind(descendant).fetch_one(&pool).await?;
        let entry = HotlapCommentResponse::find(&counted, selected).await?;
        assert_eq!(
            counted.count(),
            1,
            "recursive thread includes selected comment"
        );
        assert_eq!(entry.reply_count, 1);
        assert_eq!(entry.replies.len(), 1);
        assert_eq!(entry.replies[0].id, descendant);
        assert_eq!(entry.replies[0].reply_count, 1);
        assert_eq!(entry.replies[0].replies[0].id, nested);
        let Json(branch) = collection(
            &counted,
            comment::Entity::find().filter(comment::Column::ParentId.eq(selected)),
            PaginationQuery {
                page: 1,
                per_page: 50,
            },
        )
        .await?;
        assert_eq!(counted.count(), 3);
        assert_eq!(branch.pagination.total_items, 1);
        assert_eq!(branch.items[0].replies[0].id, nested);
        Ok(())
    }
}
