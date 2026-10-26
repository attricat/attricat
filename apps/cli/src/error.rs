//! Errors `acli` reports as JSON on standard error.

use serde_json::{Value, json};
use thiserror::Error;

use crate::transport::MAX_RESPONSE_BYTES;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    Transport(String),
    #[error("{message}")]
    Api {
        status: u16,
        code: String,
        message: String,
        /// The API's optional machine-readable `error.details`.
        details: Option<Value>,
    },
    #[error("server returned invalid JSON")]
    InvalidResponse,
    #[error("server response exceeded {MAX_RESPONSE_BYTES} bytes")]
    ResponseTooLarge,
}

impl CliError {
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Self::Input(_) => 2,
            Self::Transport(_) => 3,
            Self::Api { .. } => 4,
            Self::InvalidResponse | Self::ResponseTooLarge => 5,
        }
    }

    pub(crate) fn json(&self) -> Value {
        match self {
            Self::Input(message) => {
                json!({ "error": { "code": "invalid_input", "message": message, "status": null } })
            }
            Self::Transport(message) => {
                json!({ "error": { "code": "request_failed", "message": message, "status": null } })
            }
            Self::Api {
                status,
                code,
                message,
                details,
            } => {
                let mut error = json!({ "code": code, "message": message, "status": status });
                if let Some(details) = details {
                    error["details"] = details.clone();
                }
                json!({ "error": error })
            }
            Self::InvalidResponse => {
                json!({ "error": { "code": "invalid_response", "message": self.to_string(), "status": null } })
            }
            Self::ResponseTooLarge => {
                json!({ "error": { "code": "response_too_large", "message": self.to_string(), "status": null } })
            }
        }
    }
}
