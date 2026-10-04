use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::repository::RepositoryError;

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
    pub(super) fn global_relationship_search_budget_exceeded() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "global_relationship_search_budget_exceeded",
            message: "global relationship search exceeded its request budget; refine the query"
                .to_owned(),
            details: None,
        }
    }
    pub(super) fn global_relationship_search_timed_out() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "global_relationship_search_timed_out",
            message:
                "global relationship search exceeded its database time limit; refine the query"
                    .to_owned(),
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
    fn from(error: RepositoryError) -> Self {
        // Preserve the cause only for server errors, not expected conflicts or
        // validation failures, while keeping it out of the HTTP response.
        let cause = error.to_string();
        let response = match error {
            RepositoryError::StaleEntity => Self {
                status: StatusCode::CONFLICT,
                code: "stale_entity",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::StatusPreconditionRequired => Self {
                status: StatusCode::PRECONDITION_REQUIRED,
                code: "status_precondition_required",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::StatusTransitionForbidden(_) => Self {
                status: StatusCode::FORBIDDEN,
                code: "status_transition_forbidden",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::StatusSeparationOfDuties { .. } => Self {
                status: StatusCode::FORBIDDEN,
                code: "status_separation_of_duties",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::RecordLocked { .. } => Self {
                status: StatusCode::CONFLICT,
                code: "record_locked",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidRetentionHold(_) => Self::invalid_input(error.to_string()),
            RepositoryError::NotFound(resource) => Self::not_found(resource),
            RepositoryError::ActorNotAuthorized => Self::forbidden(),
            RepositoryError::ReservedAnnotationNamespace(_)
            | RepositoryError::InvalidAnnotationPatch(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_annotation_patch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AnnotationNamespaceAdoptionRequired(_) => Self {
                status: StatusCode::CONFLICT,
                code: "annotation_namespace_adoption_required",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::ProtectedAnnotationNamespace(_) => Self {
                status: StatusCode::CONFLICT,
                code: "protected_annotation_namespace",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AnnotationRevisionConflict { .. } => Self {
                status: StatusCode::CONFLICT,
                code: "annotation_revision_conflict",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::IdempotencyKeyReused => Self {
                status: StatusCode::CONFLICT,
                code: "idempotency_key_reused",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidComment => Self::invalid_input(error.to_string()),
            RepositoryError::CommentConflict => Self {
                status: StatusCode::CONFLICT,
                code: "comment_conflict",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvitationInvalid => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invitation_invalid",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AttributeNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_not_applicable",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidFilePolicy => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_file_policy",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidSystemMetadata => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_system_metadata",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidSystemTags => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_system_tags",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::FileAttributeReadonly => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "file_attribute_readonly",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::FileReferencesChanged => Self {
                status: StatusCode::CONFLICT,
                code: "file_references_changed",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidFileReferences => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_file_references",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::FileCardinality => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "file_cardinality_exceeded",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AttributeKindMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_kind_mismatch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AttributeValueTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_type_mismatch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::AttributeValueSchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_schema_mismatch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::EntitySchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "entity_schema_mismatch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::EntityBlueprintCurrent => Self {
                status: StatusCode::CONFLICT,
                code: "entity_blueprint_current",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::MigrationTargetChanged => Self {
                status: StatusCode::CONFLICT,
                code: "migration_target_changed",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::MigrationNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_not_applicable",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::BlueprintMigrationNotSafe => Self {
                status: StatusCode::CONFLICT,
                code: "blueprint_migration_not_safe",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::MigrationNeedsResolution(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_needs_resolution",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackAssetStorageUnavailable => Self::storage_unavailable(),
            RepositoryError::SolutionPackPlanNotReady => Self {
                status: StatusCode::CONFLICT,
                code: "solution_pack_plan_not_ready",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackPlanExpired => Self {
                status: StatusCode::CONFLICT,
                code: "solution_pack_plan_expired",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackPlanStale => Self {
                status: StatusCode::CONFLICT,
                code: "solution_pack_plan_stale",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackAssetObjectIntegrityFailed => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "asset_object_integrity_failed",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackApplicationInvalid => Self {
                status: StatusCode::CONFLICT,
                code: "solution_pack_application_invalid",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::SolutionPackApplicationFailed(_) => Self {
                status: StatusCode::CONFLICT,
                code: "solution_pack_application_failed",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidStoredAttributeValue => {
                Self::internal("stored attribute value is invalid")
            }
            RepositoryError::RelationshipTargetTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "relationship_target_type_mismatch",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::EntityBatchOperationFailed {
                index,
                entity_id,
                source,
            } => {
                // Keep the failing operation's own status and code so clients
                // recover exactly as for the single-entity request.
                let mut inner = Self::from(*source);
                inner.message = format!("operation {index}: {}", inner.message);
                let mut details = match inner.details.take() {
                    Some(serde_json::Value::Object(details)) => details,
                    _ => serde_json::Map::new(),
                };
                details.insert("operation_index".to_owned(), serde_json::json!(index));
                details.insert("entity_id".to_owned(), serde_json::json!(entity_id));
                inner.details = Some(serde_json::Value::Object(details));
                inner
            }
            RepositoryError::InvalidEntityBatch(_) => Self::invalid_input(error.to_string()),
            RepositoryError::EntityIdTaken(_) => Self {
                status: StatusCode::CONFLICT,
                code: "entity_id_taken",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::UniqueKeyConflict {
                ref key,
                ref context,
                ref values,
                conflicting_entity_id,
            } => Self {
                status: StatusCode::CONFLICT,
                code: "unique_key_conflict",
                message: error.to_string(),
                details: Some(serde_json::json!({
                    "key": key,
                    "context": context,
                    "values": values,
                    "conflicting_entity_id": conflicting_entity_id,
                })),
            },
            RepositoryError::UniqueKeyDuplicates {
                ref duplicates,
                total,
            } => Self {
                status: StatusCode::CONFLICT,
                code: "unique_key_duplicates",
                message: error.to_string(),
                details: Some(serde_json::json!({
                    "duplicates": duplicates,
                    "total": total,
                })),
            },
            RepositoryError::RelationshipCycle {
                ref attribute,
                ref path,
            } => Self {
                status: StatusCode::CONFLICT,
                code: "relationship_cycle",
                message: error.to_string(),
                details: Some(serde_json::json!({
                    "attribute": attribute,
                    "path": path,
                })),
            },
            RepositoryError::RelationshipHierarchyViolations {
                ref attribute,
                ref cycles,
                ref multiple_parents,
            } => Self {
                status: StatusCode::CONFLICT,
                code: "relationship_hierarchy_violations",
                message: error.to_string(),
                details: Some(serde_json::json!({
                    "attribute": attribute,
                    "cycles": cycles,
                    "multiple_parents": multiple_parents,
                })),
            },
            RepositoryError::RelationshipCardinalityConflict { .. } => Self {
                status: StatusCode::CONFLICT,
                code: "relationship_cardinality_conflict",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::PublicationChannelDisabled => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "publication_channel_disabled",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::PublicationActorRequired => {
                Self::internal("an authenticated user is required to publish an entity")
            }
            RepositoryError::TokenPermissionsUnavailable => Self::forbidden(),
            RepositoryError::InvalidReusableAttributeCode
            | RepositoryError::InvalidLexiconEntry(_)
            | RepositoryError::InvalidReusableAttributeDefinition(_)
            | RepositoryError::ReusableAttributeNotPublished
            | RepositoryError::InvalidPreview
            | RepositoryError::InvalidHierarchyRelationship
            | RepositoryError::InvalidAgentState(_)
            | RepositoryError::InvalidExtension(_)
            | RepositoryError::InvalidExtensionTransition(_)
            | RepositoryError::InvalidDomainEvent(_)
            | RepositoryError::InvalidSolutionPackPlan(_)
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
                details: None,
            },
            RepositoryError::InvalidBlueprintDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_blueprint_definition",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidWorkflowDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_workflow_definition",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::InvalidRuleDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_rule_definition",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::ExtensionAlreadyInstalled => Self {
                status: StatusCode::CONFLICT,
                code: "extension_already_installed",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::ApprovalAlreadyDecided => Self {
                status: StatusCode::CONFLICT,
                code: "approval_already_decided",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::ReusableAttributeAlreadyAttached
            | RepositoryError::BlueprintCodeTaken
            | RepositoryError::CatalogCodeTaken
            | RepositoryError::WorkflowCodeTaken
            | RepositoryError::RuleCodeTaken => Self {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::BlueprintNotPublished => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "blueprint_not_published",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::WorkflowNotPublished => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "workflow_not_published",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::RuleNotPublished => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "rule_not_published",
                message: error.to_string(),
                details: None,
            },
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                Self {
                    status: StatusCode::CONFLICT,
                    code: "conflict",
                    message: "a record with the same unique value already exists".to_owned(),
                    details: None,
                }
            }
            RepositoryError::BootstrapWorkspaceNotActive
            | RepositoryError::InvalidBootstrapPassword(_) => {
                Self::internal("bootstrap configuration is invalid")
            }
            RepositoryError::RelationshipSearchBudgetExceeded { .. } => {
                Self::global_relationship_search_budget_exceeded()
            }
            RepositoryError::RelationshipSearchTimedOut => {
                Self::global_relationship_search_timed_out()
            }
            RepositoryError::Task(_) => Self::internal("task queue operation failed"),
            RepositoryError::Database(_) => Self::internal("database operation failed"),
        };
        if response.status.is_server_error() {
            tracing::error!(error = %cause, "repository operation failed");
        }
        response
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
