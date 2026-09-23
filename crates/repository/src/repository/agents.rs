use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by_user_id: Option<Uuid>,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub run_id: Option<Uuid>,
    pub sequence: i64,
    pub role: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
    pub attachments: Vec<ConversationMessageAttachment>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct ConversationMessageRow {
    id: Uuid,
    conversation_id: Uuid,
    run_id: Option<Uuid>,
    sequence: i64,
    role: String,
    content: Value,
    created_at: DateTime<Utc>,
}

impl From<ConversationMessageRow> for ConversationMessage {
    fn from(row: ConversationMessageRow) -> Self {
        Self {
            id: row.id,
            conversation_id: row.conversation_id,
            run_id: row.run_id,
            sequence: row.sequence,
            role: row.role,
            content: row.content,
            created_at: row.created_at,
            attachments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ConversationMessageAttachment {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub status: String,
}

#[derive(sqlx::FromRow)]
struct ConversationMessageAttachmentRow {
    message_id: Uuid,
    id: Uuid,
    filename: String,
    mime_type: String,
    byte_size: i64,
    status: String,
}

impl From<ConversationMessageAttachmentRow> for ConversationMessageAttachment {
    fn from(row: ConversationMessageAttachmentRow) -> Self {
        Self {
            id: row.id,
            filename: row.filename,
            mime_type: row.mime_type,
            byte_size: row.byte_size,
            status: row.status,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct AgentRun {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub origin: String,
    pub status: String,
    pub provider_base_url: String,
    pub model: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct AgentToolCall {
    pub id: Uuid,
    pub run_id: Uuid,
    pub sequence: i64,
    pub provider_call_id: Option<String>,
    pub tool_name: String,
    pub arguments: Value,
    pub change_summary: Option<String>,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub state: String,
    pub decided_by_user_id: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct AgentRunEvent {
    pub id: Uuid,
    pub run_id: Uuid,
    pub sequence: i64,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalDecision {
    Approve,
    Reject,
}

impl CatalogRepository {
    /// System permissions are application data, not migration behavior. Keep
    /// this idempotent bootstrap explicit so SQL migrations remain declarative.
    pub async fn ensure_agent_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('agents.run', 'Create, inspect, and decide conversational agent runs') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx).await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'agents.run') ON CONFLICT DO NOTHING")
                .bind(role_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_conversation(
        &self,
        created_by_user_id: Option<Uuid>,
        title: &str,
    ) -> Result<Conversation, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as("INSERT INTO conversations (id, workspace_id, created_by_user_id, title) VALUES ($1, $2, $3, $4) RETURNING id, workspace_id, created_by_user_id, title, created_at, updated_at, archived_at")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(created_by_user_id).bind(title)
            .fetch_one(&self.pool).await?)
    }

    pub async fn conversation_messages(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<ConversationMessage>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let rows: Vec<ConversationMessageRow> = sqlx::query_as("SELECT m.id, m.conversation_id, m.run_id, m.sequence, m.role, m.content, m.created_at FROM conversation_messages m JOIN conversations c ON c.id = m.conversation_id WHERE m.conversation_id = $1 AND c.workspace_id = $2 ORDER BY m.sequence")
            .bind(conversation_id).bind(workspace_id).fetch_all(&self.pool).await?;
        let mut messages: Vec<ConversationMessage> = rows.into_iter().map(Into::into).collect();
        let message_ids: Vec<Uuid> = messages.iter().map(|message| message.id).collect();
        let attachments: Vec<ConversationMessageAttachmentRow> = sqlx::query_as("SELECT a.message_id, f.id, f.display_filename AS filename, f.mime_type, f.byte_size, f.status FROM conversation_message_attachments a JOIN files f ON f.id = a.file_id AND f.workspace_id = a.workspace_id WHERE a.workspace_id = $1 AND a.message_id = ANY($2) AND f.deleted_at IS NULL ORDER BY a.message_id, a.position")
            .bind(workspace_id).bind(&message_ids).fetch_all(&self.pool).await?;
        let mut by_message: HashMap<Uuid, Vec<ConversationMessageAttachment>> = HashMap::new();
        for attachment in attachments {
            by_message
                .entry(attachment.message_id)
                .or_default()
                .push(attachment.into());
        }
        for message in &mut messages {
            message.attachments = by_message.remove(&message.id).unwrap_or_default();
        }
        Ok(messages)
    }

    /// Appends under a conversation row lock so each thread's durable sequence
    /// remains gap-free even when provider and user writes race.
    pub async fn append_conversation_message(
        &self,
        conversation_id: Uuid,
        run_id: Option<Uuid>,
        role: &str,
        content: Value,
    ) -> Result<ConversationMessage, RepositoryError> {
        self.append_conversation_message_with_attachments(
            conversation_id,
            run_id,
            role,
            content,
            &[],
        )
        .await
    }

    pub async fn append_conversation_message_with_attachments(
        &self,
        conversation_id: Uuid,
        run_id: Option<Uuid>,
        role: &str,
        content: Value,
        attachment_ids: &[Uuid],
    ) -> Result<ConversationMessage, RepositoryError> {
        if !matches!(role, "system" | "user" | "assistant" | "tool") {
            return Err(RepositoryError::InvalidAgentState("invalid message role"));
        }
        let mut tx = self.pool.begin().await?;
        let message = self
            .append_conversation_message_in_tx(
                &mut tx,
                conversation_id,
                run_id,
                role,
                content,
                attachment_ids,
            )
            .await?;
        tx.commit().await?;
        Ok(message)
    }

    async fn append_conversation_message_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        conversation_id: Uuid,
        run_id: Option<Uuid>,
        role: &str,
        content: Value,
        attachment_ids: &[Uuid],
    ) -> Result<ConversationMessage, RepositoryError> {
        self.ensure_task_fence(tx).await?;
        let found = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM conversations WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(conversation_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut **tx)
        .await?;
        if found.is_none() {
            return Err(RepositoryError::NotFound("conversation"));
        }
        let sequence: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence) + 1, 0) FROM conversation_messages WHERE conversation_id = $1")
            .bind(conversation_id).fetch_one(&mut **tx).await?;
        let row: ConversationMessageRow = sqlx::query_as("INSERT INTO conversation_messages (id, conversation_id, run_id, sequence, role, content) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, conversation_id, run_id, sequence, role, content, created_at")
            .bind(Uuid::new_v4()).bind(conversation_id).bind(run_id).bind(sequence).bind(role).bind(content)
            .fetch_one(&mut **tx).await?;
        let message: ConversationMessage = row.into();
        if !attachment_ids.is_empty() {
            let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
            let files: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM files WHERE workspace_id = $1 AND deleted_at IS NULL AND id = ANY($2) FOR UPDATE")
                .bind(workspace_id).bind(attachment_ids).fetch_all(&mut **tx).await?;
            if files.len() != attachment_ids.len() {
                return Err(RepositoryError::NotFound("file"));
            }
            for (position, file_id) in attachment_ids.iter().enumerate() {
                sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, $5)")
                    .bind(Uuid::new_v4()).bind(workspace_id).bind(message.id).bind(file_id).bind(position as i32)
                    .execute(&mut **tx).await?;
            }
            sqlx::query("UPDATE files SET attachment_expires_at = NULL, updated_at = now() WHERE workspace_id = $1 AND id = ANY($2)")
                .bind(workspace_id)
                .bind(attachment_ids)
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1")
            .bind(conversation_id)
            .execute(&mut **tx)
            .await?;
        Ok(message)
    }

    /// Atomically records the user's message and queues its run. A failed
    /// enqueue must not leave text that a later unrelated run will replay.
    pub async fn submit_agent_message(
        &self,
        conversation_id: Uuid,
        actor: Uuid,
        content: Value,
        attachment_ids: &[Uuid],
        provider_base_url: &str,
        model: &str,
    ) -> Result<AgentRun, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.append_conversation_message_in_tx(
            &mut tx,
            conversation_id,
            None,
            "user",
            content,
            attachment_ids,
        )
        .await?;
        let run = self
            .create_agent_run_in_tx(&mut tx, conversation_id, actor, provider_base_url, model)
            .await?;
        tx.commit().await?;
        Ok(run)
    }

    /// Creates a run tied to the authenticated human that initiated it. The
    /// runner must re-authorize this principal before executing a mutation.
    pub async fn create_agent_run_for_user(
        &self,
        conversation_id: Uuid,
        actor: Uuid,
        provider_base_url: &str,
        model: &str,
    ) -> Result<AgentRun, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let run = self
            .create_agent_run_in_tx(&mut tx, conversation_id, actor, provider_base_url, model)
            .await?;
        tx.commit().await?;
        Ok(run)
    }

    async fn create_agent_run_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        conversation_id: Uuid,
        actor: Uuid,
        provider_base_url: &str,
        model: &str,
    ) -> Result<AgentRun, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let run: AgentRun = sqlx::query_as("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model, initiated_by_user_id) SELECT $1, $2, $3, 'interactive', 'queued', $4, $5, $6 WHERE EXISTS (SELECT 1 FROM conversations WHERE id = $3 AND workspace_id = $2) RETURNING id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(conversation_id).bind(provider_base_url).bind(model).bind(actor)
            .fetch_optional(&mut **tx).await?.ok_or(RepositoryError::NotFound("conversation"))?;
        self.enqueue_task(
            tx,
            crate::task_queue::TaskInsert {
                workspace_id,
                kind: crate::task_queue::TaskKind::AgentRunV1,
                subject_id: run.id,
                generation: 0,
                payload: serde_json::json!({"agent_run_id": run.id.to_string()}),
                correlation_id: None,
                causation_id: None,
            },
        )
        .await?;
        Ok(run)
    }

    /// Atomically advances a run through the foundation lifecycle. Terminal
    /// states are immutable and always receive a finish timestamp.
    pub async fn transition_agent_run(
        &self,
        run_id: Uuid,
        next_status: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<AgentRun, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let current: AgentRun = sqlx::query_as("SELECT id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("agent run"))?;
        let valid = matches!(
            (current.status.as_str(), next_status),
            ("queued", "running" | "cancelled")
                | (
                    "running",
                    "awaiting_approval" | "completed" | "failed" | "cancelled"
                )
                | ("awaiting_approval", "queued" | "cancelled")
        );
        if !valid {
            return Err(RepositoryError::InvalidAgentState("invalid run transition"));
        }
        let terminal = matches!(
            next_status,
            "completed" | "failed" | "cancelled" | "skipped"
        );
        let run = sqlx::query_as("UPDATE agent_runs SET status = $2, started_at = CASE WHEN $2 = 'running' THEN COALESCE(started_at, now()) ELSE started_at END, finished_at = CASE WHEN $3 THEN now() ELSE NULL END, error_code = $4, error_message = $5 WHERE id = $1 RETURNING id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(run_id).bind(next_status).bind(terminal).bind(error_code).bind(error_message).fetch_one(&mut *tx).await?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, $3, 'status', $4)")
            .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(serde_json::json!({"status": next_status})).execute(&mut *tx).await?;
        if terminal {
            sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, $3, 'terminal', $4)")
                .bind(Uuid::new_v4()).bind(run_id).bind(sequence + 1).bind(serde_json::json!({"status": next_status, "code": error_code})).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(run)
    }

    /// Claims a queued run while holding the shared task token. A reclaimed
    /// running run is terminalized rather than resumed because its provider
    /// request may have produced unobserved effects.
    pub async fn claim_queued_agent_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<AgentRun>, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let current: AgentRun = sqlx::query_as("SELECT id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
            .bind(run_id).bind(ws).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("agent run"))?;
        if current.status == "running" {
            let sequence: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
            )
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query("UPDATE agent_runs SET status='failed',finished_at=now(),error_code='interrupted',error_message='agent task lease expired during provider execution' WHERE id=$1 AND workspace_id=$2 AND status='running'")
                .bind(run_id).bind(ws).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) VALUES ($1,$2,$3,'status','{\"status\":\"failed\"}'::jsonb),($4,$2,$3+1,'terminal','{\"status\":\"failed\",\"code\":\"interrupted\"}'::jsonb)")
                .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(Uuid::new_v4()).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        }
        if current.status != "queued" {
            tx.commit().await?;
            return Ok(None);
        }
        let run = sqlx::query_as("UPDATE agent_runs SET status='running',started_at=COALESCE(started_at,now()) WHERE id=$1 AND workspace_id=$2 AND status='queued' RETURNING id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(run_id).bind(ws).fetch_one(&mut *tx).await?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) VALUES ($1,$2,$3,'status','{\"status\":\"running\"}'::jsonb)")
            .bind(Uuid::new_v4()).bind(run_id).bind(sequence).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(run))
    }

    /// Startup recovery only acquires runs whose latest shared task lease has
    /// expired. The replacement recovery lease fences the lifecycle update and
    /// acknowledgement, so another API replica's healthy handler is untouched.
    pub async fn recover_interrupted_agent_runs(&self) -> Result<(), RepositoryError> {
        loop {
            let mut tx = self.pool.begin().await?;
            let candidate: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as("SELECT t.id,t.workspace_id,t.subject_id FROM tasks t JOIN agent_runs r ON r.id=t.subject_id AND r.workspace_id=t.workspace_id WHERE t.kind='agent_run.v1' AND t.status='leased' AND t.lease_until<=now() AND r.status='running' AND t.generation=(SELECT max(newer.generation) FROM tasks newer WHERE newer.workspace_id=t.workspace_id AND newer.kind=t.kind AND newer.subject_id=t.subject_id) AND NOT EXISTS (SELECT 1 FROM tasks active WHERE active.workspace_id=t.workspace_id AND active.kind=t.kind AND active.subject_id=t.subject_id AND active.status='leased' AND active.lease_until>now()) FOR UPDATE OF t,r SKIP LOCKED LIMIT 1")
                .fetch_optional(&mut *tx).await?;
            let Some((task_id, workspace_id, run_id)) = candidate else {
                tx.commit().await?;
                break;
            };
            let owner = format!("agent-recovery:{}", Uuid::new_v4());
            let token = Uuid::new_v4();
            if sqlx::query("UPDATE tasks SET lease_owner=$2,lease_token=$3,lease_until=now()+interval '60 seconds',updated_at=now() WHERE id=$1 AND status='leased' AND lease_until<=now()")
                .bind(task_id).bind(&owner).bind(token).execute(&mut *tx).await?.rows_affected() != 1 {
                tx.rollback().await?;
                continue;
            }
            let fenced: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tasks WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>now() FOR KEY SHARE)")
                .bind(task_id).bind(&owner).bind(token).fetch_one(&mut *tx).await?;
            if !fenced {
                tx.rollback().await?;
                continue;
            }
            let sequence: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id=$1",
            )
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query("UPDATE agent_runs SET status='failed',finished_at=now(),error_code='interrupted',error_message='agent process restarted during execution' WHERE id=$1 AND workspace_id=$2 AND status='running'")
                .bind(run_id).bind(workspace_id).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO agent_run_events (id,run_id,sequence,event_type,payload) VALUES ($1,$2,$3,'status','{\"status\":\"failed\"}'::jsonb),($4,$2,$3+1,'terminal','{\"status\":\"failed\",\"code\":\"interrupted\"}'::jsonb)")
                .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(Uuid::new_v4()).execute(&mut *tx).await?;
            sqlx::query("UPDATE tasks SET status='succeeded',lease_owner=NULL,lease_token=NULL,lease_until=NULL,completed_at=now(),updated_at=now() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>now()")
                .bind(task_id).bind(&owner).bind(token).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        Ok(())
    }

    pub async fn agent_run_initiator(&self, run_id: Uuid) -> Result<(Uuid, Uuid), RepositoryError> {
        sqlx::query_as("SELECT initiated_by_user_id, workspace_id FROM agent_runs WHERE id = $1 AND workspace_id = $2")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_optional(&self.pool).await?
            .and_then(|(actor, workspace): (Option<Uuid>, Uuid)| actor.map(|actor| (actor, workspace)))
            .ok_or(RepositoryError::InvalidAgentState("agent run has no initiating user"))
    }

    pub async fn get_agent_run(&self, run_id: Uuid) -> Result<AgentRun, RepositoryError> {
        sqlx::query_as("SELECT id, conversation_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 AND workspace_id = $2")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&self.pool).await?.ok_or(RepositoryError::NotFound("agent run"))
    }

    pub async fn decided_agent_tool_calls(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<AgentToolCall>, RepositoryError> {
        Ok(sqlx::query_as("SELECT call.id, call.run_id, call.sequence, call.provider_call_id, call.tool_name, call.arguments, call.change_summary, call.result, call.error, call.state, call.decided_by_user_id, call.decided_at, call.created_at, call.completed_at FROM agent_tool_calls call JOIN agent_runs run ON run.id = call.run_id WHERE call.run_id = $1 AND run.workspace_id = $2 AND call.state IN ('approved', 'rejected') AND call.completed_at IS NULL ORDER BY call.sequence")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
    }

    /// Whether this run still has mutations awaiting a human decision.
    pub async fn has_pending_agent_tool_calls(
        &self,
        run_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM agent_tool_calls call JOIN agent_runs run ON run.id = call.run_id WHERE call.run_id = $1 AND run.workspace_id = $2 AND call.state = 'pending_approval')")
            .bind(run_id)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn create_agent_tool_call(
        &self,
        run_id: Uuid,
        provider_call_id: Option<&str>,
        tool_name: &str,
        arguments: Value,
        change_summary: Option<&str>,
        state: &str,
    ) -> Result<AgentToolCall, RepositoryError> {
        if !matches!(
            state,
            "pending_approval" | "approved" | "rejected" | "completed" | "failed"
        ) {
            return Err(RepositoryError::InvalidAgentState(
                "invalid tool call state",
            ));
        }
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM agent_runs WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(run_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(RepositoryError::NotFound("agent run"));
        }
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_tool_calls WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        // Every newly proposed or automatically executed call starts undecided.
        // Only `decide_tool_call` may set an approval decision and timestamp.
        let call = sqlx::query_as("INSERT INTO agent_tool_calls (id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, state) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, result, error, state, decided_by_user_id, decided_at, created_at, completed_at")
            .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(provider_call_id).bind(tool_name).bind(arguments).bind(change_summary).bind(state).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(call)
    }

    pub async fn complete_agent_tool_call(
        &self,
        tool_call_id: Uuid,
        result: Result<Value, Value>,
    ) -> Result<AgentToolCall, RepositoryError> {
        let (result, error, state) = match result {
            Ok(value) => (Some(value), None, "completed"),
            Err(value) => (None, Some(value), "failed"),
        };
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let call = sqlx::query_as("UPDATE agent_tool_calls call SET result = $3, error = $4, state = CASE WHEN call.state = 'rejected' THEN 'rejected' ELSE $5 END, completed_at = now() FROM agent_runs run WHERE call.id = $1 AND call.run_id = run.id AND run.workspace_id = $2 AND call.state IN ('approved', 'pending_approval', 'rejected') RETURNING call.id, call.run_id, call.sequence, call.provider_call_id, call.tool_name, call.arguments, call.change_summary, call.result, call.error, call.state, call.decided_by_user_id, call.decided_at, call.created_at, call.completed_at")
            .bind(tool_call_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(result).bind(error).bind(state).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::InvalidAgentState("tool call cannot be completed"))?;
        tx.commit().await?;
        Ok(call)
    }

    pub async fn append_run_event(
        &self,
        run_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<AgentRunEvent, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.ensure_task_fence(&mut tx).await?;
        let exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM agent_runs WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(run_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(RepositoryError::NotFound("agent run"));
        }
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let event = sqlx::query_as("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, $3, $4, $5) RETURNING id, run_id, sequence, event_type, payload, created_at")
            .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(event_type).bind(payload).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(event)
    }

    /// Makes an approval decision exactly once. The run transition and durable
    /// event share one transaction, so a restart cannot observe a decision
    /// without its replayable status evidence.
    pub async fn decide_tool_call(
        &self,
        tool_call_id: Uuid,
        actor: Uuid,
        decision: ApprovalDecision,
    ) -> Result<AgentToolCall, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let call: AgentToolCall = sqlx::query_as("SELECT id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, result, error, state, decided_by_user_id, decided_at, created_at, completed_at FROM agent_tool_calls WHERE id = $1 AND EXISTS (SELECT 1 FROM agent_runs run WHERE run.id = agent_tool_calls.run_id AND run.workspace_id = $2) FOR UPDATE")
            .bind(tool_call_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("agent tool call"))?;
        if call.state != "pending_approval" {
            return Err(RepositoryError::ApprovalAlreadyDecided);
        }
        let state = match decision {
            ApprovalDecision::Approve => "approved",
            ApprovalDecision::Reject => "rejected",
        };
        let call: AgentToolCall = sqlx::query_as("UPDATE agent_tool_calls SET state = $2, decided_by_user_id = $3, decided_at = now() WHERE id = $1 RETURNING id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, result, error, state, decided_by_user_id, decided_at, created_at, completed_at")
            .bind(tool_call_id).bind(state).bind(actor).fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE agent_runs SET status = 'queued' WHERE id = $1 AND status = 'awaiting_approval'").bind(call.run_id).execute(&mut *tx).await?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
        )
        .bind(call.run_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, $3, 'status', $4)")
            .bind(Uuid::new_v4()).bind(call.run_id).bind(sequence).bind(serde_json::json!({"tool_call_id": tool_call_id, "decision": state})).execute(&mut *tx).await?;
        let generation: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(generation) + 1, 0) FROM tasks WHERE workspace_id = $1 AND kind = 'agent_run.v1' AND subject_id = $2")
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(call.run_id).fetch_one(&mut *tx).await?;
        self.enqueue_task(
            &mut tx,
            crate::task_queue::TaskInsert {
                workspace_id: self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID),
                kind: crate::task_queue::TaskKind::AgentRunV1,
                subject_id: call.run_id,
                generation,
                payload: serde_json::json!({"agent_run_id": call.run_id.to_string()}),
                correlation_id: None,
                causation_id: None,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(call)
    }
}

impl CatalogRepository {
    pub async fn list_conversations(&self) -> Result<Vec<Conversation>, RepositoryError> {
        Ok(sqlx::query_as("SELECT id, workspace_id, created_by_user_id, title, created_at, updated_at, archived_at FROM conversations WHERE workspace_id = $1 AND archived_at IS NULL ORDER BY updated_at DESC")
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
    }

    pub async fn get_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<Conversation, RepositoryError> {
        sqlx::query_as("SELECT id, workspace_id, created_by_user_id, title, created_at, updated_at, archived_at FROM conversations WHERE id = $1 AND workspace_id = $2 AND archived_at IS NULL")
            .bind(conversation_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("conversation"))
    }

    pub async fn update_conversation_title(
        &self,
        conversation_id: Uuid,
        title: &str,
    ) -> Result<Conversation, RepositoryError> {
        sqlx::query_as("UPDATE conversations SET title = $3, updated_at = now() WHERE id = $1 AND workspace_id = $2 AND archived_at IS NULL RETURNING id, workspace_id, created_by_user_id, title, created_at, updated_at, archived_at")
            .bind(conversation_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(title).fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("conversation"))
    }

    /// Conversations are retained for run/message audit history, but hidden
    /// from ordinary list/read calls after deletion.
    pub async fn archive_conversation(&self, conversation_id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE conversations SET archived_at = now(), updated_at = now() WHERE id = $1 AND workspace_id = $2 AND archived_at IS NULL")
            .bind(conversation_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("conversation"));
        }
        Ok(())
    }

    pub async fn agent_runs_for_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<AgentRun>, RepositoryError> {
        Ok(sqlx::query_as("SELECT r.id, r.conversation_id, r.origin, r.status, r.provider_base_url, r.model, r.started_at, r.finished_at, r.error_code, r.error_message, r.created_at FROM agent_runs r JOIN conversations c ON c.id = r.conversation_id WHERE r.conversation_id = $1 AND c.workspace_id = $2 ORDER BY r.created_at DESC")
            .bind(conversation_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
    }

    pub async fn pending_agent_tool_calls(
        &self,
        conversation_id: Option<Uuid>,
    ) -> Result<Vec<AgentToolCall>, RepositoryError> {
        Ok(sqlx::query_as("SELECT call.id, call.run_id, call.sequence, call.provider_call_id, call.tool_name, call.arguments, call.change_summary, call.result, call.error, call.state, call.decided_by_user_id, call.decided_at, call.created_at, call.completed_at FROM agent_tool_calls call JOIN agent_runs run ON run.id = call.run_id WHERE run.workspace_id = $1 AND call.state = 'pending_approval' AND ($2::uuid IS NULL OR run.conversation_id = $2) ORDER BY call.created_at")
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(conversation_id).fetch_all(&self.pool).await?)
    }

    pub async fn agent_run_events_after(
        &self,
        run_id: Uuid,
        after: i64,
    ) -> Result<Vec<AgentRunEvent>, RepositoryError> {
        Ok(sqlx::query_as("SELECT event.id, event.run_id, event.sequence, event.event_type, event.payload, event.created_at FROM agent_run_events event JOIN agent_runs run ON run.id = event.run_id WHERE event.run_id = $1 AND run.workspace_id = $2 AND event.sequence > $3 ORDER BY event.sequence")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(after).fetch_all(&self.pool).await?)
    }
}

impl CatalogRepository {
    pub async fn agent_event_sequence(
        &self,
        run_id: Uuid,
        event_id: Uuid,
    ) -> Result<i64, RepositoryError> {
        sqlx::query_scalar("SELECT event.sequence FROM agent_run_events event JOIN agent_runs run ON run.id = event.run_id WHERE event.id = $1 AND event.run_id = $2 AND run.workspace_id = $3")
            .bind(event_id).bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&self.pool).await?
            .ok_or(RepositoryError::NotFound("agent run event"))
    }
}
