use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain_events::{BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1, CONTEXT_CREATED_V1},
    extension_policy,
    extensions::{
        ExtensionLayoutPlacement, Manifest, classify_extension_layout_placement,
        valid_contribution_key,
    },
    model::{CreateAttributeContext, CreateBlueprint},
    solution_packs::{
        BlueprintPublication, InstalledExtensionSnapshot,
        MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        PlanningExploreNavigationEntry, PlanningWorkspaceSnapshot, SOLUTION_PACK_PLAN_EXPIRY_HOURS,
        SolutionPackCheckDefinition, SolutionPackCheckPredicate, SolutionPackExtensionRequirement,
        SolutionPackPlanDraft, ValidatedSolutionPack, build_create_only_plan,
        evaluate_extension_requirement, json_deep_contains, parse_version_req,
    },
};

use super::{
    CatalogRepository, EventPublisher, ExploreNavigationEntry, RepositoryError,
    blueprints::blueprint_event, contexts::context_event,
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
    pub readme_markdown: Option<String>,
    pub release_notes_markdown: Option<String>,
    pub setup_checklist: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[sqlx(skip)]
    pub mappings: Vec<SolutionPackPlanMapping>,
    #[sqlx(skip)]
    pub actions: Vec<SolutionPackPlanAction>,
    #[sqlx(skip)]
    pub conflicts: Vec<SolutionPackPlanConflict>,
    #[sqlx(skip)]
    pub extension_requirements: Vec<SolutionPackPlanExtensionRequirement>,
    #[sqlx(skip)]
    pub checks: Vec<SolutionPackCheckSummary>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackCheckSummary {
    pub position: i64,
    pub key: String,
    pub title: String,
    pub predicate_type: String,
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

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackPlanExtensionRequirement {
    pub position: i64,
    pub logical_key: String,
    pub extension_id: String,
    pub version_requirement: String,
    pub required: bool,
    pub configuration_template_path: Option<String>,
    pub configuration_template_sha256: Option<String>,
    pub status: String,
    pub reason_code: String,
    pub installed_release_id: Option<Uuid>,
    pub installed_version: Option<String>,
    pub installed_state: Option<String>,
    pub configuration_matches: Option<bool>,
}

#[derive(sqlx::FromRow)]
struct PrivatePlanExtensionRequirement {
    logical_key: String,
    extension_id: String,
    version_requirement: String,
    required: bool,
    configuration_template_path: Option<String>,
    configuration_template_sha256: Option<String>,
    status: String,
    installed_release_id: Option<Uuid>,
    installed_version: Option<String>,
    evaluation_template: Value,
}

#[derive(sqlx::FromRow)]
struct InstalledExtensionRow {
    extension_id: String,
    installed_release_id: Uuid,
    version: String,
    state: String,
    configuration: Value,
    manifest: Value,
}

fn installed_extension_snapshot(
    installed: InstalledExtensionRow,
) -> Result<(String, InstalledExtensionSnapshot), RepositoryError> {
    // Historical/synthetic requirement-only rows may not have a decodable
    // manifest. They can still satisfy the pre-existing version/configuration
    // requirement contract, but cannot prove a contribution declaration.
    let contributions = serde_json::from_value::<Manifest>(installed.manifest)
        .ok()
        .into_iter()
        .flat_map(|manifest| manifest.ui)
        .filter_map(|contribution| {
            contribution.outlet.map(|outlet| {
                let outlet = serde_json::to_value(outlet)
                    .expect("UI outlet serialization cannot fail")
                    .as_str()
                    .expect("UI outlet serializes as a string")
                    .to_owned();
                (
                    format!("{}:{}", installed.extension_id, contribution.id),
                    outlet,
                )
            })
        })
        .collect();
    let policy_compatible =
        extension_policy::allows(&installed.extension_id, installed.installed_release_id);
    Ok((
        installed.extension_id,
        InstalledExtensionSnapshot {
            installed_release_id: installed.installed_release_id,
            version: installed.version,
            state: installed.state,
            configuration: installed.configuration,
            policy_compatible,
            contributions,
        },
    ))
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
        let settings: Value = sqlx::query_scalar(
            "SELECT settings FROM workspaces WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        let settings_object = settings.as_object();
        let navigation_value = settings_object
            .and_then(|settings| settings.get("explore_navigation"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let extension_layout = settings_object
            .and_then(|settings| settings.get("extension_layout"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"version":1,"outlets":{}}));
        let extension_layout_valid = settings_object.is_some()
            && super::extensions::validate_workspace_extension_layout(&extension_layout).is_ok();
        let (explore_navigation, explore_navigation_valid) = match settings_object.and_then(|_| {
            super::workspace_navigation::parse_stored_explore_navigation(navigation_value).ok()
        }) {
            Some(entries) => (
                entries
                    .into_iter()
                    .map(|entry| PlanningExploreNavigationEntry {
                        blueprint_code: entry.blueprint_code,
                        visible_to_role_codes: entry.visible_to_role_codes,
                    })
                    .collect(),
                true,
            ),
            None => (Vec::new(), false),
        };
        let role_codes = sqlx::query_scalar::<_, String>(
            "SELECT code FROM roles WHERE is_system OR workspace_id = $1 ORDER BY code",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let published_entity_codes = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT code FROM blueprints WHERE workspace_id = $1 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL AND code IS NOT NULL ORDER BY code",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        let extension_ids = pack
            .manifest()
            .extensions
            .iter()
            .map(|requirement| requirement.id.clone())
            .collect::<Vec<_>>();
        let installed_extensions = if extension_ids.is_empty() {
            Default::default()
        } else {
            sqlx::query_as::<_, InstalledExtensionRow>(
                "SELECT i.extension_id, i.installed_release_id, r.version, i.state, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id AND r.workspace_id = i.workspace_id WHERE i.workspace_id = $1 AND i.extension_id = ANY($2) ORDER BY i.extension_id",
            )
            .bind(workspace_id)
            .bind(&extension_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(installed_extension_snapshot)
            .collect::<Result<_, _>>()?
        };
        let draft = build_create_only_plan(
            pack,
            prefix,
            publication,
            &PlanningWorkspaceSnapshot {
                workspace_id,
                physical_codes,
                default_context_id,
                installed_extensions,
                explore_navigation,
                explore_navigation_valid,
                extension_layout,
                extension_layout_valid,
                role_codes,
                published_entity_codes,
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
            "INSERT INTO solution_pack_plans (id, workspace_id, actor_user_id, actor_token_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, created_at, expires_at, readme_markdown, release_notes_markdown, setup_checklist) VALUES ($1, $2, $3, $4, 'local_archive', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)",
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
        .bind(&pack.guidance().readme_markdown)
        .bind(&pack.guidance().release_notes_markdown)
        .bind(
            pack.guidance()
                .setup_checklist
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?,
        )
        .execute(&mut *tx)
        .await?;
        insert_plan_rows(&mut tx, workspace_id, id, &draft).await?;
        insert_plan_checks(&mut tx, workspace_id, id, pack.checks()).await?;
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
            "SELECT id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, readme_markdown, release_notes_markdown, setup_checklist, created_at, expires_at FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2",
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
        plan.extension_requirements = sqlx::query_as::<_, SolutionPackPlanExtensionRequirement>(
            "SELECT position, logical_key, extension_id, version_requirement, required, configuration_template_path, configuration_template_sha256, status, reason_code, installed_release_id, installed_version, installed_state, configuration_matches FROM solution_pack_plan_extension_requirements WHERE workspace_id = $1 AND plan_id = $2 ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?;
        plan.checks = load_plan_check_summaries(&self.pool, workspace_id, plan_id).await?;
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
    let extension_requirements = draft
        .extension_requirements
        .iter()
        .enumerate()
        .map(
            |(position, requirement)| SolutionPackPlanExtensionRequirement {
                position: position as i64,
                logical_key: requirement.logical_key.clone(),
                extension_id: requirement.extension_id.clone(),
                version_requirement: requirement.version_requirement.clone(),
                required: requirement.required,
                configuration_template_path: requirement.configuration_template_path.clone(),
                configuration_template_sha256: requirement.configuration_template_sha256.clone(),
                status: requirement.status.to_owned(),
                reason_code: requirement.reason_code.to_owned(),
                installed_release_id: requirement.installed_release_id,
                installed_version: requirement.installed_version.clone(),
                installed_state: requirement.installed_state.clone(),
                configuration_matches: requirement.configuration_matches,
            },
        )
        .collect();
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
        readme_markdown: pack.guidance().readme_markdown.clone(),
        release_notes_markdown: pack.guidance().release_notes_markdown.clone(),
        setup_checklist: pack.guidance().setup_checklist.as_ref().map(|checklist| {
            serde_json::to_value(checklist).expect("validated checklist serializes")
        }),
        created_at,
        expires_at,
        mappings,
        actions,
        conflicts,
        extension_requirements,
        checks: pack
            .checks()
            .iter()
            .enumerate()
            .map(|(position, check)| SolutionPackCheckSummary {
                position: position as i64,
                key: check.key.clone(),
                title: check.title.clone(),
                predicate_type: check.predicate.predicate_type().to_owned(),
            })
            .collect(),
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
    for (position, requirement) in draft.extension_requirements.iter().enumerate() {
        sqlx::query("INSERT INTO solution_pack_plan_extension_requirements (plan_id, workspace_id, position, logical_key, extension_id, version_requirement, required, configuration_template_path, configuration_template_sha256, status, reason_code, installed_release_id, installed_version, installed_state, configuration_matches, evaluation_template) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position as i64)
            .bind(&requirement.logical_key)
            .bind(&requirement.extension_id)
            .bind(&requirement.version_requirement)
            .bind(requirement.required)
            .bind(&requirement.configuration_template_path)
            .bind(&requirement.configuration_template_sha256)
            .bind(requirement.status)
            .bind(requirement.reason_code)
            .bind(requirement.installed_release_id)
            .bind(&requirement.installed_version)
            .bind(&requirement.installed_state)
            .bind(requirement.configuration_matches)
            .bind(&requirement.evaluation_template)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn insert_plan_checks(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
    checks: &[SolutionPackCheckDefinition],
) -> Result<(), RepositoryError> {
    for (position, check) in checks.iter().enumerate() {
        sqlx::query("INSERT INTO solution_pack_plan_check_definitions (plan_id,workspace_id,position,check_key,title,predicate) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position as i64)
            .bind(&check.key)
            .bind(&check.title)
            .bind(serde_json::to_value(&check.predicate).map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn load_plan_check_summaries(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    plan_id: Uuid,
) -> Result<Vec<SolutionPackCheckSummary>, RepositoryError> {
    let rows = sqlx::query_as::<_, (i64, String, String, Value)>(
        "SELECT position,check_key,title,predicate FROM solution_pack_plan_check_definitions WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(position, key, title, predicate)| {
            let predicate: SolutionPackCheckPredicate =
                serde_json::from_value(predicate).map_err(|_| {
                    RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted check predicate".into(),
                    )
                })?;
            Ok(SolutionPackCheckSummary {
                position,
                key,
                title,
                predicate_type: predicate.predicate_type().to_owned(),
            })
        })
        .collect()
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
    pub readme_markdown: Option<String>,
    pub release_notes_markdown: Option<String>,
    pub setup_checklist: Option<Value>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    pub steps: Vec<SolutionPackApplicationStep>,
    #[sqlx(skip)]
    pub checks: Vec<SolutionPackCheckSummary>,
    #[sqlx(skip)]
    pub latest_check_run: Option<SolutionPackCheckRunSummary>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackCheckRunSummary {
    pub id: Uuid,
    pub application_id: Uuid,
    pub request_id: Uuid,
    pub correlation_id: Uuid,
    pub trigger: String,
    pub total_count: i64,
    pub passed_count: i64,
    pub failed_count: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolutionPackCheckRun {
    #[serde(flatten)]
    pub summary: SolutionPackCheckRunSummary,
    pub results: Vec<SolutionPackCheckResult>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackCheckResult {
    pub position: i64,
    pub key: String,
    pub title: String,
    #[sqlx(rename = "predicate_type")]
    #[serde(rename = "type")]
    pub predicate_type: String,
    pub passed: bool,
    pub reason_code: String,
    pub summary: String,
    pub evidence: Value,
    pub evaluated_at: DateTime<Utc>,
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
    action: String,
    normalized_payload: Value,
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
    action: String,
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
    #[serde(default)]
    extension_contributions: Vec<ExtensionContributionSnapshotPayload>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionContributionSnapshotPayload {
    contribution: String,
    outlet: String,
    installed_release_id: Uuid,
    installed_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionLayoutPayload {
    entries: Vec<ExtensionLayoutPayloadEntry>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionLayoutPayloadEntry {
    contribution: String,
    outlet: String,
    hidden: bool,
    promoted: bool,
    installed_release_id: Uuid,
    installed_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExploreNavigationPayload {
    entries: Vec<ExploreNavigationPayloadEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExploreNavigationPayloadEntry {
    #[allow(dead_code)]
    blueprint_key: String,
    blueprint_code: String,
    visible_to_role_codes: Vec<String>,
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
        self.ensure_initial_solution_pack_check_run(application_id)
            .await?;
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
            "SELECT id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, readme_markdown, release_notes_markdown, setup_checklist, created_at, expires_at FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2 FOR UPDATE",
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
        if let Err(error) = self
            .revalidate_required_extension_requirements(&mut tx, plan_id)
            .await
        {
            if matches!(error, RepositoryError::SolutionPackPlanStale)
                && let Some((application_id, _)) = existing_application
            {
                sqlx::query("UPDATE solution_pack_applications SET state='invalid', diagnostic_code='plan_stale', diagnostic_message='persisted extension requirements no longer match the workspace', updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND state <> 'completed'")
                    .bind(workspace_id)
                    .bind(application_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
            return Err(error);
        }
        let mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            "SELECT m.position, m.resource_kind, m.logical_key, m.target_id, m.target_code, m.target_version, m.mapping_kind FROM solution_pack_plan_mappings m JOIN solution_pack_plan_actions a ON a.plan_id=m.plan_id AND a.logical_key=m.logical_key WHERE m.workspace_id = $1 AND m.plan_id = $2 AND a.action IN ('create','append','satisfied') ORDER BY m.position",
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
            "INSERT INTO solution_pack_applications (id, workspace_id, plan_id, actor_user_id, actor_token_id, request_id, correlation_id, source_kind, source_metadata, archive_sha256, pack_id, pack_version, blueprint_publication, state, mapping_snapshot, readme_markdown, release_notes_markdown, setup_checklist) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'running',$14,$15,$16,$17) ON CONFLICT (plan_id) DO NOTHING RETURNING id",
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
        .bind(&plan.readme_markdown)
        .bind(&plan.release_notes_markdown)
        .bind(&plan.setup_checklist)
        .fetch_optional(&mut *tx)
        .await?;
        let application_id = match inserted {
            Some(id) => {
                sqlx::query(
                    "INSERT INTO solution_pack_application_steps (application_id, workspace_id, position, plan_id, resource_kind, logical_key, target_id, target_code, target_version, state) SELECT $1, a.workspace_id, a.position, a.plan_id, a.resource_kind, a.logical_key, m.target_id, m.target_code, m.target_version, 'pending' FROM solution_pack_plan_actions a JOIN solution_pack_plan_mappings m ON m.plan_id = a.plan_id AND m.logical_key = a.logical_key WHERE a.workspace_id = $2 AND a.plan_id = $3 AND a.action IN ('create','append','satisfied') ORDER BY a.position",
                )
                .bind(id)
                .bind(workspace_id)
                .bind(plan_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("INSERT INTO solution_pack_application_check_definitions (application_id,workspace_id,position,check_key,title,predicate) SELECT $1,workspace_id,position,check_key,title,predicate FROM solution_pack_plan_check_definitions WHERE workspace_id=$2 AND plan_id=$3 ORDER BY position")
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

    async fn revalidate_required_extension_requirements(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let requirements = sqlx::query_as::<_, PrivatePlanExtensionRequirement>(
            "SELECT logical_key, extension_id, version_requirement, required, configuration_template_path, configuration_template_sha256, status, installed_release_id, installed_version, evaluation_template FROM solution_pack_plan_extension_requirements WHERE workspace_id=$1 AND plan_id=$2 AND required ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&mut **tx)
        .await?;
        if requirements.is_empty() {
            return Ok(());
        }
        let extension_ids = requirements
            .iter()
            .map(|requirement| requirement.extension_id.clone())
            .collect::<Vec<_>>();
        let installed_extensions = sqlx::query_as::<_, InstalledExtensionRow>(
            "SELECT i.extension_id, i.installed_release_id, r.version, i.state, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id=i.installed_release_id AND r.workspace_id=i.workspace_id WHERE i.workspace_id=$1 AND i.extension_id = ANY($2) FOR SHARE OF i, r",
        )
        .bind(workspace_id)
        .bind(extension_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(installed_extension_snapshot)
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        for persisted in requirements {
            if !persisted.required || persisted.status != "satisfied" {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "invalid persisted required extension requirement".into(),
                ));
            }
            let configuration_template = match (
                persisted.configuration_template_path,
                persisted.configuration_template_sha256,
            ) {
                (Some(path), Some(sha256)) => Some(
                    crate::solution_packs::SolutionPackConfigurationTemplateRef { path, sha256 },
                ),
                (None, None) => None,
                _ => {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted extension configuration template reference".into(),
                    ));
                }
            };
            let requirement = SolutionPackExtensionRequirement {
                key: persisted.logical_key,
                id: persisted.extension_id,
                version: persisted.version_requirement,
                required: true,
                configuration_template,
            };
            let installed = installed_extensions.get(&requirement.id);
            let evaluated = evaluate_extension_requirement(
                &requirement,
                requirement
                    .configuration_template
                    .as_ref()
                    .map(|_| &persisted.evaluation_template),
                installed,
            );
            if evaluated.status != "satisfied"
                || installed.map(|value| value.installed_release_id)
                    != persisted.installed_release_id
                || installed.map(|value| value.version.as_str())
                    != persisted.installed_version.as_deref()
            {
                return Err(RepositoryError::SolutionPackPlanStale);
            }
        }
        Ok(())
    }

    async fn revalidate_extension_contributions(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        contributions: &[ExtensionContributionSnapshotPayload],
    ) -> Result<(), RepositoryError> {
        if contributions.is_empty() {
            return Ok(());
        }
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let extension_ids = contributions
            .iter()
            .filter_map(|entry| {
                entry
                    .contribution
                    .split_once(':')
                    .map(|(id, _)| id.to_owned())
            })
            .collect::<Vec<_>>();
        if extension_ids.len() != contributions.len() {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "invalid persisted extension contribution".into(),
            ));
        }
        let installed = sqlx::query_as::<_, InstalledExtensionRow>(
            "SELECT i.extension_id, i.installed_release_id, r.version, i.state, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id=i.installed_release_id AND r.workspace_id=i.workspace_id WHERE i.workspace_id=$1 AND i.extension_id=ANY($2) FOR SHARE OF i, r",
        )
        .bind(workspace_id)
        .bind(&extension_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(installed_extension_snapshot)
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        for entry in contributions {
            let extension_id = entry
                .contribution
                .split_once(':')
                .map(|(id, _)| id)
                .expect("checked contribution key");
            let Some(snapshot) = installed.get(extension_id) else {
                return Err(RepositoryError::SolutionPackPlanStale);
            };
            if snapshot.installed_release_id != entry.installed_release_id
                || snapshot.version != entry.installed_version
                || snapshot.state == "quarantined"
                || !snapshot.policy_compatible
                || snapshot
                    .contributions
                    .get(&entry.contribution)
                    .map(String::as_str)
                    != Some(entry.outlet.as_str())
            {
                return Err(RepositoryError::SolutionPackPlanStale);
            }
        }
        Ok(())
    }

    async fn revalidate_application_steps(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        application_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let steps = sqlx::query_as::<_, RevalidationStep>(
            "SELECT s.resource_kind,s.target_id,s.target_code,s.state,a.action,a.normalized_payload,a.preconditions FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 ORDER BY s.position",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_all(&mut **tx)
        .await?;
        for step in steps {
            if step.resource_kind == "workspace_setting" {
                if step.target_id != workspace_id {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workspace setting mapping is invalid".into(),
                    ));
                }
                if step.target_code == "explore_navigation" {
                    let payload = parse_explore_navigation_payload(step.normalized_payload)?;
                    let current = self
                        .lock_explore_navigation_in_transaction(tx, workspace_id)
                        .await?;
                    if step.state == "completed" || step.action == "satisfied" {
                        self.validate_explore_navigation_entries_in_transaction(
                            tx,
                            workspace_id,
                            &payload,
                        )
                        .await
                        .map_err(solution_pack_navigation_error)?;
                    }
                    let all_exact = navigation_entries_all_exact(&current, &payload);
                    let appendable = navigation_entries_appendable(&current, &payload);
                    let valid = if step.state == "completed" || step.action == "satisfied" {
                        all_exact
                    } else {
                        step.action == "append" && appendable
                    };
                    if !valid {
                        return Err(RepositoryError::SolutionPackPlanStale);
                    }
                } else if step.target_code == "extension_layout" {
                    let payload = parse_extension_layout_payload(step.normalized_payload)?;
                    self.revalidate_extension_contributions(
                        tx,
                        &extension_layout_snapshots(&payload),
                    )
                    .await?;
                    let current = self
                        .lock_workspace_extension_layout_in_transaction(tx, workspace_id)
                        .await
                        .map_err(solution_pack_layout_error)?;
                    let all_exact = extension_layout_entries_all_exact(&current, &payload);
                    let appendable = extension_layout_entries_appendable(&current, &payload);
                    let valid = if step.state == "completed" || step.action == "satisfied" {
                        all_exact
                    } else {
                        step.action == "append" && appendable
                    };
                    if !valid {
                        return Err(RepositoryError::SolutionPackPlanStale);
                    }
                } else {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workspace setting mapping is invalid".into(),
                    ));
                }
                continue;
            }
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
        let (state, plan_id) = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT state,plan_id FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
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
            "SELECT s.position,s.resource_kind,s.logical_key,s.target_id,s.target_code,s.target_version,a.action,a.normalized_payload FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 AND s.state='pending' ORDER BY s.position LIMIT 1",
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
        // Requirements are evidence, never mutation steps. Recheck them in the
        // same transaction as every resource mutation and hold a shared lock on
        // the installation rows so extension lifecycle changes cannot race the
        // check for that step.
        self.revalidate_required_extension_requirements(&mut tx, plan_id)
            .await?;
        *attempted_position = Some(step.position);
        if step.resource_kind != "workspace_setting" {
            self.ensure_target_absent(&mut tx, &step).await?;
        }
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
                self.revalidate_extension_contributions(&mut tx, &payload.extension_contributions)
                    .await?;
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
            "workspace_setting" => {
                if step.target_id != workspace_id
                    || !matches!(step.action.as_str(), "append" | "satisfied")
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workspace setting step is invalid".into(),
                    ));
                }
                if step.target_code == "explore_navigation" {
                    let desired =
                        parse_explore_navigation_payload(step.normalized_payload.clone())?;
                    let current = self
                        .lock_explore_navigation_in_transaction(&mut tx, workspace_id)
                        .await?;
                    self.validate_explore_navigation_entries_in_transaction(
                        &mut tx,
                        workspace_id,
                        &desired,
                    )
                    .await
                    .map_err(solution_pack_navigation_error)?;
                    let all_exact = navigation_entries_all_exact(&current, &desired);
                    let outcome = if step.action == "satisfied" {
                        if !all_exact {
                            return Err(RepositoryError::SolutionPackPlanStale);
                        }
                        "satisfied"
                    } else if !navigation_entries_appendable(&current, &desired) {
                        return Err(RepositoryError::SolutionPackPlanStale);
                    } else if all_exact {
                        "satisfied"
                    } else {
                        let mut merged = current;
                        for entry in &desired {
                            if !merged
                                .iter()
                                .any(|current| current.blueprint_code == entry.blueprint_code)
                            {
                                merged.push(entry.clone());
                            }
                        }
                        self.write_explore_navigation_in_transaction(
                            &mut tx,
                            workspace_id,
                            &merged,
                        )
                        .await?;
                        "appended"
                    };
                    (
                        serde_json::json!({
                            "setting": "explore_navigation",
                            "entry_count": desired.len(),
                            "outcome": outcome,
                        }),
                        Vec::new(),
                    )
                } else if step.target_code == "extension_layout" {
                    let desired = parse_extension_layout_payload(step.normalized_payload.clone())?;
                    self.revalidate_extension_contributions(
                        &mut tx,
                        &extension_layout_snapshots(&desired),
                    )
                    .await?;
                    let current = self
                        .lock_workspace_extension_layout_in_transaction(&mut tx, workspace_id)
                        .await
                        .map_err(solution_pack_layout_error)?;
                    let before_sha256 = json_sha256(&current)?;
                    let all_exact = extension_layout_entries_all_exact(&current, &desired);
                    let (outcome, merged) = if step.action == "satisfied" {
                        if !all_exact {
                            return Err(RepositoryError::SolutionPackPlanStale);
                        }
                        ("satisfied", current)
                    } else if !extension_layout_entries_appendable(&current, &desired) {
                        return Err(RepositoryError::SolutionPackPlanStale);
                    } else if all_exact {
                        ("satisfied", current)
                    } else {
                        let merged = merge_extension_layout(current, &desired)?;
                        self.write_workspace_extension_layout_in_transaction(
                            &mut tx,
                            workspace_id,
                            &merged,
                        )
                        .await?;
                        ("appended", merged)
                    };
                    let after_sha256 = json_sha256(&merged)?;
                    (
                        serde_json::json!({
                            "setting": "extension_layout",
                            "entry_count": desired.len(),
                            "outcome": outcome,
                            "before_sha256": before_sha256,
                            "after_sha256": after_sha256,
                        }),
                        Vec::new(),
                    )
                } else {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workspace setting step is invalid".into(),
                    ));
                }
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
            "SELECT id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot,readme_markdown,release_notes_markdown,setup_checklist,started_at,updated_at,completed_at FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id).bind(application_id).fetch_optional(&mut *tx).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        application.steps = sqlx::query_as::<_, SolutionPackApplicationStep>(
            "SELECT position,resource_kind,logical_key,target_id,target_code,target_version,state,diagnostic_code,diagnostic_message,result_snapshot,created_at,updated_at,completed_at FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 ORDER BY position",
        ).bind(workspace_id).bind(application_id).fetch_all(&mut *tx).await?;
        application.checks =
            load_application_check_summaries(&mut tx, workspace_id, application_id).await?;
        application.latest_check_run =
            load_latest_check_run(&mut tx, workspace_id, application_id).await?;
        tx.commit().await?;
        Ok(Some(application))
    }

    async fn ensure_initial_solution_pack_check_run(
        &self,
        application_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM solution_pack_check_runs WHERE workspace_id=$1 AND application_id=$2 AND trigger='post_apply')")
            .bind(workspace_id)
            .bind(application_id)
            .fetch_one(&self.pool)
            .await?;
        if !exists {
            self.create_solution_pack_check_run(application_id, "post_apply")
                .await?;
        }
        Ok(())
    }

    pub async fn rerun_solution_pack_checks(
        &self,
        application_id: Uuid,
    ) -> Result<SolutionPackCheckRun, RepositoryError> {
        self.create_solution_pack_check_run(application_id, "manual")
            .await
    }

    pub async fn list_solution_pack_check_runs(
        &self,
        application_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SolutionPackCheckRunSummary>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let application_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2)")
            .bind(workspace_id).bind(application_id).fetch_one(&self.pool).await?;
        if !application_exists {
            return Err(RepositoryError::NotFound("solution-pack application"));
        }
        Ok(sqlx::query_as::<_, SolutionPackCheckRunSummary>("SELECT id,application_id,request_id,correlation_id,trigger,total_count,passed_count,failed_count,started_at,completed_at FROM solution_pack_check_runs WHERE workspace_id=$1 AND application_id=$2 ORDER BY completed_at DESC,id DESC LIMIT $3 OFFSET $4")
            .bind(workspace_id).bind(application_id).bind(limit).bind(offset).fetch_all(&self.pool).await?)
    }

    pub async fn get_solution_pack_check_run(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<SolutionPackCheckRun>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let Some(summary) = sqlx::query_as::<_, SolutionPackCheckRunSummary>("SELECT id,application_id,request_id,correlation_id,trigger,total_count,passed_count,failed_count,started_at,completed_at FROM solution_pack_check_runs WHERE workspace_id=$1 AND application_id=$2 AND id=$3")
            .bind(workspace_id).bind(application_id).bind(run_id).fetch_optional(&self.pool).await? else {
            return Ok(None);
        };
        let results = sqlx::query_as::<_, SolutionPackCheckResult>("SELECT position,check_key AS key,title,predicate_type,passed,reason_code,summary,evidence,evaluated_at FROM solution_pack_check_results WHERE workspace_id=$1 AND run_id=$2 ORDER BY position")
            .bind(workspace_id).bind(run_id).fetch_all(&self.pool).await?;
        Ok(Some(SolutionPackCheckRun { summary, results }))
    }

    async fn create_solution_pack_check_run(
        &self,
        application_id: Uuid,
        trigger: &str,
    ) -> Result<SolutionPackCheckRun, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut tx = self.pool.begin().await?;
        let application = sqlx::query_as::<_, (Uuid, String)>("SELECT plan_id,state FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE")
            .bind(workspace_id).bind(application_id).fetch_optional(&mut *tx).await?
            .ok_or(RepositoryError::NotFound("solution-pack application"))?;
        if application.1 != "completed" {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        if trigger == "post_apply"
            && let Some(run_id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM solution_pack_check_runs WHERE workspace_id=$1 AND application_id=$2 AND trigger='post_apply'")
                .bind(workspace_id).bind(application_id).fetch_optional(&mut *tx).await?
        {
            tx.commit().await?;
            return self.get_solution_pack_check_run(application_id, run_id).await?
                .ok_or(RepositoryError::NotFound("solution-pack check run"));
        }
        let definitions = sqlx::query_as::<_, (i64, String, String, Value)>("SELECT position,check_key,title,predicate FROM solution_pack_application_check_definitions WHERE workspace_id=$1 AND application_id=$2 ORDER BY position")
            .bind(workspace_id).bind(application_id).fetch_all(&mut *tx).await?;
        let started_at = Utc::now();
        let mut results = Vec::with_capacity(definitions.len());
        for (position, key, title, predicate) in definitions {
            let predicate: SolutionPackCheckPredicate =
                serde_json::from_value(predicate).map_err(|_| {
                    RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted check predicate".into(),
                    )
                })?;
            let (passed, reason_code, summary, evidence) = evaluate_solution_pack_check(
                &mut tx,
                workspace_id,
                application_id,
                application.0,
                &predicate,
            )
            .await?;
            results.push(SolutionPackCheckResult {
                position,
                key,
                title,
                predicate_type: predicate.predicate_type().to_owned(),
                passed,
                reason_code,
                summary,
                evidence,
                evaluated_at: Utc::now(),
            });
        }
        let completed_at = Utc::now();
        let passed_count = results.iter().filter(|result| result.passed).count() as i64;
        let run_id = Uuid::new_v4();
        let request_id = self
            .audit_context
            .as_ref()
            .map_or_else(Uuid::new_v4, |audit| audit.request_id);
        let correlation_id = self
            .audit_context
            .as_ref()
            .map_or(request_id, |audit| audit.correlation_id);
        let actor_user_id = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id);
        let actor_token_id = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_token_id);
        let run = SolutionPackCheckRun {
            summary: SolutionPackCheckRunSummary {
                id: run_id,
                application_id,
                request_id,
                correlation_id,
                trigger: trigger.to_owned(),
                total_count: results.len() as i64,
                passed_count,
                failed_count: results.len() as i64 - passed_count,
                started_at,
                completed_at,
            },
            results,
        };
        let response_size = serde_json::to_vec(&run)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?
            .len();
        if response_size > MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "solution-pack check response exceeds the size limit".to_owned(),
            ));
        }
        sqlx::query("INSERT INTO solution_pack_check_runs (id,workspace_id,application_id,actor_user_id,actor_token_id,request_id,correlation_id,trigger,total_count,passed_count,failed_count,started_at,completed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
            .bind(run_id).bind(workspace_id).bind(application_id).bind(actor_user_id).bind(actor_token_id)
            .bind(request_id).bind(correlation_id).bind(trigger).bind(run.summary.total_count).bind(run.summary.passed_count)
            .bind(run.summary.failed_count).bind(started_at).bind(completed_at).execute(&mut *tx).await?;
        for result in &run.results {
            sqlx::query("INSERT INTO solution_pack_check_results (run_id,workspace_id,position,check_key,title,predicate_type,passed,reason_code,summary,evidence,evaluated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                .bind(run_id).bind(workspace_id).bind(result.position).bind(&result.key).bind(&result.title)
                .bind(&result.predicate_type).bind(result.passed).bind(&result.reason_code).bind(&result.summary)
                .bind(&result.evidence).bind(result.evaluated_at).execute(&mut *tx).await?;
        }
        let mut audit_repository = self.clone();
        if let Some(audit) = audit_repository.audit_context.as_mut() {
            audit.target = serde_json::json!({"type":"solution_pack_check_run","id":run_id,"application_id":application_id,"trigger":trigger});
        }
        audit_repository.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(run)
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

async fn load_application_check_summaries(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
) -> Result<Vec<SolutionPackCheckSummary>, RepositoryError> {
    let rows = sqlx::query_as::<_, (i64, String, String, Value)>("SELECT position,check_key,title,predicate FROM solution_pack_application_check_definitions WHERE workspace_id=$1 AND application_id=$2 ORDER BY position")
        .bind(workspace_id).bind(application_id).fetch_all(&mut **tx).await?;
    rows.into_iter()
        .map(|(position, key, title, value)| {
            let predicate: SolutionPackCheckPredicate =
                serde_json::from_value(value).map_err(|_| {
                    RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted check predicate".into(),
                    )
                })?;
            Ok(SolutionPackCheckSummary {
                position,
                key,
                title,
                predicate_type: predicate.predicate_type().to_owned(),
            })
        })
        .collect()
}

async fn load_latest_check_run(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
) -> Result<Option<SolutionPackCheckRunSummary>, RepositoryError> {
    Ok(sqlx::query_as::<_, SolutionPackCheckRunSummary>("SELECT id,application_id,request_id,correlation_id,trigger,total_count,passed_count,failed_count,started_at,completed_at FROM solution_pack_check_runs WHERE workspace_id=$1 AND application_id=$2 ORDER BY completed_at DESC,id DESC LIMIT 1")
        .bind(workspace_id).bind(application_id).fetch_optional(&mut **tx).await?)
}

async fn evaluate_solution_pack_check(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
    plan_id: Uuid,
    predicate: &SolutionPackCheckPredicate,
) -> Result<(bool, String, String, Value), RepositoryError> {
    match predicate {
        SolutionPackCheckPredicate::BlueprintPublished { blueprint } => {
            let target_code =
                check_blueprint_target(tx, workspace_id, application_id, blueprint).await?;
            let Some(target_code) = target_code else {
                return Ok(unresolvable());
            };
            let published: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM blueprints WHERE workspace_id=$1 AND code=$2 AND status='published' AND deleted_at IS NULL)")
                .bind(workspace_id).bind(&target_code).fetch_one(&mut **tx).await?;
            Ok(check_outcome(
                published,
                "published",
                "not_published",
                serde_json::json!({"blueprint_code":target_code}),
            ))
        }
        SolutionPackCheckPredicate::ExploreNavigationEntryPresent { blueprint } => {
            let target_code =
                check_blueprint_target(tx, workspace_id, application_id, blueprint).await?;
            let Some(target_code) = target_code else {
                return Ok(unresolvable());
            };
            let settings: Value = sqlx::query_scalar(
                "SELECT settings FROM workspaces WHERE id=$1 AND deleted_at IS NULL",
            )
            .bind(workspace_id)
            .fetch_one(&mut **tx)
            .await?;
            let navigation = settings
                .get("explore_navigation")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            let entries =
                match super::workspace_navigation::parse_stored_explore_navigation(navigation) {
                    Ok(entries) => entries,
                    Err(()) => {
                        return Ok(check_outcome(
                            false,
                            "present",
                            "invalid_workspace_navigation",
                            serde_json::json!({"blueprint_code":target_code}),
                        ));
                    }
                };
            let published: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM blueprints WHERE workspace_id=$1 AND code=$2 AND kind='entity' AND status='published' AND deleted_at IS NULL)")
                .bind(workspace_id)
                .bind(&target_code)
                .fetch_one(&mut **tx)
                .await?;
            let present = published
                && entries
                    .iter()
                    .any(|entry| entry.blueprint_code == target_code);
            Ok(check_outcome(
                present,
                "present",
                if published {
                    "missing"
                } else {
                    "not_published"
                },
                serde_json::json!({"blueprint_code":target_code}),
            ))
        }
        SolutionPackCheckPredicate::ExtensionInstalled { extension }
        | SolutionPackCheckPredicate::ExtensionEnabled { extension }
        | SolutionPackCheckPredicate::ExtensionConfigurationMatches { extension } => {
            let requirement = sqlx::query_as::<_, (String, String, Value)>("SELECT extension_id,version_requirement,evaluation_template FROM solution_pack_plan_extension_requirements WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3")
                .bind(workspace_id).bind(plan_id).bind(extension).fetch_optional(&mut **tx).await?;
            let Some((extension_id, version_requirement, template)) = requirement else {
                return Ok(unresolvable());
            };
            let installed = sqlx::query_as::<_, (Uuid, String, String, Value, bool)>("SELECT i.installed_release_id,r.version,i.state,i.configuration,w.extensions_enabled FROM extension_installations i JOIN installed_extension_releases r ON r.id=i.installed_release_id AND r.workspace_id=i.workspace_id JOIN workspaces w ON w.id=i.workspace_id WHERE i.workspace_id=$1 AND i.extension_id=$2")
                .bind(workspace_id).bind(&extension_id).fetch_optional(&mut **tx).await?;
            let Some((release_id, version, state, configuration, workspace_enabled)) = installed
            else {
                return Ok(check_outcome(
                    false,
                    "satisfied",
                    "not_installed",
                    serde_json::json!({"extension_id":extension_id}),
                ));
            };
            let version_matches = parse_version_req(&version_requirement)
                .ok()
                .zip(semver::Version::parse(&version).ok())
                .is_some_and(|(requirement, version)| requirement.matches(&version));
            let installed_ok = version_matches && state != "quarantined";
            let evidence =
                serde_json::json!({"extension_id":extension_id,"version":version,"state":state});
            match predicate {
                SolutionPackCheckPredicate::ExtensionInstalled { .. } => Ok(check_outcome(
                    installed_ok,
                    "installed",
                    if !version_matches {
                        "incompatible_version"
                    } else if state == "quarantined" {
                        "quarantined"
                    } else {
                        "not_installed"
                    },
                    evidence,
                )),
                SolutionPackCheckPredicate::ExtensionEnabled { .. } => {
                    let (enabled, failure_reason) = extension_enabled_evaluation(
                        installed_ok,
                        &state,
                        workspace_enabled,
                        extension_policy::allows(&extension_id, release_id),
                    );
                    Ok(check_outcome(enabled, "enabled", failure_reason, evidence))
                }
                SolutionPackCheckPredicate::ExtensionConfigurationMatches { .. } => {
                    let matches = installed_ok && json_deep_contains(&configuration, &template);
                    Ok(check_outcome(
                        matches,
                        "configuration_matches",
                        if !installed_ok {
                            "not_installed"
                        } else {
                            "configuration_mismatch"
                        },
                        evidence,
                    ))
                }
                _ => unreachable!(),
            }
        }
        SolutionPackCheckPredicate::WorkspaceExtensionLayoutPlacementPresent { contribution } => {
            let payload: Option<Value> = sqlx::query_scalar("SELECT normalized_payload FROM solution_pack_plan_actions WHERE workspace_id=$1 AND plan_id=$2 AND logical_key='workspace/extension-layout'")
                .bind(workspace_id).bind(plan_id).fetch_optional(&mut **tx).await?;
            let Some(payload) = payload else {
                return Ok(unresolvable());
            };
            let entries = parse_extension_layout_payload(payload)?;
            let Some(desired) = entries
                .into_iter()
                .find(|entry| entry.contribution == *contribution)
            else {
                return Ok(unresolvable());
            };
            let settings: Value = sqlx::query_scalar(
                "SELECT settings FROM workspaces WHERE id=$1 AND deleted_at IS NULL",
            )
            .bind(workspace_id)
            .fetch_one(&mut **tx)
            .await?;
            let layout = settings
                .get("extension_layout")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"version":1,"outlets":{}}));
            let placement = classify_extension_layout_placement(
                &layout,
                contribution,
                &desired.outlet,
                desired.hidden,
                desired.promoted,
            );
            let passed = placement == ExtensionLayoutPlacement::Exact;
            Ok(check_outcome(
                passed,
                "present",
                "placement_mismatch",
                serde_json::json!({"contribution":contribution,"outlet":desired.outlet,"hidden":desired.hidden,"promoted":desired.promoted}),
            ))
        }
    }
}

async fn check_blueprint_target(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
    logical_key: &str,
) -> Result<Option<String>, RepositoryError> {
    Ok(sqlx::query_scalar("SELECT target_code FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 AND logical_key=$3 AND state='completed'")
        .bind(workspace_id).bind(application_id).bind(logical_key).fetch_optional(&mut **tx).await?)
}

fn extension_enabled_evaluation(
    installed_ok: bool,
    state: &str,
    workspace_enabled: bool,
    policy_compatible: bool,
) -> (bool, &'static str) {
    if !installed_ok {
        (false, "not_installed")
    } else if state != "enabled" {
        (false, "disabled")
    } else if !workspace_enabled {
        (false, "workspace_extensions_disabled")
    } else if !policy_compatible {
        (false, "policy_incompatible")
    } else {
        (true, "enabled")
    }
}

fn check_outcome(
    passed: bool,
    pass_reason: &str,
    fail_reason: &str,
    evidence: Value,
) -> (bool, String, String, Value) {
    let reason = if passed { pass_reason } else { fail_reason };
    (
        passed,
        reason.to_owned(),
        if passed {
            "Check passed."
        } else {
            "Check did not pass."
        }
        .to_owned(),
        evidence,
    )
}

fn unresolvable() -> (bool, String, String, Value) {
    (
        false,
        "not_resolvable".into(),
        "The pack reference cannot be resolved for this application.".into(),
        serde_json::json!({}),
    )
}

fn parse_extension_layout_payload(
    value: Value,
) -> Result<Vec<ExtensionLayoutPayloadEntry>, RepositoryError> {
    let payload: ExtensionLayoutPayload = serde_json::from_value(value).map_err(|_| {
        RepositoryError::InvalidSolutionPackPlan(
            "invalid persisted extension layout payload".into(),
        )
    })?;
    if payload.entries.is_empty()
        || payload.entries.len() > crate::solution_packs::MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES
    {
        return Err(RepositoryError::InvalidSolutionPackPlan(
            "persisted extension layout entry count is invalid".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for entry in &payload.entries {
        if !valid_contribution_key(&entry.contribution)
            || !seen.insert(entry.contribution.as_str())
            || serde_json::from_value::<crate::extensions::UiOutlet>(Value::String(
                entry.outlet.clone(),
            ))
            .is_err()
            || (entry.promoted && (entry.outlet != "navigation" || entry.hidden))
        {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "persisted extension layout entry is invalid".into(),
            ));
        }
    }
    Ok(payload.entries)
}

fn extension_layout_snapshots(
    entries: &[ExtensionLayoutPayloadEntry],
) -> Vec<ExtensionContributionSnapshotPayload> {
    entries
        .iter()
        .map(|entry| ExtensionContributionSnapshotPayload {
            contribution: entry.contribution.clone(),
            outlet: entry.outlet.clone(),
            installed_release_id: entry.installed_release_id,
            installed_version: entry.installed_version.clone(),
        })
        .collect()
}

fn extension_layout_entries_all_exact(
    current: &Value,
    desired: &[ExtensionLayoutPayloadEntry],
) -> bool {
    desired.iter().all(|entry| {
        classify_extension_layout_placement(
            current,
            &entry.contribution,
            &entry.outlet,
            entry.hidden,
            entry.promoted,
        ) == ExtensionLayoutPlacement::Exact
    })
}

fn extension_layout_entries_appendable(
    current: &Value,
    desired: &[ExtensionLayoutPayloadEntry],
) -> bool {
    desired.iter().all(|entry| {
        matches!(
            classify_extension_layout_placement(
                current,
                &entry.contribution,
                &entry.outlet,
                entry.hidden,
                entry.promoted,
            ),
            ExtensionLayoutPlacement::Exact | ExtensionLayoutPlacement::Absent
        )
    })
}

fn merge_extension_layout(
    mut current: Value,
    desired: &[ExtensionLayoutPayloadEntry],
) -> Result<Value, RepositoryError> {
    for entry in desired {
        if classify_extension_layout_placement(
            &current,
            &entry.contribution,
            &entry.outlet,
            entry.hidden,
            entry.promoted,
        ) == ExtensionLayoutPlacement::Exact
        {
            continue;
        }
        let outlets = current
            .get_mut("outlets")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                RepositoryError::InvalidSolutionPackPlan(
                    "persisted workspace extension layout is invalid".into(),
                )
            })?;
        let outlet = outlets.entry(entry.outlet.clone()).or_insert_with(|| {
            if entry.outlet == "navigation" {
                serde_json::json!({"order": [], "hidden": [], "promoted": []})
            } else {
                serde_json::json!({"order": [], "hidden": []})
            }
        });
        let list = if entry.hidden { "hidden" } else { "order" };
        outlet[list]
            .as_array_mut()
            .expect("validated outlet list")
            .push(Value::String(entry.contribution.clone()));
        if entry.promoted {
            outlet["promoted"]
                .as_array_mut()
                .expect("navigation promotion list")
                .push(Value::String(entry.contribution.clone()));
        }
    }
    super::extensions::validate_workspace_extension_layout(&current)?;
    Ok(current)
}

fn json_sha256(value: &Value) -> Result<String, RepositoryError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn parse_explore_navigation_payload(
    value: Value,
) -> Result<Vec<ExploreNavigationEntry>, RepositoryError> {
    let payload: ExploreNavigationPayload = serde_json::from_value(value).map_err(|_| {
        RepositoryError::InvalidSolutionPackPlan(
            "invalid persisted Explore navigation payload".into(),
        )
    })?;
    if payload.entries.is_empty()
        || payload.entries.len()
            > crate::solution_packs::MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES
    {
        return Err(RepositoryError::InvalidSolutionPackPlan(
            "persisted Explore navigation entry count is invalid".into(),
        ));
    }
    Ok(payload
        .entries
        .into_iter()
        .map(|entry| ExploreNavigationEntry {
            blueprint_code: entry.blueprint_code,
            visible_to_role_codes: entry.visible_to_role_codes,
        })
        .collect())
}

fn canonical_navigation_entry(entry: &ExploreNavigationEntry) -> ExploreNavigationEntry {
    let mut entry = entry.clone();
    entry.visible_to_role_codes.sort();
    entry
}

fn navigation_entries_all_exact(
    current: &[ExploreNavigationEntry],
    desired: &[ExploreNavigationEntry],
) -> bool {
    desired.iter().all(|desired| {
        let desired = canonical_navigation_entry(desired);
        let mut matches = current
            .iter()
            .filter(|current| current.blueprint_code == desired.blueprint_code);
        matches
            .next()
            .is_some_and(|current| canonical_navigation_entry(current) == desired)
            && matches.next().is_none()
    })
}

fn navigation_entries_appendable(
    current: &[ExploreNavigationEntry],
    desired: &[ExploreNavigationEntry],
) -> bool {
    desired.iter().all(|desired| {
        let mut matches = current
            .iter()
            .filter(|current| current.blueprint_code == desired.blueprint_code);
        match matches.next() {
            None => true,
            Some(current) => {
                canonical_navigation_entry(current) == canonical_navigation_entry(desired)
                    && matches.next().is_none()
            }
        }
    })
}

fn solution_pack_navigation_error(error: RepositoryError) -> RepositoryError {
    match error {
        RepositoryError::Database(_) => error,
        _ => RepositoryError::SolutionPackPlanStale,
    }
}

fn solution_pack_layout_error(error: RepositoryError) -> RepositoryError {
    match error {
        RepositoryError::Database(_) => error,
        _ => RepositoryError::SolutionPackPlanStale,
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

    #[test]
    fn extension_enabled_check_fails_closed_for_host_policy() {
        assert_eq!(
            extension_enabled_evaluation(true, "enabled", true, false),
            (false, "policy_incompatible")
        );
        assert_eq!(
            extension_enabled_evaluation(true, "enabled", true, true),
            (true, "enabled")
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn failed_navigation_step_revalidates_and_resumes(pool: sqlx::PgPool) {
        let workspace_id = CatalogRepository::DEFAULT_WORKSPACE_ID;
        let repository = CatalogRepository::new(pool.clone());
        let blueprint_id = Uuid::new_v4();
        let mut blueprint_tx = pool.begin().await.unwrap();
        repository
            .create_blueprint_in_transaction(
                &mut blueprint_tx,
                blueprint_id,
                CreateBlueprint {
                    definition: r#"
format_version = 1
code = "retry_product"
name = "Retry product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
                    .to_owned(),
                },
            )
            .await
            .unwrap();
        repository
            .publish_blueprint_in_transaction(&mut blueprint_tx, blueprint_id, 1)
            .await
            .unwrap();
        blueprint_tx.commit().await.unwrap();

        let plan_id = Uuid::new_v4();
        let application_id = Uuid::new_v4();
        let logical_key = "workspace/explore-navigation";
        sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at) VALUES ($1,$2,'local_archive','{\"side_loaded\":true}'::jsonb,$3,1,'attricat.retry-navigation','Retry navigation','1.0.0','Retry navigation','^1.0','retry','publish',true,clock_timestamp() + interval '24 hours')")
            .bind(plan_id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,0,'workspace_setting',$3,$2,'explore_navigation','workspace','{\"setting\":\"explore_navigation\"}'::jsonb)")
            .bind(plan_id).bind(workspace_id).bind(logical_key).execute(&pool).await.unwrap();
        let payload = serde_json::json!({"entries":[{
            "blueprint_key":"blueprints/product",
            "blueprint_code":"retry_product",
            "visible_to_role_codes":[]
        }]});
        sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,0,'workspace_setting',$3,'append','target_absent','{\"setting\":\"explore_navigation\"}'::jsonb,$4,'[]'::jsonb)")
            .bind(plan_id).bind(workspace_id).bind(logical_key).bind(&payload).execute(&pool).await.unwrap();
        let request_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.retry-navigation','1.0.0','publish','failed','step_failed','synthetic transient failure','[]'::jsonb)")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(request_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,state,diagnostic_code,diagnostic_message) VALUES ($1,$2,0,$3,'workspace_setting',$4,$2,'explore_navigation','failed','step_failed','synthetic transient failure')")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(logical_key).execute(&pool).await.unwrap();

        let application = repository.apply_solution_pack_plan(plan_id).await.unwrap();
        assert_eq!(application.state, "completed");
        assert_eq!(application.steps[0].state, "completed");
        assert_eq!(
            application.steps[0].result_snapshot.as_ref().unwrap()["outcome"],
            "appended"
        );
        let navigation: Value =
            sqlx::query_scalar("SELECT settings->'explore_navigation' FROM workspaces WHERE id=$1")
                .bind(workspace_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(navigation[0]["blueprint_code"], "retry_product");
    }

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
