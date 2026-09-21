//! Persistence defaults and domain constants used by repository consumers.

pub use catalog_domain::constants::*;

/// Global request connections, shared by every workspace.
pub const REQUEST_POOL_CONNECTIONS: u32 = 10;
/// Global task/worker connections, shared by every workspace.
pub const TASK_POOL_CONNECTIONS: u32 = 10;
