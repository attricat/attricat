//! Persistence defaults and domain constants used by repository consumers.

pub use attricat_domain::constants::*;

/// Password sign-in attempts per account in a rate-limit window. Wrong
/// current passwords in a password change count too, and a successful
/// sign-in starts the count again.
pub const LOGIN_ATTEMPTS_PER_WINDOW: i32 = 20;
/// Global request connections, shared by every workspace.
pub const REQUEST_POOL_CONNECTIONS: u32 = 10;
/// Global task/worker connections, shared by every workspace.
pub const TASK_POOL_CONNECTIONS: u32 = 10;
