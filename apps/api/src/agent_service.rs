//! Provider-independent agent domain helpers.
//!
//! Orchestration is intentionally deferred; this boundary owns the UTC cron
//! contract used by the future scheduler and keeps parsing out of HTTP routes.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;
use thiserror::Error;

pub fn next_utc_schedule_run(
    expression: &str,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, AgentScheduleError> {
    let schedule = Schedule::from_str(expression).map_err(|_| AgentScheduleError::InvalidCron)?;
    schedule
        .after(&after)
        .next()
        .ok_or(AgentScheduleError::NoFutureOccurrence)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentScheduleError {
    #[error("cron expression is invalid")]
    InvalidCron,
    #[error("cron expression has no future UTC occurrence")]
    NoFutureOccurrence,
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::next_utc_schedule_run;

    #[test]
    fn computes_a_utc_cron_occurrence() {
        let after = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(
            next_utc_schedule_run("0 */5 * * * *", after).unwrap(),
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 5, 0).unwrap()
        );
    }
}
