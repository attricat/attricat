//! Bounds for list endpoints whose contract is a bare JSON array.

use axum::{
    Json,
    http::{HeaderName, HeaderValue},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use super::error::ApiError;

const MAX_ARRAY_LIMIT: i64 = 500;
const MAX_OFFSET: i64 = 10_000;
const NEXT_OFFSET: HeaderName = HeaderName::from_static("x-next-offset");

/// Optional `limit`/`offset`. Omitting both returns the newest
/// [`MAX_ARRAY_LIMIT`] items, so a growing table never produces an unbounded
/// response.
#[derive(Clone, Copy, Default, Deserialize)]
pub(super) struct ArrayPage {
    limit: Option<i64>,
    offset: Option<i64>,
}

impl ArrayPage {
    pub(super) fn new(limit: Option<i64>, offset: Option<i64>) -> Self {
        Self { limit, offset }
    }

    pub(super) fn bounds(self) -> Result<(i64, i64), ApiError> {
        let limit = self.limit.unwrap_or(MAX_ARRAY_LIMIT);
        let offset = self.offset.unwrap_or(0);
        if !(1..=MAX_ARRAY_LIMIT).contains(&limit) || !(0..=MAX_OFFSET).contains(&offset) {
            return Err(ApiError::invalid_input(format!(
                "limit must be 1-{MAX_ARRAY_LIMIT} and offset must be 0-{MAX_OFFSET}"
            )));
        }
        Ok((limit, offset))
    }
}

/// Serializes one page as an array; `x-next-offset` signals more items.
pub(super) fn array_response<T: Serialize>(
    (items, has_more): (Vec<T>, bool),
    (limit, offset): (i64, i64),
) -> Response {
    let mut response = Json(items).into_response();
    if let Some(next) = has_more
        .then_some(offset + limit)
        .filter(|next| *next <= MAX_OFFSET)
    {
        response
            .headers_mut()
            .insert(NEXT_OFFSET, HeaderValue::from(next));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_pages_default_to_a_bounded_first_page() {
        assert_eq!(ArrayPage::default().bounds().unwrap(), (MAX_ARRAY_LIMIT, 0));
        assert_eq!(
            ArrayPage::new(Some(10), Some(20)).bounds().unwrap(),
            (10, 20)
        );
        for (limit, offset) in [
            (0, 0),
            (MAX_ARRAY_LIMIT + 1, 0),
            (1, -1),
            (1, MAX_OFFSET + 1),
        ] {
            assert!(ArrayPage::new(Some(limit), Some(offset)).bounds().is_err());
        }
    }

    #[test]
    fn next_offset_is_advertised_only_when_more_items_exist() {
        let more = array_response((vec![1, 2], true), (2, 4));
        assert_eq!(more.headers()[NEXT_OFFSET], "6");
        let last = array_response((vec![1], false), (2, 4));
        assert!(last.headers().get(NEXT_OFFSET).is_none());
    }
}
