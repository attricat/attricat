//! Repository failures. [`super::error_codes`] maps each to its stable API code.
use super::*;

attricat_domain::code_enum! {
    /// What owns a code that must be unique in its workspace, reported as
    /// `details.resource` of `code_taken`.
    pub enum CodedResource (UnknownCodedResource: "coded resource") {
        Blueprint => "blueprint",
        Context => "context",
        Workflow => "workflow",
        Rule => "rule",
        Team => "team",
        ReusableAttribute => "reusable_attribute",
        ReusableAttributeGroup => "reusable_attribute_group",
        /// An extension connection, named by its connection key.
        Connection => "connection",
    }
}

/// Why a status transition was refused to its actor.
#[derive(Debug)]
pub struct StatusTransitionDenial {
    pub attribute: String,
    pub context: String,
    pub from: String,
    pub to: String,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error(
        "comment must contain 1 to {} characters and no null characters",
        record_comments::MAX_COMMENT_LENGTH
    )]
    InvalidComment,
    #[error("comment changed; reload before saving again")]
    CommentConflict,
    #[error("record changed since it was loaded; refresh before saving")]
    StaleRecord,
    #[error("status edits require expected_updated_at from the record form")]
    StatusPreconditionRequired,
    #[error(
        "status transition of '{}' from {} to {} is not permitted (context: {}): {}",
        .0.attribute, .0.from, .0.to, .0.context, .0.reason
    )]
    StatusTransitionForbidden(Box<StatusTransitionDenial>),
    #[error(
        "status transition of '{attribute}' must be made by someone other than the user who made the '{edge}' transition (context: {context})"
    )]
    StatusSeparationOfDuties {
        attribute: String,
        context: String,
        edge: String,
    },
    #[error("'{attribute}' is '{status}', which does not lock the record (context: {context})")]
    StatusNotLocked {
        attribute: String,
        context: String,
        status: String,
    },
    #[error(
        "'{attribute}' is locked while the record is '{status}' (context: {context}); use a permitted correction transition first"
    )]
    RecordLocked {
        attribute: String,
        context: String,
        status: String,
    },
    #[error("invalid retention hold: {0}")]
    InvalidRetentionHold(String),
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("the initiating user is no longer authorized for this record")]
    ActorNotAuthorized,
    #[error("annotation namespace '{0}' is reserved")]
    ReservedAnnotationNamespace(String),
    #[error(
        "annotation namespace '{0}' contains existing annotations; an operator must adopt it before the extension can write"
    )]
    AnnotationNamespaceAdoptionRequired(String),
    #[error(
        "annotations in extension namespace '{0}' can only be changed by that extension or an operator repair"
    )]
    ProtectedAnnotationNamespace(String),
    #[error("extension annotations changed; expected revision {expected}, found {actual}")]
    AnnotationRevisionConflict { expected: i64, actual: i64 },
    #[error("invalid extension annotation patch: {0}")]
    InvalidAnnotationPatch(String),
    #[error("idempotency key was already used with a different request")]
    IdempotencyKeyReused,
    #[error(transparent)]
    InvalidLexiconEntry(#[from] attricat_lexicon::EntryError),
    #[error("ATTRICAT_WORKSPACE_ID does not identify an active workspace")]
    BootstrapWorkspaceNotActive,
    #[error("invalid bootstrap password: {0}")]
    InvalidBootstrapPassword(String),
    #[error("the invitation is invalid, expired, revoked, already accepted, or for another email")]
    InvitationInvalid,
    #[error("the context code 'default' is reserved")]
    ReservedContextCode,
    #[error("code must contain only ASCII letters, numbers, hyphens, and underscores")]
    InvalidCode,
    #[error("context data must be a JSON object")]
    InvalidContextData,
    #[error(
        "system metadata must be a JSON object no larger than {} KiB",
        system_annotations::MAX_SYSTEM_METADATA_BYTES / 1024
    )]
    InvalidSystemMetadata,
    #[error(
        "system tags must contain at most {} unique, non-blank strings no longer than {} bytes",
        system_annotations::MAX_SYSTEM_TAGS,
        system_annotations::MAX_SYSTEM_TAG_BYTES
    )]
    InvalidSystemTags,
    #[error("context was not found")]
    InvalidContext,
    #[error("the default context cannot be changed or deleted")]
    DefaultContextProtected,
    #[error("a context cannot be its own descendant")]
    ContextCycle,
    #[error("a context with descendants, active values or active extension runs cannot be deleted")]
    ContextInUse,
    #[error("the workspace's last owner cannot be removed; transfer ownership first")]
    LastWorkspaceOwner,
    #[error("a role needs at least one permission")]
    RolePermissionsRequired,
    #[error("a saved search with this name already exists")]
    SavedSearchNameTaken,
    #[error("attribute can only be edited in the default context")]
    DefaultContextOnly,
    #[error("attribute does not belong to the record blueprint version")]
    AttributeNotApplicable,
    #[error(
        "reusable attribute namespace and code must each contain only ASCII letters, numbers, hyphens, and underscores"
    )]
    InvalidReusableAttributeCode,
    #[error("invalid reusable attribute definition: {0}")]
    InvalidReusableAttributeDefinition(String),
    #[error(
        "reusable attribute revisions must be published before they can be attached or added to a group"
    )]
    ReusableAttributeNotPublished,
    #[error("the record already has an attachment for this reusable attribute definition")]
    ReusableAttributeAlreadyAttached,
    #[error("file attribute policy is invalid")]
    InvalidFilePolicy,
    #[error("file attribute is read-only")]
    FileAttributeReadonly,
    #[error(
        "attribute '{attribute}' is generated when the record is created and cannot be written"
    )]
    GeneratedAttributeReadonly { attribute: String },
    #[error(
        "the sequence's next value is already {current}; it can only be raised, not set to {requested}"
    )]
    GeneratedSequenceNotRaised { current: i64, requested: i64 },
    /// Counters are keyed by period, so a new `reset` would restart numbering
    /// and hand out numbers that earlier revisions already issued.
    #[error(
        "generated attribute '{attribute}' resets {previous} in an earlier revision and cannot reset {requested}"
    )]
    GeneratedResetChanged {
        attribute: String,
        previous: attricat_domain::attribute_options::SequenceReset,
        requested: attricat_domain::attribute_options::SequenceReset,
    },
    #[error("invalid generated sequence request: {0}")]
    InvalidGeneratedSequence(String),
    #[error("file references changed; refresh and try again")]
    FileReferencesChanged,
    #[error("file references must be unique existing attachments in an allowed order")]
    InvalidFileReferences,
    #[error("file count is incompatible with the attribute cardinality")]
    FileCardinality,
    #[error("provide exactly one of attribute_id or attribute_code")]
    InvalidAttributeSelector,
    #[error("attribute kind does not match the supplied value")]
    AttributeKindMismatch,
    #[error("value does not match the attribute type")]
    AttributeValueTypeMismatch,
    #[error(
        "value for attribute '{attribute}' does not match its schema at '{instance_path}': {message}"
    )]
    AttributeValueSchemaMismatch {
        attribute: String,
        instance_path: String,
        message: String,
    },
    #[error(
        "resolved record values for context '{context}' do not match the record schema at '{instance_path}': {message}"
    )]
    RecordSchemaMismatch {
        context: String,
        instance_path: String,
        message: String,
    },
    #[error("record checks failed: {}", summarize_violations(.0))]
    RecordCheckFailed(Vec<CheckViolation>),
    #[error("status transition conditions are not met: {}", summarize_violations(.0))]
    TransitionConditionsUnmet(Vec<CheckViolation>),
    #[error("enforcing rules are violated: {}", summarize_violations(.0))]
    RuleViolation(Vec<CheckViolation>),
    #[error("publication to '{context}' is blocked by failing checks: {}", summarize_violations(.violations))]
    PublicationChecksFailed {
        context: String,
        violations: Vec<CheckViolation>,
    },
    #[error("an enforcing rule needs a completed dry run of this revision before it is enabled")]
    RuleDryRunRequired,
    #[error(
        "the latest dry run stopped at its candidate limit before checking every record; enable with accept_existing_violations to accept the records it did not check"
    )]
    RuleDryRunTruncated(i64),
    #[error(
        "the latest dry run found {0} existing violations; fix them or enable with accept_existing_violations"
    )]
    RuleHasExistingViolations(i64),
    #[error("stored attribute value does not match its attribute type")]
    InvalidStoredAttributeValue,
    #[error("relationship target does not match the attribute target blueprint")]
    RelationshipTargetTypeMismatch,
    #[error("one-to-one relationship cardinality conflict for '{attribute}'")]
    RelationshipCardinalityConflict {
        attribute: String,
        context_id: Option<Uuid>,
        source_record_id: Uuid,
        target_record_id: Uuid,
        conflicting_source_record_id: Option<Uuid>,
    },
    #[error(
        "unique key '{key}' already has values {values} on record {conflicting_record_id} in context '{context}'"
    )]
    UniqueKeyConflict {
        key: String,
        context: String,
        /// Boxed: a `Value` keeps its key order, which would make every
        /// `RepositoryError` this variant's size.
        values: Box<Value>,
        conflicting_record_id: Uuid,
    },
    #[error(
        "{total} unique key value(s) are already shared by more than one record; resolve the duplicates first: {}",
        structural_constraints::describe_duplicates(.duplicates)
    )]
    UniqueKeyDuplicates {
        duplicates: Vec<UniqueKeyDuplicate>,
        total: usize,
    },
    #[error(
        "relationship '{attribute}' would create a cycle: {}",
        .path.iter().map(Uuid::to_string).collect::<Vec<_>>().join(" -> ")
    )]
    RelationshipCycle { attribute: String, path: Vec<Uuid> },
    #[error(
        "existing '{attribute}' relationships contain {} cycle(s) and {} records with more than one parent; resolve them before publishing: {}",
        .cycles.len(),
        .multiple_parents.len(),
        structural_constraints::describe_hierarchy_violations(.cycles, .multiple_parents)
    )]
    RelationshipHierarchyViolations {
        attribute: String,
        cycles: Vec<Vec<Uuid>>,
        multiple_parents: Vec<Uuid>,
    },
    #[error("invalid record batch: {0}")]
    InvalidRecordBatch(String),
    #[error("record {0} already exists")]
    RecordIdTaken(Uuid),
    #[error(
        "a copy leaves out unique key values that the record schema requires: {}; create the record instead",
        .attributes.join(", ")
    )]
    DuplicateNeedsUniqueValues { attributes: Vec<String> },
    #[error("batch operation {index} failed; no changes were applied: {source}")]
    RecordBatchOperationFailed {
        index: usize,
        record_id: Option<Uuid>,
        source: Box<RepositoryError>,
    },
    #[error("record preview must be a JSON object organized by context")]
    InvalidPreview,
    #[error(
        "record relationship preview exceeds the expansion limit; reduce relationship depth or item limit"
    )]
    PreviewExpansionLimit,
    #[error("hierarchy field must be a self-targeting relationship")]
    InvalidHierarchyRelationship,
    #[error("invalid blueprint definition: {0}")]
    InvalidBlueprintDefinition(String),
    #[error("{0}")]
    InvalidSolutionPackPlan(String),
    #[error("solution-pack asset storage is unavailable")]
    SolutionPackAssetStorageUnavailable,
    #[error("solution-pack plan is not ready to apply")]
    SolutionPackPlanNotReady,
    #[error("solution-pack plan has expired")]
    SolutionPackPlanExpired,
    #[error("solution-pack plan preconditions no longer match the workspace")]
    SolutionPackPlanStale,
    #[error("solution-pack presentation asset object integrity failed")]
    SolutionPackAssetObjectIntegrityFailed,
    #[error("solution-pack application is invalid and cannot be resumed")]
    SolutionPackApplicationInvalid,
    #[error("solution-pack application failed: {0}")]
    SolutionPackApplicationFailed(String),
    #[error("global relationship search exceeded its {dimension} budget")]
    RelationshipSearchBudgetExceeded { dimension: &'static str },
    #[error("global relationship search exceeded its PostgreSQL statement timeout")]
    RelationshipSearchTimedOut,
    #[error("the definition is identical to version {version} of this blueprint")]
    BlueprintRevisionUnchanged { version: i64 },
    #[error("the definition is identical to version {version} of this rule")]
    RuleRevisionUnchanged { version: i64 },
    #[error("the {0} code is already in use")]
    CodeTaken(CodedResource),
    #[error("invalid team: {0}")]
    InvalidTeam(String),
    #[error("invalid agent settings: {0}")]
    InvalidAgentSettings(String),
    #[error("invalid tag colors: {0}")]
    InvalidTagColors(String),
    #[error("invalid workflow definition: {0}")]
    InvalidWorkflowDefinition(String),
    #[error("invalid rule definition: {0}")]
    InvalidRuleDefinition(String),
    #[error("invalid publication channel: {0}")]
    InvalidPublicationChannel(String),
    #[error("rule is not enabled; enable it first, or start a dry run")]
    RuleNotEnabled,
    #[error("workflow revision must be published before it can be enabled")]
    WorkflowNotPublished,
    #[error("workflow has no enabled revision to run; enable it first")]
    WorkflowNotEnabled,
    #[error("blueprint revision is not published")]
    BlueprintNotPublished,
    #[error("record is already on the latest blueprint revision")]
    RecordBlueprintCurrent,
    #[error("the latest blueprint revision changed; refresh the migration preview")]
    MigrationTargetChanged,
    #[error("migration does not apply to this record")]
    MigrationNotApplicable,
    #[error("this blueprint revision is not eligible for safe automatic migration")]
    BlueprintMigrationNotSafe,
    #[error("migration needs resolutions for: {}", .0.join(", "))]
    MigrationNeedsResolution(Vec<String>),
    #[error("invalid agent state: {0}")]
    InvalidAgentState(&'static str),
    #[error("invalid extension: {0}")]
    InvalidExtension(String),
    #[error("invalid extension lifecycle transition: {0}")]
    InvalidExtensionTransition(&'static str),
    #[error("extension is already installed")]
    ExtensionAlreadyInstalled,
    #[error("the file for this action is no longer available; choose it again")]
    ExtensionInputFileUnavailable,
    #[error("{0}")]
    ExportMappingConflict(String),
    #[error("invalid domain event: {0}")]
    InvalidDomainEvent(#[from] crate::domain_events::EventContractError),
    #[error("publication context is not an enabled channel")]
    PublicationChannelDisabled,
    #[error("an authenticated user is required to publish a record")]
    PublicationActorRequired,
    #[error("requested token permissions are not available to the current user")]
    TokenPermissionsUnavailable,
    #[error("an approval decision has already been recorded")]
    ApprovalAlreadyDecided,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Task(#[from] tasks::TaskError),
}

impl RepositoryError {
    pub fn invalid_blueprint_definition(error: impl std::fmt::Display) -> Self {
        Self::InvalidBlueprintDefinition(error.to_string())
    }
}
