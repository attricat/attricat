//! Keyset pages of newest-first lists: a request continues before the
//! `before_time` and `before_id` of the last item it has, and a page says
//! whether more follow (`has_more`), like record comments and notifications.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::ApiError;

fn incomplete() -> ApiError {
    ApiError::invalid_input("every cursor field is required".to_owned())
}

/// Continues a list ordered by a timestamp and an ID.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageQuery {
    before_time: Option<DateTime<Utc>>,
    before_id: Option<Uuid>,
}

impl PageQuery {
    pub fn before(self) -> Result<Option<(DateTime<Utc>, Uuid)>, ApiError> {
        match (self.before_time, self.before_id) {
            (Some(time), Some(id)) => Ok(Some((time, id))),
            (None, None) => Ok(None),
            _ => Err(incomplete()),
        }
    }
}

/// Continues a list ordered by a timestamp, an ID and a handler, for keys
/// that have no ID of their own.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HandlerPageQuery {
    before_time: Option<DateTime<Utc>>,
    before_id: Option<Uuid>,
    before_handler: Option<String>,
}

impl HandlerPageQuery {
    pub fn before(self) -> Result<Option<(DateTime<Utc>, Uuid, String)>, ApiError> {
        match (self.before_time, self.before_id, self.before_handler) {
            (Some(time), Some(id), Some(handler)) => Ok(Some((time, id, handler))),
            (None, None, None) => Ok(None),
            _ => Err(incomplete()),
        }
    }
}

#[derive(Serialize)]
pub(super) struct Page<T> {
    pub items: Vec<T>,
    pub has_more: bool,
}

impl<T> Page<T> {
    /// Builds a page from up to `size + 1` rows fetched for it: the extra row
    /// only shows that another page follows.
    pub fn from_rows(mut rows: Vec<T>, size: usize) -> Self {
        let has_more = rows.len() > size;
        rows.truncate(size);
        Self {
            items: rows,
            has_more,
        }
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            has_more: self.has_more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_says_whether_more_rows_follow() {
        let page = Page::from_rows(vec![1, 2, 3], 2);
        assert_eq!((page.items, page.has_more), (vec![1, 2], true));
        let last = Page::from_rows(vec![1, 2], 2);
        assert_eq!((last.items, last.has_more), (vec![1, 2], false));
    }

    #[test]
    fn a_cursor_needs_all_of_its_fields() {
        let at = Utc::now();
        assert!(PageQuery::default().before().unwrap().is_none());
        assert!(
            PageQuery {
                before_time: Some(at),
                before_id: None
            }
            .before()
            .is_err()
        );
        assert!(
            HandlerPageQuery {
                before_time: Some(at),
                before_id: Some(Uuid::nil()),
                before_handler: None,
            }
            .before()
            .is_err()
        );
    }
}
