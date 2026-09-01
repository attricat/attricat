use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub created_by_user_id: Option<Uuid>,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub run_id: Option<Uuid>,
    pub sequence: i64,
    pub role: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AgentRun {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub schedule_id: Option<Uuid>,
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

#[derive(Clone, Debug, sqlx::FromRow)]
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

#[derive(Clone, Debug, sqlx::FromRow)]
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

    /// Appends under a conversation row lock so each thread's durable sequence
    /// remains gap-free even when provider and user writes race.
    pub async fn append_conversation_message(
        &self,
        conversation_id: Uuid,
        run_id: Option<Uuid>,
        role: &str,
        content: Value,
    ) -> Result<ConversationMessage, RepositoryError> {
        if !matches!(role, "system" | "user" | "assistant" | "tool") {
            return Err(RepositoryError::InvalidAgentState("invalid message role"));
        }
        let mut tx = self.pool.begin().await?;
        let found = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM conversations WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(conversation_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *tx)
        .await?;
        if found.is_none() {
            return Err(RepositoryError::NotFound("conversation"));
        }
        let sequence: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence) + 1, 0) FROM conversation_messages WHERE conversation_id = $1")
            .bind(conversation_id).fetch_one(&mut *tx).await?;
        let message = sqlx::query_as("INSERT INTO conversation_messages (id, conversation_id, run_id, sequence, role, content) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, conversation_id, run_id, sequence, role, content, created_at")
            .bind(Uuid::new_v4()).bind(conversation_id).bind(run_id).bind(sequence).bind(role).bind(content)
            .fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1")
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(message)
    }

    /// Creates a queued run with a provider metadata snapshot. The API key is
    /// deliberately not an argument and cannot enter this table.
    pub async fn create_agent_run(
        &self,
        conversation_id: Uuid,
        schedule_id: Option<Uuid>,
        origin: &str,
        provider_base_url: &str,
        model: &str,
    ) -> Result<AgentRun, RepositoryError> {
        if !matches!(origin, "interactive" | "scheduled" | "manual") {
            return Err(RepositoryError::InvalidAgentState("invalid run origin"));
        }
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM conversations WHERE id = $1 AND workspace_id = $2 FOR UPDATE",
        )
        .bind(conversation_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(RepositoryError::NotFound("conversation"));
        }
        let run = sqlx::query_as("INSERT INTO agent_runs (id, workspace_id, conversation_id, schedule_id, origin, status, provider_base_url, model) VALUES ($1, $2, $3, $4, $5, 'queued', $6, $7) RETURNING id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(conversation_id).bind(schedule_id).bind(origin).bind(provider_base_url).bind(model)
            .fetch_one(&mut *tx).await?;
        tx.commit().await?;
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
        let current: AgentRun = sqlx::query_as("SELECT id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 FOR UPDATE")
            .bind(run_id).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("agent run"))?;
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
        let run = sqlx::query_as("UPDATE agent_runs SET status = $2, started_at = CASE WHEN $2 = 'running' THEN COALESCE(started_at, now()) ELSE started_at END, finished_at = CASE WHEN $3 THEN now() ELSE NULL END, error_code = $4, error_message = $5 WHERE id = $1 RETURNING id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(run_id).bind(next_status).bind(terminal).bind(error_code).bind(error_message).fetch_one(&mut *tx).await?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence) + 1, 0) FROM agent_run_events WHERE run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, $3, 'status', $4)")
            .bind(Uuid::new_v4()).bind(run_id).bind(sequence).bind(serde_json::json!({"status": next_status})).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn append_run_event(
        &self,
        run_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<AgentRunEvent, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let exists =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM agent_runs WHERE id = $1 FOR UPDATE")
                .bind(run_id)
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
        let call: AgentToolCall = sqlx::query_as("SELECT id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, result, error, state, decided_by_user_id, decided_at, created_at, completed_at FROM agent_tool_calls WHERE id = $1 FOR UPDATE")
            .bind(tool_call_id).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("agent tool call"))?;
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
        tx.commit().await?;
        Ok(call)
    }
}
