use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::solution_packs::{
    BlueprintPublication, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES, PlanningWorkspaceSnapshot,
    SOLUTION_PACK_PLAN_EXPIRY_HOURS, SolutionPackPlanDraft, ValidatedSolutionPack,
    build_create_only_plan,
};

use super::{CatalogRepository, RepositoryError};

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackPlan {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub workspace_id: Uuid,
    pub source_kind: String,
    pub source_metadata: Value,
    pub archive_sha256: String,
    pub manifest_version: i64,
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub pack_description: String,
    pub host_api: String,
    pub prefix: String,
    pub blueprint_publication: String,
    pub ready: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[sqlx(skip)]
    pub mappings: Vec<SolutionPackPlanMapping>,
    #[sqlx(skip)]
    pub actions: Vec<SolutionPackPlanAction>,
    #[sqlx(skip)]
    pub conflicts: Vec<SolutionPackPlanConflict>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackPlanMapping {
    pub position: i64,
    pub resource_kind: String,
    pub logical_key: String,
    pub target_id: Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    pub mapping_kind: String,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackPlanAction {
    pub position: i64,
    pub resource_kind: String,
    pub logical_key: String,
    pub action: String,
    pub reason_code: String,
    pub summary: Value,
    pub preconditions: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolutionPackPlanConflict {
    pub position: i64,
    pub resource_kind: String,
    pub logical_key: String,
    pub action: String,
    pub reason_code: String,
}

impl CatalogRepository {
    /// Solution-pack permissions are bootstrapped in application code so the
    /// database migration history remains declarative.
    pub async fn ensure_solution_pack_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('solution_packs.manage', 'Inspect and manage solution packs') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'solution_packs.manage') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_solution_pack_plan(
        &self,
        pack: &ValidatedSolutionPack,
        prefix: &str,
        publication: BlueprintPublication,
    ) -> Result<SolutionPackPlan, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;
        let default_context_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
        )
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        let physical_codes = sqlx::query_scalar::<_, String>(
            "SELECT code FROM blueprints WHERE workspace_id = $1 AND code IS NOT NULL UNION SELECT code FROM attribute_contexts WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let draft = build_create_only_plan(
            pack,
            prefix,
            publication,
            &PlanningWorkspaceSnapshot {
                workspace_id,
                physical_codes,
                default_context_id,
            },
        )
        .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;

        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(SOLUTION_PACK_PLAN_EXPIRY_HOURS);
        let actor_user_id = self
            .audit_context
            .as_ref()
            .and_then(|context| context.actor_user_id);
        let actor_token_id = self
            .audit_context
            .as_ref()
            .and_then(|context| context.actor_token_id);
        let manifest = pack.manifest();
        let plan = materialize_plan(
            (id, workspace_id),
            pack,
            prefix,
            publication,
            (created_at, expires_at),
            &draft,
        );
        let response_size = serde_json::to_vec(&plan)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?
            .len();
        if response_size > MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "solution-pack plan summary exceeds the size limit".to_owned(),
            ));
        }
        sqlx::query(
            "INSERT INTO solution_pack_plans (id, workspace_id, actor_user_id, actor_token_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, created_at, expires_at) VALUES ($1, $2, $3, $4, 'local_archive', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(actor_token_id)
        .bind(serde_json::json!({"side_loaded": true}))
        .bind(pack.archive_sha256())
        .bind(i64::from(manifest.manifest_version))
        .bind(&manifest.id)
        .bind(&manifest.name)
        .bind(&manifest.version)
        .bind(&manifest.description)
        .bind(&manifest.catalog.host_api)
        .bind(prefix)
        .bind(publication.as_str())
        .bind(draft.ready)
        .bind(created_at)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        insert_plan_rows(&mut tx, workspace_id, id, &draft).await?;
        let mut audit_repository = self.clone();
        if let Some(audit) = audit_repository.audit_context.as_mut() {
            audit.target = serde_json::json!({"type": "solution_pack_plan", "id": id});
        }
        audit_repository.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(plan)
    }

    pub async fn get_solution_pack_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Option<SolutionPackPlan>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let Some(mut plan) = sqlx::query_as::<_, SolutionPackPlan>(
            "SELECT id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, created_at, expires_at FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        plan.mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            "SELECT position, resource_kind, logical_key, target_id, target_code, target_version, mapping_kind FROM solution_pack_plan_mappings WHERE workspace_id = $1 AND plan_id = $2 ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        plan.actions = sqlx::query_as::<_, SolutionPackPlanAction>(
            "SELECT position, resource_kind, logical_key, action, reason_code, summary, preconditions FROM solution_pack_plan_actions WHERE workspace_id = $1 AND plan_id = $2 ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        plan.conflicts = conflict_summaries(&plan.actions);
        Ok(Some(plan))
    }
}

fn materialize_plan(
    identity: (Uuid, Uuid),
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    timestamps: (DateTime<Utc>, DateTime<Utc>),
    draft: &SolutionPackPlanDraft,
) -> SolutionPackPlan {
    let (id, workspace_id) = identity;
    let (created_at, expires_at) = timestamps;
    let manifest = pack.manifest();
    let mappings = draft
        .mappings
        .iter()
        .enumerate()
        .map(|(position, mapping)| SolutionPackPlanMapping {
            position: position as i64,
            resource_kind: mapping.resource_kind.to_owned(),
            logical_key: mapping.logical_key.clone(),
            target_id: mapping.target_id,
            target_code: mapping.target_code.clone(),
            target_version: mapping.target_version,
            mapping_kind: mapping.mapping_kind.to_owned(),
        })
        .collect();
    let actions = draft
        .actions
        .iter()
        .enumerate()
        .map(|(position, action)| SolutionPackPlanAction {
            position: position as i64,
            resource_kind: action.resource_kind.to_owned(),
            logical_key: action.logical_key.clone(),
            action: action.action.to_owned(),
            reason_code: action.reason_code.to_owned(),
            summary: action.summary.clone(),
            preconditions: action.preconditions.clone(),
        })
        .collect::<Vec<_>>();
    let conflicts = conflict_summaries(&actions);
    SolutionPackPlan {
        id,
        workspace_id,
        source_kind: "local_archive".to_owned(),
        source_metadata: serde_json::json!({"side_loaded": true}),
        archive_sha256: pack.archive_sha256().to_owned(),
        manifest_version: i64::from(manifest.manifest_version),
        pack_id: manifest.id.clone(),
        pack_name: manifest.name.clone(),
        pack_version: manifest.version.clone(),
        pack_description: manifest.description.clone(),
        host_api: manifest.catalog.host_api.clone(),
        prefix: prefix.to_owned(),
        blueprint_publication: publication.as_str().to_owned(),
        ready: draft.ready,
        created_at,
        expires_at,
        mappings,
        actions,
        conflicts,
    }
}

fn conflict_summaries(actions: &[SolutionPackPlanAction]) -> Vec<SolutionPackPlanConflict> {
    actions
        .iter()
        .filter(|action| matches!(action.action.as_str(), "conflict" | "blocked"))
        .map(|action| SolutionPackPlanConflict {
            position: action.position,
            resource_kind: action.resource_kind.clone(),
            logical_key: action.logical_key.clone(),
            action: action.action.clone(),
            reason_code: action.reason_code.clone(),
        })
        .collect()
}

async fn insert_plan_rows(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
    draft: &SolutionPackPlanDraft,
) -> Result<(), RepositoryError> {
    for (position, mapping) in draft.mappings.iter().enumerate() {
        sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id, workspace_id, position, resource_kind, logical_key, target_id, target_code, target_version, mapping_kind, snapshot) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position as i64)
            .bind(mapping.resource_kind)
            .bind(&mapping.logical_key)
            .bind(mapping.target_id)
            .bind(&mapping.target_code)
            .bind(mapping.target_version)
            .bind(mapping.mapping_kind)
            .bind(&mapping.snapshot)
            .execute(&mut **tx)
            .await?;
    }
    for (position, action) in draft.actions.iter().enumerate() {
        sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id, workspace_id, position, resource_kind, logical_key, action, reason_code, summary, normalized_payload, preconditions) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position as i64)
            .bind(action.resource_kind)
            .bind(&action.logical_key)
            .bind(action.action)
            .bind(action.reason_code)
            .bind(&action.summary)
            .bind(&action.normalized_payload)
            .bind(&action.preconditions)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
