//! ISO country catalogue endpoint.

use axum::{Json, extract::Query};
use celes::Country;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::api::{ApiError, ApiState, ListResponse};

/// Canonical ISO 3166-1 country metadata.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CountrySummary {
    code: &'static str,
    name: &'static str,
}

/// Search for the country catalogue.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct CountrySearchQuery {
    /// Country code or name search text.
    q: Option<String>,
}

/// Builds country catalogue routes.
pub(super) fn router() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(list))
}

#[utoipa::path(
    get,
    path = "/api/v1/countries",
    operation_id = "list_countries",
    tag = "countries",
    params(CountrySearchQuery),
    responses(
        (status = 200, body = ListResponse<CountrySummary>)
    )
)]
pub(crate) async fn list(
    Query(query): Query<CountrySearchQuery>,
) -> Result<Json<ListResponse<CountrySummary>>, ApiError> {
    let search = query.q.as_deref().unwrap_or_default().trim().to_lowercase();
    let mut countries: Vec<CountrySummary> = Country::get_countries()
        .into_iter()
        .map(|country| CountrySummary {
            code: country.alpha2,
            name: country.long_name,
        })
        .filter(|country| country.matches(&search))
        .collect();
    // Put exact code matches first; keep catalogue order for other matches.
    countries.sort_by_key(|country| !country.code.eq_ignore_ascii_case(&search));
    Ok(Json(ListResponse::from(countries)))
}

impl CountrySummary {
    /// Matches the country code or name literally.
    fn matches(&self, search: &str) -> bool {
        search.is_empty()
            || self.code.to_lowercase().contains(search)
            || self.name.to_lowercase().contains(search)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn catalogue_is_complete_and_omits_pagination() {
        let Json(response) = list(Query(CountrySearchQuery { q: None })).await.unwrap();
        assert_eq!(response.items.len(), Country::get_countries().len());
        assert!(
            serde_json::to_value(response)
                .unwrap()
                .get("pagination")
                .is_none()
        );
        let Json(response) = list(Query(CountrySearchQuery {
            q: Some("gb".into()),
        }))
        .await
        .unwrap();
        assert_eq!(response.items[0].code, "GB");
    }
}
