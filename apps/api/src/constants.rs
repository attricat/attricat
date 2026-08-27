/// Defaults for bounded request expansion and pagination.
pub const DEFAULT_PREVIEW_RELATIONSHIP_DEPTH: u8 = 3;
pub const DEFAULT_PREVIEW_RELATIONSHIP_ITEMS: u32 = 10;
pub const DEFAULT_ENTITY_PAGE_SIZE: u32 = 100;
pub const DEFAULT_INCOMING_RELATIONSHIP_PAGE_SIZE: u32 = 50;
pub const DEFAULT_RELATIONSHIP_FACET_NODES: u32 = 100;
pub const DEFAULT_DATA_HEALTH_CACHE_TTL_SECONDS: u64 = 300;

/// Connection pools are deliberately small: request pools are workspace-scoped.
pub const MAINTENANCE_POOL_CONNECTIONS: u32 = 1;
pub const REQUEST_POOL_CONNECTIONS: u32 = 5;

pub const SESSION_COOKIE: &str = "catalog_session";
pub const CSRF_COOKIE: &str = "catalog_csrf";
pub const SESSION_LIFETIME_HOURS: i64 = 8;
