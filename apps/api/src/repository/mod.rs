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
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, AttributeValueHistory,
        AttributeValueSelector, Blueprint, BlueprintWithAttributes, CreateBlueprint, Entity,
        EntityHierarchyItem, EntityHierarchyResponse, EntityIdentity, EntityMigrationPreview,
        EntityPreview, EntityPreviewPage, FormAttributeValue, IncomingRelationshipItem,
        IncomingRelationshipSelector, IncomingRelationshipsPage, MigrateEntityRequest,
        MigrationIssue, NewAttributeValue, RelationshipMutation, RelationshipTargets,
        RelationshipTreeFacetChildItem, RelationshipTreeFacetChildrenResponse,
        RelationshipTreeFacetItem, RelationshipTreeFacetResponse, ResolvedEntityPreviewResponse,
    },
};

mod agents;
mod blueprints;
mod contexts;
mod entity_commands;
mod entity_migration;
mod entity_projection;
mod entity_search;
mod files;
mod health;
mod members;
mod roles;
mod sessions;
mod tokens;
mod values;

pub use agents::{
    AgentRun, AgentRunEvent, AgentToolCall, ApprovalDecision, Conversation, ConversationMessage,
};
pub(crate) use entity_search::decode_search_cursor;
pub(crate) use files::{FileMetadata, FileObject, FilePolicy, FileUploadResult, NewUploadedFile};
pub(crate) use members::{WorkspaceInvitation, WorkspaceMember};
pub(crate) use roles::{Permission, WorkspaceGrantTarget, WorkspaceRole};
pub(crate) use tokens::PersonalApiToken;

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
    #[error("entity preview must be a JSON object organized by context")]
    InvalidPreview,
    #[error("hierarchy field must be a self-targeting relationship")]
    InvalidHierarchyRelationship,
    #[error("invalid blueprint definition: {0}")]
    InvalidBlueprintDefinition(String),
    #[error("blueprint code is already owned by another blueprint")]
    BlueprintCodeTaken,
    #[error("blueprint revision is not published")]
    BlueprintNotPublished,
    #[error("entity is already on the latest blueprint revision")]
    EntityBlueprintCurrent,
    #[error("the latest blueprint revision changed; refresh the migration preview")]
    MigrationTargetChanged,
    #[error("migration does not apply to this entity")]
    MigrationNotApplicable,
    #[error("migration needs resolutions for: {}", .0.join(", "))]
    MigrationNeedsResolution(Vec<String>),
    #[error("invalid agent state: {0}")]
    InvalidAgentState(&'static str),
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

impl CatalogRepository {
    const DEFAULT_WORKSPACE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            workspace_id: None,
            workspace_pools: None,
            audit_context: None,
        }
    }

    pub fn with_workspace_pool_factory(pool: PgPool, connect_options: PgConnectOptions) -> Self {
        Self {
            pool,
            workspace_id: None,
            audit_context: None,
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
            });
        };
        let mut pools = cache.pools.lock().await;
        if let Some(pool) = pools.get(&workspace_id) {
            return Ok(Self {
                pool: pool.clone(),
                workspace_id: Some(workspace_id),
                workspace_pools: self.workspace_pools.clone(),
                audit_context: self.audit_context.clone(),
            });
        }
        self.ensure_default_context(workspace_id).await?;
        if pools.len() >= MAX_WORKSPACE_POOLS {
            if let Some(workspace_id) = pools.keys().next().copied() {
                if let Some(pool) = pools.remove(&workspace_id) {
                    pool.close().await;
                }
            }
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
        })
    }

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
    pub(in crate::repository) async fn commit_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        if let Some(audit) = &self.audit_context {
            sqlx::query(
                "INSERT INTO audit_events (id, workspace_id, actor_user_id, actor_token_id, request_id, correlation_id, action, authorization_scope, target, outcome, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'success', $10)",
            )
            .bind(Uuid::new_v4())
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .bind(audit.actor_user_id)
            .bind(audit.actor_token_id)
            .bind(audit.request_id)
            .bind(audit.correlation_id)
            .bind(&audit.action)
            .bind(&audit.authorization_scope)
            .bind(&audit.target)
            .bind(&audit.metadata)
            .execute(&mut *transaction)
            .await?;
        }
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
