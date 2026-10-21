//! HTTP transport defaults and domain constants used by route consumers.

pub use attricat_domain::constants::*;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: usize = 30;
pub const DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS: usize = 256;
pub const DEFAULT_HTTP_DEFAULT_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_HTTP_MAX_EVENT_STREAMS: usize = 128;
pub const DEFAULT_HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL: usize = 4;
pub const DEFAULT_HTTP_EVENT_STREAM_LIFETIME_SECONDS: u64 = 15 * 60;
/// Concurrent readiness probes; further probes report not ready at once.
pub const READINESS_PROBE_PERMITS: usize = 2;
