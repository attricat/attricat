use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use catalog_validation::is_valid_code;
use chrono::{DateTime, NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use sqlx::{
    PgPool, Postgres, Transaction,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    constants::REQUEST_POOL_CONNECTIONS,
    domain_events::{AffectedFactV1, EventSource, EventSourceKind, NewDomainEvent},
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, AttributeValueHistory,
        AttributeValueSelector, Blueprint, BlueprintEntityPublicationSummary,
        BlueprintWithAttributes, CreateBlueprint, Entity, EntityAuditChange, EntityHierarchyItem,
        EntityHierarchyResponse, EntityIdentity, EntityMigrationPreview, EntityPreview,
        EntityPreviewPage, EntityPublicationStatus, FormAttributeValue, IncomingRelationshipItem,
        IncomingRelationshipSelector, IncomingRelationshipsPage, MatchExplanation, MatchPathEdge,
        MigrateEntityRequest, MigrationIssue, NewAttributeValue, PublicationChannel,
        RelatedEntityPreview, RelationshipMutation, RelationshipTargets,
        RelationshipTreeFacetChildItem, RelationshipTreeFacetChildrenResponse,
        RelationshipTreeFacetItem, RelationshipTreeFacetResponse, ResolvedEntityPreviewResponse,
    },
};

mod agents;
mod audit_events;
mod blueprint_migration_batches;
mod blueprints;
mod contexts;
mod domain_events;
mod entity_commands;
mod entity_migration;
mod entity_projection;
mod entity_publications;
mod entity_search;
mod extension_registries;
mod extension_scoped_configuration;
mod extension_storage;
mod extensions;
mod files;
mod health;
mod members;
mod roles;
mod sessions;
mod solution_packs;
mod tokens;
mod values;
mod workflow_runs;
mod workflows;
mod workspace_navigation;

pub use agents::{
    AgentRun, AgentRunEvent, AgentToolCall, ApprovalDecision, Conversation, ConversationMessage,
};
pub(crate) use audit_events::{AuditEventFilter, AuditEventPage};
pub use domain_events::{EventConsumer, EventDelivery, EventPublisher, FailedEventDelivery};
pub(crate) use entity_search::{
    EntityRelationshipFilter, EntitySearchFilter, EntitySearchSort, decode_search_cursor,
};
pub use extension_registries::ExtensionRegistrySource;
pub use extension_scoped_configuration::ExtensionConfigurationScope;
pub use extension_storage::{
    ExtensionStorageEntry, ExtensionStorageError, ExtensionStoragePage,
    MAX_EXTENSION_STORAGE_LIST_LIMIT,
};
pub use extensions::{
    ClientExtensionContribution, ExtensionGrant, ExtensionInstallation, ExtensionLifecycleRecord,
    ExtensionRuntimeInstallation, ExtensionState, InstalledExtension,
};
pub(crate) use files::{FileMetadata, FileObject, FilePolicy, FileUploadResult, NewUploadedFile};
pub(crate) use members::{WorkspaceInvitation, WorkspaceMember};
pub(crate) use roles::{Permission, WorkspaceGrantTarget, WorkspaceRole};
#[cfg(test)]
pub(crate) use solution_packs::SolutionPackCheckResult;
pub(crate) use solution_packs::{
    SolutionPackApplication, SolutionPackApplicationSummary, SolutionPackCheckRun,
    SolutionPackCheckRunSummary, SolutionPackPlan,
};
pub(crate) use tokens::PersonalApiToken;
pub use workflow_runs::WorkflowRun;
pub(crate) use workflow_runs::{ClaimedWorkflowRun, WorkflowActionResult};
pub(crate) use workspace_navigation::{ExploreNavigationEntry, ExploreNavigationItem};

#[derive(Debug, sqlx::FromRow)]
pub struct UserAccount {
    pub display_name: Option<String>,
    pub email: String,
}

#[derive(Clone)]
/// The stable catalog persistence facade. Feature modules add inherent methods
/// here so HTTP handlers and other callers do not depend on storage internals.
pub struct CatalogRepository {
    pub(in crate::repository) pool: PgPool,
    workspace_id: Option<Uuid>,
    workspace_pools: Option<Arc<WorkspacePoolCache>>,
    audit_context: Option<AuditContext>,
    event_context: Option<EventCommandContext>,
    extension_id: Option<String>,
}

#[derive(Clone)]
pub(crate) struct EventCommandContext {
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
pub(crate) struct AuditContext {
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
pub(crate) struct AuditEventChange {
    pub entity_id: Uuid,
    pub attribute_id: Uuid,
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub context_code: Option<String>,
    pub relationship_target_entity_id: Option<Uuid>,
    pub change_kind: &'static str,
    pub before_value: Option<Value>,
    pub after_value: Option<Value>,
}

#[derive(Clone)]
pub(crate) struct AgentAuditAttribution {
    pub run_id: Uuid,
    pub conversation_id: Uuid,
    pub tool_call_id: Uuid,
    pub tool_name: String,
    pub approval_decision: Option<String>,
    pub approved_by_user_id: Option<Uuid>,
}

struct WorkspacePoolCache {
    connect_options: PgConnectOptions,
    pools: Mutex<HashMap<Uuid, PgPool>>,
}

const MAX_WORKSPACE_POOLS: usize = 32;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("the invitation is invalid, expired, revoked, already accepted, or for another email")]
    InvitationInvalid,
    #[error("the context code 'default' is reserved")]
    ReservedContextCode,
    #[error("code must contain only ASCII letters, numbers, hyphens, and underscores")]
    InvalidCode,
    #[error("context data must be a JSON object")]
    InvalidContextData,
    #[error("system metadata must be a JSON object no larger than 64 KiB")]
    InvalidSystemMetadata,
    #[error(
        "system tags must contain at most 100 unique, non-blank strings no longer than 128 bytes"
    )]
    InvalidSystemTags,
    #[error("context was not found")]
    InvalidContext,
    #[error("the default context cannot be changed or deleted")]
    DefaultContextProtected,
    #[error("a context cannot be its own descendant")]
    ContextCycle,
    #[error("a context with descendants or active values cannot be deleted")]
    ContextInUse,
    #[error("attribute can only be edited in the default context")]
    DefaultContextOnly,
    #[error("attribute does not belong to the entity blueprint version")]
    AttributeNotApplicable,
    #[error("file attribute policy is invalid")]
    InvalidFilePolicy,
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
        "resolved entity values for context '{context}' do not match the entity schema at '{instance_path}': {message}"
    )]
    EntitySchemaMismatch {
        context: String,
        instance_path: String,
        message: String,
    },
    #[error("stored attribute value does not match its attribute type")]
    InvalidStoredAttributeValue,
    #[error("relationship target does not match the attribute target blueprint")]
    RelationshipTargetTypeMismatch,
    #[error("one-to-one relationship cardinality conflict for '{attribute}'")]
    RelationshipCardinalityConflict {
        attribute: String,
        context_id: Option<Uuid>,
        source_entity_id: Uuid,
        target_entity_id: Uuid,
        conflicting_source_entity_id: Option<Uuid>,
    },
    #[error("entity preview must be a JSON object organized by context")]
    InvalidPreview,
    #[error("hierarchy field must be a self-targeting relationship")]
    InvalidHierarchyRelationship,
    #[error("invalid blueprint definition: {0}")]
    InvalidBlueprintDefinition(String),
    #[error("{0}")]
    InvalidSolutionPackPlan(String),
    #[error("solution-pack plan is not ready to apply")]
    SolutionPackPlanNotReady,
    #[error("solution-pack plan has expired")]
    SolutionPackPlanExpired,
    #[error("solution-pack plan preconditions no longer match the workspace")]
    SolutionPackPlanStale,
    #[error("solution-pack application is invalid and cannot be resumed")]
    SolutionPackApplicationInvalid,
    #[error("solution-pack application failed: {0}")]
    SolutionPackApplicationFailed(String),
    #[error("blueprint code is already owned by another blueprint")]
    BlueprintCodeTaken,
    #[error("catalog code is already in use")]
    CatalogCodeTaken,
    #[error("workflow code is already in use")]
    WorkflowCodeTaken,
    #[error("invalid workflow definition: {0}")]
    InvalidWorkflowDefinition(String),
    #[error("workflow revision must be published before it can be enabled")]
    WorkflowNotPublished,
    #[error("blueprint revision is not published")]
    BlueprintNotPublished,
    #[error("entity is already on the latest blueprint revision")]
    EntityBlueprintCurrent,
    #[error("the latest blueprint revision changed; refresh the migration preview")]
    MigrationTargetChanged,
    #[error("migration does not apply to this entity")]
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
    #[error("invalid domain event: {0}")]
    InvalidDomainEvent(#[from] crate::domain_events::EventContractError),
    #[error("publication context is not an enabled channel")]
    PublicationChannelDisabled,
    #[error("an authenticated user is required to publish an entity")]
    PublicationActorRequired,
    #[error("requested token permissions are not available to the current user")]
    TokenPermissionsUnavailable,
    #[error("an approval decision has already been recorded")]
    ApprovalAlreadyDecided,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    pub(crate) fn invalid_blueprint_definition(error: impl std::fmt::Display) -> Self {
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

impl CatalogRepository {
    const DEFAULT_WORKSPACE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            workspace_id: None,
            workspace_pools: None,
            audit_context: None,
            event_context: None,
            extension_id: None,
        }
    }

    pub fn with_workspace_pool_factory(pool: PgPool, connect_options: PgConnectOptions) -> Self {
        Self {
            pool,
            workspace_id: None,
            audit_context: None,
            event_context: None,
            extension_id: None,
            workspace_pools: Some(Arc::new(WorkspacePoolCache {
                connect_options,
                pools: Mutex::new(HashMap::new()),
            })),
        }
    }

    /// Returns a repository whose pool is permanently restricted to one
    /// server-derived workspace by the connection's RLS setting.
    pub async fn for_workspace(&self, workspace_id: Uuid) -> Result<Self, RepositoryError> {
        let Some(cache) = &self.workspace_pools else {
            self.ensure_default_context(workspace_id).await?;
            return Ok(Self {
                pool: self.pool.clone(),
                workspace_id: Some(workspace_id),
                workspace_pools: None,
                audit_context: self.audit_context.clone(),
                event_context: self.event_context.clone(),
                extension_id: self.extension_id.clone(),
            });
        };
        let mut pools = cache.pools.lock().await;
        if let Some(pool) = pools.get(&workspace_id) {
            return Ok(Self {
                pool: pool.clone(),
                workspace_id: Some(workspace_id),
                workspace_pools: self.workspace_pools.clone(),
                audit_context: self.audit_context.clone(),
                event_context: self.event_context.clone(),
                extension_id: self.extension_id.clone(),
            });
        }
        self.ensure_default_context(workspace_id).await?;
        if pools.len() >= MAX_WORKSPACE_POOLS
            && let Some(workspace_id) = pools.keys().next().copied()
            && let Some(pool) = pools.remove(&workspace_id)
        {
            pool.close().await;
        }
        let pool = PgPoolOptions::new()
            .max_connections(REQUEST_POOL_CONNECTIONS)
            .after_connect(move |_connection, _| Box::pin(async move { Ok(()) }))
            .connect_with(cache.connect_options.clone())
            .await?;
        pools.insert(workspace_id, pool.clone());
        Ok(Self {
            pool,
            workspace_id: Some(workspace_id),
            workspace_pools: self.workspace_pools.clone(),
            audit_context: self.audit_context.clone(),
            event_context: self.event_context.clone(),
            extension_id: self.extension_id.clone(),
        })
    }

    pub(crate) fn for_event_handler(
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
                .get("workflow_causal_depth")
                .and_then(Value::as_u64)
                .filter(|depth| *depth <= 8)
                .unwrap_or(0) as usize,
            workflow_root_trigger_event_id: event
                .metadata
                .get("workflow_root_trigger_event_id")
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

    /// Adds immutable workflow run provenance to the worker audit/event context.
    pub(crate) fn for_workflow_run(
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
    pub(crate) fn for_extension(&self, extension_id: &str) -> Self {
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
                action: "catalog.extensions.attribute_values.write".to_owned(),
                authorization_scope: serde_json::json!({
                    "capability": "catalog.write",
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

impl CatalogRepository {
    async fn ensure_default_context(&self, workspace_id: Uuid) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"INSERT INTO attribute_contexts (id, workspace_id, code, data, parent_id)
               SELECT $1, id, 'default', '{}'::jsonb, NULL
               FROM workspaces
               WHERE id = $2 AND deleted_at IS NULL
               ON CONFLICT (workspace_id, code) DO NOTHING"#,
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge_value_history(&self, retention_days: i64) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM attribute_value_history WHERE archived_at < now() - ($1 * interval '1 day')",
        )
        .bind(retention_days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) fn with_audit_context(mut self, audit_context: AuditContext) -> Self {
        self.audit_context = Some(audit_context);
        self
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

    pub(in crate::repository) async fn write_audit_event_with_publication_metadata(
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
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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

    pub(in crate::repository) async fn commit_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Commits a catalog mutation, its audit evidence, and an outbox event as
    /// one database transaction. Callers retain control of event vocabulary.
    pub(in crate::repository) async fn commit_mutation_with_event(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        event: crate::domain_events::NewDomainEvent,
    ) -> Result<(), RepositoryError> {
        self.write_audit_event(&mut transaction).await?;
        self.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Appends a declared extension event through the durable outbox. The
    /// caller supplies an invocation-time authorized release snapshot.
    pub(crate) async fn emit_extension_event(
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
        let mut event =
            repository.core_event(&contract.event_type, aggregate_kind, aggregate_id, payload);
        event.metadata = serde_json::json!({"event_contract": contract_id, "event_contract_version": contract.version});
        repository.write_audit_event(&mut transaction).await?;
        repository.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(in crate::repository) fn core_event(
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
                    .unwrap_or_else(|| "catalog_api".to_owned()),
            },
            metadata: event_metadata(self.audit_context.as_ref(), self.event_context.is_some()),
            payload,
        }
    }

    pub(in crate::repository) fn affected_facts(
        changes: &[AuditEventChange],
    ) -> Vec<AffectedFactV1> {
        changes
            .iter()
            .map(|change| AffectedFactV1 {
                attribute_id: change.attribute_id,
                attribute_code: change.attribute_code.clone(),
                context_id: change.context_id,
                context_code: change.context_code.clone(),
                relationship_target_entity_id: change.relationship_target_entity_id,
                change_kind: change.change_kind.to_owned(),
                before_value: change.before_value.clone(),
                after_value: change.after_value.clone(),
            })
            .collect()
    }

    pub(in crate::repository) async fn commit_entity_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        changes: Vec<AuditEventChange>,
        mut event: NewDomainEvent,
    ) -> Result<(), RepositoryError> {
        let retained_role = self
            .reconcile_entity_publication(&mut transaction, event.aggregate_id, "entity_changed")
            .await?;
        let publication_metadata = match retained_role {
            Some(role_code) => {
                event.metadata["publication"] = serde_json::json!({
                    "disposition": "retained",
                    "role_code": role_code,
                });
                serde_json::json!({
                    "disposition": "retained",
                    "role_code": event.metadata["publication"]["role_code"],
                })
            }
            None => {
                event.metadata["publication"] = serde_json::json!({
                    "disposition": "withdrawn",
                    "reason": "entity_changed",
                });
                serde_json::json!({
                    "disposition": "withdrawn",
                    "reason": "entity_changed",
                })
            }
        };
        if let Some(audit_event_id) = self
            .write_audit_event_with_publication_metadata(
                &mut transaction,
                Some(publication_metadata),
            )
            .await?
        {
            for change in changes {
                sqlx::query("INSERT INTO audit_event_changes (id, audit_event_id, workspace_id, entity_id, attribute_id, attribute_code, context_id, context_code, change_kind, before_value, after_value) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)")
                    .bind(Uuid::new_v4())
                    .bind(audit_event_id)
                    .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                    .bind(change.entity_id)
                    .bind(change.attribute_id)
                    .bind(change.attribute_code)
                    .bind(change.context_id)
                    .bind(change.context_code)
                    .bind(change.change_kind)
                    .bind(change.before_value)
                    .bind(change.after_value)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        self.enqueue_event(&mut transaction, event).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Returns only the authenticated principal's account fields. Callers must
    /// derive `user_id` from authentication rather than accepting it from a request.
    pub async fn user_account(&self, user_id: Uuid) -> Result<UserAccount, RepositoryError> {
        sqlx::query_as("SELECT display_name, email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::from)
    }

    pub async fn is_active_user(&self, user_id: Uuid) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND state = 'active')",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?)
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
    pub async fn is_authorized(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        // Resolve grant scope in the application-owned repository query.  A
        // non-workspace grant can only authorize the requested tenant target.
        Ok(sqlx::query_scalar(
            "WITH RECURSIVE grants AS (SELECT g.scope_type, g.scope_target_id FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN role_permissions rp ON rp.role_id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL AND rp.permission_code = $3), target AS (SELECT 'blueprint'::text kind, id FROM blueprints WHERE workspace_id = $2 AND (id = $4 OR code = $5) UNION ALL SELECT 'entity', id FROM entities WHERE workspace_id = $2 AND id = $4 UNION ALL SELECT 'context', id FROM attribute_contexts WHERE workspace_id = $2 AND (id = $4 OR code = $5)), ancestors AS (SELECT c.id, c.parent_id FROM attribute_contexts c JOIN target t ON t.kind = 'context' AND t.id = c.id UNION ALL SELECT p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (($4 IS NOT NULL OR $5 IS NOT NULL) AND ((g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM target WHERE kind = 'blueprint' AND id = g.scope_target_id)) OR (g.scope_type = 'entity' AND EXISTS (SELECT 1 FROM target WHERE kind = 'entity' AND id = g.scope_target_id)) OR (g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM entities e JOIN target t ON t.kind = 'entity' AND t.id = e.id WHERE e.workspace_id = $2 AND e.blueprint_id = g.scope_target_id)) OR (g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors WHERE id = g.scope_target_id)) OR ($5 = '__context_list__' AND g.scope_type = 'context_subtree'))))",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(permission)
        .bind(target_id)
        .bind(target_code)
        .fetch_one(&self.pool)
        .await?)
    }
}

pub(crate) fn validate_code(value: &str) -> Result<(), RepositoryError> {
    if is_valid_code(value) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCode)
    }
}

pub(crate) fn missing_required_fields(message: &str, target_codes: &HashSet<&str>) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPoolOptions;

    use super::*;

    #[tokio::test]
    async fn handler_commands_preserve_event_lineage() {
        let repository = CatalogRepository::new(
            PgPoolOptions::new()
                .connect_lazy("postgres://postgres:postgres@localhost/catalog")
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
            source_name: "catalog_api".to_owned(),
            metadata: serde_json::json!({
                "initiating_actor_user_id": initiating_actor_user_id.to_string(),
                "initiating_actor_token_id": initiating_actor_token_id.to_string(),
            }),
            payload: serde_json::json!({}),
        };
        let event = repository
            .for_event_handler(&trigger, "catalog.computed_fields")
            .core_event(
                "context.updated.v1",
                "context",
                trigger.aggregate_id,
                serde_json::json!({}),
            );
        assert_eq!(event.correlation_id, trigger.correlation_id);
        assert_eq!(event.causation_id, Some(trigger.id));
        assert_eq!(event.source.kind.as_str(), "worker");
        assert_eq!(event.source.name, "catalog.computed_fields");
        let extension_event = repository
            .for_event_handler(&trigger, "catalog.extensions.wasm")
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
            .for_event_handler(&trigger, "catalog.extensions.wasm")
            .for_extension("acme.computed");
        let audit = extension_repository.audit_context.as_ref().unwrap();
        assert_eq!(audit.correlation_id, trigger.correlation_id);
        assert_eq!(audit.actor_user_id, Some(initiating_actor_user_id));
        assert_eq!(audit.actor_token_id, Some(initiating_actor_token_id));
        assert_eq!(audit.action, "catalog.extensions.wasm.execute");
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
