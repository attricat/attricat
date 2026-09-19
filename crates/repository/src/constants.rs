//! Persistence defaults and domain constants used by repository consumers.

pub use catalog_domain::constants::*;

/// Request pools are workspace-scoped, so keep this bounded for the database.
pub const REQUEST_POOL_CONNECTIONS: u32 = 10;
