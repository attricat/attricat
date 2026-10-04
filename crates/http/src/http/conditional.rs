//! Conditional GET support for immutable or version-keyed representations.
//!
//! Handlers authorize the request and resolve the entity tag from database
//! metadata first; a matching `If-None-Match` then returns `304 Not Modified`
//! without touching object storage.

use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};

/// Private caching for a representation whose URL never changes content.
pub(super) const IMMUTABLE: &str = "private, max-age=31536000, immutable";
/// Private caching for a representation that must be revalidated each use.
pub(super) const REVALIDATE: &str = "private, no-cache";

/// A strong entity tag value, quoted.
pub(super) fn entity_tag(value: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!("\"{value}\"")).ok()
}

/// RFC 9110 `If-None-Match` evaluation (weak comparison).
pub(super) fn not_modified(request: &HeaderMap, etag: &HeaderValue) -> bool {
    let Ok(etag) = etag.to_str() else {
        return false;
    };
    let opaque = |tag: &str| tag.trim().trim_start_matches("W/").to_owned();
    request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|candidate| candidate.trim() == "*" || opaque(candidate) == opaque(etag))
}

pub(super) fn not_modified_response(etag: HeaderValue, cache_control: &'static str) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NOT_MODIFIED;
    let headers = response.headers_mut();
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_none_match_uses_weak_comparison_lists_and_wildcards() {
        let etag = entity_tag("abc").unwrap();
        let request = |value: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static(value));
            headers
        };
        assert!(not_modified(&request("\"abc\""), &etag));
        assert!(not_modified(&request("W/\"abc\""), &etag));
        assert!(not_modified(&request("\"x\", \"abc\""), &etag));
        assert!(not_modified(&request("*"), &etag));
        assert!(!not_modified(&request("\"abcd\""), &etag));
        assert!(!not_modified(&HeaderMap::new(), &etag));
    }
}
