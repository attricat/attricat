//! Polling cadence shared by the leader-elected background coordinators.

use std::time::Duration;

/// How often a replica that is not the coordinator leader rechecks leadership.
pub const STANDBY_POLL: Duration = Duration::from_secs(1);
/// The pause between a leader's coordination passes.
pub const IDLE_POLL: Duration = Duration::from_millis(250);
/// The pause after a leader cannot discover the workspaces to coordinate.
pub const DISCOVERY_ERROR_BACKOFF: Duration = Duration::from_secs(5);
