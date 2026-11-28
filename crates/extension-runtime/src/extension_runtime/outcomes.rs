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
/// The first retry delay, doubled for each later attempt.
const RETRY_BACKOFF_BASE: Duration = Duration::from_secs(5);
/// The longest delay between retries without a requested delay.
const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(60 * 60);

/// A record-level failure reported by a component.
#[derive(Clone, Debug)]
pub struct Rejection {
    pub code: String,
    pub message: String,
    pub record_id: Option<Uuid>,
    pub connection: Option<String>,
}

impl Rejection {
    /// The diagnostic a run keeps for this rejection. Stored run errors keep
    /// only structured JSON (redacted), so the code goes in as a field beside
    /// the message's (see [`message_fields`]). A code that is not a plain
    /// identifier is left out.
    pub fn diagnostic(&self) -> String {
        let mut diagnostic = message_fields(&self.message);
        if is_identifier(&self.code) {
            diagnostic.insert(
                REJECTION_CODE_FIELD.into(),
                serde_json::Value::String(self.code.clone()),
            );
        }
        serde_json::Value::Object(diagnostic).to_string()
    }
}

use crate::repository::{
    RUN_DIAGNOSTIC_CODE_FIELD as REJECTION_CODE_FIELD,
    RUN_DIAGNOSTIC_MESSAGE_FIELD as MESSAGE_FIELD,
};

/// The diagnostic a run keeps for a temporary failure's message.
pub fn retry_diagnostic(message: &str) -> String {
    serde_json::Value::Object(message_fields(message)).to_string()
}

/// A typed failure's message as diagnostic fields: a JSON object keeps its
/// own fields, and text goes in [`MESSAGE_FIELD`]. The component wrote the
/// text for people to read, unlike a trap's, so it is kept; the run store
/// still redacts sensitive field names.
fn message_fields(message: &str) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(message) {
        Ok(serde_json::Value::Object(fields)) => fields,
        _ if message.trim().is_empty() => serde_json::Map::new(),
        _ => serde_json::Map::from_iter([(
            MESSAGE_FIELD.to_owned(),
            serde_json::Value::String(message.to_owned()),
        )]),
    }
}

fn is_identifier(code: &str) -> bool {
    !code.is_empty()
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
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

/// The code of a rejection whose component gave none, or an unusable one.
const DEFAULT_REJECTION_CODE: &str = "rejected";

/// A 1.3 command's refusal, for the person who invoked the command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRejection {
    pub code: String,
    pub message: String,
}

impl CommandRejection {
    /// Bounds a component's rejection like any other outcome it reports.
    pub(super) fn new(code: &str, message: &str) -> Self {
        let code = bounded_text(code, MAX_REJECTION_CODE_BYTES);
        Self {
            code: if is_identifier(&code) {
                code
            } else {
                DEFAULT_REJECTION_CODE.into()
            },
            message: bounded_text(message, MAX_OUTCOME_MESSAGE_BYTES),
        }
    }
}

/// Truncates component text at a character boundary.
pub(super) fn bounded_text(value: &str, max_bytes: usize) -> String {
    value[..value.floor_char_boundary(max_bytes)].to_owned()
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
                        DEFAULT_REJECTION_CODE.into()
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
    (RETRY_BACKOFF_BASE * (1u32 << exponent)).min(MAX_RETRY_BACKOFF)
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

    fn rejection(code: &str, message: &str) -> Rejection {
        Rejection {
            code: code.into(),
            message: message.into(),
            record_id: None,
            connection: None,
        }
    }

    #[test]
    fn rejection_diagnostics_keep_the_code_and_message() {
        assert_eq!(
            rejection("invalid_checkpoint", "invalid checkpoint").diagnostic(),
            r#"{"message":"invalid checkpoint","rejection_code":"invalid_checkpoint"}"#
        );
        assert_eq!(
            rejection("invalid_checkpoint", " ").diagnostic(),
            r#"{"rejection_code":"invalid_checkpoint"}"#
        );
        // A structured message keeps its fields; the code is the host's.
        assert_eq!(
            rejection("template_changed", r#"{"version":2,"rejection_code":"x"}"#).diagnostic(),
            r#"{"version":2,"rejection_code":"template_changed"}"#
        );
        assert_eq!(rejection("token=secret value", "").diagnostic(), "{}");
    }

    #[test]
    fn command_rejections_are_bounded_and_keep_a_usable_code() {
        assert_eq!(
            CommandRejection::new("client_rejected", "NIP is invalid"),
            CommandRejection {
                code: "client_rejected".into(),
                message: "NIP is invalid".into()
            }
        );
        assert_eq!(
            CommandRejection::new("token=abc", "x").code,
            DEFAULT_REJECTION_CODE
        );
        assert_eq!(CommandRejection::new("", "x").code, DEFAULT_REJECTION_CODE);
        assert_eq!(
            CommandRejection::new("c", &"é".repeat(MAX_OUTCOME_MESSAGE_BYTES))
                .message
                .len(),
            MAX_OUTCOME_MESSAGE_BYTES
        );
    }

    #[test]
    fn retry_diagnostics_keep_the_message() {
        assert_eq!(
            retry_diagnostic("Fakturownia is unavailable"),
            r#"{"message":"Fakturownia is unavailable"}"#
        );
        assert_eq!(retry_diagnostic(r#"{"status":503}"#), r#"{"status":503}"#);
        assert_eq!(retry_diagnostic(""), "{}");
    }
}
