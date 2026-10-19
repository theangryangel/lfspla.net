//! Public, enabled community cards in catalogue display order.

use super::attachments::{self, Attachment};
use crate::{
    api::{ApiError, ApiState, ErrorResponse, ListResponse},
    models::community_card::{self, CommunityCardContent, CommunityCardTheme},
};
use axum::{Json, extract::State};
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct CommunityCard {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) sequence: i32,
    pub(crate) sponsored: bool,
    pub(crate) theme: CommunityCardTheme,
    pub(crate) content: CommunityCardContent,
    #[schema(required)]
    pub(crate) image: Option<Attachment>,
}

pub(crate) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list_community_cards))
}

#[utoipa::path(get, path = "/api/v1/community-cards", operation_id = "list_community_cards", tag = "community-cards",
    responses((status = 200, description = "Enabled community cards in display order", body = ListResponse<CommunityCard>), (status = 500, description = "Community cards could not be loaded", body = ErrorResponse)))]
pub(crate) async fn list_community_cards(
    State(state): State<ApiState>,
) -> Result<Json<ListResponse<CommunityCard>>, ApiError> {
    let records = community_card::Entity::visible_at(time::OffsetDateTime::now_utc())
        .all(&state.database)
        .await
        .map_err(ApiError::database)?;
    let images = attachments::load(
        &state.database,
        records.iter().filter_map(|card| card.image_attachment_id),
    )
    .await?;
    let items = records
        .into_iter()
        .map(|card| {
            Ok(CommunityCard {
                id: card.id,
                title: card.title,
                sequence: card.sequence,
                sponsored: card.sponsored,
                theme: serde_json::from_value(serde_json::Value::String(card.theme))
                    .map_err(|error| ApiError::internal(&error, "community card theme"))?,
                content: serde_json::from_value(card.content)
                    .map_err(|error| ApiError::internal(&error, "community card content"))?,
                image: card
                    .image_attachment_id
                    .and_then(|id| images.get(&id).cloned()),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(ListResponse::from(items)))
}

#[cfg(test)]
mod tests;
