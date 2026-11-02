//! Bounds what a multipart parser has to hold in memory. Upload routes stream
//! file parts to disk with no overall body limit, but the parser buffers
//! everything up to the first boundary and each part's headers, so a body that
//! never completes them would otherwise be held in memory as it arrives.

use axum::{
    body::Body, extract::Request, http::header::CONTENT_TYPE, middleware::Next, response::Response,
};
use futures_util::StreamExt;

/// The most bytes allowed before a part's headers end: the preamble, boundary
/// and headers together. Real clients send well under a kilobyte.
const MAX_MULTIPART_FRAMING_BYTES: usize = 64 * 1024;
const HEADERS_END: &[u8] = b"\r\n\r\n";

pub(super) async fn limit_multipart_framing(request: Request, next: Next) -> Response {
    let Some(boundary) = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(boundary)
    else {
        // Not multipart, so the extractor rejects it without parsing the body.
        return next.run(request).await;
    };
    let mut scanner = FramingScanner::new(&boundary);
    let (parts, body) = request.into_parts();
    let body = Body::from_stream(body.into_data_stream().map(move |chunk| {
        let chunk = chunk?;
        if scanner.feed(&chunk) {
            Ok(chunk)
        } else {
            Err(axum::Error::new("multipart framing is too long"))
        }
    }));
    next.run(Request::from_parts(parts, body)).await
}

fn boundary(content_type: &str) -> Option<String> {
    let value = content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("boundary")
            .then_some(value)
    })?;
    let value = value.trim().trim_matches('"');
    (!value.is_empty()).then(|| value.to_owned())
}

/// Where a multipart body is, which decides what ends this stretch of it.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    /// Before the first boundary, which the parser buffers everything up to.
    Preamble,
    /// After a boundary, up to the end of the part's headers.
    Headers,
    /// A part's content, which the parser streams out.
    Content,
}

/// Follows a multipart body across chunks.
struct FramingScanner {
    first_boundary: Vec<u8>,
    delimiter: Vec<u8>,
    position: Position,
    framing_bytes: usize,
    /// The end of the previous chunk, which can hold the start of a marker.
    tail: Vec<u8>,
}

impl FramingScanner {
    fn new(boundary: &str) -> Self {
        Self {
            first_boundary: format!("--{boundary}").into_bytes(),
            delimiter: format!("\r\n--{boundary}").into_bytes(),
            position: Position::Preamble,
            framing_bytes: 0,
            tail: Vec::new(),
        }
    }

    /// Takes the next chunk and returns whether the framing is within its limit.
    fn feed(&mut self, chunk: &[u8]) -> bool {
        let mut data = std::mem::take(&mut self.tail);
        // Bytes carried over were counted with the chunk they came in.
        let carried = data.len();
        data.extend_from_slice(chunk);
        let mut start = 0;
        loop {
            let marker: &[u8] = match self.position {
                Position::Preamble => &self.first_boundary,
                Position::Headers => HEADERS_END,
                Position::Content => &self.delimiter,
            };
            let found = data[start..]
                .windows(marker.len())
                .position(|window| window == marker);
            let end = found.map_or(data.len(), |found| start + found + marker.len());
            if self.position != Position::Content {
                self.framing_bytes += end.saturating_sub(start.max(carried));
                if self.framing_bytes > MAX_MULTIPART_FRAMING_BYTES {
                    return false;
                }
            }
            if found.is_none() {
                let keep_from = data.len().saturating_sub(marker.len() - 1).max(start);
                self.tail = data[keep_from..].to_vec();
                return true;
            }
            start = end;
            self.position = match self.position {
                Position::Preamble | Position::Content => Position::Headers,
                Position::Headers => {
                    self.framing_bytes = 0;
                    Position::Content
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDARY: &str = "XBOUNDARYX";

    fn over_limit() -> Vec<u8> {
        vec![b'a'; MAX_MULTIPART_FRAMING_BYTES + 1]
    }

    #[test]
    fn reads_the_boundary_from_the_content_type() {
        assert_eq!(
            boundary("multipart/form-data; boundary=x").as_deref(),
            Some("x")
        );
        assert_eq!(
            boundary("multipart/form-data; boundary=\"a b\"; charset=utf-8").as_deref(),
            Some("a b")
        );
        assert_eq!(boundary("application/json"), None);
    }

    #[test]
    fn the_boundary_parameter_name_ignores_case_and_needs_its_own_parameter() {
        assert_eq!(
            boundary("multipart/form-data; BOUNDARY=x").as_deref(),
            Some("x")
        );
        assert_eq!(boundary("multipart/form-data; xboundary=x"), None);
    }

    #[test]
    fn a_blank_line_before_the_first_boundary_does_not_end_the_limit() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        assert!(scanner.feed(b"\r\n\r\n"));
        assert!(!scanner.feed(&over_limit()));
    }

    #[test]
    fn a_body_that_never_starts_a_part_is_refused() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        assert!(!scanner.feed(&over_limit()));
    }

    #[test]
    fn framing_is_counted_across_chunks() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        let chunk = vec![b'a'; MAX_MULTIPART_FRAMING_BYTES / 2 + 1];
        assert!(scanner.feed(&chunk));
        assert!(!scanner.feed(&chunk));
    }

    #[test]
    fn file_content_of_any_size_is_allowed_and_the_next_headers_are_limited() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        assert!(scanner.feed(b"--XBOUNDARYX\r\nContent-Disposition: form-data\r\n\r\n"));
        for _ in 0..4 {
            assert!(scanner.feed(&over_limit()));
        }
        assert!(scanner.feed(b"\r\n--XBOUNDARYX\r\n"));
        assert!(!scanner.feed(&over_limit()));
    }

    #[test]
    fn markers_split_across_chunks_are_found() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        assert!(scanner.feed(b"--XBOUND"));
        assert!(scanner.feed(b"ARYX\r\nName: x\r\n\r"));
        assert!(scanner.feed(b"\n"));
        assert!(scanner.feed(&over_limit()));
        assert!(scanner.feed(b"\r\n--XBOUND"));
        assert!(scanner.feed(b"ARYX\r\n"));
        assert!(!scanner.feed(&over_limit()));
    }

    #[test]
    fn a_complete_body_with_a_closing_boundary_is_allowed() {
        let mut scanner = FramingScanner::new(BOUNDARY);
        assert!(scanner.feed(b"--XBOUNDARYX\r\nName: x\r\n\r\ncontent\r\n--XBOUNDARYX--\r\n"));
    }
}
