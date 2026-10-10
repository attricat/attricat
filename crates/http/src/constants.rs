//! HTTP transport defaults and domain constants used by route consumers.

pub use attricat_domain::constants::*;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: usize = 30;
pub const DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS: usize = 256;
pub const DEFAULT_HTTP_DEFAULT_BODY_BYTES: usize = 2 * 1024 * 1024;
