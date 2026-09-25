//! Shared collection envelopes and one-based page pagination.
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::ApiError;

/// Parameters shared by endpoints that support page navigation.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(default)]
pub(crate) struct PaginationQuery {
    /// One-based page number; defaults to 1.
    #[param(minimum = 1, default = 1)]
    pub page: u64,
    /// Results per page, from 1 to 100; defaults to 50.
    #[param(minimum = 1, maximum = 100, default = 50)]
    pub per_page: u64,
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: u64::from(super::PER_PAGE),
        }
    }
}

impl PaginationQuery {
    /// Validate before arithmetic and keep the SQL offset within BIGINT.
    pub fn offset(&self) -> Result<u64, ApiError> {
        let offset = self
            .page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(self.per_page));
        if !(1..=100).contains(&self.per_page) || offset.is_none_or(|n| n > i64::MAX as u64) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_pagination",
                "Use page >= 1 and per_page between 1 and 100, with an offset within the supported range",
            ));
        }
        Ok(offset.expect("validated offset"))
    }

    pub fn metadata(&self, total_items: u64) -> Pagination {
        Pagination {
            page: self.page,
            per_page: self.per_page,
            total_items,
            total_pages: total_items.div_ceil(self.per_page),
        }
    }
}

/// Counts describe the filtered collection before pagination.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct Pagination {
    pub page: u64,
    pub per_page: u64,
    pub total_items: u64,
    /// Zero for an empty collection. Out-of-range pages return empty items.
    pub total_pages: u64,
}

/// Common JSON envelope for collection endpoints.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ListResponse<T> {
    pub items: Vec<T>,
}

impl<T> From<Vec<T>> for ListResponse<T> {
    fn from(items: Vec<T>) -> Self {
        Self { items }
    }
}

/// Collection with page navigation and filtered totals.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub pagination: Pagination,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_page_boundaries() {
        let query: PaginationQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(query.offset().unwrap(), 0);
        assert_eq!(query.metadata(0).total_pages, 0);
        assert_eq!(query.metadata(50).total_pages, 1);
        assert_eq!(query.metadata(51).total_pages, 2);
        let query: PaginationQuery = serde_urlencoded::from_str("page=3&per_page=50").unwrap();
        assert_eq!(query.offset().unwrap(), 100);
        assert_eq!(query.metadata(51).page, 3);
    }

    #[test]
    fn invalid_values_and_overflow_are_rejected() {
        for (page, per_page) in [
            (0, 50),
            (1, 0),
            (1, 101),
            (u64::MAX, 100),
            (i64::MAX as u64 + 2, 1),
        ] {
            assert!(PaginationQuery { page, per_page }.offset().is_err());
        }
        for query in ["page=-1", "page=abc"] {
            assert!(serde_urlencoded::from_str::<PaginationQuery>(query).is_err());
        }
    }

    #[test]
    fn unpaginated_lists_omit_pagination() {
        let list: ListResponse<u8> = vec![1, 2].into();
        assert_eq!(
            serde_json::to_value(list).unwrap(),
            serde_json::json!({"items": [1, 2]})
        );
    }
}
