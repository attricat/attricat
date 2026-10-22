//! Typed outcomes of event handlers and operation batches (`attricat:host@1.1`).
//!
//! A 1.0 handler can only succeed or fail, and every failure quarantines the
//! installation. A 1.1 handler distinguishes a record-level rejection (recorded,
//! never retried), a temporary failure (retried with backoff) and a component
//! fault (quarantine). Traps, fuel or memory exhaustion and timeouts are faults.

use std::time::Duration;

use uuid::Uuid;

use super::unified::host_unified::attricat::host::outcomes as wit;

/// The longest rejection code a component may report.
const MAX_REJECTION_CODE_BYTES: usize = 128;
/// The longest outcome message kept from a component.
const MAX_OUTCOME_MESSAGE_BYTES: usize = 2048;
/// The longest delay a component may request before a retry or resumption.
pub(super) const MAX_COMPONENT_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

/// A record-level failure reported by a component.
#[derive(Clone, Debug)]
pub struct Rejection {
    pub code: String,
    pub message: String,
    pub record_id: Option<Uuid>,
    pub connection: Option<String>,
}

/// What one handler invocation, or one operation batch, ended with.
#[derive(Clone, Debug)]
pub enum InvocationFailure {
    Rejected(Rejection),
    Retry {
        message: String,
        after: Option<Duration>,
    },
    Fault(String),
}

impl InvocationFailure {
    pub fn message(&self) -> &str {
        match self {
            Self::Rejected(rejection) => &rejection.message,
            Self::Retry { message, .. } | Self::Fault(message) => message,
        }
    }
}

/// Truncates component text at a character boundary.
pub(super) fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

/// A component-requested delay, capped at [`MAX_COMPONENT_DELAY`].
pub(super) fn component_delay(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds).min(MAX_COMPONENT_DELAY)
}

impl From<wit::Failure> for InvocationFailure {
    fn from(failure: wit::Failure) -> Self {
        match failure {
            wit::Failure::Rejected(rejection) => {
                let code = bounded_text(&rejection.code, MAX_REJECTION_CODE_BYTES);
                Self::Rejected(Rejection {
                    code: if code.is_empty() {
                        "rejected".into()
                    } else {
                        code
                    },
                    message: bounded_text(&rejection.message, MAX_OUTCOME_MESSAGE_BYTES),
                    // An unparseable record ID is the component's mistake; the
                    // rejection is still recorded, without a record status.
                    record_id: rejection.record_id.and_then(|id| id.parse().ok()),
                    connection: rejection
                        .connection
                        .filter(|connection| !connection.is_empty()),
                })
            }
            wit::Failure::Retry(retry) => Self::Retry {
                message: bounded_text(&retry.message, MAX_OUTCOME_MESSAGE_BYTES),
                after: retry.after_ms.map(component_delay),
            },
            wit::Failure::Fault(message) => {
                Self::Fault(bounded_text(&message, MAX_OUTCOME_MESSAGE_BYTES))
            }
        }
    }
}

/// The exponential backoff of an attempt (1-based) without a requested delay:
/// 5 s, 10 s, 20 s, ... capped at one hour.
pub(super) fn retry_backoff(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(10);
    Duration::from_secs(5 * (1u64 << exponent)).min(Duration::from_secs(60 * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(retry_backoff(1), Duration::from_secs(5));
        assert_eq!(retry_backoff(2), Duration::from_secs(10));
        assert_eq!(retry_backoff(30), Duration::from_secs(3600));
    }

    #[test]
    fn component_failures_are_bounded() {
        let failure = InvocationFailure::from(wit::Failure::Rejected(wit::Rejection {
            code: String::new(),
            message: "é".repeat(2000),
            record_id: Some("not-a-uuid".into()),
            connection: Some(String::new()),
        }));
        let InvocationFailure::Rejected(rejection) = failure else {
            panic!("rejection expected");
        };
        assert_eq!(rejection.code, "rejected");
        assert!(rejection.message.len() <= MAX_OUTCOME_MESSAGE_BYTES);
        assert!(rejection.record_id.is_none() && rejection.connection.is_none());
        let InvocationFailure::Retry { after, .. } =
            InvocationFailure::from(wit::Failure::Retry(wit::Retry {
                message: "busy".into(),
                after_ms: Some(u64::MAX),
            }))
        else {
            panic!("retry expected");
        };
        assert_eq!(after, Some(MAX_COMPONENT_DELAY));
    }
}
