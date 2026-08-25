use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::json;

use crate::repository::RepositoryError;

#[derive(Debug)]
pub(super) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl ApiError {
    pub(super) fn unauthenticated() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthenticated",
            message: "authentication is required".to_owned(),
        }
    }
    pub(super) fn csrf_failed() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "csrf_failed",
            message: "a valid CSRF token is required".to_owned(),
        }
    }
    pub(super) fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited",
            message: "too many login attempts; try again later".to_owned(),
        }
    }
    pub(super) fn invalid_credentials() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "invalid_credentials",
            message: "invalid email or password".to_owned(),
        }
    }
    pub(super) fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "you are not authorized to perform this action".to_owned(),
        }
    }
    pub(super) fn not_found(resource: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{resource} was not found"),
        }
    }
    pub(super) fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.to_owned(),
        }
    }
    pub(super) fn invalid_input(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_input",
            message,
        }
    }
    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.to_owned(),
        }
    }
    pub(super) fn from_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;
        match rejection {
            JsonRejection::JsonDataError(_) => Self::invalid_input(
                "request body does not match the expected JSON shape".to_owned(),
            ),
            JsonRejection::JsonSyntaxError(_)
            | JsonRejection::MissingJsonContentType(_)
            | JsonRejection::BytesRejection(_) => Self::bad_request("request body is malformed"),
            _ => Self::bad_request("request body is malformed"),
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
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound(resource) => Self::not_found(resource),
            RepositoryError::InvitationInvalid => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invitation_invalid",
                message: error.to_string(),
            },
            RepositoryError::AttributeNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_not_applicable",
                message: error.to_string(),
            },
            RepositoryError::AttributeKindMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_kind_mismatch",
                message: error.to_string(),
            },
            RepositoryError::AttributeValueTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_type_mismatch",
                message: error.to_string(),
            },
            RepositoryError::AttributeValueSchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_schema_mismatch",
                message: error.to_string(),
            },
            RepositoryError::EntitySchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "entity_schema_mismatch",
                message: error.to_string(),
            },
            RepositoryError::EntityBlueprintCurrent => Self {
                status: StatusCode::CONFLICT,
                code: "entity_blueprint_current",
                message: error.to_string(),
            },
            RepositoryError::MigrationTargetChanged => Self {
                status: StatusCode::CONFLICT,
                code: "migration_target_changed",
                message: error.to_string(),
            },
            RepositoryError::MigrationNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_not_applicable",
                message: error.to_string(),
            },
            RepositoryError::MigrationNeedsResolution(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_needs_resolution",
                message: error.to_string(),
            },
            RepositoryError::InvalidStoredAttributeValue => {
                Self::internal("stored attribute value is invalid")
            }
            RepositoryError::RelationshipTargetTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "relationship_target_type_mismatch",
                message: error.to_string(),
            },
            RepositoryError::InvalidPreview
            | RepositoryError::InvalidHierarchyRelationship
            | RepositoryError::ReservedContextCode
            | RepositoryError::InvalidCode
            | RepositoryError::InvalidContextData
            | RepositoryError::InvalidContext
            | RepositoryError::DefaultContextProtected
            | RepositoryError::ContextCycle
            | RepositoryError::ContextInUse
            | RepositoryError::DefaultContextOnly
            | RepositoryError::InvalidAttributeSelector => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_input",
                message: error.to_string(),
            },
            RepositoryError::InvalidBlueprintDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_blueprint_definition",
                message: error.to_string(),
            },
            RepositoryError::BlueprintCodeTaken => Self {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: error.to_string(),
            },
            RepositoryError::BlueprintNotPublished => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "blueprint_not_published",
                message: error.to_string(),
            },
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                Self {
                    status: StatusCode::CONFLICT,
                    code: "conflict",
                    message: "a record with the same unique value already exists".to_owned(),
                }
            }
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.code().as_deref() == Some("P0001")
                    && database_error.message().starts_with("actor may not") =>
            {
                Self::forbidden()
            }
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.code().as_deref() == Some("P0001")
                    && is_workspace_validation_error(database_error.message()) =>
            {
                Self::invalid_input(database_error.message().to_owned())
            }
            RepositoryError::Database(_) => Self::internal("database operation failed"),
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
}
fn is_workspace_validation_error(message: &str) -> bool {
    [
        "role does not belong",
        "role does not exist",
        "workspace-local role does not exist",
        "source role does not exist",
        "replacement role does not exist",
        "scope must target",
        "grants must target",
        "blueprint family does not belong",
        "entity does not belong",
        "context does not belong",
        "invalid invitation scope",
        "invalid grant scope",
        "owner invitations must be workspace scoped",
        "owner grants must be workspace scoped",
        "only an active owner",
        "only a workspace owner",
        "workspace must retain at least one active owner",
        "target membership is not active",
        "ownership target must be",
        "role permissions exceed",
        "role contains an unknown permission",
        "role code must",
        "only workspace-local roles may be retired",
        "role has active grants",
        "token expiry must be in the future",
        "token permissions",
        "invitation digest or expiry is invalid",
    ]
    .iter()
    .any(|expected| message.contains(expected))
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: &self.message
                }
            })),
        )
            .into_response()
    }
}
