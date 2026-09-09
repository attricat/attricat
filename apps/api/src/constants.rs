/// Defaults for bounded request expansion and pagination.
pub const DEFAULT_PREVIEW_RELATIONSHIP_DEPTH: u8 = 3;
pub const DEFAULT_PREVIEW_RELATIONSHIP_ITEMS: u32 = 10;
pub const DEFAULT_ENTITY_PAGE_SIZE: u32 = 100;
pub const DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE: u32 = 50;
pub const DEFAULT_RELATIONSHIP_FACET_NODES: u32 = 100;
pub const DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS: u64 = 300;
pub const DEFAULT_PAGE_SIZE: u32 = 20;
pub const DEFAULT_LIST_PAGE_SIZE: u32 = 50;
pub const DEFAULT_STALE_AFTER_DAYS: u16 = 90;
pub const MAX_STALE_AFTER_DAYS: u16 = 3650;
pub const DEFAULT_FILE_UPLOAD_MAX_BYTES: usize = 50 * 1024 * 1024;
pub const DEFAULT_FILE_UPLOAD_MAX_FILES: usize = 10;
pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: usize = 30;
pub const DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS: usize = 256;
pub const DEFAULT_HTTP_DEFAULT_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const SECONDS_PER_HOUR: i64 = 60 * 60;

/// Request pools are workspace-scoped, so keep this bounded for the database.
pub const MAINTENANCE_POOL_CONNECTIONS: u32 = 1;
pub const REQUEST_POOL_CONNECTIONS: u32 = 10;

pub const SESSION_COOKIE: &str = "catalog_session";
pub const CSRF_COOKIE: &str = "catalog_csrf";
pub const SESSION_LIFETIME_HOURS: i64 = 8;
/// Conversation attachments are reclaimed unless a message claims them promptly.
pub const CONVERSATION_ATTACHMENT_LIFETIME_SECONDS: i64 = 15 * 60;
