use super::*;
use crate::model::{CreateWorkflow, Workflow};
use crate::persistence_rows::{Db, IntoDomain};
use chrono::Utc;
use uuid::Uuid;

const WORKFLOW_FIELDS: &str = "w.id, w.code, w.name, w.version, w.status, w.definition, w.definition_hash, w.compiled_plan, w.published_at, w.created_at, l.enabled_version, (w.compiled_plan @> '{\"triggers\":[{\"type\":\"manual\"}]}'::jsonb) AS manual_enabled";

impl AttricatRepository {
    pub async fn list_workflows(&self) -> Result<Vec<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.workspace_id=$1 ORDER BY w.created_at DESC, w.version DESC"
        );
        Ok(sqlx::query_as::<_, Db<Workflow>>(&q)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn list_workflow_revisions(
        &self,
        id: Uuid,
    ) -> Result<Vec<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.workspace_id=$2 ORDER BY w.version DESC"
        );
        Ok(sqlx::query_as::<_, Db<Workflow>>(&q)
            .bind(id)
            .bind(self.workspace_id.0)
            .fetch_all(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn get_workflow_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Option<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.version=$2 AND w.workspace_id=$3"
        );
        Ok(sqlx::query_as::<_, Db<Workflow>>(&q)
            .bind(id)
            .bind(version)
            .bind(self.workspace_id.0)
            .fetch_optional(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn get_workflow(&self, id: Uuid) -> Result<Option<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.workspace_id=$2 ORDER BY w.version DESC LIMIT 1"
        );
        Ok(sqlx::query_as::<_, Db<Workflow>>(&q)
            .bind(id)
            .bind(self.workspace_id.0)
            .fetch_optional(&self.pool)
            .await?
            .into_domain())
    }
    pub async fn create_workflow(
        &self,
        input: CreateWorkflow,
    ) -> Result<Workflow, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let id = self
            .create_workflow_in_transaction(&mut tx, Uuid::new_v4(), input)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, 1)
            .await?
            .ok_or(RepositoryError::NotFound("workflow"))
    }
    /// Shared mutation seam for callers that create a workflow with a chosen
    /// id as part of a larger transaction, such as seed application.
    pub(super) async fn create_workflow_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        input: CreateWorkflow,
    ) -> Result<Uuid, RepositoryError> {
        let compiled = attricat_workflow::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        let ws = self.workspace_id.0;
        // A versioned primary key cannot express a unique workflow family code.
        // Serialize first revisions by workspace and code, as blueprint creation does.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("workflow-code:{ws}:{}", compiled.code))
            .execute(&mut **tx)
            .await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workflows WHERE workspace_id=$1 AND code=$2)",
        )
        .bind(ws)
        .bind(&compiled.code)
        .fetch_one(&mut **tx)
        .await?;
        if exists {
            return Err(RepositoryError::WorkflowCodeTaken);
        }
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO workflows (id,workspace_id,code,name,version,definition,definition_hash,compiled_plan) VALUES($1,$2,$3,$4,1,$5,$6,$7)").bind(id).bind(ws).bind(&compiled.code).bind(&compiled.name).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO workflow_lifecycles(workflow_id,workspace_id) VALUES($1,$2)")
            .bind(id)
            .bind(ws)
            .execute(&mut **tx)
            .await?;
        Ok(id)
    }
    pub async fn create_workflow_revision(
        &self,
        id: Uuid,
        input: CreateWorkflow,
    ) -> Result<Workflow, RepositoryError> {
        let compiled = attricat_workflow::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let previous:Option<(i64,String)>=sqlx::query_as("SELECT version,code FROM workflows WHERE id=$1 AND workspace_id=$2 ORDER BY version DESC LIMIT 1 FOR UPDATE").bind(id).bind(ws).fetch_optional(&mut *tx).await?;
        let Some((version, code)) = previous else {
            return Err(RepositoryError::NotFound("workflow"));
        };
        if code != compiled.code {
            return Err(RepositoryError::InvalidWorkflowDefinition(
                "workflow code cannot change across revisions".into(),
            ));
        };
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO workflows (id,workspace_id,code,name,version,definition,definition_hash,compiled_plan) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(id).bind(ws).bind(&compiled.code).bind(&compiled.name).bind(version+1).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, version + 1)
            .await?
            .ok_or(RepositoryError::NotFound("workflow"))
    }
    pub async fn publish_workflow_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Workflow, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.publish_workflow_revision_in_transaction(&mut tx, id, version)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, version)
            .await?
            .ok_or(RepositoryError::NotFound("workflow revision"))
    }
    pub(super) async fn publish_workflow_revision_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let affected=sqlx::query("UPDATE workflows SET status='published',published_at=COALESCE(published_at,now()) WHERE id=$1 AND version=$2 AND workspace_id=$3").bind(id).bind(version).bind(ws).execute(&mut **tx).await?;
        if affected.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("workflow revision"));
        };
        Ok(())
    }
    pub async fn enable_workflow_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Workflow, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.enable_workflow_revision_in_transaction(&mut tx, id, version)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, version)
            .await?
            .ok_or(RepositoryError::NotFound("workflow revision"))
    }
    pub(super) async fn enable_workflow_revision_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
        version: i64,
    ) -> Result<(), RepositoryError> {
        let ws = self.workspace_id.0;
        let published:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflows WHERE id=$1 AND version=$2 AND workspace_id=$3 AND status='published')").bind(id).bind(version).bind(ws).fetch_one(&mut **tx).await?;
        if !published {
            return Err(RepositoryError::WorkflowNotPublished);
        };
        // Event enqueue acquires this same transaction-scoped workspace lock before
        // allocating an outbox sequence. Holding it through the high-water read and
        // lifecycle update prevents a later-committing event from falling below the
        // activation boundary.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("workflow-activation-boundary:{ws}"))
            .execute(&mut **tx)
            .await?;
        sqlx::query("UPDATE workflow_lifecycles SET enabled_version=$2, activation_sequence=(SELECT COALESCE(max(sequence), 0) FROM domain_events WHERE workspace_id=$3), enabled_at=now(), disabled_at=NULL, updated_at=now() WHERE workflow_id=$1 AND workspace_id=$3").bind(id).bind(version).bind(ws).execute(&mut **tx).await?;
        // Create schedule cursors in the enable transaction. A scheduler that starts
        // later observes a durable post-enable boundary instead of inventing one.
        let plan: Value = sqlx::query_scalar(
            "SELECT compiled_plan FROM workflows WHERE id=$1 AND version=$2 AND workspace_id=$3",
        )
        .bind(id)
        .bind(version)
        .bind(ws)
        .fetch_one(&mut **tx)
        .await?;
        let compiled: attricat_workflow::CompiledWorkflow = serde_json::from_value(plan)
            .map_err(|error| RepositoryError::InvalidWorkflowDefinition(error.to_string()))?;
        for (trigger_index, trigger) in compiled.triggers.iter().enumerate() {
            let attricat_workflow::Trigger::Schedule { cron, .. } = trigger else {
                continue;
            };
            let schedule = attricat_workflow::parse_six_field_cron(cron).map_err(|_| {
                RepositoryError::InvalidWorkflowDefinition("stored schedule cron is invalid".into())
            })?;
            let next = schedule.after(&Utc::now()).next().ok_or_else(|| {
                RepositoryError::InvalidWorkflowDefinition(
                    "schedule has no future occurrence".into(),
                )
            })?;
            sqlx::query("INSERT INTO workflow_schedule_states(workspace_id,workflow_id,workflow_version,trigger_index,next_run_at) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING")
                .bind(ws).bind(id).bind(version).bind(trigger_index as i32).bind(next).execute(&mut **tx).await?;
        }
        Ok(())
    }
    pub async fn disable_workflow(&self, id: Uuid) -> Result<Workflow, RepositoryError> {
        let ws = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        // This conflicts with fan-out's shared lock and action execution's
        // exclusive lifecycle check, making disable a durable execution fence.
        let lifecycle_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT workflow_id FROM workflow_lifecycles WHERE workflow_id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(id)
        .bind(ws)
        .fetch_optional(&mut *tx)
        .await?;
        if lifecycle_id.is_none() {
            return Err(RepositoryError::NotFound("workflow"));
        }
        let n=sqlx::query("UPDATE workflow_lifecycles SET enabled_version=NULL,disabled_at=now(),updated_at=now() WHERE workflow_id=$1 AND workspace_id=$2").bind(id).bind(ws).execute(&mut *tx).await?;
        if n.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("workflow"));
        };
        // Cancel queued envelopes in this same lifecycle transaction. Leased
        // tasks remain token-owned until their next action boundary observes
        // the cancelled domain row, so disable never races a stale worker.
        let cancelled: Vec<Uuid> = sqlx::query_scalar("UPDATE workflow_runs SET status='cancelled',cancelled_at=clock_timestamp(),updated_at=clock_timestamp() WHERE workspace_id=$1 AND workflow_id=$2 AND status='pending' RETURNING id")
            .bind(ws).bind(id).fetch_all(&mut *tx).await?;
        self.cancel_queued_tasks_for_subjects(
            &mut tx,
            ws,
            crate::task_queue::TaskKind::WorkflowRunV1,
            &cancelled,
        )
        .await?;
        self.commit_mutation(tx).await?;
        self.get_workflow(id)
            .await?
            .ok_or(RepositoryError::NotFound("workflow"))
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Permissions are application data, leaving migrations declarative.
    pub async fn ensure_workflow_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('workflows.read', 'Read workspace workflows and revisions'), ('workflows.manage', 'Create and manage workspace workflows') ON CONFLICT (code) DO NOTHING").execute(&mut *tx).await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) SELECT $1, code FROM permissions WHERE code IN ('workflows.read', 'workflows.manage') ON CONFLICT DO NOTHING").bind(role_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
