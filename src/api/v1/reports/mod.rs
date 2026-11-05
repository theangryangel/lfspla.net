//! Report views and moderation over flagged conversation entries.
use crate::{
    api::{
        ApiError, ApiState, ErrorResponse, ListResponse,
        extractors::{AuthenticatedPlayer, Moderator},
    },
    models::{hotlap, hotlap_comment as report, report_reason::ReportReason},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

pub(crate) use super::comments::HotlapCommentResponse as ReportResponse;

#[derive(Serialize, ToSchema)]
pub(crate) struct HotlapReportSummary {
    hotlap_id: i64,
    total: u64,
    unresolved: u64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SummaryQuery {
    /// Comma-separated positive hotlap IDs, maximum 100.
    hotlap_ids: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveReport {
    reason: ReportReason,
    body: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveReport {
    resolved: bool,
}

pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(summaries))
        .routes(routes!(mine, save))
        .routes(routes!(withdraw, resolve))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
}

#[utoipa::path(get, path = "/api/v1/hotlaps/reports/summaries", operation_id = "get_report_summaries", tag = "reports",
    params(SummaryQuery), responses((status = 200, body = ListResponse<HotlapReportSummary>, description = "Public flag counts for the requested hotlaps"),
    (status = 400, body = ErrorResponse, description = "Invalid hotlap IDs")))]
pub(crate) async fn summaries(
    State(state): State<ApiState>,
    Query(query): Query<SummaryQuery>,
) -> Result<Json<ListResponse<HotlapReportSummary>>, ApiError> {
    let ids = query
        .hotlap_ids
        .split(',')
        .take(101)
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|ids| !ids.is_empty() && ids.len() <= 100 && ids.iter().all(|id| *id > 0))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_hotlap_ids",
                "Supply 1–100 positive hotlap IDs separated by commas.",
            )
        })?;
    // Aggregate in SQL: counts stay bounded even when a lap has many flags.
    use sea_orm::{FromQueryResult, QueryTrait};
    #[derive(FromQueryResult)]
    struct Counts {
        hotlap_id: i64,
        total: i64,
        unresolved: i64,
    }
    let query = report::Entity::find()
        .select_only()
        .column(report::Column::HotlapId)
        .column_as(sea_orm::sea_query::Expr::cust("COUNT(*)"), "total")
        .column_as(
            sea_orm::sea_query::Expr::cust("COUNT(*) FILTER (WHERE resolved_at IS NULL)"),
            "unresolved",
        )
        .filter(report::Column::HotlapId.is_in(ids))
        .filter(report::Column::Reason.is_not_null())
        .filter(report::Column::DeletedAt.is_null())
        .group_by(report::Column::HotlapId)
        .order_by_asc(report::Column::HotlapId);
    let rows = Counts::find_by_statement(query.build(sea_orm::DatabaseBackend::Postgres))
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| HotlapReportSummary {
                hotlap_id: row.hotlap_id,
                total: row.total as u64,
                unresolved: row.unresolved as u64,
            })
            .collect::<Vec<_>>()
            .into(),
    ))
}

#[utoipa::path(get, path = "/api/v1/hotlaps/{hotlap}/report", operation_id = "get_my_report", tag = "reports",
    security(("cookie_session" = []), ("personal_access_token" = [])), params(("hotlap" = i64, Path)),
    responses((status = 200, body = Option<ReportResponse>, description = "Your flag, or null if you have not flagged this lap"),
    (status = 401, body = ErrorResponse, description = "Authentication required")))]
pub(crate) async fn mine(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path(id): Path<i64>,
) -> Result<Json<Option<ReportResponse>>, ApiError> {
    let row = report::Entity::find()
        .filter(report::Column::HotlapId.eq(id))
        .filter(report::Column::AuthorId.eq(player.id))
        .filter(report::Column::Reason.is_not_null())
        .filter(report::Column::DeletedAt.is_null())
        .one(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(Json(match row {
        Some(row) => Some(super::comments::find(&state.database, row.id).await?),
        None => None,
    }))
}

#[utoipa::path(put, path = "/api/v1/hotlaps/{hotlap}/report", operation_id = "save_my_report", tag = "reports",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("hotlap" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")), request_body = SaveReport,
    responses((status = 200, body = ReportResponse, description = "Report comment created or edited; changed text or reason becomes unresolved"),
    (status = 400, body = ErrorResponse, description = "Invalid reason"), (status = 401, body = ErrorResponse, description = "Authentication required"),
    (status = 403, body = ErrorResponse, description = "Invalid CSRF token"), (status = 404, body = ErrorResponse, description = "Published hotlap not found")))]
pub(crate) async fn save(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path(id): Path<i64>,
    Json(request): Json<SaveReport>,
) -> Result<Json<ReportResponse>, ApiError> {
    let body = super::comments::text(&request.body)?;
    let txn = state.database.begin().await.map_err(ApiError::database)?;
    // Use the lap as a common lock for submissions, edits and moderator resolution.
    hotlap::Entity::find_by_id(id)
        .filter(hotlap::Column::State.eq(hotlap::HotlapState::Valid))
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("hotlap_not_found", "Published hotlap"))?;
    let existing = report::Entity::find()
        .filter(report::Column::HotlapId.eq(id))
        .filter(report::Column::AuthorId.eq(player.id))
        .filter(report::Column::Reason.is_not_null())
        .filter(report::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?;
    let row = if let Some(row) = existing {
        if row.reason == Some(request.reason) && row.body.as_deref() == Some(body.as_str()) {
            row
        } else {
            let mut active = row.into_active_model();
            active.reason = Set(Some(request.reason));
            active.body = Set(Some(body));
            active.updated_at = Set(OffsetDateTime::now_utc());
            active.resolved_at = Set(None);
            active.resolved_by = Set(None);
            active.update(&txn).await.map_err(ApiError::database)?
        }
    } else {
        report::ActiveModel {
            hotlap_id: Set(Some(id)),
            original_hotlap_id: Set(id),
            author_id: Set(player.id),
            reason: Set(Some(request.reason)),
            body: Set(Some(body)),
            ..Default::default()
        }
        .insert(&txn)
        .await
        .map_err(ApiError::database)?
    };
    let response = super::comments::find(&txn, row.id).await?;
    txn.commit().await.map_err(ApiError::database)?;
    Ok(Json(response))
}

#[utoipa::path(delete, path = "/api/v1/hotlaps/{hotlap}/comments/{comment}/report", operation_id = "withdraw_my_report", tag = "reports",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("hotlap" = i64, Path), ("comment" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")),
    responses((status = 204, description = "Flag withdrawn; the comment and replies remain"), (status = 401, body = ErrorResponse, description = "Authentication required"),
    (status = 403, body = ErrorResponse, description = "Invalid CSRF token")))]
pub(crate) async fn withdraw(
    State(state): State<ApiState>,
    AuthenticatedPlayer(player): AuthenticatedPlayer,
    Path((hotlap_id, id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    report::Entity::update_many()
        .filter(report::Column::Id.eq(id))
        .filter(report::Column::OriginalHotlapId.eq(hotlap_id))
        .filter(report::Column::AuthorId.eq(player.id))
        .col_expr(
            report::Column::Reason,
            sea_orm::sea_query::Expr::value(Option::<String>::None),
        )
        .col_expr(
            report::Column::ResolvedAt,
            sea_orm::sea_query::Expr::value(Option::<OffsetDateTime>::None),
        )
        .col_expr(
            report::Column::ResolvedBy,
            sea_orm::sea_query::Expr::value(Option::<i64>::None),
        )
        .exec(&state.database)
        .await
        .map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(patch, path = "/api/v1/hotlaps/{hotlap}/comments/{comment}/report", operation_id = "resolve_report", tag = "reports",
    security(("cookie_session" = []), ("personal_access_token" = [])),
    params(("hotlap" = i64, Path), ("comment" = i64, Path), ("X-CSRF-Token" = String, Header, description = "Required for cookie authentication")), request_body = ResolveReport,
    responses((status = 204, description = "This flag resolved or reopened"),
    (status = 401, body = ErrorResponse, description = "Authentication required"), (status = 403, body = ErrorResponse, description = "Moderator access required or invalid CSRF token"),
    (status = 404, body = ErrorResponse, description = "Report not found")))]
pub(crate) async fn resolve(
    State(state): State<ApiState>,
    Moderator(moderator): Moderator,
    Path((hotlap_id, id)): Path<(i64, i64)>,
    Json(request): Json<ResolveReport>,
) -> Result<StatusCode, ApiError> {
    let txn = state.database.begin().await.map_err(ApiError::database)?;
    let row = report::Entity::find_by_id(id)
        .filter(report::Column::OriginalHotlapId.eq(hotlap_id))
        .filter(report::Column::Reason.is_not_null())
        .filter(report::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(&txn)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found("report_not_found", "Report"))?;
    if row.resolved_at.is_some() != request.resolved {
        let mut active = row.into_active_model();
        active.resolved_at = Set(request.resolved.then(OffsetDateTime::now_utc));
        active.resolved_by = Set(request.resolved.then_some(moderator.id));
        active.update(&txn).await.map_err(ApiError::database)?;
    }
    txn.commit().await.map_err(ApiError::database)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests;
