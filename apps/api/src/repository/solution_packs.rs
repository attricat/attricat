use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain_events::{BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1, CONTEXT_CREATED_V1},
    model::{CreateAttributeContext, CreateBlueprint},
    solution_packs::{
        BlueprintPublication, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES, PlanningWorkspaceSnapshot,
        SOLUTION_PACK_PLAN_EXPIRY_HOURS, SolutionPackPlanDraft, ValidatedSolutionPack,
        build_create_only_plan,
    },
};

use super::{
    CatalogRepository, EventPublisher, RepositoryError, blueprints::blueprint_event,
    contexts::context_event,
};

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

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackApplication {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub workspace_id: Uuid,
    pub plan_id: Uuid,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub source_kind: String,
    pub source_metadata: Value,
    pub archive_sha256: String,
    pub pack_id: String,
    pub pack_version: String,
    pub blueprint_publication: String,
    pub state: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub mapping_snapshot: Value,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    pub steps: Vec<SolutionPackApplicationStep>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackApplicationSummary {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub source_kind: String,
    pub source_metadata: Value,
    pub archive_sha256: String,
    pub pack_id: String,
    pub pack_version: String,
    pub blueprint_publication: String,
    pub state: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackApplicationStep {
    pub position: i64,
    pub resource_kind: String,
    pub logical_key: String,
    pub target_id: Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    pub state: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub result_snapshot: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct RevalidationStep {
    resource_kind: String,
    target_id: Uuid,
    target_code: String,
    state: String,
    preconditions: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetAbsentPrecondition {
    kind: String,
    resource_kind: String,
    code: String,
}

#[derive(sqlx::FromRow)]
struct PendingApplicationStep {
    position: i64,
    resource_kind: String,
    logical_key: String,
    target_id: Uuid,
    target_code: String,
    target_version: Option<i64>,
    normalized_payload: Value,
}

struct StepApplicationError {
    position: Option<i64>,
    error: RepositoryError,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextPayload {
    code: String,
    data: Value,
    parent_id: Uuid,
    #[allow(dead_code)]
    parent_code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlueprintPayload {
    definition: String,
    version: i64,
    publication: String,
}

impl CatalogRepository {
    pub async fn apply_solution_pack_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<SolutionPackApplication, RepositoryError> {
        let application_id = self.prepare_solution_pack_application(plan_id).await?;
        loop {
            match self.apply_next_solution_pack_step(application_id).await {
                Ok(true) => continue,
                Ok(false) => break,
                Err(failure) => {
                    if !matches!(
                        failure.error,
                        RepositoryError::SolutionPackApplicationFailed(_)
                            | RepositoryError::SolutionPackApplicationInvalid
                    ) && self
                        .record_solution_pack_failure(
                            application_id,
                            failure.position,
                            &failure.error,
                        )
                        .await?
                    {
                        // The commit may have succeeded even though the client observed an
                        // error. Reconcile the exact attempted step and keep applying when its
                        // durable evidence is already complete.
                        continue;
                    }
                    return Err(failure.error);
                }
            }
        }
        self.get_solution_pack_application(application_id)
            .await?
            .ok_or(RepositoryError::NotFound("solution-pack application"))
    }

    async fn prepare_solution_pack_application(
        &self,
        plan_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let plan = sqlx::query_as::<_, SolutionPackPlan>(
            "SELECT id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, created_at, expires_at FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("solution-pack plan"))?;
        let existing_application = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,state FROM solution_pack_applications WHERE workspace_id=$1 AND plan_id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((application_id, state)) = &existing_application {
            if state == "completed" {
                tx.commit().await?;
                return Ok(*application_id);
            }
            if state == "invalid" {
                return Err(RepositoryError::SolutionPackApplicationInvalid);
            }
        } else {
            if !plan.ready {
                return Err(RepositoryError::SolutionPackPlanNotReady);
            }
            // Expiry prevents starting new work, but cannot strand durable work that
            // already began while the immutable plan was valid.
            if plan.expires_at <= Utc::now() {
                return Err(RepositoryError::SolutionPackPlanExpired);
            }
        }
        let mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            "SELECT m.position, m.resource_kind, m.logical_key, m.target_id, m.target_code, m.target_version, m.mapping_kind FROM solution_pack_plan_mappings m JOIN solution_pack_plan_actions a ON a.plan_id=m.plan_id AND a.logical_key=m.logical_key WHERE m.workspace_id = $1 AND m.plan_id = $2 AND a.action='create' ORDER BY m.position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&mut *tx)
        .await?;
        let mapping_snapshot = serde_json::to_value(&mappings)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
        let application_id = Uuid::new_v4();
        let actor_user_id = self
            .audit_context
            .as_ref()
            .and_then(|value| value.actor_user_id);
        let actor_token_id = self
            .audit_context
            .as_ref()
            .and_then(|value| value.actor_token_id);
        let request_id = self
            .audit_context
            .as_ref()
            .map_or_else(Uuid::new_v4, |value| value.request_id);
        let correlation_id = self
            .audit_context
            .as_ref()
            .map_or(request_id, |value| value.correlation_id);
        let inserted = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO solution_pack_applications (id, workspace_id, plan_id, actor_user_id, actor_token_id, request_id, correlation_id, source_kind, source_metadata, archive_sha256, pack_id, pack_version, blueprint_publication, state, mapping_snapshot) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'running',$14) ON CONFLICT (plan_id) DO NOTHING RETURNING id",
        )
        .bind(application_id)
        .bind(workspace_id)
        .bind(plan_id)
        .bind(actor_user_id)
        .bind(actor_token_id)
        .bind(request_id)
        .bind(correlation_id)
        .bind(&plan.source_kind)
        .bind(&plan.source_metadata)
        .bind(&plan.archive_sha256)
        .bind(&plan.pack_id)
        .bind(&plan.pack_version)
        .bind(&plan.blueprint_publication)
        .bind(mapping_snapshot)
        .fetch_optional(&mut *tx)
        .await?;
        let application_id = match inserted {
            Some(id) => {
                sqlx::query(
                    "INSERT INTO solution_pack_application_steps (application_id, workspace_id, position, plan_id, resource_kind, logical_key, target_id, target_code, target_version, state) SELECT $1, a.workspace_id, a.position, a.plan_id, a.resource_kind, a.logical_key, m.target_id, m.target_code, m.target_version, 'pending' FROM solution_pack_plan_actions a JOIN solution_pack_plan_mappings m ON m.plan_id = a.plan_id AND m.logical_key = a.logical_key WHERE a.workspace_id = $2 AND a.plan_id = $3 AND a.action = 'create' ORDER BY a.position",
                )
                .bind(id)
                .bind(workspace_id)
                .bind(plan_id)
                .execute(&mut *tx)
                .await?;
                let mut audit_repository = self.clone();
                if let Some(audit) = audit_repository.audit_context.as_mut() {
                    audit.target = serde_json::json!({"type":"solution_pack_application","id":id,"plan_id":plan_id});
                }
                audit_repository.write_audit_event(&mut tx).await?;
                id
            }
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM solution_pack_applications WHERE workspace_id = $1 AND plan_id = $2 FOR UPDATE",
            )
            .bind(workspace_id)
            .bind(plan_id)
            .fetch_one(&mut *tx)
            .await?,
        };
        let state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE workspace_id = $1 AND id = $2",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_one(&mut *tx)
        .await?;
        if state == "invalid" {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        if state != "completed" {
            if let Err(error) = self
                .revalidate_application_steps(&mut tx, application_id)
                .await
            {
                if matches!(error, RepositoryError::SolutionPackPlanStale) {
                    sqlx::query("UPDATE solution_pack_applications SET state='invalid', diagnostic_code='plan_stale', diagnostic_message='persisted target preconditions no longer match the workspace', updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
                        .bind(workspace_id)
                        .bind(application_id)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                }
                return Err(error);
            }
            sqlx::query("UPDATE solution_pack_application_steps SET state='pending', diagnostic_code=NULL, diagnostic_message=NULL, updated_at=clock_timestamp() WHERE workspace_id=$1 AND application_id=$2 AND state='failed'")
                .bind(workspace_id)
                .bind(application_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE solution_pack_applications SET state='running', diagnostic_code=NULL, diagnostic_message=NULL, updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
                .bind(workspace_id)
                .bind(application_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(application_id)
    }

    async fn revalidate_application_steps(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        application_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let steps = sqlx::query_as::<_, RevalidationStep>(
            "SELECT s.resource_kind,s.target_id,s.target_code,s.state,a.preconditions FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 ORDER BY s.position",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_all(&mut **tx)
        .await?;
        for step in steps {
            let preconditions: Vec<TargetAbsentPrecondition> =
                serde_json::from_value(step.preconditions).map_err(|_| {
                    RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted application preconditions".into(),
                    )
                })?;
            if preconditions.len() != 1
                || preconditions[0].kind != "target_absent"
                || preconditions[0].resource_kind != step.resource_kind
                || preconditions[0].code != step.target_code
            {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "persisted application preconditions do not match the step".into(),
                ));
            }
            if !matches!(step.resource_kind.as_str(), "context" | "blueprint") {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource kind".into(),
                ));
            }
            let matches =
                super::workspace_resource_code_matches(tx, workspace_id, &step.target_code).await?;
            let valid = if step.state == "completed" {
                matches.as_slice() == [(step.resource_kind.clone(), step.target_id)]
            } else {
                matches.is_empty()
            };
            if !valid {
                return Err(RepositoryError::SolutionPackPlanStale);
            }
        }
        Ok(())
    }

    async fn apply_next_solution_pack_step(
        &self,
        application_id: Uuid,
    ) -> Result<bool, StepApplicationError> {
        let mut attempted_position = None;
        self.apply_next_solution_pack_step_inner(application_id, &mut attempted_position)
            .await
            .map_err(|error| StepApplicationError {
                position: attempted_position,
                error,
            })
    }

    async fn apply_next_solution_pack_step_inner(
        &self,
        application_id: Uuid,
        attempted_position: &mut Option<i64>,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("solution-pack application"))?;
        if state == "completed" {
            tx.commit().await?;
            return Ok(false);
        }
        if state == "invalid" {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        if state == "failed" {
            return Err(RepositoryError::SolutionPackApplicationFailed(
                "retry must revalidate the application before continuing".to_owned(),
            ));
        }
        let step = sqlx::query_as::<_, PendingApplicationStep>(
            "SELECT s.position,s.resource_kind,s.logical_key,s.target_id,s.target_code,s.target_version,a.normalized_payload FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 AND s.state='pending' ORDER BY s.position LIMIT 1",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(step) = step else {
            sqlx::query("UPDATE solution_pack_applications SET state='completed', completed_at=clock_timestamp(), updated_at=clock_timestamp(), diagnostic_code=NULL, diagnostic_message=NULL WHERE workspace_id=$1 AND id=$2")
                    .bind(workspace_id).bind(application_id).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(false);
        };
        *attempted_position = Some(step.position);
        self.ensure_target_absent(&mut tx, &step).await?;
        let (result_snapshot, events) = match step.resource_kind.as_str() {
            "context" => {
                let payload: ContextPayload =
                    serde_json::from_value(step.normalized_payload.clone()).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid persisted context payload".into(),
                        )
                    })?;
                if payload.code != step.target_code {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted context mapping does not match its payload".into(),
                    ));
                }
                let context = self
                    .create_context_in_transaction(
                        &mut tx,
                        step.target_id,
                        CreateAttributeContext {
                            code: payload.code,
                            data: payload.data,
                            parent_id: Some(payload.parent_id),
                        },
                    )
                    .await
                    .map_err(solution_pack_mutation_error)?;
                (
                    serde_json::json!({"id":context.id,"code":context.code,"parent_id":context.parent_id}),
                    vec![context_event(self, CONTEXT_CREATED_V1, &context)],
                )
            }
            "blueprint" => {
                let payload: BlueprintPayload =
                    serde_json::from_value(step.normalized_payload.clone()).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid persisted blueprint payload".into(),
                        )
                    })?;
                if payload.version != 1 || step.target_version != Some(1) {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted blueprint version is invalid".into(),
                    ));
                }
                let created = self
                    .create_blueprint_in_transaction(
                        &mut tx,
                        step.target_id,
                        CreateBlueprint {
                            definition: payload.definition,
                        },
                    )
                    .await
                    .map_err(solution_pack_mutation_error)?;
                if created.blueprint.code != step.target_code {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted blueprint mapping does not match its payload".into(),
                    ));
                }
                let mut events = vec![blueprint_event(
                    self,
                    BLUEPRINT_CREATED_V1,
                    &created.blueprint,
                )];
                let blueprint = match payload.publication.as_str() {
                    "draft" => created.blueprint,
                    "publish" => {
                        let published = self
                            .publish_blueprint_in_transaction(&mut tx, step.target_id, 1)
                            .await?;
                        events.push(blueprint_event(self, BLUEPRINT_PUBLISHED_V1, &published));
                        published
                    }
                    _ => {
                        return Err(RepositoryError::InvalidSolutionPackPlan(
                            "persisted blueprint publication is invalid".into(),
                        ));
                    }
                };
                (
                    serde_json::json!({"id":blueprint.id,"code":blueprint.code,"version":blueprint.version,"status":blueprint.status}),
                    events,
                )
            }
            _ => {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource kind".into(),
                ));
            }
        };
        sqlx::query("UPDATE solution_pack_application_steps SET state='completed', result_snapshot=$3, completed_at=clock_timestamp(), updated_at=clock_timestamp() WHERE workspace_id=$1 AND application_id=$2 AND position=$4 AND state='pending'")
            .bind(workspace_id).bind(application_id).bind(result_snapshot).bind(step.position).execute(&mut *tx).await?;
        sqlx::query("UPDATE solution_pack_applications SET updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id).bind(application_id).execute(&mut *tx).await?;
        let mut audit_repository = self.clone();
        if let Some(audit) = audit_repository.audit_context.as_mut() {
            audit.target = serde_json::json!({"type":step.resource_kind,"id":step.target_id,"application_id":application_id,"logical_key":step.logical_key});
        }
        audit_repository.write_audit_event(&mut tx).await?;
        for event in events {
            self.enqueue_event(&mut tx, event).await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    async fn ensure_target_absent(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        step: &PendingApplicationStep,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        if !matches!(step.resource_kind.as_str(), "context" | "blueprint") {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "unsupported persisted resource kind".into(),
            ));
        }
        super::lock_workspace_resource_code(tx, workspace_id, &step.target_code).await?;
        let exists = !super::workspace_resource_code_matches(tx, workspace_id, &step.target_code)
            .await?
            .is_empty();
        if exists {
            Err(RepositoryError::SolutionPackPlanStale)
        } else {
            Ok(())
        }
    }

    async fn record_solution_pack_failure(
        &self,
        application_id: Uuid,
        attempted_position: Option<i64>,
        error: &RepositoryError,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let application_state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_one(&mut *tx)
        .await?;
        if application_state == "completed" {
            tx.commit().await?;
            return Ok(true);
        }
        let step_state = match attempted_position {
            Some(position) => sqlx::query_scalar::<_, String>(
                "SELECT state FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 AND position=$3",
            )
            .bind(workspace_id)
            .bind(application_id)
            .bind(position)
            .fetch_optional(&mut *tx)
            .await?,
            None => None,
        };
        if step_state.as_deref() == Some("completed") {
            tx.commit().await?;
            return Ok(true);
        }
        if attempted_position.is_none() {
            let pending = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 AND state='pending')",
            )
            .bind(workspace_id)
            .bind(application_id)
            .fetch_one(&mut *tx)
            .await?;
            if !pending {
                tx.commit().await?;
                return Ok(true);
            }
        }
        let (state, code) = match error {
            RepositoryError::SolutionPackPlanStale => ("invalid", "plan_stale"),
            RepositoryError::SolutionPackApplicationInvalid => ("invalid", "application_invalid"),
            _ => ("failed", "step_failed"),
        };
        let message = bounded_diagnostic(&error.to_string());
        sqlx::query("UPDATE solution_pack_applications SET state=$3, diagnostic_code=$4, diagnostic_message=$5, updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND state <> 'completed'")
            .bind(workspace_id).bind(application_id).bind(state).bind(code).bind(&message).execute(&mut *tx).await?;
        if let Some(position) = attempted_position {
            sqlx::query("UPDATE solution_pack_application_steps SET state='failed', diagnostic_code=$4, diagnostic_message=$5, updated_at=clock_timestamp() WHERE workspace_id=$1 AND application_id=$2 AND position=$3 AND state='pending'")
                .bind(workspace_id).bind(application_id).bind(position).bind(code).bind(message).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(false)
    }

    pub async fn get_solution_pack_application(
        &self,
        application_id: Uuid,
    ) -> Result<Option<SolutionPackApplication>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let Some(mut application) = sqlx::query_as::<_, SolutionPackApplication>(
            "SELECT id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot,started_at,updated_at,completed_at FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id).bind(application_id).fetch_optional(&mut *tx).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        application.steps = sqlx::query_as::<_, SolutionPackApplicationStep>(
            "SELECT position,resource_kind,logical_key,target_id,target_code,target_version,state,diagnostic_code,diagnostic_message,result_snapshot,created_at,updated_at,completed_at FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 ORDER BY position",
        ).bind(workspace_id).bind(application_id).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(application))
    }

    pub async fn list_solution_pack_applications(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SolutionPackApplicationSummary>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as::<_, SolutionPackApplicationSummary>(
            "SELECT id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,started_at,updated_at,completed_at FROM solution_pack_applications WHERE workspace_id=$1 ORDER BY started_at DESC,id DESC LIMIT $2 OFFSET $3",
        ).bind(workspace_id).bind(limit).bind(offset).fetch_all(&self.pool).await?)
    }
}

fn solution_pack_mutation_error(error: RepositoryError) -> RepositoryError {
    match error {
        RepositoryError::BlueprintCodeTaken | RepositoryError::CatalogCodeTaken => {
            RepositoryError::SolutionPackPlanStale
        }
        RepositoryError::Database(sqlx::Error::Database(database_error))
            if database_error.is_unique_violation() =>
        {
            RepositoryError::SolutionPackPlanStale
        }
        error => error,
    }
}

fn bounded_diagnostic(message: &str) -> String {
    message.chars().take(1024).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    async fn ambiguous_commit_reconciliation_never_fails_a_later_pending_step(pool: sqlx::PgPool) {
        let workspace_id = CatalogRepository::DEFAULT_WORKSPACE_ID;
        let plan_id = Uuid::new_v4();
        let application_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at) VALUES ($1,$2,'local_archive','{\"side_loaded\":true}'::jsonb,$3,1,'attricat.reconcile','Reconcile','1.0.0','Reconcile','^1.0','reconcile','draft',true,clock_timestamp() + interval '24 hours')")
            .bind(plan_id)
            .bind(workspace_id)
            .bind("0".repeat(64))
            .execute(&pool)
            .await
            .unwrap();
        for position in 0_i64..2 {
            let logical_key = format!("contexts/{position}");
            let target_id = Uuid::new_v4();
            let target_code = format!("reconcile_{position}");
            sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,$3,'context',$4,$5,$6,'create','{}'::jsonb)")
                .bind(plan_id).bind(workspace_id).bind(position).bind(&logical_key).bind(target_id).bind(&target_code).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,$3,'context',$4,'create','target_absent','{}'::jsonb,'{}'::jsonb,$5)")
                .bind(plan_id).bind(workspace_id).bind(position).bind(&logical_key).bind(serde_json::json!([{"kind":"target_absent","resource_kind":"context","code":target_code}])).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.reconcile','1.0.0','draft','running','[]'::jsonb)")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(Uuid::new_v4()).bind("0".repeat(64)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,state) SELECT $1,$2,m.position,m.plan_id,m.resource_kind,m.logical_key,m.target_id,m.target_code,'pending' FROM solution_pack_plan_mappings m WHERE m.plan_id=$3 ORDER BY m.position")
            .bind(application_id).bind(workspace_id).bind(plan_id).execute(&pool).await.unwrap();

        // Model a server-side successful commit whose client observed a connection
        // error, followed by another applier completing the next step before the
        // first request can reconcile its synthetic commit error.
        let mut concurrent_apply = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM solution_pack_applications WHERE id=$1 FOR UPDATE")
            .bind(application_id)
            .fetch_one(&mut *concurrent_apply)
            .await
            .unwrap();
        sqlx::query("UPDATE solution_pack_application_steps SET state='completed',result_snapshot='{}'::jsonb,completed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE application_id=$1")
            .bind(application_id)
            .execute(&mut *concurrent_apply)
            .await
            .unwrap();
        let repository = CatalogRepository::new(pool.clone());
        let reconciliation = tokio::spawn(async move {
            repository
                .record_solution_pack_failure(
                    application_id,
                    Some(0),
                    &RepositoryError::Database(sqlx::Error::PoolClosed),
                )
                .await
        });
        tokio::task::yield_now().await;
        concurrent_apply.commit().await.unwrap();
        assert!(reconciliation.await.unwrap().unwrap());
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM solution_pack_applications WHERE id=$1"
            )
            .bind(application_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "running"
        );
        assert_eq!(
            sqlx::query_scalar::<_, Vec<String>>("SELECT array_agg(state ORDER BY position) FROM solution_pack_application_steps WHERE application_id=$1")
                .bind(application_id).fetch_one(&pool).await.unwrap(),
            vec!["completed".to_owned(), "completed".to_owned()]
        );
    }
}
