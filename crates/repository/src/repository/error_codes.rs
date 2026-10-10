//! The single mapping from [`RepositoryError`] to the stable error contract
//! shared by the HTTP API and agent tool results: a machine-readable code, a
//! status class, a client-safe message and optional structured details.
//!
//! The status classes follow one rule for write failures: `Conflict` (409)
//! means the current state of other data blocks the write (another record
//! holds the key, a lock, a stale revision), while `Unprocessable` (422) means
//! the submitted content itself fails declared checks or schemas. Check
//! failures carry `details.violations[]`; every other code that has details
//! uses a flat object documented in `docs/api.md`.

use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{CheckViolation, RepositoryError};

macro_rules! error_codes {
    ($($variant:ident = $code:literal,)+) => {
        /// The closed set of machine-readable error codes. Serializes as its
        /// snake_case wire string, which clients and agents switch on.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum ErrorCode {
            $($variant,)+
        }

        impl ErrorCode {
            /// Every code, for contract tests and documentation checks.
            pub const ALL: &[Self] = &[$(Self::$variant,)+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code,)+
                }
            }
        }
    };
}

error_codes! {
    AnnotationNamespaceAdoptionRequired = "annotation_namespace_adoption_required",
    AnnotationRevisionConflict = "annotation_revision_conflict",
    ApprovalAlreadyDecided = "approval_already_decided",
    AssetObjectIntegrityFailed = "asset_object_integrity_failed",
    AttributeKindMismatch = "attribute_kind_mismatch",
    AttributeNotApplicable = "attribute_not_applicable",
    AttributeValueSchemaMismatch = "attribute_value_schema_mismatch",
    AttributeValueTypeMismatch = "attribute_value_type_mismatch",
    BlueprintMigrationNotSafe = "blueprint_migration_not_safe",
    BlueprintNotPublished = "blueprint_not_published",
    CommentConflict = "comment_conflict",
    Conflict = "conflict",
    RecordBlueprintCurrent = "record_blueprint_current",
    RecordCheckFailed = "record_check_failed",
    RecordIdTaken = "record_id_taken",
    RecordSchemaMismatch = "record_schema_mismatch",
    ExtensionAlreadyInstalled = "extension_already_installed",
    FileAttributeReadonly = "file_attribute_readonly",
    FileCardinalityExceeded = "file_cardinality_exceeded",
    FileReferencesChanged = "file_references_changed",
    Forbidden = "forbidden",
    GlobalRelationshipSearchBudgetExceeded = "global_relationship_search_budget_exceeded",
    GlobalRelationshipSearchTimedOut = "global_relationship_search_timed_out",
    IdempotencyKeyReused = "idempotency_key_reused",
    InternalError = "internal_error",
    InvalidAnnotationPatch = "invalid_annotation_patch",
    InvalidBlueprintDefinition = "invalid_blueprint_definition",
    InvalidFilePolicy = "invalid_file_policy",
    InvalidFileReferences = "invalid_file_references",
    InvalidInput = "invalid_input",
    InvalidRuleDefinition = "invalid_rule_definition",
    InvalidSystemMetadata = "invalid_system_metadata",
    InvalidSystemTags = "invalid_system_tags",
    InvalidWorkflowDefinition = "invalid_workflow_definition",
    InvitationInvalid = "invitation_invalid",
    MigrationNeedsResolution = "migration_needs_resolution",
    MigrationNotApplicable = "migration_not_applicable",
    MigrationTargetChanged = "migration_target_changed",
    NotFound = "not_found",
    ProtectedAnnotationNamespace = "protected_annotation_namespace",
    PublicationChannelDisabled = "publication_channel_disabled",
    PublicationChecksFailed = "publication_checks_failed",
    RecordLocked = "record_locked",
    RelationshipCardinalityConflict = "relationship_cardinality_conflict",
    RelationshipCycle = "relationship_cycle",
    RelationshipHierarchyViolations = "relationship_hierarchy_violations",
    RelationshipTargetTypeMismatch = "relationship_target_type_mismatch",
    RuleDryRunRequired = "rule_dry_run_required",
    RuleHasExistingViolations = "rule_has_existing_violations",
    RuleNotEnabled = "rule_not_enabled",
    RuleNotPublished = "rule_not_published",
    RuleViolation = "rule_violation",
    SolutionPackApplicationFailed = "solution_pack_application_failed",
    SolutionPackApplicationInvalid = "solution_pack_application_invalid",
    SolutionPackPlanExpired = "solution_pack_plan_expired",
    SolutionPackPlanNotReady = "solution_pack_plan_not_ready",
    SolutionPackPlanStale = "solution_pack_plan_stale",
    StaleRecord = "stale_record",
    StatusPreconditionRequired = "status_precondition_required",
    StatusSeparationOfDuties = "status_separation_of_duties",
    StatusTransitionForbidden = "status_transition_forbidden",
    StorageUnavailable = "storage_unavailable",
    TransitionConditionsUnmet = "transition_conditions_unmet",
    UniqueKeyConflict = "unique_key_conflict",
    UniqueKeyDuplicates = "unique_key_duplicates",
    WorkflowNotEnabled = "workflow_not_enabled",
    WorkflowNotPublished = "workflow_not_published",
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

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
    pub code: ErrorCode,
    /// Safe to show to clients and agents; internal failures use a generic
    /// message instead of the underlying cause.
    pub message: String,
    pub details: Option<Value>,
}

impl ErrorDescription {
    fn new(class: ErrorClass, code: ErrorCode, message: impl Into<String>) -> Self {
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
    code: ErrorCode,
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
    pub fn code(&self) -> ErrorCode {
        self.describe().code
    }

    /// Maps this error to its stable code, status class, client-safe message
    /// and structured details. This is the only place codes are assigned.
    pub fn describe(&self) -> ErrorDescription {
        use ErrorClass::{
            Conflict, Forbidden, Internal, NotFound, PreconditionRequired, Unavailable,
            Unprocessable,
        };
        use ErrorCode as Code;
        let message = self.to_string();
        let plain = |class, code| ErrorDescription::new(class, code, message.clone());
        match self {
            Self::RecordCheckFailed(violations) => {
                check_failure(Code::RecordCheckFailed, message, violations, None)
            }
            Self::TransitionConditionsUnmet(violations) => {
                check_failure(Code::TransitionConditionsUnmet, message, violations, None)
            }
            Self::RuleViolation(violations) => {
                check_failure(Code::RuleViolation, message, violations, None)
            }
            Self::PublicationChecksFailed {
                context,
                violations,
            } => check_failure(
                Code::PublicationChecksFailed,
                message,
                violations,
                Some(context),
            ),
            Self::RuleDryRunRequired => plain(Conflict, Code::RuleDryRunRequired),
            Self::RuleDryRunTruncated(count) => plain(Conflict, Code::RuleDryRunRequired)
                .with_details(json!({ "truncated": true, "existing_violations": count })),
            Self::RuleHasExistingViolations(count) => {
                plain(Conflict, Code::RuleHasExistingViolations)
                    .with_details(json!({ "existing_violations": count }))
            }
            Self::StaleRecord => plain(Conflict, Code::StaleRecord),
            Self::StatusPreconditionRequired => {
                plain(PreconditionRequired, Code::StatusPreconditionRequired)
            }
            Self::StatusTransitionForbidden(denial) => {
                plain(Forbidden, Code::StatusTransitionForbidden).with_details(json!({
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
            } => plain(Forbidden, Code::StatusSeparationOfDuties).with_details(json!({
                "attribute": attribute,
                "context": context,
                "edge": edge,
            })),
            Self::RecordLocked {
                attribute,
                context,
                status,
            } => plain(Conflict, Code::RecordLocked).with_details(json!({
                "attribute": attribute,
                "context": context,
                "status": status,
            })),
            Self::InvalidRetentionHold(_)
            | Self::InvalidComment
            | Self::InvalidRecordBatch(_)
            | Self::InvalidReusableAttributeCode
            | Self::InvalidLexiconEntry(_)
            | Self::InvalidReusableAttributeDefinition(_)
            | Self::ReusableAttributeNotPublished
            | Self::InvalidPreview
            | Self::PreviewExpansionLimit
            | Self::InvalidHierarchyRelationship
            | Self::InvalidAgentState(_)
            | Self::InvalidExtension(_)
            | Self::InvalidExtensionTransition(_)
            | Self::InvalidDomainEvent(_)
            | Self::InvalidSolutionPackPlan(_)
            | Self::ReservedContextCode
            | Self::InvalidCode
            | Self::InvalidTeam(_)
            | Self::InvalidPublicationChannel(_)
            | Self::InvalidContextData
            | Self::InvalidContext
            | Self::DefaultContextProtected
            | Self::ContextCycle
            | Self::ContextInUse
            | Self::DefaultContextOnly
            | Self::InvalidAttributeSelector => plain(Unprocessable, Code::InvalidInput),
            Self::NotFound(_) => plain(NotFound, Code::NotFound),
            Self::ActorNotAuthorized | Self::TokenPermissionsUnavailable => ErrorDescription::new(
                Forbidden,
                Code::Forbidden,
                "you are not authorized to perform this action",
            ),
            Self::ReservedAnnotationNamespace(_) | Self::InvalidAnnotationPatch(_) => {
                plain(Unprocessable, Code::InvalidAnnotationPatch)
            }
            Self::AnnotationNamespaceAdoptionRequired(_) => {
                plain(Conflict, Code::AnnotationNamespaceAdoptionRequired)
            }
            Self::ProtectedAnnotationNamespace(_) => {
                plain(Conflict, Code::ProtectedAnnotationNamespace)
            }
            Self::AnnotationRevisionConflict { expected, actual } => {
                plain(Conflict, Code::AnnotationRevisionConflict)
                    .with_details(json!({ "expected": expected, "actual": actual }))
            }
            Self::IdempotencyKeyReused => plain(Conflict, Code::IdempotencyKeyReused),
            Self::CommentConflict => plain(Conflict, Code::CommentConflict),
            Self::InvitationInvalid => plain(Unprocessable, Code::InvitationInvalid),
            Self::AttributeNotApplicable => plain(Unprocessable, Code::AttributeNotApplicable),
            Self::InvalidFilePolicy => plain(Unprocessable, Code::InvalidFilePolicy),
            Self::InvalidSystemMetadata => plain(Unprocessable, Code::InvalidSystemMetadata),
            Self::InvalidSystemTags => plain(Unprocessable, Code::InvalidSystemTags),
            Self::FileAttributeReadonly => plain(Unprocessable, Code::FileAttributeReadonly),
            Self::FileReferencesChanged => plain(Conflict, Code::FileReferencesChanged),
            Self::InvalidFileReferences => plain(Unprocessable, Code::InvalidFileReferences),
            Self::FileCardinality => plain(Unprocessable, Code::FileCardinalityExceeded),
            Self::AttributeKindMismatch => plain(Unprocessable, Code::AttributeKindMismatch),
            Self::AttributeValueTypeMismatch => {
                plain(Unprocessable, Code::AttributeValueTypeMismatch)
            }
            Self::AttributeValueSchemaMismatch {
                attribute,
                instance_path,
                ..
            } => plain(Unprocessable, Code::AttributeValueSchemaMismatch).with_details(json!({
                "attribute": attribute,
                "instance_path": instance_path,
            })),
            Self::RecordSchemaMismatch {
                context,
                instance_path,
                ..
            } => plain(Unprocessable, Code::RecordSchemaMismatch).with_details(json!({
                "context": context,
                "instance_path": instance_path,
            })),
            Self::RecordBlueprintCurrent => plain(Conflict, Code::RecordBlueprintCurrent),
            Self::MigrationTargetChanged => plain(Conflict, Code::MigrationTargetChanged),
            Self::MigrationNotApplicable => plain(Unprocessable, Code::MigrationNotApplicable),
            Self::BlueprintMigrationNotSafe => plain(Conflict, Code::BlueprintMigrationNotSafe),
            Self::MigrationNeedsResolution(_) => {
                plain(Unprocessable, Code::MigrationNeedsResolution)
            }
            Self::SolutionPackAssetStorageUnavailable => ErrorDescription::new(
                Unavailable,
                Code::StorageUnavailable,
                "object storage is unavailable",
            ),
            Self::SolutionPackPlanNotReady => plain(Conflict, Code::SolutionPackPlanNotReady),
            Self::SolutionPackPlanExpired => plain(Conflict, Code::SolutionPackPlanExpired),
            Self::SolutionPackPlanStale => plain(Conflict, Code::SolutionPackPlanStale),
            Self::SolutionPackAssetObjectIntegrityFailed => {
                plain(Unprocessable, Code::AssetObjectIntegrityFailed)
            }
            Self::SolutionPackApplicationInvalid => {
                plain(Conflict, Code::SolutionPackApplicationInvalid)
            }
            Self::SolutionPackApplicationFailed(_) => {
                plain(Conflict, Code::SolutionPackApplicationFailed)
            }
            Self::InvalidStoredAttributeValue => ErrorDescription::new(
                Internal,
                Code::InternalError,
                "stored attribute value is invalid",
            ),
            Self::RelationshipTargetTypeMismatch => {
                plain(Unprocessable, Code::RelationshipTargetTypeMismatch)
            }
            Self::RecordBatchOperationFailed {
                index,
                record_id,
                source,
            } => {
                // Keep the failing operation's own class and code so clients
                // recover exactly as for the single-record request.
                let mut inner = source.describe();
                inner.message = format!("operation {index}: {}", inner.message);
                let mut details = match inner.details.take() {
                    Some(Value::Object(details)) => details,
                    _ => Map::new(),
                };
                details.insert("operation_index".to_owned(), json!(index));
                details.insert("record_id".to_owned(), json!(record_id));
                inner.details = Some(Value::Object(details));
                inner
            }
            Self::RecordIdTaken(record_id) => {
                plain(Conflict, Code::RecordIdTaken).with_details(json!({ "record_id": record_id }))
            }
            Self::UniqueKeyConflict {
                key,
                context,
                values,
                conflicting_record_id,
            } => plain(Conflict, Code::UniqueKeyConflict).with_details(json!({
                "key": key,
                "context": context,
                "values": values,
                "conflicting_record_id": conflicting_record_id,
            })),
            Self::UniqueKeyDuplicates { duplicates, total } => {
                plain(Conflict, Code::UniqueKeyDuplicates).with_details(json!({
                    "duplicates": duplicates,
                    "total": total,
                }))
            }
            Self::RelationshipCycle { attribute, path } => plain(Conflict, Code::RelationshipCycle)
                .with_details(json!({ "attribute": attribute, "path": path })),
            Self::RelationshipHierarchyViolations {
                attribute,
                cycles,
                multiple_parents,
            } => plain(Conflict, Code::RelationshipHierarchyViolations).with_details(json!({
                "attribute": attribute,
                "cycles": cycles,
                "multiple_parents": multiple_parents,
            })),
            Self::RelationshipCardinalityConflict {
                attribute,
                context_id,
                source_record_id,
                target_record_id,
                conflicting_source_record_id,
            } => plain(Conflict, Code::RelationshipCardinalityConflict).with_details(json!({
                "attribute": attribute,
                "context_id": context_id,
                "source_record_id": source_record_id,
                "target_record_id": target_record_id,
                "conflicting_source_record_id": conflicting_source_record_id,
            })),
            Self::PublicationChannelDisabled => {
                plain(Unprocessable, Code::PublicationChannelDisabled)
            }
            Self::PublicationActorRequired => plain(Internal, Code::InternalError),
            Self::InvalidBlueprintDefinition(_) => {
                plain(Unprocessable, Code::InvalidBlueprintDefinition)
            }
            Self::InvalidWorkflowDefinition(_) => {
                plain(Unprocessable, Code::InvalidWorkflowDefinition)
            }
            Self::InvalidRuleDefinition(_) => plain(Unprocessable, Code::InvalidRuleDefinition),
            Self::ExtensionAlreadyInstalled => plain(Conflict, Code::ExtensionAlreadyInstalled),
            Self::ApprovalAlreadyDecided => plain(Conflict, Code::ApprovalAlreadyDecided),
            Self::ReusableAttributeAlreadyAttached
            | Self::BlueprintCodeTaken
            | Self::CatalogCodeTaken
            | Self::WorkflowCodeTaken
            | Self::RuleCodeTaken => plain(Conflict, Code::Conflict),
            Self::BlueprintNotPublished => plain(Unprocessable, Code::BlueprintNotPublished),
            Self::WorkflowNotPublished => plain(Unprocessable, Code::WorkflowNotPublished),
            Self::WorkflowNotEnabled => plain(Unprocessable, Code::WorkflowNotEnabled),
            Self::RuleNotPublished => plain(Unprocessable, Code::RuleNotPublished),
            Self::RuleNotEnabled => plain(Unprocessable, Code::RuleNotEnabled),
            Self::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                ErrorDescription::new(
                    Conflict,
                    Code::Conflict,
                    "a record with the same unique value already exists",
                )
            }
            Self::BootstrapWorkspaceNotActive | Self::InvalidBootstrapPassword(_) => {
                ErrorDescription::new(
                    Internal,
                    Code::InternalError,
                    "bootstrap configuration is invalid",
                )
            }
            Self::RelationshipSearchBudgetExceeded { .. } => ErrorDescription::new(
                Unprocessable,
                Code::GlobalRelationshipSearchBudgetExceeded,
                "global relationship search exceeded its request budget; refine the query",
            ),
            Self::RelationshipSearchTimedOut => ErrorDescription::new(
                Unprocessable,
                Code::GlobalRelationshipSearchTimedOut,
                "global relationship search exceeded its database time limit; refine the query",
            ),
            Self::Task(_) => {
                ErrorDescription::new(Internal, Code::InternalError, "task queue operation failed")
            }
            Self::Database(_) => {
                ErrorDescription::new(Internal, Code::InternalError, "database operation failed")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorClass, ErrorCode, RepositoryError};
    use crate::repository::{CheckSource, CheckViolation, StatusTransitionDenial};
    use serde_json::json;
    use uuid::Uuid;

    fn violation() -> CheckViolation {
        CheckViolation {
            source: CheckSource::RecordCheck,
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
        let record_id = Uuid::new_v4();
        let described = RepositoryError::RecordBatchOperationFailed {
            index: 2,
            record_id: Some(record_id),
            source: Box::new(RepositoryError::RecordCheckFailed(vec![violation()])),
        }
        .describe();
        assert_eq!(described.class, ErrorClass::Unprocessable);
        assert_eq!(described.code, ErrorCode::RecordCheckFailed);
        assert!(
            described
                .message
                .starts_with("operation 2: record checks failed")
        );
        let details = described.details.unwrap();
        assert_eq!(details["operation_index"], 2);
        assert_eq!(details["record_id"], json!(record_id));
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
            (ErrorClass::Conflict, ErrorCode::RecordLocked)
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
        assert_eq!(duties.code(), ErrorCode::StatusSeparationOfDuties);
        assert_eq!(duties.describe().details.unwrap()["edge"], "submit");
    }

    #[test]
    fn internal_failures_hide_their_cause() {
        let described = RepositoryError::Database(sqlx::Error::PoolTimedOut).describe();
        assert_eq!(described.class, ErrorClass::Internal);
        assert_eq!(described.message, "database operation failed");
    }

    #[test]
    fn codes_are_unique_snake_case_wire_strings() {
        let mut seen = std::collections::HashSet::new();
        for code in ErrorCode::ALL {
            let wire = code.as_str();
            assert!(seen.insert(wire), "duplicate code {wire}");
            assert!(
                wire.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
                "{wire} is not snake_case"
            );
            assert_eq!(serde_json::to_value(code).unwrap(), json!(wire));
        }
    }
}
