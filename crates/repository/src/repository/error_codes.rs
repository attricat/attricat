//! The single mapping from [`RepositoryError`] to the stable error contract
//! shared by the HTTP API and agent tool results: a machine-readable code, a
//! status class, a client-safe message and optional structured details.
//!
//! The status classes follow one rule for write failures: `Conflict` (409)
//! means the current state of other data blocks the write (another entity
//! holds the key, a lock, a stale revision), while `Unprocessable` (422) means
//! the submitted content itself fails declared checks or schemas. Check
//! failures carry `details.violations[]`; every other code that has details
//! uses a flat object documented in `docs/api.md`.

use serde_json::{Map, Value, json};

use super::{CheckViolation, RepositoryError};

/// Code reported when status transition conditions or guarding rules fail.
pub const TRANSITION_CONDITIONS_UNMET: &str = "transition_conditions_unmet";

/// Transport-neutral severity of a repository error. The HTTP layer maps each
/// class to exactly one status code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    /// 403: the actor may not perform this action.
    Forbidden,
    /// 404: the addressed resource does not exist or is not visible.
    NotFound,
    /// 409: the current state of other data blocks the write.
    Conflict,
    /// 422: the submitted content fails declared checks or validation.
    Unprocessable,
    /// 428: the request needs an optimistic-concurrency precondition.
    PreconditionRequired,
    /// 500: an internal failure; the message never includes the cause.
    Internal,
    /// 503: a dependency such as object storage is unavailable.
    Unavailable,
}

/// The client-facing contract of one repository error.
#[derive(Debug)]
pub struct ErrorDescription {
    pub class: ErrorClass,
    pub code: &'static str,
    /// Safe to show to clients and agents; internal failures use a generic
    /// message instead of the underlying cause.
    pub message: String,
    pub details: Option<Value>,
}

impl ErrorDescription {
    fn new(class: ErrorClass, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            class,
            code,
            message: message.into(),
            details: None,
        }
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

fn check_failure(
    code: &'static str,
    message: String,
    violations: &[CheckViolation],
    context: Option<&str>,
) -> ErrorDescription {
    let mut details = json!({ "violations": violations });
    if let Some(context) = context {
        details["context"] = json!(context);
    }
    ErrorDescription::new(ErrorClass::Unprocessable, code, message).with_details(details)
}

impl RepositoryError {
    /// The stable error code clients and agents switch on.
    pub fn code(&self) -> &'static str {
        self.describe().code
    }

    /// Maps this error to its stable code, status class, client-safe message
    /// and structured details. This is the only place codes are assigned.
    pub fn describe(&self) -> ErrorDescription {
        use ErrorClass::{
            Conflict, Forbidden, Internal, NotFound, PreconditionRequired, Unavailable,
            Unprocessable,
        };
        let message = self.to_string();
        let plain = |class, code| ErrorDescription::new(class, code, message.clone());
        match self {
            Self::EntityCheckFailed(violations) => {
                check_failure("entity_check_failed", message, violations, None)
            }
            Self::TransitionConditionsUnmet(violations) => {
                check_failure(TRANSITION_CONDITIONS_UNMET, message, violations, None)
            }
            Self::RuleViolation(violations) => {
                check_failure("rule_violation", message, violations, None)
            }
            Self::PublicationChecksFailed {
                context,
                violations,
            } => check_failure(
                "publication_checks_failed",
                message,
                violations,
                Some(context),
            ),
            Self::RuleDryRunRequired => plain(Conflict, "rule_dry_run_required"),
            Self::RuleHasExistingViolations(count) => {
                plain(Conflict, "rule_has_existing_violations")
                    .with_details(json!({ "existing_violations": count }))
            }
            Self::StaleEntity => plain(Conflict, "stale_entity"),
            Self::StatusPreconditionRequired => {
                plain(PreconditionRequired, "status_precondition_required")
            }
            Self::StatusTransitionForbidden(denial) => {
                plain(Forbidden, "status_transition_forbidden").with_details(json!({
                    "attribute": denial.attribute,
                    "context": denial.context,
                    "from": denial.from,
                    "to": denial.to,
                    "reason": denial.reason,
                }))
            }
            Self::StatusSeparationOfDuties {
                attribute,
                context,
                edge,
            } => plain(Forbidden, "status_separation_of_duties").with_details(json!({
                "attribute": attribute,
                "context": context,
                "edge": edge,
            })),
            Self::RecordLocked {
                attribute,
                context,
                status,
            } => plain(Conflict, "record_locked").with_details(json!({
                "attribute": attribute,
                "context": context,
                "status": status,
            })),
            Self::InvalidRetentionHold(_)
            | Self::InvalidComment
            | Self::InvalidEntityBatch(_)
            | Self::InvalidReusableAttributeCode
            | Self::InvalidLexiconEntry(_)
            | Self::InvalidReusableAttributeDefinition(_)
            | Self::ReusableAttributeNotPublished
            | Self::InvalidPreview
            | Self::InvalidHierarchyRelationship
            | Self::InvalidAgentState(_)
            | Self::InvalidExtension(_)
            | Self::InvalidExtensionTransition(_)
            | Self::InvalidDomainEvent(_)
            | Self::InvalidSolutionPackPlan(_)
            | Self::ReservedContextCode
            | Self::InvalidCode
            | Self::InvalidTeam(_)
            | Self::InvalidContextData
            | Self::InvalidContext
            | Self::DefaultContextProtected
            | Self::ContextCycle
            | Self::ContextInUse
            | Self::DefaultContextOnly
            | Self::InvalidAttributeSelector => plain(Unprocessable, "invalid_input"),
            Self::NotFound(_) => plain(NotFound, "not_found"),
            Self::ActorNotAuthorized | Self::TokenPermissionsUnavailable => ErrorDescription::new(
                Forbidden,
                "forbidden",
                "you are not authorized to perform this action",
            ),
            Self::ReservedAnnotationNamespace(_) | Self::InvalidAnnotationPatch(_) => {
                plain(Unprocessable, "invalid_annotation_patch")
            }
            Self::AnnotationNamespaceAdoptionRequired(_) => {
                plain(Conflict, "annotation_namespace_adoption_required")
            }
            Self::ProtectedAnnotationNamespace(_) => {
                plain(Conflict, "protected_annotation_namespace")
            }
            Self::AnnotationRevisionConflict { expected, actual } => {
                plain(Conflict, "annotation_revision_conflict")
                    .with_details(json!({ "expected": expected, "actual": actual }))
            }
            Self::IdempotencyKeyReused => plain(Conflict, "idempotency_key_reused"),
            Self::CommentConflict => plain(Conflict, "comment_conflict"),
            Self::InvitationInvalid => plain(Unprocessable, "invitation_invalid"),
            Self::AttributeNotApplicable => plain(Unprocessable, "attribute_not_applicable"),
            Self::InvalidFilePolicy => plain(Unprocessable, "invalid_file_policy"),
            Self::InvalidSystemMetadata => plain(Unprocessable, "invalid_system_metadata"),
            Self::InvalidSystemTags => plain(Unprocessable, "invalid_system_tags"),
            Self::FileAttributeReadonly => plain(Unprocessable, "file_attribute_readonly"),
            Self::FileReferencesChanged => plain(Conflict, "file_references_changed"),
            Self::InvalidFileReferences => plain(Unprocessable, "invalid_file_references"),
            Self::FileCardinality => plain(Unprocessable, "file_cardinality_exceeded"),
            Self::AttributeKindMismatch => plain(Unprocessable, "attribute_kind_mismatch"),
            Self::AttributeValueTypeMismatch => {
                plain(Unprocessable, "attribute_value_type_mismatch")
            }
            Self::AttributeValueSchemaMismatch {
                attribute,
                instance_path,
                ..
            } => plain(Unprocessable, "attribute_value_schema_mismatch").with_details(json!({
                "attribute": attribute,
                "instance_path": instance_path,
            })),
            Self::EntitySchemaMismatch {
                context,
                instance_path,
                ..
            } => plain(Unprocessable, "entity_schema_mismatch").with_details(json!({
                "context": context,
                "instance_path": instance_path,
            })),
            Self::EntityBlueprintCurrent => plain(Conflict, "entity_blueprint_current"),
            Self::MigrationTargetChanged => plain(Conflict, "migration_target_changed"),
            Self::MigrationNotApplicable => plain(Unprocessable, "migration_not_applicable"),
            Self::BlueprintMigrationNotSafe => plain(Conflict, "blueprint_migration_not_safe"),
            Self::MigrationNeedsResolution(_) => plain(Unprocessable, "migration_needs_resolution"),
            Self::SolutionPackAssetStorageUnavailable => ErrorDescription::new(
                Unavailable,
                "storage_unavailable",
                "object storage is unavailable",
            ),
            Self::SolutionPackPlanNotReady => plain(Conflict, "solution_pack_plan_not_ready"),
            Self::SolutionPackPlanExpired => plain(Conflict, "solution_pack_plan_expired"),
            Self::SolutionPackPlanStale => plain(Conflict, "solution_pack_plan_stale"),
            Self::SolutionPackAssetObjectIntegrityFailed => {
                plain(Unprocessable, "asset_object_integrity_failed")
            }
            Self::SolutionPackApplicationInvalid => {
                plain(Conflict, "solution_pack_application_invalid")
            }
            Self::SolutionPackApplicationFailed(_) => {
                plain(Conflict, "solution_pack_application_failed")
            }
            Self::InvalidStoredAttributeValue => ErrorDescription::new(
                Internal,
                "internal_error",
                "stored attribute value is invalid",
            ),
            Self::RelationshipTargetTypeMismatch => {
                plain(Unprocessable, "relationship_target_type_mismatch")
            }
            Self::EntityBatchOperationFailed {
                index,
                entity_id,
                source,
            } => {
                // Keep the failing operation's own class and code so clients
                // recover exactly as for the single-entity request.
                let mut inner = source.describe();
                inner.message = format!("operation {index}: {}", inner.message);
                let mut details = match inner.details.take() {
                    Some(Value::Object(details)) => details,
                    _ => Map::new(),
                };
                details.insert("operation_index".to_owned(), json!(index));
                details.insert("entity_id".to_owned(), json!(entity_id));
                inner.details = Some(Value::Object(details));
                inner
            }
            Self::EntityIdTaken(entity_id) => {
                plain(Conflict, "entity_id_taken").with_details(json!({ "entity_id": entity_id }))
            }
            Self::UniqueKeyConflict {
                key,
                context,
                values,
                conflicting_entity_id,
            } => plain(Conflict, "unique_key_conflict").with_details(json!({
                "key": key,
                "context": context,
                "values": values,
                "conflicting_entity_id": conflicting_entity_id,
            })),
            Self::UniqueKeyDuplicates { duplicates, total } => {
                plain(Conflict, "unique_key_duplicates").with_details(json!({
                    "duplicates": duplicates,
                    "total": total,
                }))
            }
            Self::RelationshipCycle { attribute, path } => plain(Conflict, "relationship_cycle")
                .with_details(json!({ "attribute": attribute, "path": path })),
            Self::RelationshipHierarchyViolations {
                attribute,
                cycles,
                multiple_parents,
            } => plain(Conflict, "relationship_hierarchy_violations").with_details(json!({
                "attribute": attribute,
                "cycles": cycles,
                "multiple_parents": multiple_parents,
            })),
            Self::RelationshipCardinalityConflict {
                attribute,
                context_id,
                source_entity_id,
                target_entity_id,
                conflicting_source_entity_id,
            } => plain(Conflict, "relationship_cardinality_conflict").with_details(json!({
                "attribute": attribute,
                "context_id": context_id,
                "source_entity_id": source_entity_id,
                "target_entity_id": target_entity_id,
                "conflicting_source_entity_id": conflicting_source_entity_id,
            })),
            Self::PublicationChannelDisabled => {
                plain(Unprocessable, "publication_channel_disabled")
            }
            Self::PublicationActorRequired => plain(Internal, "internal_error"),
            Self::InvalidBlueprintDefinition(_) => {
                plain(Unprocessable, "invalid_blueprint_definition")
            }
            Self::InvalidWorkflowDefinition(_) => {
                plain(Unprocessable, "invalid_workflow_definition")
            }
            Self::InvalidRuleDefinition(_) => plain(Unprocessable, "invalid_rule_definition"),
            Self::ExtensionAlreadyInstalled => plain(Conflict, "extension_already_installed"),
            Self::ApprovalAlreadyDecided => plain(Conflict, "approval_already_decided"),
            Self::ReusableAttributeAlreadyAttached
            | Self::BlueprintCodeTaken
            | Self::CatalogCodeTaken
            | Self::WorkflowCodeTaken
            | Self::RuleCodeTaken => plain(Conflict, "conflict"),
            Self::BlueprintNotPublished => plain(Unprocessable, "blueprint_not_published"),
            Self::WorkflowNotPublished => plain(Unprocessable, "workflow_not_published"),
            Self::RuleNotPublished => plain(Unprocessable, "rule_not_published"),
            Self::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                ErrorDescription::new(
                    Conflict,
                    "conflict",
                    "a record with the same unique value already exists",
                )
            }
            Self::BootstrapWorkspaceNotActive | Self::InvalidBootstrapPassword(_) => {
                ErrorDescription::new(
                    Internal,
                    "internal_error",
                    "bootstrap configuration is invalid",
                )
            }
            Self::RelationshipSearchBudgetExceeded { .. } => ErrorDescription::new(
                Unprocessable,
                "global_relationship_search_budget_exceeded",
                "global relationship search exceeded its request budget; refine the query",
            ),
            Self::RelationshipSearchTimedOut => ErrorDescription::new(
                Unprocessable,
                "global_relationship_search_timed_out",
                "global relationship search exceeded its database time limit; refine the query",
            ),
            Self::Task(_) => {
                ErrorDescription::new(Internal, "internal_error", "task queue operation failed")
            }
            Self::Database(_) => {
                ErrorDescription::new(Internal, "internal_error", "database operation failed")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorClass, RepositoryError};
    use crate::repository::{CheckSource, CheckViolation, StatusTransitionDenial};
    use serde_json::json;
    use uuid::Uuid;

    fn violation() -> CheckViolation {
        CheckViolation {
            source: CheckSource::EntityCheck,
            code: "has-owner".into(),
            message: "An owner is required".into(),
            contexts: vec!["default".into()],
            attributes: vec!["owner".into()],
            severity: None,
            transition: None,
            evidence: json!({}),
        }
    }

    #[test]
    fn batch_failures_keep_the_operation_code_and_details() {
        let entity_id = Uuid::new_v4();
        let described = RepositoryError::EntityBatchOperationFailed {
            index: 2,
            entity_id: Some(entity_id),
            source: Box::new(RepositoryError::EntityCheckFailed(vec![violation()])),
        }
        .describe();
        assert_eq!(described.class, ErrorClass::Unprocessable);
        assert_eq!(described.code, "entity_check_failed");
        assert!(
            described
                .message
                .starts_with("operation 2: entity checks failed")
        );
        let details = described.details.unwrap();
        assert_eq!(details["operation_index"], 2);
        assert_eq!(details["entity_id"], json!(entity_id));
        assert_eq!(details["violations"][0]["code"], "has-owner");
    }

    #[test]
    fn record_controls_carry_structured_details() {
        let locked = RepositoryError::RecordLocked {
            attribute: "title".into(),
            context: "default".into(),
            status: "approved".into(),
        }
        .describe();
        assert_eq!(
            (locked.class, locked.code),
            (ErrorClass::Conflict, "record_locked")
        );
        assert_eq!(
            locked.details,
            Some(json!({"attribute":"title","context":"default","status":"approved"}))
        );

        let forbidden =
            RepositoryError::StatusTransitionForbidden(Box::new(StatusTransitionDenial {
                attribute: "status".into(),
                context: "default".into(),
                from: "draft".into(),
                to: "approved".into(),
                reason: "requires approver".into(),
            }))
            .describe();
        assert_eq!(forbidden.class, ErrorClass::Forbidden);
        assert_eq!(forbidden.details.unwrap()["to"], "approved");

        let duties = RepositoryError::StatusSeparationOfDuties {
            attribute: "status".into(),
            context: "default".into(),
            edge: "submit".into(),
        };
        assert_eq!(duties.code(), "status_separation_of_duties");
        assert_eq!(duties.describe().details.unwrap()["edge"], "submit");
    }

    #[test]
    fn internal_failures_hide_their_cause() {
        let described = RepositoryError::Database(sqlx::Error::PoolTimedOut).describe();
        assert_eq!(described.class, ErrorClass::Internal);
        assert_eq!(described.message, "database operation failed");
    }
}
