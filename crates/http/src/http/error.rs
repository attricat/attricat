use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::repository::{ErrorClass, ErrorCode, RepositoryError};

#[derive(Debug)]
pub(super) struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
    /// Machine-readable context for clients that recover from the error,
    /// such as the record holding a conflicting unique key.
    details: Option<serde_json::Value>,
}
impl ApiError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }
    #[cfg(test)]
    pub(super) fn status(&self) -> StatusCode {
        self.status
    }
    pub(super) fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthenticated,
            "authentication is required",
        )
    }
    pub(super) fn csrf_failed() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ErrorCode::CsrfFailed,
            "a valid CSRF token is required",
        )
    }
    pub(super) fn rate_limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "too many login attempts; try again later",
        )
    }
    pub(super) fn invalid_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidCredentials,
            "invalid email or password",
        )
    }
    pub(super) fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "you are not authorized to perform this action",
        )
    }
    /// The shared demo accounts must keep working for every visitor.
    pub(super) fn disabled_in_demo() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            ErrorCode::DisabledInDemo,
            "this action is disabled in the demo",
        )
    }
    pub(super) fn not_found(resource: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            format!("{resource} was not found"),
        )
    }
    pub(super) fn service_unavailable(message: &'static str) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::ServiceUnavailable,
            message,
        )
    }
    /// A handler exceeded the server-side request deadline. This is not a 408:
    /// the client sent its request in time, and some clients automatically
    /// replay 408 responses, which is unsafe for partially applied mutations.
    pub(super) fn request_timeout() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::RequestTimeout,
            "request exceeded the server time limit",
        )
    }
    /// An upstream dependency (e.g. the LLM provider) returned an unusable
    /// response to a valid request.
    pub(super) fn bad_gateway(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, ErrorCode::BadGateway, message)
    }
    pub(super) fn internal(message: &'static str) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::InternalError,
            message,
        )
    }
    pub(super) fn invalid_search_query(message: String) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput, message)
    }
    pub(super) fn conflict(message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, ErrorCode::Conflict, message)
    }
    pub(super) fn invalid_input(message: String) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidInput,
            message,
        )
    }
    /// A 422 with its own code, for failures the web UI can trigger and
    /// translates by code.
    pub(super) fn unprocessable(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, message)
    }
    /// Structured context a client needs to explain or recover from the error.
    pub(super) fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    pub(super) fn password_reset_invalid() -> Self {
        Self::unprocessable(
            ErrorCode::PasswordResetInvalid,
            "password reset link is invalid or expired",
        )
    }
    pub(super) fn password_policy(error: attricat_domain::account::PasswordPolicyError) -> Self {
        use attricat_domain::account::{
            MAXIMUM_PASSWORD_BYTES, MINIMUM_PASSWORD_LENGTH, PasswordPolicyError,
        };
        let message = error.to_string();
        match error {
            PasswordPolicyError::TooShort => {
                Self::unprocessable(ErrorCode::PasswordTooShort, message)
                    .with_details(serde_json::json!({ "min": MINIMUM_PASSWORD_LENGTH }))
            }
            PasswordPolicyError::TooLong => {
                Self::unprocessable(ErrorCode::PasswordTooLong, message)
                    .with_details(serde_json::json!({ "max": MAXIMUM_PASSWORD_BYTES }))
            }
            PasswordPolicyError::MissingLetter => {
                Self::unprocessable(ErrorCode::PasswordMissingLetter, message)
            }
            PasswordPolicyError::MissingNumber => {
                Self::unprocessable(ErrorCode::PasswordMissingNumber, message)
            }
            PasswordPolicyError::MissingSymbol => {
                Self::unprocessable(ErrorCode::PasswordMissingSymbol, message)
            }
        }
    }
    /// Text outside `min..=max` characters.
    pub(super) fn text_length(label: &str, min: usize, max: usize) -> Self {
        Self::unprocessable(
            ErrorCode::TextLengthInvalid,
            format!("{label} must be {min}-{max} characters"),
        )
        .with_details(serde_json::json!({ "field": label, "min": min, "max": max }))
    }
    pub(super) fn relationship_sort_requires_single_version() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::RelationshipPathSortRequiresSingleResultVersion,
            "related-value sorting requires results from one blueprint version",
        )
    }
    pub(super) fn storage_conflict() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::StorageConflict,
            "storage revision conflict",
        )
    }
    pub(super) fn storage_quota_exceeded() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::StorageQuotaExceeded,
            "extension storage quota exceeded",
        )
    }
    pub(super) fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::PayloadTooLarge,
            "request body exceeds the configured size limit",
        )
    }
    pub(super) fn file_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::FileTooLarge,
            "file exceeds the configured size limit",
        )
    }
    pub(super) fn file_count_exceeded() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::FileCountExceeded,
            "request exceeds the configured file count limit",
        )
    }
    pub(super) fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::UnsupportedMediaType,
            "file type is not allowed",
        )
    }
    pub(super) fn invalid_file(message: &'static str) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidFile,
            message,
        )
    }
    pub(super) fn invalid_range() -> Self {
        Self::new(
            StatusCode::RANGE_NOT_SATISFIABLE,
            ErrorCode::InvalidRange,
            "requested byte range is not satisfiable",
        )
    }
    pub(super) fn file_processing() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::FileProcessing,
            "file is not available for download",
        )
    }
    pub(super) fn storage_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::StorageUnavailable,
            "object storage is unavailable",
        )
    }
    fn bad_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message)
    }
    pub(super) fn from_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;
        match rejection {
            JsonRejection::JsonDataError(_) => Self::invalid_input(
                "request body does not match the expected JSON shape".to_owned(),
            ),
            JsonRejection::MissingJsonContentType(_) => Self::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ErrorCode::UnsupportedMediaType,
                "request body must be `application/json`",
            ),
            JsonRejection::BytesRejection(rejection) => Self::from_bytes_rejection(rejection),
            _ => Self::bad_request("request body is malformed"),
        }
    }
    pub(super) fn from_bytes_rejection(
        rejection: axum::extract::rejection::BytesRejection,
    ) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::payload_too_large()
        } else {
            Self::bad_request("request body is malformed")
        }
    }
    pub(super) fn from_path_rejection(_: axum::extract::rejection::PathRejection) -> Self {
        Self::bad_request("path parameters are invalid")
    }
    pub(super) fn from_query_rejection(_: axum::extract::rejection::QueryRejection) -> Self {
        Self::bad_request("query parameters are invalid")
    }
}
impl From<RepositoryError> for ApiError {
    /// Uses the repository's single code mapping and adds the HTTP status
    /// for its class.
    fn from(error: RepositoryError) -> Self {
        let description = error.describe();
        let status = match description.class {
            ErrorClass::Forbidden => StatusCode::FORBIDDEN,
            ErrorClass::NotFound => StatusCode::NOT_FOUND,
            ErrorClass::Conflict => StatusCode::CONFLICT,
            ErrorClass::Unprocessable => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorClass::PreconditionRequired => StatusCode::PRECONDITION_REQUIRED,
            ErrorClass::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorClass::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        };
        if status.is_server_error() {
            // Preserve the cause only for server errors, while keeping it out
            // of the HTTP response.
            tracing::error!(error = %error, "repository operation failed");
        }
        Self {
            status,
            code: description.code,
            message: description.message,
            details: description.details,
        }
    }
}
#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}
#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: ErrorCode,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(code = self.code.as_str(), message = %self.message, "API operation failed");
        }
        (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: &self.message,
                    details: self.details.as_ref(),
                },
            }),
        )
            .into_response()
    }
}
