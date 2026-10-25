//! Workspace-scoped catalog persistence.
//!
//! # Lock order
//!
//! Transactions that write records take their locks in this order, skipping
//! any they do not need, so concurrent writers cannot deadlock:
//!
//! 1. Workflow execution only: the workflow lifecycle row, then the run row.
//! 2. The workspace relationship lock
//!    ([`AttricatRepository::lock_relationship_cardinality_writes`]), taken
//!    by every write that changes relationship values and by context
//!    reparenting.
//! 3. A legacy extension upsert's lookup lock.
//! 4. Record rows. A transaction that locks several takes them in ID order.
//! 5. Extension annotation namespace locks, in sorted name order.
//!
//! Record writers take the workspace's record-writer advisory lock shared
//! right before their first record row lock (inside `lock_record`,
//! `lock_record_with_generations`, record insertion, batch pre-locking, bulk
//! publication and extension upsert lookups), and the per-family unique-key locks shared while they
//! validate, after their record rows. Blueprint publication takes the
//! workspace row (by advancing a generation) and then its family's key lock
//! exclusively. Context creation and reparenting take the workspace row
//! first, then (reparenting only) the relationship lock, then the
//! record-writer lock exclusively, which waits for that workspace's
//! in-flight record writers only, and then the family key locks
//! exclusively. Context deletion also takes the workspace row before the
//! context row. Solution-pack blueprint and context steps lock the
//! workspace row before their `attribute_contexts` share lock.
//!
//! The record seams (`create_record_in_transaction`,
//! `update_record_in_transaction`, `delete_record_in_transaction`) take the
//! locks they need themselves; re-taking a lock the transaction already
//! holds is a no-op, so a caller that pre-locks must only respect the order.

use std::{collections::HashSet, sync::Arc};

use attricat_cache::{CacheKey, Policy};

use attricat_validation::is_valid_code;
use chrono::DateTime;
use serde_json::{Map, Value};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    domain_events::{AffectedFactV1, EventSource, EventSourceKind, NewDomainEvent},
    model::{
        AppendAttributeValues, AttachReusableAttribute, Attribute, AttributeContext,
        AttributeValue, AttributeValueHistory, AttributeValueSelector, Blueprint,
        BlueprintRecordPublicationSummary, BlueprintWithAttributes, CreateBlueprint,
        CreateReusableAttribute, CreateReusableAttributeGroup, FormAttributeValue,
        IncomingRelationshipField, IncomingRelationshipItem, IncomingRelationshipSelector,
        IncomingRelationshipsPage, MatchExplanation, MatchPathEdge, MigrateRecordRequest,
        MigrationIssue, NewAttributeValue, PublicationChannel, Record, RecordAuditChange,
        RecordHierarchyItem, RecordHierarchyResponse, RecordIdentity, RecordLabel,
        RecordMigrationPreview, RecordPreview, RecordPreviewPage, RecordPublicationStatus,
        RecordReusableAttribute, RelatedRecordPreview, RelationshipMutation, RelationshipTargets,
        RelationshipTreeFacetChildItem, RelationshipTreeFacetChildrenResponse,
        ResolvedRecordPreviewResponse, ReusableAttribute, ReusableAttributeGroup,
    },
};

mod agent_settings;
mod agents;
mod audit_events;
mod avatars;
mod blueprint_connector_jobs;
mod blueprint_migration_batches;
mod blueprints;
mod bootstrap;
mod checks;
mod contexts;
mod domain_events;
mod error_codes;
mod extension_annotations;
mod extension_attricat_commands;
mod extension_attricat_data;
mod extension_connections;
mod extension_connector_pages;
mod extension_export_state;
mod extension_host_resources;
mod extension_interactive_operations;
mod extension_operation_artifacts;
mod extension_operation_http_transfer;
mod extension_operation_schedules;
mod extension_operation_staging;
mod extension_operations;
mod extension_registries;
mod extension_scoped_configuration;
mod extension_storage;
mod extensions;
mod files;
mod generations;
mod health;
mod leadership;
mod lexicon;
mod members;
mod notifications;
mod presentation_assets;
mod record_batches;
mod record_commands;
mod record_comments;
mod record_migration;
mod record_projection;
mod record_publications;
mod record_search;
mod record_values;
mod references;
mod retention;
mod retention_holds;
mod reusable_attributes;
mod roles;
mod rules;
mod saved_views;
mod scope;
mod search_scope;
mod sessions;
mod solution_packs;
mod status;
mod structural_constraints;
mod system_annotations;
mod system_users;
mod tasks;
mod teams;
mod tokens;
mod upload_intents;
mod values;
mod workflow_actions;
mod workflow_runs;
mod workflows;
mod workspace_navigation;
mod write_context;

/// System tag that marks records created as solution-pack sample data. Only
/// the host may add it; ordinary tag writes and record copies drop it.
pub(crate) const SAMPLE_SYSTEM_TAG: &str = "attricat.sample";

/// Violations named in a summary message; the rest are omitted.
const MAX_SUMMARIZED_VIOLATIONS: usize = 5;

fn summarize_violations(violations: &[CheckViolation]) -> String {
    violations
        .iter()
        .take(MAX_SUMMARIZED_VIOLATIONS)
        .map(|violation| format!("{} ({})", violation.message, violation.code))
        .collect::<Vec<_>>()
        .join("; ")
}

pub use agent_settings::{
    AgentSettings, BulkEditPolicy, DEFAULT_BULK_EDIT_MAX_RECORDS, MAX_BULK_EDIT_MAX_RECORDS,
};
pub use checks::{
    CheckSource, CheckTransition, CheckViolation, MAX_REPORTED_VIOLATIONS, PublicationReadiness,
};
pub use error_codes::{ErrorClass, ErrorCode, ErrorDescription};
pub use lexicon::{LexiconEntry, LexiconImportMode, LexiconImportSummary};
pub use notifications::{
    MAX_NOTIFICATION_BODY_LENGTH, MAX_NOTIFICATION_TITLE_LENGTH, NOTIFICATION_PAGE_SIZE,
    NewNotification, Notification, NotificationCursor, NotificationSubject,
    NotificationSubjectKind, kinds as notification_kinds, notification_excerpt,
};
pub use record_comments::{COMMENT_PAGE_SIZE, RecordComment};
pub use saved_views::SavedView;
pub use structural_constraints::UniqueKeyDuplicate;
pub use system_users::{SYSTEM_USER_DISPLAY_NAME, system_user_email};

pub use agents::{
    AgentRun, AgentRunEvent, AgentToolCall, ApprovalDecision, Conversation, ConversationMessage,
};
pub use attricat_cache::QueryCache;
pub use attricat_domain::model::{FileMetadata, FileVariantMetadata};
pub use audit_events::{AuditEventFilter, AuditEventPage};
pub use avatars::{AVATAR_VARIANT_KIND, OwnAvatar};
pub use blueprint_connector_jobs::{
    BlueprintConnectorJob, ConnectorRunDetails, ConnectorRunMode, MAX_CONNECTOR_RECORD_FILTER,
};
pub use domain_events::{
    EventConsumer, EventDelivery, EventPublisher, FailedEventDelivery, MaterializeScope,
};
pub use extension_annotations::{
    ExtensionAnnotationNamespace, ExtensionAnnotationPatch, ExtensionAnnotations,
    MAX_ANNOTATION_PATCH_OPERATIONS,
};
pub use extension_attricat_data::{
    ExtensionAttricatBatch, ExtensionAttricatChangePage, ExtensionAttricatIntent,
    ExtensionAttricatIntentOutcome, ExtensionAttricatIntentStatus, ExtensionAttricatPage,
    ExtensionAttricatPageRequest, MAX_EXTENSION_ATTRICAT_PAGE_SIZE, MAX_EXTENSION_BATCH_INTENTS,
    MAX_EXTENSION_LOOKUP_VALUE_BYTES,
};
pub use extension_connections::{
    ConnectionAccessToken, ConnectionCredentials, ConnectionRefresh, DeviceAuthorization,
    ExtensionConnection, ExtensionConnectionPatch, IssuedTokens, NewExtensionConnection,
    PUBLIC_FILE_LINK_GRANTS, use_extension_credential_key,
};
pub use extension_connector_pages::{
    ConnectorPageV2Request, MAX_EXPANDED_RELATIONSHIPS, MAX_WITHDRAWN_PAGE,
};
pub use extension_export_state::{
    EXPORT_STATUSES, ExportBegin, ExportConnectionLabel, ExportMapping, ExportMappingUpdate,
    ExportRetrySelection, ExportStatusCount, HandlerAttempt, HandlerAttemptOutcome, HandlerFailure,
    MAX_EXPORT_LEASE, MAX_EXPORT_MAPPING_PAGE, MAX_EXPORT_RETRY_RECORDS, MappingKey,
    RejectionRecord, valid_connection_key,
};
pub use extension_interactive_operations::{
    INITIATOR_ACCESS_REVOKED, InteractiveRun, InteractiveRunArtifact, InteractiveRunFailure,
    InteractiveRunScope, InteractiveRunStatus, OPERATION_OUTPUT_RETENTION_DAYS,
    StartInteractiveOperation,
};
pub use extension_operation_artifacts::{ExtensionOperationArtifact, MAX_OPERATION_ARTIFACT_BYTES};
pub use extension_operation_http_transfer::{
    CompletedHttpInput, DeliveryState, ExtensionHttpDelivery,
};
pub use extension_operation_schedules::{
    CreateExtensionOperationSchedule, ExtensionOperationSchedule,
};
pub use extension_operations::{
    ClaimedExtensionOperationRun, ExtensionOperationRun, StartExtensionOperation,
};
pub use extension_registries::ExtensionRegistrySource;
pub use extension_scoped_configuration::ExtensionConfigurationScope;
pub use extension_storage::{
    ExtensionStorageEntry, ExtensionStorageError, ExtensionStoragePage,
    MAX_EXTENSION_STORAGE_LIST_LIMIT,
};
pub use extensions::{
    ClientExtensionContribution, ExportDestinationDescriptor, ExtensionGrant,
    ExtensionInstallation, ExtensionLifecycleRecord, ExtensionRuntimeInstallation, ExtensionState,
    InstalledExtension, extension_manifest_sha256, required_extension_grants,
};
pub use files::{FileObject, FilePolicy, FileUploadResult, NewUploadedFile, StagedUploadResult};
pub use generations::WorkspaceGenerations;
pub use leadership::CoordinatorLeadership;
pub use members::{WorkspaceInvitation, WorkspaceMember};
pub use presentation_assets::{MAX_PRESENTATION_ASSET_PAGE_SIZE, PresentationAsset};
pub use record_search::{
    RecordRelationshipFilter, RecordSearchFilter, RecordSearchScope, RecordSearchSort,
    RelationshipFacetHop, SEARCH_FILTER_EQ_ANY, SearchContext, decode_search_cursor,
};
pub use retention::RetentionDays;
pub use retention_holds::FileRetentionHold;
pub use roles::{Permission, WorkspaceGrantTarget, WorkspaceRole};
pub use rules::{ClaimedRuleRun, RuleCandidateResult};
pub use search_scope::{
    MAX_SEARCH_FILTERS, SearchScope, SearchScopeError, SearchScopeRequest, intersect_record_ids,
};
pub use solution_packs::SolutionPackCheckResult;
pub use solution_packs::{
    CreateSolutionPackPlanRequest, PlannedExtensionInstall, SolutionPackApplication,
    SolutionPackApplicationSummary, SolutionPackCheckRun, SolutionPackCheckRunSummary,
    SolutionPackPlan,
};
pub use status::{
    RECORD_CONTROL_PAGE_SIZE, RecordApproval, StatusTransitionAccess, UnlockRecordStatus,
};
pub use tasks::{
    BackgroundProcessingStatus, ClaimedTask, TaskError, TaskSummary, bounded_task_error_message,
};
pub use teams::{
    DirectoryTeam, DirectoryUser, DirectoryUserProfile, DirectoryUserTeam, Team, WorkspaceDirectory,
};
pub use tokens::{AuthenticatedToken, PersonalApiToken};
pub use upload_intents::AbandonedUpload;
pub use workflow_runs::WorkflowRun;
pub use workflow_runs::{ClaimedWorkflowRun, WorkflowActionResult, WorkflowRunTarget};
pub use workspace_navigation::{ExploreNavigationEntry, ExploreNavigationItem};

#[derive(Debug, sqlx::FromRow)]
pub struct UserAccount {
    pub display_name: Option<String>,
    pub email: String,
    /// IANA time zone name; `None` follows the client's own zone.
    pub time_zone: Option<String>,
}

#[derive(Clone)]
/// The stable catalog persistence facade. Feature modules add inherent methods
/// here so HTTP handlers and other callers do not depend on storage internals.
pub struct AttricatRepository<S = WorkspaceScope> {
    pub(crate) pool: PgPool,
    workspace_id: S,
    audit_context: Option<AuditContext>,
    event_context: Option<EventCommandContext>,
    task_fence: Option<TaskFence>,
    extension_id: Option<String>,
    /// The signed-in principal whose current access bounds an interactive
    /// extension run. It is rechecked at each host call; selection membership
    /// is never treated as permission.
    authorization_actor: Option<AuthorizationActor>,
    /// The status attribute an owner or administrator is unlocking: this
    /// write may leave its locking status for any option, outside the
    /// declared transitions. Set only by `unlock_record_status`.
    status_unlock: Option<String>,
    /// Shared by every repository derived from the same composition root.
    pub(crate) cache: QueryCache,
    /// The workspace's generations as read by this request's authentication.
    /// Generation-keyed caches are consulted only when this is known.
    generations: Option<WorkspaceGenerations>,
}

/// A user, and optionally the personal API token, that initiated an
/// interactive extension run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorizationActor {
    pub user_id: Uuid,
    pub token_id: Option<Uuid>,
}

/// Workspace data methods exist only on this scope; it cannot be absent.
#[derive(Clone, Copy)]
pub struct WorkspaceScope(Uuid);

/// Authentication, bootstrap and queue coordination have no implicit tenant.
#[derive(Clone, Copy)]
pub struct SystemScope;

/// A process-level repository has no implicit workspace and cannot read
/// workspace data until explicitly scoped.
///
/// ```compile_fail
/// # use attricat_repository::repository::AttricatRepository;
/// # async fn example(pool: sqlx::PgPool) {
/// AttricatRepository::system(pool).list_contexts().await;
/// # }
/// ```
pub type SystemRepository = AttricatRepository<SystemScope>;

mod scope_sealed {
    pub trait Sealed {}
}
impl scope_sealed::Sealed for WorkspaceScope {}
impl scope_sealed::Sealed for SystemScope {}

pub trait RepositoryScope: scope_sealed::Sealed + Clone + Send + Sync {
    #[doc(hidden)]
    fn audit_workspace(&self) -> Option<Uuid>;
}
impl RepositoryScope for WorkspaceScope {
    fn audit_workspace(&self) -> Option<Uuid> {
        Some(self.0)
    }
}
impl RepositoryScope for SystemScope {
    fn audit_workspace(&self) -> Option<Uuid> {
        None
    }
}

#[derive(Clone)]
pub struct TaskFence {
    pub task_id: Uuid,
    pub lease_owner: String,
    pub lease_token: Uuid,
}

#[derive(Clone)]
pub struct EventCommandContext {
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub handler_name: String,
    pub initiating_actor_user_id: Option<Uuid>,
    pub initiating_actor_token_id: Option<Uuid>,
    pub workflow_causal_depth: usize,
    pub workflow_root_trigger_event_id: Uuid,
}

/// Server-derived request metadata written with the same transaction as a
/// successful mutation. Requests denied before a repository mutation are not
/// audited.
#[derive(Clone)]
pub struct AuditContext {
    pub actor_user_id: Option<Uuid>,
    pub actor_token_id: Option<Uuid>,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub action: String,
    pub authorization_scope: Value,
    pub target: Value,
    pub metadata: Value,
    pub agent: Option<AgentAuditAttribution>,
}

/// Durable executor and approval provenance for an agent mutation. This is
/// stored in normalized audit columns, not reconstructed from metadata.
#[derive(Clone)]
pub struct AuditEventChange {
    pub record_id: Uuid,
    pub attribute_id: Uuid,
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub context_code: Option<String>,
    pub relationship_target_record_id: Option<Uuid>,
    pub change_kind: &'static str,
    pub before_value: Option<Value>,
    pub after_value: Option<Value>,
}

#[derive(Clone)]
pub struct AgentAuditAttribution {
    pub run_id: Uuid,
    pub conversation_id: Uuid,
    pub tool_call_id: Uuid,
    pub tool_name: String,
    pub approval_decision: Option<String>,
    pub approved_by_user_id: Option<Uuid>,
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
        values: Value,
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
    #[error("catalog code is already in use")]
    AttricatCodeTaken,
    #[error("invalid team: {0}")]
    InvalidTeam(String),
    #[error("invalid agent settings: {0}")]
    InvalidAgentSettings(String),
    #[error("workflow code is already in use")]
    WorkflowCodeTaken,
    #[error("invalid workflow definition: {0}")]
    InvalidWorkflowDefinition(String),
    #[error("rule code is already in use")]
    RuleCodeTaken,
    #[error("invalid rule definition: {0}")]
    InvalidRuleDefinition(String),
    #[error("invalid publication channel: {0}")]
    InvalidPublicationChannel(String),
    #[error("rule revision must be published before it can be enabled")]
    RuleNotPublished,
    #[error("rule has no enabled revision to run; enable it first, or start a dry run")]
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
    #[error("{0}")]
    ExportMappingConflict(String),
    #[error("a connection with this name already exists")]
    ExtensionConnectionExists,
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

pub(super) async fn lock_workspace_resource_code(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    code: &str,
) -> Result<(), RepositoryError> {
    let lock_key = format!("{workspace_id}:{code}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(lock_key)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

pub(super) async fn workspace_resource_code_matches(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    code: &str,
) -> Result<Vec<(String, Uuid)>, RepositoryError> {
    Ok(sqlx::query_as::<_, (String, Uuid)>(
        "SELECT 'blueprint'::text,id FROM blueprints WHERE workspace_id=$1 AND code=$2 UNION ALL SELECT 'context'::text,id FROM attribute_contexts WHERE workspace_id=$1 AND code=$2",
    )
    .bind(workspace_id)
    .bind(code)
    .fetch_all(&mut **transaction)
    .await?)
}

impl AttricatRepository {
    #[cfg(test)]
    const DEFAULT_WORKSPACE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

    /// The workspace this repository is scoped to.
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id.0
    }

    /// Creates a repository that is scoped to a workspace at construction.
    /// Request handlers and task handlers should use this constructor (or a
    /// scope derived with [`Self::for_workspace`]).
    pub fn new(pool: PgPool, workspace_id: Uuid) -> Self {
        AttricatRepository::system(pool).for_workspace(workspace_id)
    }

    /// Creates the deliberately unscoped session/maintenance repository.
    ///
    /// This is reserved for bootstrap, authentication/session lookup, and task
    /// queue claiming. It must not be placed in a request extension or passed
    /// to a task handler that accesses workspace-owned data.
    pub fn system(pool: PgPool) -> SystemRepository {
        AttricatRepository {
            pool,
            workspace_id: SystemScope,
            audit_context: None,
            event_context: None,
            task_fence: None,
            extension_id: None,
            authorization_actor: None,
            status_unlock: None,
            cache: QueryCache::default(),
            generations: None,
        }
    }

    pub fn for_event_handler(
        &self,
        event: &crate::domain_events::DomainEvent,
        handler_name: &str,
    ) -> Self {
        let mut repository = self.clone();
        repository.event_context = Some(EventCommandContext {
            correlation_id: event.correlation_id,
            causation_id: event.id,
            handler_name: handler_name.to_owned(),
            initiating_actor_user_id: event_metadata_uuid(event, "initiating_actor_user_id"),
            initiating_actor_token_id: event_metadata_uuid(event, "initiating_actor_token_id"),
            workflow_causal_depth: event
                .metadata
                .get(workflow_runs::CAUSAL_DEPTH_METADATA_KEY)
                .and_then(Value::as_u64)
                .filter(|depth| *depth <= workflow_runs::MAX_WORKFLOW_CAUSAL_DEPTH as u64)
                .unwrap_or(0) as usize,
            workflow_root_trigger_event_id: event
                .metadata
                .get(workflow_runs::ROOT_TRIGGER_EVENT_METADATA_KEY)
                .and_then(Value::as_str)
                .and_then(|id| id.parse().ok())
                .unwrap_or(event.id),
        });
        // Background mutations must retain audit evidence even though there is
        // no HTTP request audit middleware. The original initiating actor is
        // recovered from the durable event envelope, never supplied by a worker.
        if repository.audit_context.is_none() {
            repository.audit_context = Some(AuditContext {
                actor_user_id: event_metadata_uuid(event, "initiating_actor_user_id"),
                actor_token_id: event_metadata_uuid(event, "initiating_actor_token_id"),
                request_id: Uuid::new_v4(),
                correlation_id: event.correlation_id,
                action: format!("{}.execute", handler_name),
                authorization_scope: serde_json::json!({"worker": handler_name}),
                target: serde_json::json!({"trigger_event_id": event.id}),
                metadata: serde_json::json!({"trigger_event_id": event.id, "causation_id": event.id, "worker_source": handler_name}),
                agent: None,
            });
        }
        repository
    }

    /// Attaches the shared task lease to all extension-triggered mutations.
    /// Every commit re-checks this token in its own transaction, so a stale
    /// delivery cannot write catalog facts after a reclaim.
    pub fn for_event_delivery_task(
        &self,
        event: &crate::domain_events::DomainEvent,
        handler_name: &str,
        task: &ClaimedTask,
    ) -> Self {
        self.for_event_handler(event, handler_name).for_task(task)
    }

    /// Attaches a claimed task's lease to every subsequent write. Each commit
    /// re-checks the exact token in its own transaction, so a reclaimed
    /// execution (agent run, workflow action, rule checkpoint, extension
    /// operation, blueprint migration batch) cannot persist anything.
    pub fn for_task(&self, task: &ClaimedTask) -> Self {
        let mut repository = self.clone();
        repository.task_fence = Some(TaskFence {
            task_id: task.id,
            lease_owner: task.lease_owner.clone(),
            lease_token: task.lease_token,
        });
        repository
    }

    /// Adds immutable workflow run provenance to the worker audit/event context.
    pub fn for_workflow_run(
        &self,
        workflow_id: Uuid,
        revision: i64,
        run_id: Uuid,
        action_index: usize,
    ) -> Self {
        let mut repository = self.clone();
        if let Some(audit) = repository.audit_context.as_mut() {
            // Event sources include the immutable workflow UUID (`workflow:<id>`),
            // but audit action identifiers deliberately use the constrained,
            // stable operation name rather than a namespaced source identifier.
            audit.action = "workflow.execute".to_owned();
            audit.metadata = serde_json::json!({
                "workflow_id": workflow_id,
                "workflow_revision": revision,
                "workflow_run_id": run_id,
                "trigger_event_id": repository.event_context.as_ref().map(|context| context.causation_id),
                "action_index": action_index,
                "workflow_causal_depth": repository.event_context.as_ref().map(|context| context.workflow_causal_depth + 1).unwrap_or(1),
                "workflow_root_trigger_event_id": repository.event_context.as_ref().map(|context| context.workflow_root_trigger_event_id),
            });
            audit.target = serde_json::json!({"workflow_id": workflow_id, "workflow_revision": revision, "run_id": run_id, "trigger_event_id": repository.event_context.as_ref().map(|context| context.causation_id), "action_index": action_index});
        }
        repository
    }

    /// Marks a mutation as extension-originated. Event-handler invocations
    /// retain their triggering correlation and causation; browser commands keep
    /// the authenticated request correlation and have no causation ID.
    pub fn for_extension(&self, extension_id: &str) -> Self {
        let mut repository = self.clone();
        repository.extension_id = Some(extension_id.to_owned());
        // Browser commands retain their authenticated request audit context.
        // Event deliveries have no browser request, so synthesize durable plugin
        // audit provenance before an extension can mutate catalog state.
        if repository.audit_context.is_none() {
            repository.audit_context = Some(AuditContext {
                actor_user_id: repository
                    .event_context
                    .as_ref()
                    .and_then(|context| context.initiating_actor_user_id),
                actor_token_id: repository
                    .event_context
                    .as_ref()
                    .and_then(|context| context.initiating_actor_token_id),
                request_id: Uuid::new_v4(),
                correlation_id: repository
                    .event_context
                    .as_ref()
                    .map(|context| context.correlation_id)
                    .unwrap_or_else(Uuid::new_v4),
                action: "attricat.extensions.attribute_values.write".to_owned(),
                authorization_scope: serde_json::json!({
                    "capability": "attricat.write",
                    "extension_id": extension_id,
                }),
                target: serde_json::json!({ "type": "extension", "id": extension_id }),
                metadata: serde_json::json!({ "extension_id": extension_id }),
                agent: None,
            });
        }
        // A handler already has worker audit provenance, but extension identity
        // is still required to attribute that handler's catalog mutations.
        if let Some(audit) = repository.audit_context.as_mut()
            && let Some(metadata) = audit.metadata.as_object_mut()
        {
            metadata.insert(
                "extension_id".to_owned(),
                Value::String(extension_id.to_owned()),
            );
        }
        repository
    }
}

fn event_metadata_uuid(event: &crate::domain_events::DomainEvent, key: &str) -> Option<Uuid> {
    event.metadata.get(key)?.as_str()?.parse().ok()
}

fn initiating_actor_metadata(audit: Option<&AuditContext>) -> Value {
    let mut metadata = Value::Object(serde_json::Map::new());
    add_initiating_actor_metadata(&mut metadata, audit);
    metadata
}

fn event_metadata(audit: Option<&AuditContext>, is_worker: bool) -> Value {
    let mut metadata = initiating_actor_metadata(audit);
    if is_worker
        && let (Some(target), Some(source)) = (
            metadata.as_object_mut(),
            audit.map(|context| &context.metadata),
        )
        && let Some(source) = source.as_object()
    {
        target.extend(source.clone());
    }
    metadata
}

fn add_initiating_actor_metadata(metadata: &mut Value, audit: Option<&AuditContext>) {
    let Some(metadata) = metadata.as_object_mut() else {
        return;
    };
    if let Some(actor_user_id) = audit.and_then(|context| context.actor_user_id) {
        metadata.insert(
            "initiating_actor_user_id".to_owned(),
            Value::String(actor_user_id.to_string()),
        );
    }
    if let Some(actor_token_id) = audit.and_then(|context| context.actor_token_id) {
        metadata.insert(
            "initiating_actor_token_id".to_owned(),
            Value::String(actor_token_id.to_string()),
        );
    }
}

fn system_audit_context(action: &str, target: Value, metadata: Value) -> AuditContext {
    AuditContext {
        actor_user_id: None,
        actor_token_id: None,
        request_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        action: action.to_owned(),
        authorization_scope: serde_json::json!({"type": "workspace"}),
        target,
        metadata,
        agent: None,
    }
}

impl<S> AttricatRepository<S> {
    /// Uses `cache` for this repository and every repository derived from it.
    /// The composition root shares one cache across requests and workers.
    pub fn with_cache(mut self, cache: QueryCache) -> Self {
        self.cache = cache;
        self
    }

    pub fn cache(&self) -> &QueryCache {
        &self.cache
    }

    /// Records the workspace generations this request's authentication read.
    pub fn with_generations(mut self, generations: Option<WorkspaceGenerations>) -> Self {
        self.generations = generations;
        self
    }

    pub fn generations(&self) -> Option<WorkspaceGenerations> {
        self.generations
    }
}

impl AttricatRepository {
    pub fn with_audit_context(mut self, audit_context: AuditContext) -> Self {
        self.audit_context = Some(audit_context);
        self
    }

    /// Returns a copy that audits a domain-named workspace action. A request
    /// context keeps its actor and request identifiers; without one, the event
    /// is attributed to the system. `metadata` fields are merged over the
    /// request's metadata.
    pub(crate) fn with_workspace_audit_action(
        &self,
        action: &str,
        target: Value,
        metadata: Value,
    ) -> Self {
        let audit = match self.audit_context.clone() {
            Some(mut audit) => {
                audit.action = action.to_owned();
                audit.target = target;
                if let (Some(existing), Value::Object(fields)) =
                    (audit.metadata.as_object_mut(), metadata)
                {
                    existing.extend(fields);
                }
                audit
            }
            None => system_audit_context(action, target, metadata),
        };
        let mut repository = self.clone();
        repository.audit_context = Some(audit);
        repository
    }

    /// Returns a copy that audits as-is under a request context, or as the
    /// system with the given action when no request is attached.
    pub(crate) fn with_system_audit_fallback(
        &self,
        action: &str,
        target: Value,
        metadata: Value,
    ) -> Self {
        let mut repository = self.clone();
        if repository.audit_context.is_none() {
            repository.audit_context = Some(system_audit_context(action, target, metadata));
        }
        repository
    }

    /// Bounds subsequent user-initiated operations and related-record reads
    /// by this principal's current grants and personal-token permissions.
    pub fn with_authorization_actor(mut self, actor: AuthorizationActor) -> Self {
        self.authorization_actor = Some(actor);
        self
    }

    pub fn authorization_actor(&self) -> Option<AuthorizationActor> {
        self.authorization_actor
    }

    /// Filters related-record hydration without changing unrestricted system
    /// reads. The caller still authorizes the root record separately.
    pub(crate) async fn actor_readable_record_ids(
        &self,
        record_ids: &[Uuid],
    ) -> Result<Option<HashSet<Uuid>>, RepositoryError> {
        let Some(actor) = self.authorization_actor else {
            return Ok(None);
        };
        let mut connection = self.pool.acquire().await?;
        Self::principal_record_ids_on(
            &mut connection,
            actor,
            self.workspace_id.0,
            "records.read",
            record_ids,
        )
        .await
        .map(Some)
    }

    /// Rechecks the interactive actor's live membership, grant scope and token
    /// permission for every record; see [`Self::ensure_principal_may`].
    /// Repositories without an actor are governed by their caller's
    /// authorization and are not restricted here.
    pub(crate) async fn ensure_actor_may(
        &self,
        connection: &mut sqlx::PgConnection,
        permission: &str,
        record_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let Some(actor) = self.authorization_actor else {
            return Ok(());
        };
        self.ensure_principal_may(connection, actor, permission, record_ids)
            .await
    }

    /// Checks, in one query, that a principal's current grants and, for a
    /// token, its live token permission cover every record. A workspace
    /// grant authorizes IDs that do not exist, so callers still verify
    /// existence themselves.
    pub(crate) async fn ensure_principal_may(
        &self,
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        permission: &str,
        record_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let permitted = Self::principal_record_ids_on(
            connection,
            actor,
            self.workspace_id.0,
            permission,
            record_ids,
        )
        .await?;
        if record_ids.iter().all(|id| permitted.contains(id)) {
            Ok(())
        } else {
            Err(RepositoryError::ActorNotAuthorized)
        }
    }

    /// Commits a catalog mutation, its audit evidence, and an outbox event as
    /// one database transaction. Callers retain control of event vocabulary.
    pub(crate) async fn commit_mutation_with_event(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        event: crate::domain_events::NewDomainEvent,
    ) -> Result<(), RepositoryError> {
        self.ensure_task_fence(&mut transaction).await?;
        self.write_audit_event(&mut transaction).await?;
        self.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Appends a declared extension event through the durable outbox. The
    /// caller supplies an invocation-time authorized release snapshot.
    pub async fn emit_extension_event(
        &self,
        extension_id: &str,
        installed_release_id: Uuid,
        contract_id: &str,
        aggregate_kind: &str,
        aggregate_id: Uuid,
        payload: Value,
    ) -> Result<(), RepositoryError> {
        let Some(installation) = self
            .runtime_extension_installation(extension_id, installed_release_id)
            .await?
        else {
            return Err(RepositoryError::InvalidExtension(
                "extension invocation is no longer authorized".into(),
            ));
        };
        self.emit_extension_event_with(
            &installation,
            contract_id,
            aggregate_kind,
            aggregate_id,
            payload,
        )
        .await
    }

    /// [`Self::emit_extension_event`] for a host call that has just refreshed
    /// its authorized installation snapshot.
    pub async fn emit_extension_event_with(
        &self,
        installation: &ExtensionRuntimeInstallation,
        contract_id: &str,
        aggregate_kind: &str,
        aggregate_id: Uuid,
        payload: Value,
    ) -> Result<(), RepositoryError> {
        let extension_id = installation.extension_id.as_str();
        let contract = installation
            .manifest
            .event_contracts
            .exports
            .iter()
            .find(|item| item.id == contract_id)
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "event contract is not declared by this release".into(),
                )
            })?;
        let bytes = serde_json::to_vec(&payload).map_err(|_| {
            RepositoryError::InvalidExtension("event payload cannot be serialized".into())
        })?;
        if bytes.len() > contract.max_payload_bytes as usize {
            return Err(RepositoryError::InvalidExtension(
                "event payload exceeds contract limit".into(),
            ));
        }
        crate::extensions::validate_schema(&contract.schema, &payload)
            .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
        let repository = self.for_extension(extension_id);
        let mut transaction = repository.pool.begin().await?;
        repository.ensure_task_fence(&mut transaction).await?;
        let mut event =
            repository.core_event(&contract.event_type, aggregate_kind, aggregate_id, payload);
        event.metadata = serde_json::json!({"event_contract": contract_id, "event_contract_version": contract.version});
        repository.write_audit_event(&mut transaction).await?;
        repository.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(crate) fn core_event(
        &self,
        event_type: &str,
        aggregate_kind: &str,
        aggregate_id: Uuid,
        payload: Value,
    ) -> NewDomainEvent {
        NewDomainEvent {
            event_type: event_type.to_owned(),
            aggregate_kind: aggregate_kind.to_owned(),
            aggregate_id,
            correlation_id: self
                .event_context
                .as_ref()
                .map(|context| context.correlation_id)
                .or_else(|| {
                    self.audit_context
                        .as_ref()
                        .map(|audit| audit.correlation_id)
                })
                .unwrap_or_else(Uuid::new_v4),
            causation_id: self
                .event_context
                .as_ref()
                .map(|context| context.causation_id),
            source: EventSource {
                kind: if self.extension_id.is_some() {
                    EventSourceKind::Plugin
                } else if self.event_context.is_some() {
                    EventSourceKind::Worker
                } else {
                    EventSourceKind::Api
                },
                name: self
                    .extension_id
                    .as_ref()
                    .map(|extension_id| format!("extension:{extension_id}"))
                    .or_else(|| {
                        self.event_context
                            .as_ref()
                            .map(|context| context.handler_name.clone())
                    })
                    .unwrap_or_else(|| "attricat_api".to_owned()),
            },
            metadata: event_metadata(self.audit_context.as_ref(), self.event_context.is_some()),
            payload,
        }
    }

    pub(crate) fn affected_facts(changes: &[AuditEventChange]) -> Vec<AffectedFactV1> {
        changes
            .iter()
            .map(|change| AffectedFactV1 {
                attribute_id: change.attribute_id,
                attribute_code: change.attribute_code.clone(),
                context_id: change.context_id,
                context_code: change.context_code.clone(),
                relationship_target_record_id: change.relationship_target_record_id,
                change_kind: change.change_kind.to_owned(),
                before_value: change.before_value.clone(),
                after_value: change.after_value.clone(),
            })
            .collect()
    }

    pub(crate) async fn commit_record_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        changes: Vec<AuditEventChange>,
        event: NewDomainEvent,
    ) -> Result<(), RepositoryError> {
        self.stage_record_mutation(&mut transaction, changes, event)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(in crate::repository) async fn stage_record_mutation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        changes: Vec<AuditEventChange>,
        mut event: NewDomainEvent,
    ) -> Result<(), RepositoryError> {
        self.ensure_task_fence(transaction).await?;
        let retained = self
            .reconcile_record_publication(transaction, event.aggregate_id, "record_changed")
            .await?;
        let publication_metadata = match retained {
            Some(retained) => serde_json::json!({
                "disposition": "retained",
                "role_code": retained.role_code,
                "withdrawn_context_ids": retained.withdrawn_context_ids,
            }),
            None => serde_json::json!({
                "disposition": "withdrawn",
                "reason": "record_changed",
            }),
        };
        event.metadata["publication"] = publication_metadata.clone();
        if let Some(audit_event_id) = self
            .write_audit_event_with_publication_metadata(transaction, Some(publication_metadata))
            .await?
            && !changes.is_empty()
        {
            let mut ids = Vec::with_capacity(changes.len());
            let mut record_ids = Vec::with_capacity(changes.len());
            let mut attribute_ids = Vec::with_capacity(changes.len());
            let mut attribute_codes = Vec::with_capacity(changes.len());
            let mut context_ids = Vec::with_capacity(changes.len());
            let mut context_codes = Vec::with_capacity(changes.len());
            let mut change_kinds = Vec::with_capacity(changes.len());
            let mut before_values = Vec::with_capacity(changes.len());
            let mut after_values = Vec::with_capacity(changes.len());
            for change in changes {
                ids.push(Uuid::new_v4());
                record_ids.push(change.record_id);
                attribute_ids.push(change.attribute_id);
                attribute_codes.push(change.attribute_code);
                context_ids.push(change.context_id);
                context_codes.push(change.context_code);
                change_kinds.push(change.change_kind);
                before_values.push(change.before_value);
                after_values.push(change.after_value);
            }
            sqlx::query("INSERT INTO audit_event_changes (id, audit_event_id, workspace_id, record_id, attribute_id, attribute_code, context_id, context_code, change_kind, before_value, after_value) SELECT id, $2, $3, record_id, attribute_id, attribute_code, context_id, context_code, change_kind, before_value, after_value FROM UNNEST($1::uuid[], $4::uuid[], $5::uuid[], $6::text[], $7::uuid[], $8::text[], $9::text[], $10::jsonb[], $11::jsonb[]) AS change(id, record_id, attribute_id, attribute_code, context_id, context_code, change_kind, before_value, after_value)")
                .bind(ids)
                .bind(audit_event_id)
                .bind(self.workspace_id.0)
                .bind(record_ids)
                .bind(attribute_ids)
                .bind(attribute_codes)
                .bind(context_ids)
                .bind(context_codes)
                .bind(change_kinds)
                .bind(before_values)
                .bind(after_values)
                .execute(&mut **transaction)
                .await?;
        }
        self.enqueue_event(transaction, event).await?;
        Ok(())
    }

    /// Replaces the authenticated principal's display preferences. Callers
    /// validate the zone name; `None` clears the preference.
    pub async fn update_user_preferences(
        &self,
        user_id: Uuid,
        time_zone: Option<&str>,
    ) -> Result<(), RepositoryError> {
        self.update_active_user(user_id, "time_zone", time_zone)
            .await
    }

    /// Replaces the authenticated principal's display name. Callers validate
    /// the name before storing it.
    pub async fn update_user_display_name(
        &self,
        user_id: Uuid,
        display_name: &str,
    ) -> Result<(), RepositoryError> {
        self.update_active_user(user_id, "display_name", Some(display_name))
            .await
    }

    /// Sets one text `users` column for an active user, in an audited
    /// mutation.
    async fn update_active_user(
        &self,
        user_id: Uuid,
        column: &'static str,
        value: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let sql = format!(
            "UPDATE users SET {column} = $2, updated_at = clock_timestamp() WHERE id = $1 AND state = 'active'"
        );
        let updated = sqlx::query(&sql)
            .bind(user_id)
            .bind(value)
            .execute(&mut *transaction)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("user"));
        }
        self.commit_mutation(transaction).await
    }
}

pub fn validate_code(value: &str) -> Result<(), RepositoryError> {
    if is_valid_code(value) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCode)
    }
}

/// Reusable attributes are the only attribute selectors containing a colon;
/// both parts retain the catalog code grammar so local and reusable names
/// cannot collide accidentally.
pub fn validate_attribute_selector_code(value: &str) -> Result<(), RepositoryError> {
    if is_valid_code(value)
        || value
            .split_once(':')
            .is_some_and(|(namespace, code)| is_valid_code(namespace) && is_valid_code(code))
    {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCode)
    }
}

pub fn missing_required_fields(message: &str, target_codes: &HashSet<&str>) -> Vec<String> {
    let mut fields = HashSet::new();
    for delimiter in ['"', '\''] {
        for (index, value) in message.split(delimiter).enumerate() {
            if index % 2 == 1 && target_codes.contains(value) {
                fields.insert(value.to_owned());
            }
        }
    }
    fields.into_iter().collect()
}

impl<S: RepositoryScope> AttricatRepository<S> {
    /// Explicit provisioning/repair boundary, never part of a catalog read.
    pub async fn initialize_workspace(&self, workspace_id: Uuid) -> Result<(), RepositoryError> {
        // Creating the default context advances the context generation.
        sqlx::query(
            r#"WITH inserted AS (
                   INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id)
                   SELECT $1, id, 'default', '{}'::jsonb, NULL
                   FROM workspaces
                   WHERE id = $2 AND deleted_at IS NULL
                   ON CONFLICT (workspace_id, code) DO NOTHING
                   RETURNING workspace_id
               )
               UPDATE workspaces SET contexts_generation = contexts_generation + 1
               WHERE id IN (SELECT workspace_id FROM inserted)"#,
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .execute(&self.pool)
        .await?;
        self.ensure_workspace_system_user(workspace_id).await?;
        Ok(())
    }

    /// Inserts the request audit row before committing a mutation. An audit
    /// insertion error aborts the surrounding transaction, so success cannot
    /// be returned without durable audit evidence.
    async fn write_audit_event(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<Uuid>, RepositoryError> {
        self.write_audit_event_with_publication_metadata(transaction, None)
            .await
    }

    pub(crate) async fn write_audit_event_with_publication_metadata(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        publication_metadata: Option<Value>,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let Some(audit) = &self.audit_context else {
            return Ok(None);
        };
        let event_id = Uuid::new_v4();
        let agent = audit.agent.as_ref();
        let mut metadata = audit.metadata.clone();
        if let Some(publication_metadata) = publication_metadata {
            let metadata = metadata
                .as_object_mut()
                .expect("audit metadata is an object");
            metadata.insert("publication".to_owned(), publication_metadata);
        }
        sqlx::query(
            "INSERT INTO audit_events (id, workspace_id, actor_user_id, actor_token_id, request_id, correlation_id, action, authorization_scope, target, outcome, metadata, executor_type, agent_run_id, agent_conversation_id, agent_tool_call_id, agent_tool_name, approval_decision, approved_by_user_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'success', $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(event_id)
        .bind(self.workspace_id.audit_workspace().ok_or(RepositoryError::NotFound("audit workspace"))?)
        .bind(audit.actor_user_id)
        .bind(audit.actor_token_id)
        .bind(audit.request_id)
        .bind(audit.correlation_id)
        .bind(&audit.action)
        .bind(&audit.authorization_scope)
        .bind(&audit.target)
        .bind(metadata)
        .bind(if agent.is_some() { "agent" } else { "human" })
        .bind(agent.map(|agent| agent.run_id))
        .bind(agent.map(|agent| agent.conversation_id))
        .bind(agent.map(|agent| agent.tool_call_id))
        .bind(agent.map(|agent| &agent.tool_name))
        .bind(agent.and_then(|agent| agent.approval_decision.as_deref()))
        .bind(agent.and_then(|agent| agent.approved_by_user_id))
        .execute(&mut **transaction)
        .await?;
        Ok(Some(event_id))
    }

    pub(crate) async fn ensure_task_fence(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        let Some(fence) = &self.task_fence else {
            return Ok(());
        };
        // Locking the current envelope makes this check a fence for every
        // subsequent domain write in this transaction. A reclaimer cannot
        // replace the token until this transaction commits or rolls back.
        let valid: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tasks t WHERE t.id = $1 AND t.status = 'leased' AND t.lease_owner = $2 AND t.lease_token = $3 AND t.lease_until > clock_timestamp() FOR KEY SHARE OF t)")
            .bind(fence.task_id).bind(&fence.lease_owner).bind(fence.lease_token)
            .fetch_one(&mut **transaction).await?;
        if valid {
            Ok(())
        } else {
            Err(RepositoryError::Task(TaskError::LeaseLost))
        }
    }

    pub(crate) async fn commit_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        self.ensure_task_fence(&mut transaction).await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Returns only the authenticated principal's account fields. Callers must
    /// derive `user_id` from authentication rather than accepting it from a request.
    pub async fn user_account(&self, user_id: Uuid) -> Result<UserAccount, RepositoryError> {
        sqlx::query_as("SELECT display_name, email, time_zone FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::from)
    }

    /// Whether the user is active, and the workspace's cache generations, in
    /// one query.
    pub async fn active_user_generations(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(bool, Option<WorkspaceGenerations>), RepositoryError> {
        #[allow(clippy::type_complexity)]
        let (active, attricat, contexts, extensions, access): (bool, Option<i64>, Option<i64>, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND state = 'active'), w.attricat_generation, w.contexts_generation, w.extensions_generation, w.access_generation FROM (SELECT 1) one LEFT JOIN workspaces w ON w.id = $2 AND w.deleted_at IS NULL",
            )
            .bind(user_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let generations =
            WorkspaceGenerations::from_columns(attricat, contexts, extensions, access);
        Ok((active, generations))
    }

    pub async fn is_active_principal(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL)",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Checks the durable membership/grant graph in one database operation so
    /// callers cannot learn whether an out-of-scope target exists.
    /// Returns which of `permissions` the user holds workspace-wide, in one
    /// query. Equivalent to calling [`Self::is_authorized`] with no target for
    /// each permission: only active, workspace-scoped grants count.
    pub async fn workspace_permissions(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permissions: &[&str],
    ) -> Result<HashSet<String>, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::workspace_permissions_on(&mut connection, user_id, workspace_id, Some(permissions))
            .await
    }

    /// Every permission the user holds workspace-wide at `generations`, from
    /// the process-local cache. The key carries the access generation the
    /// request's credential query read, so a revoked grant, deactivated
    /// membership or edited role is never served afterwards.
    pub async fn cached_workspace_permissions(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        generations: WorkspaceGenerations,
    ) -> Result<Arc<HashSet<String>>, RepositoryError> {
        self.cache
            .fetch(
                CacheKey::new(
                    "workspace_permissions",
                    &[&workspace_id, &user_id, &generations.access_generation],
                ),
                &[],
                // System role permissions are bootstrapped at startup without
                // advancing any generation, so entries never leave the process.
                Policy::LocalGeneration,
                || async {
                    let mut connection = self.pool.acquire().await?;
                    Self::workspace_permissions_on(&mut connection, user_id, workspace_id, None)
                        .await
                },
            )
            .await
    }

    /// [`Self::is_authorized`] that first consults the user's cached
    /// workspace-wide permissions; a workspace-wide grant authorizes every
    /// target. Anything else, including a denial, is decided by the query.
    /// `generations` must have been read for `workspace_id` in this request.
    pub async fn is_authorized_at(
        &self,
        generations: Option<WorkspaceGenerations>,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        if let Some(generations) = generations
            && self
                .cached_workspace_permissions(user_id, workspace_id, generations)
                .await?
                .contains(permission)
        {
            return Ok(true);
        }
        self.is_authorized(user_id, workspace_id, permission, target_id, target_code)
            .await
    }

    /// `permissions` limits the result; `None` returns every held permission.
    async fn workspace_permissions_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permissions: Option<&[&str]>,
    ) -> Result<HashSet<String>, RepositoryError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT rp.permission_code FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN role_permissions rp ON rp.role_id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL AND g.scope_type = 'workspace' AND g.scope_target_id = $2 AND ($3::text[] IS NULL OR rp.permission_code = ANY($3))",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(permissions)
        .fetch_all(connection)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `record_ids` one principal may access with `permission`:
    /// the user's grants and, for a token, its live token permission.
    pub(crate) async fn principal_record_ids_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: &str,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty()
            || !Self::token_permits_on(connection, actor, workspace_id, permission).await?
        {
            return Ok(HashSet::new());
        }
        Self::authorized_record_ids_on(
            connection,
            actor.user_id,
            workspace_id,
            permission,
            record_ids,
        )
        .await
    }

    /// Whether one principal may act: the user's current grant for the target
    /// and, for a personal API token, its live token permission.
    pub async fn principal_may(
        &self,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::principal_may_on(
            &mut connection,
            actor,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await
    }

    pub(crate) async fn principal_may_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        Ok(Self::is_authorized_on(
            connection,
            actor.user_id,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await?
            && Self::token_permits_on(connection, actor, workspace_id, permission).await?)
    }

    /// Whether a principal's personal API token (if any) still carries
    /// `permission`. A session always passes.
    pub async fn principal_token_permits(
        &self,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: &str,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::token_permits_on(&mut connection, actor, workspace_id, permission).await
    }

    /// A live token's permission; a session (no token) always passes.
    async fn token_permits_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: &str,
    ) -> Result<bool, RepositoryError> {
        let Some(token_id) = actor.token_id else {
            return Ok(true);
        };
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM personal_api_tokens t JOIN personal_api_token_permissions p ON p.token_id = t.id WHERE t.id = $1 AND t.user_id = $2 AND t.workspace_id = $3 AND t.revoked_at IS NULL AND (t.expires_at IS NULL OR t.expires_at > clock_timestamp()) AND p.permission_code = $4)",
        )
        .bind(token_id)
        .bind(actor.user_id)
        .bind(workspace_id)
        .bind(permission)
        .fetch_one(&mut *connection)
        .await?)
    }

    /// Returns the subset of `record_ids` the user may access with
    /// `permission`, in one query. Equivalent to calling
    /// [`Self::is_authorized`] with each ID as the target: a workspace grant
    /// authorizes every ID, while record and blueprint-family grants require
    /// the record to exist in this workspace.
    pub async fn authorized_record_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut connection = self.pool.acquire().await?;
        Self::authorized_record_ids_on(
            &mut connection,
            user_id,
            workspace_id,
            permission,
            record_ids,
        )
        .await
    }

    async fn authorized_record_ids_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let sql = format!(
            "WITH {} SELECT requested.id FROM unnest($4::uuid[]) AS requested(id) LEFT JOIN records e ON e.id = requested.id AND e.workspace_id = $2 WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (e.id IS NOT NULL AND ((g.scope_type = 'record' AND g.scope_target_id = e.id) OR (g.scope_type = 'blueprint_family' AND g.scope_target_id = e.blueprint_id))))",
            GrantCodes::Permission(permission).grant_set_sql()
        );
        let rows: Vec<Uuid> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(GrantCodes::Permission(permission).codes())
            .bind(record_ids)
            .fetch_all(connection)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `target_codes` the user may access with `permission`,
    /// in one query. Equivalent to calling [`Self::is_authorized`] with each
    /// code as the target code and no target ID.
    pub async fn authorized_target_codes(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_codes: &[String],
    ) -> Result<HashSet<String>, RepositoryError> {
        if target_codes.is_empty() {
            return Ok(HashSet::new());
        }
        let codes = GrantCodes::Permission(permission);
        let sql = format!(
            "WITH RECURSIVE {}, requested AS (SELECT DISTINCT code FROM unnest($4::text[]) AS requested(code)), ancestors AS (SELECT r.code, c.id, c.parent_id FROM requested r JOIN attribute_contexts c ON c.workspace_id = $2 AND c.code = r.code UNION ALL SELECT a.code, p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT r.code FROM requested r WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM blueprints b WHERE b.workspace_id = $2 AND b.code = r.code AND b.id = g.scope_target_id)) OR (g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors a WHERE a.code = r.code AND a.id = g.scope_target_id)) OR (r.code = '__context_list__' AND g.scope_type = 'context_subtree'))",
            codes.grant_set_sql()
        );
        let rows: Vec<String> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_codes)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `target_ids` of one kind the user may access with
    /// `permission`, in one query. Equivalent to calling
    /// [`Self::is_authorized`] with each ID as the target: a workspace grant
    /// covers every ID, while scoped grants require the target to exist in
    /// this workspace.
    pub async fn authorized_target_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        kind: AuthorizationTargetKind,
        target_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if target_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let codes = GrantCodes::Permission(permission);
        let scoped = match kind {
            AuthorizationTargetKind::Record => {
                return self
                    .authorized_record_ids(user_id, workspace_id, permission, target_ids)
                    .await;
            }
            AuthorizationTargetKind::Blueprint => {
                "g.scope_type = 'blueprint_family' AND g.scope_target_id = r.id AND EXISTS (SELECT 1 FROM blueprints b WHERE b.workspace_id = $2 AND b.id = r.id)"
            }
            AuthorizationTargetKind::Context => {
                "g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors a WHERE a.requested_id = r.id AND a.id = g.scope_target_id)"
            }
        };
        let sql = format!(
            "WITH RECURSIVE {}, requested AS (SELECT DISTINCT id FROM unnest($4::uuid[]) AS requested(id)), ancestors AS (SELECT r.id AS requested_id, c.id, c.parent_id FROM requested r JOIN attribute_contexts c ON c.workspace_id = $2 AND c.id = r.id UNION ALL SELECT a.requested_id, p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT r.id FROM requested r WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR ({scoped}))",
            codes.grant_set_sql()
        );
        let rows: Vec<Uuid> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn is_authorized(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::is_authorized_on(
            &mut connection,
            user_id,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await
    }

    /// Transaction-owning callers must reuse their connection rather than
    /// acquiring a second pool slot while holding the first.
    async fn is_authorized_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        Self::is_granted_on(
            connection,
            user_id,
            workspace_id,
            GrantCodes::Permission(permission),
            target_id,
            target_code,
        )
        .await
    }

    /// Whether the user holds a matching grant whose scope covers the
    /// target: the workspace, a blueprint family, a record (directly or
    /// through its blueprint family) or a context subtree (through the
    /// context's ancestors).
    pub(crate) async fn is_granted_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        codes: GrantCodes<'_>,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        // Resolve grant scope in the application-owned repository query. A
        // non-workspace grant can only authorize the requested tenant target.
        let sql = format!(
            "WITH RECURSIVE {}, target AS (SELECT 'blueprint'::text kind, id FROM blueprints WHERE workspace_id = $2 AND (id = $4 OR code = $5) UNION ALL SELECT 'record', id FROM records WHERE workspace_id = $2 AND id = $4 UNION ALL SELECT 'context', id FROM attribute_contexts WHERE workspace_id = $2 AND (id = $4 OR code = $5)), ancestors AS (SELECT c.id, c.parent_id FROM attribute_contexts c JOIN target t ON t.kind = 'context' AND t.id = c.id UNION ALL SELECT p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (($4 IS NOT NULL OR $5 IS NOT NULL) AND ((g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM target WHERE kind = 'blueprint' AND id = g.scope_target_id)) OR (g.scope_type = 'record' AND EXISTS (SELECT 1 FROM target WHERE kind = 'record' AND id = g.scope_target_id)) OR (g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM records e JOIN target t ON t.kind = 'record' AND t.id = e.id WHERE e.workspace_id = $2 AND e.blueprint_id = g.scope_target_id)) OR (g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors WHERE id = g.scope_target_id)) OR ($5 = '__context_list__' AND g.scope_type = 'context_subtree'))))",
            codes.grant_set_sql()
        );
        Ok(sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_id)
            .bind(target_code)
            .fetch_one(connection)
            .await?)
    }
}

/// The kind of target [`AttricatRepository::authorized_target_ids`] checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationTargetKind {
    Record,
    Blueprint,
    Context,
}

/// Which of a user's live grants count: those whose role carries a
/// permission, or those of any of the named roles.
#[derive(Clone, Copy)]
pub(crate) enum GrantCodes<'a> {
    Permission(&'a str),
    Roles(&'a [String]),
}

impl<'a> GrantCodes<'a> {
    /// The `grants(scope_type, scope_target_id)` CTE for user `$1` in
    /// workspace `$2`, matching the codes bound as `$3`: active user,
    /// membership and workspace only.
    fn grant_set_sql(&self) -> String {
        let filter = match self {
            Self::Permission(_) => {
                "JOIN role_permissions rp ON rp.role_id = g.role_id WHERE rp.permission_code = ANY($3)"
            }
            Self::Roles(_) => {
                "JOIN roles r ON r.id = g.role_id WHERE (r.is_system OR r.workspace_id = $2) AND r.code = ANY($3)"
            }
        };
        format!(
            "grants AS (SELECT g.scope_type, g.scope_target_id FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id {filter} AND m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL)"
        )
    }

    /// The codes to bind as `$3`.
    fn codes(&self) -> Vec<&'a str> {
        match *self {
            Self::Permission(permission) => vec![permission],
            Self::Roles(roles) => roles.iter().map(String::as_str).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::*;

    #[test]
    fn reusable_attribute_selectors_require_a_qualified_code() {
        assert!(validate_attribute_selector_code("weight").is_ok());
        assert!(validate_attribute_selector_code("acme:weight").is_ok());
        assert!(validate_attribute_selector_code("acme:weight:metric").is_err());
        assert!(validate_attribute_selector_code("acme weight").is_err());
    }

    #[tokio::test]
    async fn handler_commands_preserve_event_lineage() {
        let repository = AttricatRepository::system(
            PgPoolOptions::new()
                .connect_lazy("postgres://postgres:postgres@localhost/attricat")
                .unwrap(),
        );
        let initiating_actor_user_id = Uuid::new_v4();
        let initiating_actor_token_id = Uuid::new_v4();
        let trigger = crate::domain_events::DomainEvent {
            id: Uuid::new_v4(),
            sequence: 1,
            workspace_id: Uuid::new_v4(),
            occurred_at: chrono::Utc::now(),
            event_type: "context.created.v1".to_owned(),
            aggregate_kind: "context".to_owned(),
            aggregate_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            causation_id: None,
            source_kind: "api".to_owned(),
            source_name: "attricat_api".to_owned(),
            metadata: serde_json::json!({
                "initiating_actor_user_id": initiating_actor_user_id.to_string(),
                "initiating_actor_token_id": initiating_actor_token_id.to_string(),
            }),
            payload: serde_json::json!({}),
        };
        let repository = repository.for_workspace(trigger.workspace_id);
        let event = repository
            .for_event_handler(&trigger, "attricat.computed_fields")
            .core_event(
                "context.updated.v1",
                "context",
                trigger.aggregate_id,
                serde_json::json!({}),
            );
        assert_eq!(event.correlation_id, trigger.correlation_id);
        assert_eq!(event.causation_id, Some(trigger.id));
        assert_eq!(event.source.kind.as_str(), "worker");
        assert_eq!(event.source.name, "attricat.computed_fields");
        let extension_event = repository
            .for_event_handler(&trigger, "attricat.extensions.wasm")
            .for_extension("acme.computed")
            .core_event(
                "context.updated.v1",
                "context",
                trigger.aggregate_id,
                serde_json::json!({}),
            );
        assert_eq!(extension_event.correlation_id, trigger.correlation_id);
        assert_eq!(extension_event.causation_id, Some(trigger.id));
        assert_eq!(extension_event.source.kind.as_str(), "plugin");
        assert_eq!(extension_event.source.name, "extension:acme.computed");
        assert_eq!(
            extension_event.metadata["initiating_actor_user_id"],
            initiating_actor_user_id.to_string()
        );
        assert!(extension_event.validate().is_ok());
        let extension_repository = repository
            .for_event_handler(&trigger, "attricat.extensions.wasm")
            .for_extension("acme.computed");
        let audit = extension_repository.audit_context.as_ref().unwrap();
        assert_eq!(audit.correlation_id, trigger.correlation_id);
        assert_eq!(audit.actor_user_id, Some(initiating_actor_user_id));
        assert_eq!(audit.actor_token_id, Some(initiating_actor_token_id));
        assert_eq!(audit.action, "attricat.extensions.wasm.execute");
        assert_eq!(audit.metadata["extension_id"], "acme.computed");

        let command_event = repository.for_extension("acme.computed").core_event(
            "context.updated.v1",
            "context",
            trigger.aggregate_id,
            serde_json::json!({}),
        );
        assert_eq!(command_event.causation_id, None);
        assert_eq!(command_event.source.kind.as_str(), "plugin");
        assert_eq!(command_event.source.name, "extension:acme.computed");
        assert!(command_event.validate().is_ok());
    }
}
