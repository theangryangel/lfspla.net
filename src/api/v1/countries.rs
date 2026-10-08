//! ISO country catalogue endpoint.

use axum::{Json, extract::Query};
use celes::Country;
use lfsplanet_flags::CountryFlagsExt;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::api::{ApiError, ApiState, ListResponse};

/// A catalogue code and display name. Country codes are ISO alpha-2; flag codes identify display flags.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CodeNameSummary {
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
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(flags))
}

#[utoipa::path(
    get,
    path = "/api/v1/countries",
    operation_id = "list_countries",
    tag = "countries",
    params(CountrySearchQuery),
    responses(
        (status = 200, description = "Available countries", body = ListResponse<CodeNameSummary>)
    )
)]
pub(crate) async fn list(
    Query(query): Query<CountrySearchQuery>,
) -> Result<Json<ListResponse<CodeNameSummary>>, ApiError> {
    let search = query.q.as_deref().unwrap_or_default().trim().to_lowercase();
    let mut countries: Vec<CodeNameSummary> = Country::get_countries()
        .into_iter()
        .map(|country| CodeNameSummary {
            code: country.alpha2,
            name: country.long_name,
        })
        .filter(|country| country.matches(&search))
        .collect();
    // Put exact code matches first; keep catalogue order for other matches.
    countries.sort_by_key(|country| !country.code.eq_ignore_ascii_case(&search));
    Ok(Json(ListResponse::from(countries)))
}

impl CodeNameSummary {
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

#[utoipa::path(
    get,
    path = "/api/v1/countries/{code}/flags",
    operation_id = "list_country_flags",
    tag = "countries",
    params(("code" = String, Path, description = "ISO alpha-2 country code")),
    responses(
        (status = 200, description = "Available display flags, national flag first", body = ListResponse<CodeNameSummary>),
        (status = 400, description = "Invalid country code", body = crate::api::ErrorResponse)
    )
)]
pub(crate) async fn flags(
    axum::extract::Path(code): axum::extract::Path<String>,
) -> Result<Json<ListResponse<CodeNameSummary>>, ApiError> {
    let country = Country::from_alpha2(code.to_ascii_uppercase()).map_err(|_| {
        ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_country_code",
            "Choose a valid country code",
        )
    })?;
    Ok(Json(ListResponse::from(
        country
            .flags()
            .into_iter()
            .map(|flag| CodeNameSummary {
                code: flag.code.as_str(),
                name: flag.name,
            })
            .collect::<Vec<_>>(),
    )))
}

#[cfg(test)]
mod flag_tests {
    use super::*;

    #[tokio::test]
    async fn accepts_lowercase_country_codes() {
        let Json(response) = flags(axum::extract::Path("gb".into())).await.unwrap();
        assert_eq!(response.items[0].code, "gb");
        assert!(flags(axum::extract::Path("invalid".into())).await.is_err());
    }
}
