use super::{AttricatRepository, RepositoryError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct AuditEventFilter {
    pub limit: i64,
    pub offset: i64,
    pub occurred_after: Option<DateTime<Utc>>,
    pub occurred_before: Option<DateTime<Utc>>,
    pub actor_user_id: Option<Uuid>,
    pub action_category: Option<String>,
    pub target_type: Option<String>,
    pub executor_type: Option<String>,
    pub agent_run_id: Option<Uuid>,
    pub agent_tool_call_id: Option<Uuid>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub actor_user_id: Option<Uuid>,
    pub actor_display_name: Option<String>,
    pub actor_email: Option<String>,
    /// The actor's ready avatar in this workspace.
    pub actor_avatar_file_id: Option<Uuid>,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub action: String,
    pub authorization_scope: Value,
    pub target: Value,
    pub outcome: String,
    pub metadata: Value,
    pub executor_type: String,
    pub agent_run_id: Option<Uuid>,
    pub agent_conversation_id: Option<Uuid>,
    pub agent_tool_call_id: Option<Uuid>,
    pub agent_tool_name: Option<String>,
    pub approval_decision: Option<String>,
    pub approved_by_user_id: Option<Uuid>,
    pub approved_by_display_name: Option<String>,
    pub approved_by_email: Option<String>,
    pub approved_by_avatar_file_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct AuditEventPage {
    pub events: Vec<AuditEvent>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl AttricatRepository {
    pub async fn list_audit_events(
        &self,
        filter: AuditEventFilter,
    ) -> Result<AuditEventPage, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let category = filter
            .action_category
            .as_deref()
            .map(|value| format!("{value}.%"));
        let rows = sqlx::query_as::<_, AuditEvent>(
            "SELECT e.id, e.occurred_at, e.actor_user_id, actor.display_name AS actor_display_name, actor.email AS actor_email, actor_avatar.id AS actor_avatar_file_id, e.request_id, e.correlation_id, e.action, e.authorization_scope, e.target, e.outcome, e.metadata, e.executor_type, e.agent_run_id, e.agent_conversation_id, e.agent_tool_call_id, e.agent_tool_name, e.approval_decision, e.approved_by_user_id, approver.display_name AS approved_by_display_name, approver.email AS approved_by_email, approver_avatar.id AS approved_by_avatar_file_id FROM audit_events e LEFT JOIN users actor ON actor.id = e.actor_user_id LEFT JOIN users approver ON approver.id = e.approved_by_user_id LEFT JOIN workspace_memberships actor_m ON actor_m.workspace_id = e.workspace_id AND actor_m.user_id = e.actor_user_id AND actor_m.state = 'active' LEFT JOIN files actor_avatar ON actor_avatar.workspace_id = actor_m.workspace_id AND actor_avatar.id = actor_m.avatar_file_id AND actor_avatar.purpose = 'avatar' AND actor_avatar.status = 'ready' AND actor_avatar.deleted_at IS NULL LEFT JOIN workspace_memberships approver_m ON approver_m.workspace_id = e.workspace_id AND approver_m.user_id = e.approved_by_user_id AND approver_m.state = 'active' LEFT JOIN files approver_avatar ON approver_avatar.workspace_id = approver_m.workspace_id AND approver_avatar.id = approver_m.avatar_file_id AND approver_avatar.purpose = 'avatar' AND approver_avatar.status = 'ready' AND approver_avatar.deleted_at IS NULL WHERE e.workspace_id = $1 AND ($2::timestamptz IS NULL OR e.occurred_at >= $2) AND ($3::timestamptz IS NULL OR e.occurred_at <= $3) AND ($4::uuid IS NULL OR e.actor_user_id = $4) AND ($5::text IS NULL OR e.action LIKE $5) AND ($6::text IS NULL OR e.target->>'type' = $6) AND ($7::text IS NULL OR e.executor_type = $7) AND ($8::uuid IS NULL OR e.agent_run_id = $8) AND ($9::uuid IS NULL OR e.agent_tool_call_id = $9) ORDER BY e.occurred_at DESC, e.id DESC LIMIT $10 OFFSET $11",
        )
        .bind(workspace_id)
        .bind(filter.occurred_after)
        .bind(filter.occurred_before)
        .bind(filter.actor_user_id)
        .bind(category)
        .bind(&filter.target_type)
        .bind(&filter.executor_type)
        .bind(filter.agent_run_id)
        .bind(filter.agent_tool_call_id)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(&self.pool)
        .await?;
        let total = sqlx::query_scalar(
            "SELECT count(*) FROM audit_events e WHERE e.workspace_id = $1 AND ($2::timestamptz IS NULL OR e.occurred_at >= $2) AND ($3::timestamptz IS NULL OR e.occurred_at <= $3) AND ($4::uuid IS NULL OR e.actor_user_id = $4) AND ($5::text IS NULL OR e.action LIKE $5) AND ($6::text IS NULL OR e.target->>'type' = $6) AND ($7::text IS NULL OR e.executor_type = $7) AND ($8::uuid IS NULL OR e.agent_run_id = $8) AND ($9::uuid IS NULL OR e.agent_tool_call_id = $9)",
        )
        .bind(workspace_id)
        .bind(filter.occurred_after)
        .bind(filter.occurred_before)
        .bind(filter.actor_user_id)
        .bind(filter.action_category.as_deref().map(|value| format!("{value}.%")))
        .bind(&filter.target_type)
        .bind(&filter.executor_type)
        .bind(filter.agent_run_id)
        .bind(filter.agent_tool_call_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(AuditEventPage {
            events: rows,
            total,
            limit: filter.limit,
            offset: filter.offset,
        })
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Installs the audit-read permission in application code so database
    /// migrations remain declarative. Owners and administrators receive it.
    pub async fn ensure_audit_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('audit.read', 'Read workspace audit events') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'audit.read') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
