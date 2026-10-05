use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::repository::{ErrorClass, RepositoryError};

#[derive(Debug)]
pub(super) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    /// Machine-readable context for clients that recover from the error,
    /// such as the entity holding a conflicting unique key.
    details: Option<serde_json::Value>,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
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
            "unauthenticated",
            "authentication is required",
        )
    }
    pub(super) fn csrf_failed() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "csrf_failed",
            "a valid CSRF token is required",
        )
    }
    pub(super) fn rate_limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many login attempts; try again later",
        )
    }
    pub(super) fn invalid_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "invalid email or password",
        )
    }
    pub(super) fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "you are not authorized to perform this action",
        )
    }
    /// The shared demo accounts must keep working for every visitor.
    pub(super) fn disabled_in_demo() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "disabled_in_demo",
            "this action is disabled in the demo",
        )
    }
    pub(super) fn not_found(resource: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{resource} was not found"),
        )
    }
    pub(super) fn service_unavailable(message: &'static str) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            message,
        )
    }
    /// A handler exceeded the server-side request deadline. This is not a 408:
    /// the client sent its request in time, and some clients automatically
    /// replay 408 responses, which is unsafe for partially applied mutations.
    pub(super) fn request_timeout() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "request_timeout",
            "request exceeded the server time limit",
        )
    }
    /// An upstream dependency (e.g. the LLM provider) returned an unusable
    /// response to a valid request.
    pub(super) fn bad_gateway(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, "bad_gateway", message)
    }
    pub(super) fn internal(message: &'static str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
    pub(super) fn invalid_search_query(message: String) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_input", message)
    }
    pub(super) fn conflict(message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }
    pub(super) fn invalid_input(message: String) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_input", message)
    }
    pub(super) fn relationship_sort_requires_single_version() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "relationship_path_sort_requires_single_result_version",
            "related-value sorting requires results from one blueprint version",
        )
    }
    pub(super) fn storage_conflict() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "storage_conflict",
            "storage revision conflict",
        )
    }
    pub(super) fn storage_quota_exceeded() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "storage_quota_exceeded",
            "extension storage quota exceeded",
        )
    }
    pub(super) fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "request body exceeds the configured size limit",
        )
    }
    pub(super) fn file_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
            "file exceeds the configured size limit",
        )
    }
    pub(super) fn file_count_exceeded() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_count_exceeded",
            "request exceeds the configured file count limit",
        )
    }
    pub(super) fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "file type is not allowed",
        )
    }
    pub(super) fn invalid_file(message: &'static str) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_file", message)
    }
    pub(super) fn invalid_range() -> Self {
        Self::new(
            StatusCode::RANGE_NOT_SATISFIABLE,
            "invalid_range",
            "requested byte range is not satisfiable",
        )
    }
    pub(super) fn file_processing() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "file_processing",
            "file is not available for download",
        )
    }
    pub(super) fn storage_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            "object storage is unavailable",
        )
    }
    fn bad_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }
    pub(super) fn from_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;
        match rejection {
            JsonRejection::JsonDataError(_) => Self::invalid_input(
                "request body does not match the expected JSON shape".to_owned(),
            ),
            JsonRejection::MissingJsonContentType(_) => Self::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
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
            code: description.code.as_str(),
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
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(code = self.code, message = %self.message, "API operation failed");
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
