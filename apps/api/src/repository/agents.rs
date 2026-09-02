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
        Ok(sqlx::query_as("SELECT m.id, m.conversation_id, m.run_id, m.sequence, m.role, m.content, m.created_at FROM conversation_messages m JOIN conversations c ON c.id = m.conversation_id WHERE m.conversation_id = $1 AND c.workspace_id = $2 ORDER BY m.sequence")
            .bind(conversation_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
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

    /// Creates a run tied to the authenticated human that initiated it. The
    /// runner must re-authorize this principal before executing a mutation.
    pub async fn create_agent_run_for_user(
        &self,
        conversation_id: Uuid,
        actor: Uuid,
        provider_base_url: &str,
        model: &str,
    ) -> Result<AgentRun, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let run = sqlx::query_as("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model, initiated_by_user_id) SELECT $1, $2, $3, 'interactive', 'queued', $4, $5, $6 WHERE EXISTS (SELECT 1 FROM conversations WHERE id = $3 AND workspace_id = $2) RETURNING id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(conversation_id).bind(provider_base_url).bind(model).bind(actor)
            .fetch_optional(&self.pool).await?.ok_or(RepositoryError::NotFound("conversation"))?;
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
        let current: AgentRun = sqlx::query_as("SELECT id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 AND workspace_id = $2 FOR UPDATE")
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

    pub async fn agent_run_initiator(&self, run_id: Uuid) -> Result<(Uuid, Uuid), RepositoryError> {
        sqlx::query_as("SELECT initiated_by_user_id, workspace_id FROM agent_runs WHERE id = $1 AND workspace_id = $2")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_optional(&self.pool).await?
            .and_then(|(actor, workspace): (Option<Uuid>, Uuid)| actor.map(|actor| (actor, workspace)))
            .ok_or(RepositoryError::InvalidAgentState("agent run has no initiating user"))
    }

    pub async fn get_agent_run(&self, run_id: Uuid) -> Result<AgentRun, RepositoryError> {
        sqlx::query_as("SELECT id, conversation_id, schedule_id, origin, status, provider_base_url, model, started_at, finished_at, error_code, error_message, created_at FROM agent_runs WHERE id = $1 AND workspace_id = $2")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_optional(&self.pool).await?.ok_or(RepositoryError::NotFound("agent run"))
    }

    pub async fn decided_agent_tool_calls(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<AgentToolCall>, RepositoryError> {
        Ok(sqlx::query_as("SELECT call.id, call.run_id, call.sequence, call.provider_call_id, call.tool_name, call.arguments, call.change_summary, call.result, call.error, call.state, call.decided_by_user_id, call.decided_at, call.created_at, call.completed_at FROM agent_tool_calls call JOIN agent_runs run ON run.id = call.run_id WHERE call.run_id = $1 AND run.workspace_id = $2 AND call.state IN ('approved', 'rejected') ORDER BY call.sequence")
            .bind(run_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).fetch_all(&self.pool).await?)
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
        let call = sqlx::query_as("INSERT INTO agent_tool_calls (id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, state, decided_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CASE WHEN $8 IN ('approved', 'rejected') THEN now() ELSE NULL END) RETURNING id, run_id, sequence, provider_call_id, tool_name, arguments, change_summary, result, error, state, decided_by_user_id, decided_at, created_at, completed_at")
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
        let call = sqlx::query_as("UPDATE agent_tool_calls call SET result = $3, error = $4, state = CASE WHEN call.state = 'rejected' THEN 'rejected' ELSE $5 END, completed_at = now() FROM agent_runs run WHERE call.id = $1 AND call.run_id = run.id AND run.workspace_id = $2 AND call.state IN ('approved', 'pending_approval', 'rejected') RETURNING call.id, call.run_id, call.sequence, call.provider_call_id, call.tool_name, call.arguments, call.change_summary, call.result, call.error, call.state, call.decided_by_user_id, call.decided_at, call.created_at, call.completed_at")
            .bind(tool_call_id).bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID)).bind(result).bind(error).bind(state).fetch_optional(&self.pool).await?.ok_or(RepositoryError::InvalidAgentState("tool call cannot be completed"))?;
        Ok(call)
    }

    pub async fn append_run_event(
        &self,
        run_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<AgentRunEvent, RepositoryError> {
        let mut tx = self.pool.begin().await?;
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
        tx.commit().await?;
        Ok(call)
    }
}
