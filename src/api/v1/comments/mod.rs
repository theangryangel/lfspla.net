//! Public threaded comments, including reports as flagged entries.
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, PaginatedResponse, PaginationQuery,
        extractors::AuthenticatedPlayer, v1::PlayerSummary,
    },
    models::{hotlap, hotlap_comment as comment, report_reason::ReportReason},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, ExprTrait,
    FromQueryResult, IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

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
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateHotlapComment {
    body: String,
    /// Omit for a new top-level comment. Must belong to the same published lap.
    parent_id: Option<i64>,
    /// A reason flags a top-level comment as a report.
    reason: Option<ReportReason>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateHotlapComment {
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
pub(crate) fn text(value: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > 4000
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_comment_text",
            "Enter 1–4000 characters of plain text.",
        ));
    }
    Ok(value.to_owned())
}
pub(crate) async fn responses(
    database: &impl ConnectionTrait,
    selected_ids: Vec<i64>,
) -> Result<Vec<HotlapCommentResponse>, ApiError> {
    #[derive(FromQueryResult)]
    struct ThreadEntry {
        #[sea_orm(nested)]
        comment: comment::Model,
        lfs_username: String,
        display_name: String,
        country_code: Option<String>,
        flag_code: Option<String>,
        reply_count: i64,
    }
    if selected_ids.is_empty() {
        return Ok(Vec::new());
    }
    // Only SQL placeholders are interpolated; all IDs remain bound parameters.
    let placeholders = (1..=selected_ids.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Follow actual parent links, so sibling branches cannot consume the
    // descendant budget when loading a comment or a page of direct replies.
    // UNION also terminates traversal if malformed stored data contains a cycle.
    let entries = ThreadEntry::find_by_statement(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Postgres,
        include_str!("thread.sql").replace("{selected_ids}", &placeholders),
        selected_ids.iter().copied().map(sea_orm::Value::from),
    ))
    .all(database)
    .await
    .map_err(ApiError::database)?;
    let mut child_ids: std::collections::BTreeMap<i64, Vec<i64>> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        if let Some(parent) = entry.comment.parent_id {
            child_ids.entry(parent).or_default().push(entry.comment.id);
        }
    }
    let mut nodes = entries
        .into_iter()
        .map(|entry| {
            let row = entry.comment;
            (
                row.id,
                HotlapCommentResponse {
                    id: row.id,
                    hotlap_id: row.hotlap_id,
                    original_hotlap_id: row.original_hotlap_id,
                    parent_id: row.parent_id,
                    root_id: row.root_id,
                    depth: row.depth,
                    body: row.body,
                    reason: row.reason,
                    reply_count: entry.reply_count as u64,
                    replies: Vec::new(),
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
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    // Assemble the fetched subtree in memory, preserving selected-entry order.
    fn nest(
        id: i64,
        nodes: &mut std::collections::BTreeMap<i64, HotlapCommentResponse>,
        child_ids: &std::collections::BTreeMap<i64, Vec<i64>>,
    ) -> Option<HotlapCommentResponse> {
        let mut row = nodes.remove(&id)?;
        row.replies = child_ids
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|child| nest(*child, nodes, child_ids))
            .collect();
        Some(row)
    }
    Ok(selected_ids
        .into_iter()
        .filter_map(|id| nest(id, &mut nodes, &child_ids))
        .collect())
}
pub(crate) async fn find(
    database: &impl ConnectionTrait,
    id: i64,
) -> Result<HotlapCommentResponse, ApiError> {
    let row = comment::Entity::find_by_id(id)
        .select_only()
        .column(comment::Column::Id)
        .into_tuple::<i64>()
        .one(database)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment"))?;
    responses(database, vec![row])
        .await?
        .pop()
        .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment"))
}
// Shared by root ordering and the around-comment position lookup.
fn open_report_priority() -> sea_orm::sea_query::SimpleExpr {
    sea_orm::sea_query::Expr::col((comment::Entity, comment::Column::Reason))
        .is_not_null()
        .and(
            sea_orm::sea_query::Expr::col((comment::Entity, comment::Column::ResolvedAt)).is_null(),
        )
        .and(sea_orm::sea_query::Expr::col((comment::Entity, comment::Column::DeletedAt)).is_null())
}
fn ordered_roots(hotlap_id: i64) -> sea_orm::Select<comment::Entity> {
    comment::Entity::find()
        .filter(comment::Column::OriginalHotlapId.eq(hotlap_id))
        .filter(comment::Column::ParentId.is_null())
        .order_by_desc(open_report_priority())
}
async fn collection(
    database: &impl ConnectionTrait,
    query: sea_orm::Select<comment::Entity>,
    page: PaginationQuery,
) -> Result<Json<PaginatedResponse<HotlapCommentResponse>>, ApiError> {
    let offset = page.offset()?;
    let total = query
        .clone()
        .count(database)
        .await
        .map_err(ApiError::database)?;
    let rows = query
        .order_by_asc(comment::Column::Id)
        .offset(offset)
        .limit(page.per_page)
        .select_only()
        .column(comment::Column::Id)
        .into_tuple::<i64>()
        .all(database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(PaginatedResponse {
        items: responses(database, rows).await?,
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
    let mut query = ordered_roots(id);
    if let Some(parent_id) = location.parent_id {
        if location.around.is_some() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_comment_filters",
                "around cannot be combined with parent_id.",
            ));
        }
        comment::Entity::find_by_id(parent_id)
            .filter(comment::Column::OriginalHotlapId.eq(id))
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Parent on this hotlap"))?;
        query = comment::Entity::find()
            .filter(comment::Column::OriginalHotlapId.eq(id))
            .filter(comment::Column::ParentId.eq(parent_id))
            .order_by_asc(comment::Column::Id);
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
                .filter(comment::Column::OriginalHotlapId.eq(id))
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
        query = query.filter(comment::Column::AuthorId.eq(author_id));
    }
    if let Some(reported) = location.reported {
        query = query.filter(if reported {
            Condition::all()
                .add(comment::Column::Reason.is_not_null())
                .add(comment::Column::DeletedAt.is_null())
        } else {
            Condition::any()
                .add(comment::Column::Reason.is_null())
                .add(comment::Column::DeletedAt.is_not_null())
        });
    }
    if let Some(comment_id) = location.around {
        let target_root = comment::Entity::find()
            .select_only()
            .column_as(
                sea_orm::sea_query::Expr::cust("COALESCE(root_id, id)"),
                "root",
            )
            .filter(comment::Column::OriginalHotlapId.eq(id))
            .filter(comment::Column::Id.eq(comment_id))
            .into_query();
        let target = query
            .clone()
            .filter(comment::Column::Id.in_subquery(target_root))
            .one(&state.database)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found("comment_not_found", "Comment on this hotlap"))?;
        let target_is_open_report =
            target.reason.is_some() && target.resolved_at.is_none() && target.deleted_at.is_none();
        let mut precedes = Condition::any().add(
            Condition::all()
                .add(open_report_priority().eq(target_is_open_report))
                .add(comment::Column::Id.lt(target.id)),
        );
        if !target_is_open_report {
            precedes = precedes.add(open_report_priority());
        }
        let before = query
            .clone()
            .filter(precedes)
            .count(&state.database)
            .await
            .map_err(ApiError::database)?;
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
    Json(request): Json<CreateHotlapComment>,
) -> Result<(StatusCode, Json<HotlapCommentResponse>), ApiError> {
    let body = text(&request.body)?;
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
        ensure_report_available(&txn, id, player.id, None).await?;
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
    let response = find(&txn, row.id).await?;
    txn.commit().await.map_err(ApiError::database)?;
    Ok((StatusCode::CREATED, Json(response)))
}
async fn ensure_report_available(
    database: &impl ConnectionTrait,
    hotlap_id: i64,
    author_id: i64,
    excluding: Option<i64>,
) -> Result<(), ApiError> {
    let mut query = comment::Entity::find()
        .filter(comment::Column::HotlapId.eq(hotlap_id))
        .filter(comment::Column::AuthorId.eq(author_id))
        .filter(comment::Column::Reason.is_not_null())
        .filter(comment::Column::DeletedAt.is_null());
    if let Some(id) = excluding {
        query = query.filter(comment::Column::Id.ne(id));
    }
    if query
        .one(database)
        .await
        .map_err(ApiError::database)?
        .is_some()
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
    Json(request): Json<UpdateHotlapComment>,
) -> Result<Json<HotlapCommentResponse>, ApiError> {
    let body = request.body.as_deref().map(text).transpose()?;
    let content = body.is_some() || request.reason.is_some();
    if !content && request.resolved.is_none() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "empty_comment_update",
            "Supply body, reason, or resolved.",
        ));
    }
    let moderator_id = if request.resolved.is_some() {
        Some(
            crate::api::extractors::Moderator::from_player(player.clone())?
                .0
                .id,
        )
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
        .filter(comment::Column::OriginalHotlapId.eq(hotlap_id))
        .filter(comment::Column::DeletedAt.is_null())
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
        ensure_report_available(&txn, hotlap_id, row.author_id, Some(id)).await?;
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
    let response = find(&txn, id).await?;
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
    comment::Entity::update_many()
        .filter(comment::Column::Id.eq(id))
        .filter(comment::Column::OriginalHotlapId.eq(hotlap_id))
        .filter(comment::Column::AuthorId.eq(player.id))
        .filter(comment::Column::DeletedAt.is_null())
        .col_expr(
            comment::Column::Body,
            sea_orm::sea_query::Expr::value(Option::<String>::None),
        )
        .col_expr(
            comment::Column::Reason,
            sea_orm::sea_query::Expr::value(Option::<String>::None),
        )
        .col_expr(
            comment::Column::ResolvedAt,
            sea_orm::sea_query::Expr::value(Option::<OffsetDateTime>::None),
        )
        .col_expr(
            comment::Column::ResolvedBy,
            sea_orm::sea_query::Expr::value(Option::<i64>::None),
        )
        .col_expr(
            comment::Column::DeletedAt,
            sea_orm::sea_query::Expr::value(OffsetDateTime::now_utc()),
        )
        .exec(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comment_text_is_bounded_plain_text() {
        assert_eq!(text("  <b>plain</b>\ntext ").unwrap(), "<b>plain</b>\ntext");
        for value in ["", "  ", "a\0b", &"x".repeat(4001)] {
            assert!(text(value).is_err());
        }
        assert!(text(&"é".repeat(4000)).is_ok());
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
        let entry = find(&counted, selected).await?;
        assert_eq!(counted.count(), 2, "selected comment and recursive thread");
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

#[cfg(test)]
mod report_tests;
