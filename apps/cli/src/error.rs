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

    /// The API's error code, or `acli`'s own code for failures it detects.
    fn code(&self) -> &str {
        match self {
            Self::Input(_) => "invalid_input",
            Self::Transport(_) => "request_failed",
            Self::Api { code, .. } => code,
            Self::InvalidResponse => "invalid_response",
            Self::ResponseTooLarge => "response_too_large",
        }
    }

    pub(crate) fn json(&self) -> Value {
        let status = match self {
            Self::Api { status, .. } => json!(status),
            _ => Value::Null,
        };
        let mut error =
            json!({ "code": self.code(), "message": self.to_string(), "status": status });
        if let Self::Api {
            details: Some(details),
            ..
        } = self
        {
            error["details"] = details.clone();
        }
        json!({ "error": error })
    }
}
