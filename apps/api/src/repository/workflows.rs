use super::*;
use crate::model::{CreateWorkflow, Workflow};
use uuid::Uuid;

const WORKFLOW_FIELDS: &str = "w.id, w.code, w.name, w.version, w.status, w.definition, w.definition_hash, w.compiled_plan, w.published_at, w.created_at, l.enabled_version";

impl CatalogRepository {
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
    pub async fn list_workflows(&self) -> Result<Vec<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.workspace_id=$1 ORDER BY w.created_at DESC, w.version DESC"
        );
        Ok(sqlx::query_as(&q)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool)
            .await?)
    }
    pub async fn list_workflow_revisions(
        &self,
        id: Uuid,
    ) -> Result<Vec<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.workspace_id=$2 ORDER BY w.version DESC"
        );
        Ok(sqlx::query_as(&q)
            .bind(id)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool)
            .await?)
    }
    pub async fn get_workflow_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Option<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.version=$2 AND w.workspace_id=$3"
        );
        Ok(sqlx::query_as(&q)
            .bind(id)
            .bind(version)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_optional(&self.pool)
            .await?)
    }
    pub async fn get_workflow(&self, id: Uuid) -> Result<Option<Workflow>, RepositoryError> {
        let q = format!(
            "SELECT {WORKFLOW_FIELDS} FROM workflows w LEFT JOIN workflow_lifecycles l ON l.workflow_id=w.id WHERE w.id=$1 AND w.workspace_id=$2 ORDER BY w.version DESC LIMIT 1"
        );
        Ok(sqlx::query_as(&q)
            .bind(id)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_optional(&self.pool)
            .await?)
    }
    pub async fn create_workflow(
        &self,
        input: CreateWorkflow,
    ) -> Result<Workflow, RepositoryError> {
        let compiled = catalog_workflow::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workflows WHERE workspace_id=$1 AND code=$2)",
        )
        .bind(ws)
        .bind(&compiled.code)
        .fetch_one(&mut *tx)
        .await?;
        if exists {
            return Err(RepositoryError::WorkflowCodeTaken);
        }
        let id = Uuid::new_v4();
        let plan = serde_json::to_value(&compiled)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        sqlx::query("INSERT INTO workflows (id,workspace_id,code,name,version,definition,definition_hash,compiled_plan) VALUES($1,$2,$3,$4,1,$5,$6,$7)").bind(id).bind(ws).bind(&compiled.code).bind(&compiled.name).bind(input.definition).bind(&compiled.raw_definition_hash).bind(plan).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO workflow_lifecycles(workflow_id,workspace_id) VALUES($1,$2)")
            .bind(id)
            .bind(ws)
            .execute(&mut *tx)
            .await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, 1)
            .await?
            .ok_or(RepositoryError::NotFound("workflow"))
    }
    pub async fn create_workflow_revision(
        &self,
        id: Uuid,
        input: CreateWorkflow,
    ) -> Result<Workflow, RepositoryError> {
        let compiled = catalog_workflow::compile(&input.definition)
            .map_err(|e| RepositoryError::InvalidWorkflowDefinition(e.to_string()))?;
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let affected=sqlx::query("UPDATE workflows SET status='published',published_at=COALESCE(published_at,now()) WHERE id=$1 AND version=$2 AND workspace_id=$3").bind(id).bind(version).bind(ws).execute(&mut *tx).await?;
        if affected.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("workflow revision"));
        };
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, version)
            .await?
            .ok_or(RepositoryError::NotFound("workflow revision"))
    }
    pub async fn enable_workflow_revision(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<Workflow, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let published:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workflows WHERE id=$1 AND version=$2 AND workspace_id=$3 AND status='published')").bind(id).bind(version).bind(ws).fetch_one(&mut *tx).await?;
        if !published {
            return Err(RepositoryError::WorkflowNotPublished);
        };
        sqlx::query("UPDATE workflow_lifecycles SET enabled_version=$2, activation_sequence=(SELECT COALESCE(max(sequence), 0) FROM domain_events WHERE workspace_id=$3), enabled_at=now(), disabled_at=NULL, updated_at=now() WHERE workflow_id=$1 AND workspace_id=$3").bind(id).bind(version).bind(ws).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        self.get_workflow_revision(id, version)
            .await?
            .ok_or(RepositoryError::NotFound("workflow revision"))
    }
    pub async fn disable_workflow(&self, id: Uuid) -> Result<Workflow, RepositoryError> {
        let ws = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let n=sqlx::query("UPDATE workflow_lifecycles SET enabled_version=NULL,disabled_at=now(),updated_at=now() WHERE workflow_id=$1 AND workspace_id=$2").bind(id).bind(ws).execute(&mut *tx).await?;
        if n.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("workflow"));
        };
        self.commit_mutation(tx).await?;
        self.get_workflow(id)
            .await?
            .ok_or(RepositoryError::NotFound("workflow"))
    }
}
