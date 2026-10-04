//! Per-workspace retry backoff for schedule coordinator preparation.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// The delay after a workspace's first failed preparation.
const INITIAL_DELAY: Duration = Duration::from_secs(5);
/// The longest delay between preparation attempts.
const MAX_DELAY: Duration = Duration::from_secs(5 * 60);

/// Spaces out a coordinator's retries of a workspace whose preparation
/// (task backfill plus a full schedule pass) keeps failing, so one broken
/// workspace is not retried on every tick. The delay starts at 5 s, doubles
/// on every further failure up to 5 min, and resets on success.
#[derive(Default)]
pub(crate) struct PreparationBackoff {
    failures: HashMap<Uuid, (Duration, Instant)>,
}

impl PreparationBackoff {
    /// Whether the workspace's preparation may be attempted at `now`.
    pub(crate) fn ready(&self, workspace: Uuid, now: Instant) -> bool {
        self.failures
            .get(&workspace)
            .is_none_or(|(delay, failed_at)| now.duration_since(*failed_at) >= *delay)
    }

    /// Records a failed attempt and lengthens the workspace's delay.
    pub(crate) fn failed(&mut self, workspace: Uuid, now: Instant) {
        let delay = self
            .failures
            .get(&workspace)
            .map_or(INITIAL_DELAY, |(delay, _)| (*delay * 2).min(MAX_DELAY));
        self.failures.insert(workspace, (delay, now));
    }

    /// Clears the workspace's delay after a successful attempt.
    pub(crate) fn succeeded(&mut self, workspace: Uuid) {
        self.failures.remove(&workspace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_double_the_delay_up_to_the_cap_and_success_resets_it() {
        let workspace = Uuid::new_v4();
        let other = Uuid::new_v4();
        let start = Instant::now();
        let mut backoff = PreparationBackoff::default();
        assert!(backoff.ready(workspace, start));

        backoff.failed(workspace, start);
        assert!(!backoff.ready(workspace, start + Duration::from_secs(4)));
        assert!(backoff.ready(workspace, start + Duration::from_secs(5)));
        assert!(backoff.ready(other, start), "backoff is per workspace");

        let mut now = start;
        let mut delay = INITIAL_DELAY;
        for _ in 0..10 {
            now += delay;
            backoff.failed(workspace, now);
            delay = (delay * 2).min(MAX_DELAY);
            assert!(!backoff.ready(workspace, now + delay - Duration::from_millis(1)));
            assert!(backoff.ready(workspace, now + delay));
        }
        assert_eq!(delay, MAX_DELAY);

        backoff.succeeded(workspace);
        assert!(backoff.ready(workspace, now));
        backoff.failed(workspace, now);
        assert!(backoff.ready(workspace, now + INITIAL_DELAY));
    }
}
