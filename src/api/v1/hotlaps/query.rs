//! Common public collection filters, parsed at the HTTP boundary.

use crate::{api::ApiError, models::hotlap::SteeringInput};
use axum::http::StatusCode;
use celes::Country;
use sea_orm::ActiveEnum;

pub(crate) fn country_filter(value: Option<&str>) -> Result<Option<Country>, ApiError> {
    value.map(Country::from_alpha2).transpose().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_country_filter",
            "The country filter must be an ISO 3166-1 alpha-2 code",
        )
    })
}

pub(crate) fn controller_filter(value: Option<&str>) -> Result<Option<SteeringInput>, ApiError> {
    value
        .map(|value| SteeringInput::try_from_value(&value.to_owned()))
        .transpose()
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_controller_filter",
                "The controller filter is invalid",
            )
        })
}
