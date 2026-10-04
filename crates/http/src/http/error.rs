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
    #[cfg(test)]
    pub(super) fn status(&self) -> StatusCode {
        self.status
    }
    pub(super) fn unauthenticated() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthenticated",
            message: "authentication is required".to_owned(),
            details: None,
        }
    }
    pub(super) fn csrf_failed() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "csrf_failed",
            message: "a valid CSRF token is required".to_owned(),
            details: None,
        }
    }
    pub(super) fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited",
            message: "too many login attempts; try again later".to_owned(),
            details: None,
        }
    }
    pub(super) fn invalid_credentials() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "invalid_credentials",
            message: "invalid email or password".to_owned(),
            details: None,
        }
    }
    pub(super) fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "you are not authorized to perform this action".to_owned(),
            details: None,
        }
    }
    pub(super) fn not_found(resource: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{resource} was not found"),
            details: None,
        }
    }
    pub(super) fn service_unavailable(message: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "service_unavailable",
            message: message.to_owned(),
            details: None,
        }
    }
    /// A handler exceeded the server-side request deadline. This is not a 408:
    /// the client sent its request in time, and some clients automatically
    /// replay 408 responses, which is unsafe for partially applied mutations.
    pub(super) fn request_timeout() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "request_timeout",
            message: "request exceeded the server time limit".to_owned(),
            details: None,
        }
    }
    /// An upstream dependency (e.g. the LLM provider) returned an unusable
    /// response to a valid request.
    pub(super) fn bad_gateway(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "bad_gateway",
            message: message.to_owned(),
            details: None,
        }
    }
    pub(super) fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.to_owned(),
            details: None,
        }
    }
    pub(super) fn invalid_search_query(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_input",
            message,
            details: None,
        }
    }
    pub(super) fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: message.into(),
            details: None,
        }
    }

    pub(super) fn invalid_input(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_input",
            message,
            details: None,
        }
    }
    pub(super) fn relationship_sort_requires_single_version() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "relationship_path_sort_requires_single_result_version",
            message: "related-value sorting requires results from one blueprint version".to_owned(),
            details: None,
        }
    }
    pub(super) fn storage_conflict() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "storage_conflict",
            message: "storage revision conflict".to_owned(),
            details: None,
        }
    }
    pub(super) fn storage_quota_exceeded() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "storage_quota_exceeded",
            message: "extension storage quota exceeded".to_owned(),
            details: None,
        }
    }
    pub(super) fn payload_too_large() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "payload_too_large",
            message: "request body exceeds the configured size limit".to_owned(),
            details: None,
        }
    }
    pub(super) fn file_too_large() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "file_too_large",
            message: "file exceeds the configured size limit".to_owned(),
            details: None,
        }
    }
    pub(super) fn file_count_exceeded() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "file_count_exceeded",
            message: "request exceeds the configured file count limit".to_owned(),
            details: None,
        }
    }
    pub(super) fn unsupported_media_type() -> Self {
        Self {
            status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            code: "unsupported_media_type",
            message: "file type is not allowed".to_owned(),
            details: None,
        }
    }
    pub(super) fn invalid_file(message: &'static str) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_file",
            message: message.to_owned(),
            details: None,
        }
    }
    pub(super) fn invalid_range() -> Self {
        Self {
            status: StatusCode::RANGE_NOT_SATISFIABLE,
            code: "invalid_range",
            message: "requested byte range is not satisfiable".to_owned(),
            details: None,
        }
    }
    pub(super) fn file_processing() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "file_processing",
            message: "file is not available for download".to_owned(),
            details: None,
        }
    }
    pub(super) fn storage_unavailable() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "storage_unavailable",
            message: "object storage is unavailable".to_owned(),
            details: None,
        }
    }
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.to_owned(),
            details: None,
        }
    }
    pub(super) fn from_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;
        match rejection {
            JsonRejection::JsonDataError(_) => Self::invalid_input(
                "request body does not match the expected JSON shape".to_owned(),
            ),
            JsonRejection::MissingJsonContentType(_) => Self {
                status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
                code: "unsupported_media_type",
                message: "request body must be `application/json`".to_owned(),
                details: None,
            },
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
