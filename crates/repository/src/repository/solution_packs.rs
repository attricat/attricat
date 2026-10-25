use attricat_domain::states::{
    RevisionStatus, SolutionPackApplicationState, SolutionPackStepState,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain_events::{BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1},
    extension_policy,
    extensions::{
        ExtensionLayoutPlacement, Manifest, classify_extension_layout_placement,
        valid_contribution_key,
    },
    model::{
        CreateAttributeContext, CreateBlueprint, CreateRule, CreateWorkflow, NewAttributeValue,
    },
    solution_pack_extensions::ResolvedExtensionRelease,
    solution_pack_sample_data::{SampleRecord, explicit_fact_attribute_codes},
    solution_pack_seeds::{
        ContextMappingRequest, ExistingContextSnapshot, ExistingPublicationChannel,
        PrerequisiteResolution, SeedWorkspaceSnapshot, prerequisite_version_req, reused_blueprints,
        validate_context_mapping_requests,
    },
    solution_packs::{
        BlueprintMappingRequest, BlueprintPublication, ExistingBlueprintSnapshot,
        ExistingPresentationAssetSnapshot, InstalledExtensionSnapshot,
        MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES, MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES,
        MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES, MappingKind, PlanActionKind, PlanResourceKind,
        PlannedAction, PlannedMapping, PlanningExploreNavigationEntry, PlanningWorkspaceSnapshot,
        PresentationAssetMappingRequest, RequirementStatus, SOLUTION_PACK_PLAN_EXPIRY_HOURS,
        SolutionPackCheckDefinition, SolutionPackCheckPredicate, SolutionPackExtensionRequirement,
        SolutionPackPlanDraft, ValidatedSolutionPack, build_solution_pack_plan,
        evaluate_extension_requirement, json_deep_contains, parse_version_req, physical_code,
        validate_blueprint_mapping_requests, validate_presentation_asset_mapping_requests,
    },
    storage::{ObjectStore, ObjectStoreError, StoredObject, get_object_for_integrity},
};

use super::{
    AttricatRepository, EventPublisher, ExploreNavigationEntry, ExtensionState, RepositoryError,
    SAMPLE_SYSTEM_TAG, blueprints::blueprint_event, extensions::empty_workspace_extension_layout,
    record_commands::ChosenIdRecordCreate, stored_code,
};

/// Length of a hex-encoded SHA-256 digest.
const SHA256_HEX_LEN: usize = 64;
/// Largest presentation asset a persisted plan or step may reference.
const MAX_PRESENTATION_ASSET_BYTES: i64 = MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES as i64;
/// How long a sample application stays resumable after it starts.
const SAMPLE_APPLICATION_RESUMABLE_DAYS: i64 = 30;
/// How long sample files being staged stay pending before upload cleanup.
const SAMPLE_FILE_STAGING_HOURS: i64 = 1;
/// Extra time staged sample files are kept after the plan could last be applied.
const SAMPLE_FILE_CLEANUP_MARGIN_HOURS: i64 = 1;
/// Most staged asset objects one reconciliation pass examines.
const MAX_ASSET_CLEANUP_OBJECTS_PER_PASS: usize = 64;
/// Expired plans or applications cleaned up per sample-staging batch.
const SAMPLE_STAGING_CLEANUP_BATCH: usize = 64;
/// Longest step failure diagnostic stored on an application, in characters.
const MAX_STEP_DIAGNOSTIC_CHARS: usize = 1024;

pub struct CreateSolutionPackPlanRequest<'a> {
    pub prefix: &'a str,
    pub publication: BlueprintPublication,
    pub blueprint_mappings: &'a [BlueprintMappingRequest],
    pub asset_mappings: &'a [PresentationAssetMappingRequest],
    /// Explicit existing contexts selected for pack contexts.
    pub context_mappings: &'a [ContextMappingRequest],
    pub include_sample_data: bool,
    /// Official releases of missing extensions, in dependency order, that the
    /// plan may install. See [`crate::solution_pack_extensions`].
    pub official_extensions: &'a [ResolvedExtensionRelease],
}

/// A pinned official release a reviewed plan installs before its steps run.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct PlannedExtensionInstall {
    pub logical_key: String,
    pub extension_id: String,
    pub version: String,
    pub installed_release_id: Uuid,
    pub repository: String,
    pub release_id: i64,
    pub tag_name: String,
    pub asset_id: i64,
    pub asset_name: String,
    pub download_url: String,
    pub archive_sha256: String,
    pub manifest_sha256: String,
    pub configuration: Value,
}

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
    #[sqlx(try_from = "String")]
    pub blueprint_publication: BlueprintPublication,
    pub ready: bool,
    pub sample_data_selected: bool,
    pub sample_declaration_sha256: Option<String>,
    pub sample_record_count: i32,
    pub sample_automation_warning: Option<String>,
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
    #[sqlx(try_from = "String")]
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    pub target_id: Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    #[sqlx(try_from = "String")]
    pub mapping_kind: MappingKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Value>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackPlanAction {
    pub position: i64,
    #[sqlx(try_from = "String")]
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    #[sqlx(try_from = "String")]
    pub action: PlanActionKind,
    pub reason_code: String,
    pub summary: Value,
    pub preconditions: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistingAssetEvidence {
    kind: Option<String>,
    id: Uuid,
    purpose: String,
    media_type: String,
    byte_size: i64,
    sha256: String,
    source_sha256: String,
}

#[derive(sqlx::FromRow)]
struct PersistedSolutionPackPlanAction {
    #[sqlx(try_from = "String")]
    resource_kind: PlanResourceKind,
    logical_key: String,
    #[sqlx(try_from = "String")]
    action: PlanActionKind,
    reason_code: String,
    normalized_payload: Option<Value>,
    preconditions: Value,
}

#[derive(Serialize, sqlx::FromRow)]
struct PlanEvidenceMapping {
    position: i64,
    #[sqlx(try_from = "String")]
    resource_kind: PlanResourceKind,
    logical_key: String,
    target_id: Uuid,
    target_code: String,
    target_version: Option<i64>,
    #[sqlx(try_from = "String")]
    mapping_kind: MappingKind,
    snapshot: Value,
}

#[derive(Serialize, sqlx::FromRow)]
struct PlanEvidenceAction {
    position: i64,
    #[sqlx(try_from = "String")]
    resource_kind: PlanResourceKind,
    logical_key: String,
    #[sqlx(try_from = "String")]
    action: PlanActionKind,
    reason_code: String,
    summary: Value,
    normalized_payload: Option<Value>,
    preconditions: Value,
}

#[derive(Serialize, sqlx::FromRow)]
struct PlanEvidenceAssetObject {
    logical_key: String,
    target_id: Uuid,
    object_key: String,
    media_type: String,
    byte_size: i64,
    sha256: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct PlanEvidenceExtensionInstall {
    position: i64,
    #[sqlx(flatten)]
    install: PlannedExtensionInstall,
    required_grants: Value,
}

#[derive(Serialize, sqlx::FromRow)]
struct PlanEvidenceExtensionRequirement {
    position: i64,
    logical_key: String,
    extension_id: String,
    version_requirement: String,
    required: bool,
    configuration_template_path: Option<String>,
    configuration_template_sha256: Option<String>,
    #[sqlx(try_from = "String")]
    status: RequirementStatus,
    reason_code: String,
    installed_release_id: Option<Uuid>,
    installed_version: Option<String>,
    installed_state: Option<String>,
    configuration_matches: Option<bool>,
    evaluation_template: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolutionPackPlanConflict {
    pub position: i64,
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    pub action: PlanActionKind,
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
    #[sqlx(try_from = "String")]
    pub status: RequirementStatus,
    pub reason_code: String,
    pub installed_release_id: Option<Uuid>,
    pub installed_version: Option<String>,
    pub installed_state: Option<String>,
    pub configuration_matches: Option<bool>,
    /// For an `install` requirement: the pinned official release and the
    /// permissions apply grants to it.
    pub install: Option<Value>,
}

#[derive(sqlx::FromRow)]
struct PrivatePlanExtensionRequirement {
    logical_key: String,
    extension_id: String,
    version_requirement: String,
    required: bool,
    configuration_template_path: Option<String>,
    configuration_template_sha256: Option<String>,
    #[sqlx(try_from = "String")]
    status: RequirementStatus,
    installed_release_id: Option<Uuid>,
    installed_version: Option<String>,
    evaluation_template: Value,
}

#[derive(sqlx::FromRow)]
struct InstalledExtensionRow {
    extension_id: String,
    installed_release_id: Uuid,
    version: String,
    #[sqlx(try_from = "String")]
    state: ExtensionState,
    configuration: Value,
    manifest: Value,
}

/// The `solution_pack_plans` columns a plan row is read from.
const PLAN_COLUMNS: &str = "id, workspace_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, sample_data_selected, sample_declaration_sha256, sample_record_count, sample_automation_warning, readme_markdown, release_notes_markdown, setup_checklist, created_at, expires_at";
/// A plan mapping's columns before its snapshot, which readers select or mask.
const PLAN_MAPPING_COLUMNS: &str =
    "position,resource_kind,logical_key,target_id,target_code,target_version,mapping_kind";

/// Whether a stored object has exactly `size` bytes with SHA-256 `sha256`.
fn object_matches(stored: &StoredObject, size: usize, sha256: &str) -> bool {
    stored.bytes.len() == size && format!("{:x}", Sha256::digest(&stored.bytes)) == sha256
}

/// Reads a staged or existing object and checks it against its recorded
/// length and SHA-256. A missing or altered object fails integrity; an
/// unreachable store is unavailable.
async fn verify_stored_object(
    object_store: &dyn ObjectStore,
    key: &str,
    byte_size: i64,
    sha256: &str,
) -> Result<(), RepositoryError> {
    let size = usize::try_from(byte_size)
        .map_err(|_| RepositoryError::SolutionPackAssetObjectIntegrityFailed)?;
    let stored = get_object_for_integrity(object_store, key, size)
        .await
        .map_err(|error| match error {
            ObjectStoreError::Unavailable
            | ObjectStoreError::TimedOut(_)
            | ObjectStoreError::Operation(_) => {
                RepositoryError::SolutionPackAssetStorageUnavailable
            }
            ObjectStoreError::NotFound => RepositoryError::SolutionPackAssetObjectIntegrityFailed,
        })?;
    if object_matches(&stored, size, sha256) {
        Ok(())
    } else {
        Err(RepositoryError::SolutionPackAssetObjectIntegrityFailed)
    }
}

fn installed_extension_snapshot(
    installed: InstalledExtensionRow,
) -> (String, InstalledExtensionSnapshot) {
    // Historical/synthetic requirement-only rows may not have a decodable
    // manifest. They can still satisfy the pre-existing version/configuration
    // requirement contract, but cannot prove a contribution declaration.
    let contributions = serde_json::from_value::<Manifest>(installed.manifest)
        .map(|manifest| manifest_contributions(&installed.extension_id, &manifest))
        .unwrap_or_default();
    let policy_compatible =
        extension_policy::allows(&installed.extension_id, installed.installed_release_id);
    (
        installed.extension_id,
        InstalledExtensionSnapshot {
            installed_release_id: installed.installed_release_id,
            version: installed.version,
            state: installed.state,
            configuration: installed.configuration,
            policy_compatible,
            contributions,
            pending_install: false,
        },
    )
}

/// Planning view of an official release the plan would install.
fn pending_extension_snapshot(release: &ResolvedExtensionRelease) -> InstalledExtensionSnapshot {
    InstalledExtensionSnapshot {
        installed_release_id: release.installed_release_id,
        version: release.manifest.version.clone(),
        state: ExtensionState::Disabled,
        configuration: release.configuration.clone(),
        policy_compatible: extension_policy::allows(
            &release.extension_id,
            release.installed_release_id,
        ),
        contributions: manifest_contributions(&release.extension_id, &release.manifest),
        pending_install: true,
    }
}

/// Stable contribution keys mapped to their manifest-declared outlets.
fn manifest_contributions(
    extension_id: &str,
    manifest: &Manifest,
) -> std::collections::BTreeMap<String, String> {
    manifest
        .ui
        .iter()
        .filter_map(|contribution| {
            contribution.outlet.as_ref().map(|outlet| {
                let outlet = serde_json::to_value(outlet)
                    .expect("UI outlet serialization cannot fail")
                    .as_str()
                    .expect("UI outlet serializes as a string")
                    .to_owned();
                (format!("{extension_id}:{}", contribution.id), outlet)
            })
        })
        .collect()
}

/// Pack contexts a sample record sets values in.
fn sample_record_contexts(record: &SampleRecord) -> std::collections::BTreeSet<&str> {
    record
        .facts
        .iter()
        .filter_map(|fact| fact.context.as_deref())
        .chain(
            record
                .relationships
                .iter()
                .filter_map(|relationship| relationship.context.as_deref()),
        )
        .chain(
            record
                .files
                .iter()
                .filter_map(|value| value.context.as_deref()),
        )
        .collect()
}

fn canonical_sample_input(pack: &ValidatedSolutionPack, record: &SampleRecord) -> (Value, String) {
    let explicit_attributes = explicit_fact_attribute_codes(record);
    let effective_defaults = pack
        .blueprint(&record.blueprint)
        .expect("validated blueprint exists")
        .effective_attributes()
        .iter()
        .filter_map(|attribute| {
            attribute
                .default_value
                .as_ref()
                .filter(|_| !explicit_attributes.contains(attribute.code.as_str()))
                .map(|value| serde_json::json!({"attribute":attribute.code,"value":value}))
        })
        .collect::<Vec<_>>();
    let canonical_input =
        serde_json::json!({"record":record,"effective_defaults":effective_defaults});
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical_input).expect("sample record serializes"))
    );
    (canonical_input, digest)
}

/// What a plan validates against beyond its mapped resources: the codes in
/// use, Explore navigation, extension layout, roles and published record
/// blueprints. A stored setting that does not parse is reported as invalid
/// rather than failing the plan.
struct WorkspacePlanningState {
    physical_codes: std::collections::BTreeSet<String>,
    explore_navigation: Vec<PlanningExploreNavigationEntry>,
    explore_navigation_valid: bool,
    extension_layout: Value,
    extension_layout_valid: bool,
    role_codes: std::collections::BTreeSet<String>,
    published_record_codes: std::collections::BTreeSet<String>,
}

async fn workspace_planning_state(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
) -> Result<WorkspacePlanningState, RepositoryError> {
    let physical_codes = sqlx::query_scalar::<_, String>(
        "SELECT code FROM blueprints WHERE workspace_id = $1 AND code IS NOT NULL UNION SELECT code FROM attribute_contexts WHERE workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    let settings: Value =
        sqlx::query_scalar("SELECT settings FROM workspaces WHERE id = $1 AND deleted_at IS NULL")
            .bind(workspace_id)
            .fetch_one(&mut **tx)
            .await?;
    let settings_object = settings.as_object();
    let navigation_value = settings_object
        .and_then(|settings| settings.get("explore_navigation"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let extension_layout = settings_object
        .and_then(|settings| settings.get("extension_layout"))
        .cloned()
        .unwrap_or_else(empty_workspace_extension_layout);
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
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    let published_record_codes = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT code FROM blueprints WHERE workspace_id = $1 AND kind = 'record' AND status = 'published' AND deleted_at IS NULL AND code IS NOT NULL ORDER BY code",
    )
    .bind(workspace_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    Ok(WorkspacePlanningState {
        physical_codes,
        explore_navigation,
        explore_navigation_valid,
        extension_layout,
        extension_layout_valid,
        role_codes,
        published_record_codes,
    })
}

/// The existing presentation assets a plan maps pack assets to, each
/// checked against its stored object.
async fn existing_presentation_assets(
    tx: &mut Transaction<'_, Postgres>,
    object_store: &dyn ObjectStore,
    workspace_id: Uuid,
    requested: &[PresentationAssetMappingRequest],
) -> Result<std::collections::BTreeMap<String, ExistingPresentationAssetSnapshot>, RepositoryError>
{
    let mut existing_presentation_assets = std::collections::BTreeMap::new();
    for requested in requested {
        let existing = sqlx::query_as::<_, (Uuid, String, String, i64, String, String)>(
            "SELECT id,purpose,media_type,byte_size,sha256,object_key FROM presentation_assets WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id)
        .bind(requested.id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            RepositoryError::InvalidSolutionPackPlan(format!(
                "existing presentation asset '{}' was not found",
                requested.id
            ))
        })?;
        let expected_size = usize::try_from(existing.3).map_err(|_| {
            RepositoryError::InvalidSolutionPackPlan(
                "existing presentation asset size is invalid".into(),
            )
        })?;
        let object = get_object_for_integrity(object_store, &existing.5, expected_size)
            .await
            .map_err(|error| match error {
                ObjectStoreError::Unavailable
                | ObjectStoreError::TimedOut(_)
                | ObjectStoreError::Operation(_) => {
                    RepositoryError::SolutionPackAssetStorageUnavailable
                }
                ObjectStoreError::NotFound => RepositoryError::InvalidSolutionPackPlan(format!(
                    "existing presentation asset '{}' object is missing",
                    requested.id
                )),
            })?;
        if !usize::try_from(existing.3).is_ok_and(|size| object_matches(&object, size, &existing.4))
        {
            return Err(RepositoryError::InvalidSolutionPackPlan(format!(
                "existing presentation asset '{}' object integrity failed",
                requested.id
            )));
        }
        existing_presentation_assets.insert(
            requested.key.clone(),
            ExistingPresentationAssetSnapshot {
                id: existing.0,
                purpose: existing.1,
                media_type: existing.2,
                byte_size: existing.3,
                sha256: existing.4,
            },
        );
    }
    Ok(existing_presentation_assets)
}

/// The installed extensions a pack requires, plus the official releases the
/// plan would install for the missing ones, and those releases.
async fn planned_extension_snapshots<'r>(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    pack: &ValidatedSolutionPack,
    official_extensions: &'r [ResolvedExtensionRelease],
) -> Result<
    (
        std::collections::BTreeMap<String, InstalledExtensionSnapshot>,
        Vec<&'r ResolvedExtensionRelease>,
    ),
    RepositoryError,
> {
    let extension_ids = pack
        .manifest()
        .extensions
        .iter()
        .map(|requirement| requirement.id.clone())
        .collect::<Vec<_>>();
    let mut installed_extensions: std::collections::BTreeMap<_, _> = if extension_ids.is_empty() {
        Default::default()
    } else {
        sqlx::query_as::<_, InstalledExtensionRow>(
            "SELECT i.extension_id, i.installed_release_id, r.version, i.state, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id AND r.workspace_id = i.workspace_id WHERE i.workspace_id = $1 AND i.extension_id = ANY($2) ORDER BY i.extension_id",
        )
        .bind(workspace_id)
        .bind(&extension_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(installed_extension_snapshot)
        .collect()
    };
    let official_extensions = official_extensions
        .iter()
        .filter(|release| !installed_extensions.contains_key(&release.extension_id))
        .collect::<Vec<_>>();
    for release in &official_extensions {
        installed_extensions.insert(
            release.extension_id.clone(),
            pending_extension_snapshot(release),
        );
    }
    Ok((installed_extensions, official_extensions))
}

/// The latest published revision of a blueprint code, with the canonical
/// definition hash used to compare it with a pack definition.
async fn latest_published_blueprint(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    code: &str,
) -> Result<Option<ExistingBlueprintSnapshot>, RepositoryError> {
    let Some(existing) = sqlx::query_as::<_, (Uuid, String, i64, String, String, String)>(
        "SELECT id,code,version,kind,definition_hash,definition FROM blueprints WHERE workspace_id=$1 AND code=$2 AND status='published' AND deleted_at IS NULL ORDER BY version DESC LIMIT 1",
    )
    .bind(workspace_id)
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let compiled = crate::blueprint_resolver::compile_definition(tx, workspace_id, &existing.5)
        .await
        .map_err(|error| {
            RepositoryError::InvalidSolutionPackPlan(format!(
                "existing blueprint '{code}' is not valid: {error}"
            ))
        })?;
    if compiled.code != existing.1
        || compiled.kind.as_str() != existing.3
        || compiled.raw_definition_hash != existing.4
    {
        return Err(RepositoryError::InvalidSolutionPackPlan(format!(
            "existing blueprint '{code}' does not match its stored definition evidence"
        )));
    }
    let canonical_definition = toml::from_str::<toml::Value>(&existing.5)
        .ok()
        .and_then(|value| toml::to_string(&value).ok())
        .ok_or_else(|| {
            RepositoryError::InvalidSolutionPackPlan(format!(
                "existing blueprint '{code}' cannot be canonicalized"
            ))
        })?;
    Ok(Some(ExistingBlueprintSnapshot {
        id: existing.0,
        code: existing.1,
        version: existing.2,
        kind: existing.3,
        canonical_definition_hash: attricat_blueprint::raw_hash(&canonical_definition),
        definition_hash: existing.4,
    }))
}

async fn existing_publication_channel(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    context_id: Uuid,
) -> Result<Option<ExistingPublicationChannel>, RepositoryError> {
    Ok(sqlx::query_as::<_, (bool, Vec<String>, bool)>(
        "SELECT enabled,required_rule_codes,require_valid_record FROM publication_channels WHERE workspace_id=$1 AND context_id=$2",
    )
    .bind(workspace_id)
    .bind(context_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(
        |(enabled, required_rule_codes, require_valid_record)| ExistingPublicationChannel {
            enabled,
            required_rule_codes,
            require_valid_record,
        },
    ))
}

/// A workspace context and its publication channel, if any.
#[derive(sqlx::FromRow)]
struct ExistingContextRow {
    id: Uuid,
    code: String,
    channel_enabled: Option<bool>,
    required_rule_codes: Option<Vec<String>>,
    require_valid_record: Option<bool>,
}

impl ExistingContextRow {
    fn into_snapshot(self) -> ExistingContextSnapshot {
        let publication_channel = match (
            self.channel_enabled,
            self.required_rule_codes,
            self.require_valid_record,
        ) {
            (Some(enabled), Some(required_rule_codes), Some(require_valid_record)) => {
                Some(ExistingPublicationChannel {
                    enabled,
                    required_rule_codes,
                    require_valid_record,
                })
            }
            _ => None,
        };
        ExistingContextSnapshot {
            id: self.id,
            code: self.code,
            publication_channel,
        }
    }
}

/// Workspace contexts selected by id or by code, with their channels.
async fn existing_contexts(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    ids: &[Uuid],
    codes: &[&str],
) -> Result<Vec<ExistingContextSnapshot>, RepositoryError> {
    Ok(sqlx::query_as::<_, ExistingContextRow>(
        "SELECT c.id,c.code,p.enabled AS channel_enabled,p.required_rule_codes,p.require_valid_record FROM attribute_contexts c LEFT JOIN publication_channels p ON p.workspace_id=c.workspace_id AND p.context_id=c.id WHERE c.workspace_id=$1 AND (c.id=ANY($2) OR c.code=ANY($3))",
    )
    .bind(workspace_id)
    .bind(ids)
    .bind(codes)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(ExistingContextRow::into_snapshot)
    .collect())
}

/// Marks staged sample-file uploads of the given plans for immediate cleanup.
/// Files already attached to a sample record no longer have an intent.
async fn expire_sample_file_staging(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_ids: &[Uuid],
) -> Result<(), RepositoryError> {
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=clock_timestamp() WHERE workspace_id=$1 AND state='pending' AND object_key IN (SELECT object_key FROM solution_pack_plan_sample_files WHERE workspace_id=$1 AND plan_id=ANY($2))")
        .bind(workspace_id)
        .bind(plan_ids)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Sample record rows of a plan, inserted with their evidence at once.
#[derive(Default)]
struct SampleRecordRows {
    positions: Vec<i32>,
    logical_keys: Vec<String>,
    target_ids: Vec<Uuid>,
    blueprint_logical_keys: Vec<String>,
    blueprint_ids: Vec<Uuid>,
    blueprint_versions: Vec<i64>,
    digests: Vec<String>,
    scalar_counts: Vec<i32>,
    relationship_target_counts: Vec<i32>,
    canonical_inputs: Vec<Value>,
}

impl SampleRecordRows {
    async fn insert(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        plan_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if self.positions.is_empty() {
            return Ok(());
        }
        sqlx::query(
            r#"WITH rows AS (
                   SELECT * FROM unnest($3::int[],$4::text[],$5::uuid[],$6::text[],$7::uuid[],$8::bigint[],$9::text[],$10::int[],$11::int[],$12::jsonb[])
                       WITH ORDINALITY AS r(position,logical_key,target_id,blueprint_logical_key,blueprint_id,blueprint_version,digest,scalar_count,relationship_target_count,canonical_input,ordinal)
               ), records AS (
                   INSERT INTO solution_pack_plan_sample_records (plan_id,workspace_id,position,logical_key,target_id,blueprint_logical_key,blueprint_id,blueprint_version,canonical_declaration_sha256,scalar_count,relationship_target_count,canonical_input)
                   SELECT $1,$2,position,logical_key,target_id,blueprint_logical_key,blueprint_id,blueprint_version,digest,scalar_count,relationship_target_count,canonical_input FROM rows ORDER BY ordinal
               )
               INSERT INTO solution_pack_plan_sample_evidence (plan_id,workspace_id,position,logical_key,target_id,blueprint_logical_key,blueprint_id,blueprint_version,canonical_declaration_sha256,scalar_count,relationship_target_count)
               SELECT $1,$2,position,logical_key,target_id,blueprint_logical_key,blueprint_id,blueprint_version,digest,scalar_count,relationship_target_count FROM rows ORDER BY ordinal"#,
        )
        .bind(plan_id)
        .bind(workspace_id)
        .bind(&self.positions)
        .bind(&self.logical_keys)
        .bind(&self.target_ids)
        .bind(&self.blueprint_logical_keys)
        .bind(&self.blueprint_ids)
        .bind(&self.blueprint_versions)
        .bind(&self.digests)
        .bind(&self.scalar_counts)
        .bind(&self.relationship_target_counts)
        .bind(&self.canonical_inputs)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

/// A bundled sample file uploaded under an upload intent while planning.
struct StagedSampleFile {
    path: String,
    file_id: Uuid,
    object_key: String,
    filename: String,
    media_type: String,
    byte_size: i64,
    sha256: String,
}

impl AttricatRepository {
    /// Collects workspace facts for seed resources and resolves prerequisite
    /// seeds. A reused blueprint is offered to the planner exactly like an
    /// explicit mapping, so only an exact definition match is reused.
    async fn seed_workspace_snapshot(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        pack: &ValidatedSolutionPack,
        prefix: &str,
        context_mappings: &[ContextMappingRequest],
        existing_blueprints: &mut std::collections::BTreeMap<String, ExistingBlueprintSnapshot>,
    ) -> Result<SeedWorkspaceSnapshot, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut seed = SeedWorkspaceSnapshot::default();
        let requested_codes = context_mappings
            .iter()
            .map(|requested| requested.code.as_str())
            .collect::<Vec<_>>();
        let mut found = existing_contexts(tx, workspace_id, &[], &requested_codes)
            .await?
            .into_iter()
            .map(|context| (context.code.clone(), context))
            .collect::<std::collections::HashMap<_, _>>();
        for requested in context_mappings {
            let context = found.remove(&requested.code).ok_or_else(|| {
                RepositoryError::InvalidSolutionPackPlan(format!(
                    "existing context '{}' was not found",
                    requested.code
                ))
            })?;
            seed.existing_contexts
                .insert(requested.key.clone(), context);
        }
        // A code the planner would reject cannot name an existing resource.
        let codes = |resources: &[crate::solution_packs::SolutionPackResource]| {
            resources
                .iter()
                .filter_map(|resource| physical_code(prefix, &resource.key).ok())
                .collect::<Vec<_>>()
        };
        seed.rule_codes = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT code FROM rules WHERE workspace_id=$1 AND code=ANY($2)",
        )
        .bind(workspace_id)
        .bind(codes(&pack.manifest().resources.rules))
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect();
        seed.workflow_codes = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT code FROM workflows WHERE workspace_id=$1 AND code=ANY($2)",
        )
        .bind(workspace_id)
        .bind(codes(&pack.manifest().resources.workflows))
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect();

        for prerequisite in &pack.manifest().prerequisites {
            let requirement = prerequisite_version_req(prerequisite);
            let candidates = sqlx::query_as::<_, (Uuid, String)>(
                "SELECT id,pack_version FROM solution_pack_applications WHERE workspace_id=$1 AND pack_id=$2 AND state='completed' ORDER BY completed_at DESC,id",
            )
            .bind(workspace_id)
            .bind(&prerequisite.id)
            .fetch_all(&mut **tx)
            .await?;
            let selected = candidates
                .iter()
                .filter_map(|(id, version)| {
                    semver::Version::parse(version)
                        .ok()
                        .filter(|parsed| requirement.matches(parsed))
                        .map(|parsed| (parsed, *id, version.clone()))
                })
                .max_by(|left, right| left.0.cmp_precedence(&right.0));
            let resolution = match selected {
                Some((_, application_id, pack_version)) => PrerequisiteResolution::Satisfied {
                    application_id,
                    pack_version,
                },
                None if candidates.is_empty() => PrerequisiteResolution::Missing,
                None => {
                    let mut versions = candidates
                        .into_iter()
                        .map(|(_, version)| version)
                        .collect::<Vec<_>>();
                    versions.sort();
                    versions.dedup();
                    PrerequisiteResolution::Incompatible { versions }
                }
            };
            if let PrerequisiteResolution::Satisfied { application_id, .. } = &resolution {
                for (resource, reuse) in reused_blueprints(pack)
                    .filter(|(_, reuse)| reuse.prerequisite == prerequisite.key)
                {
                    // The prerequisite's own record of the blueprint it created
                    // or reused under that logical key.
                    let installed = sqlx::query_as::<_, (Uuid, String)>(
                        "SELECT target_id,target_code FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 AND resource_kind='blueprint' AND logical_key=$3 AND state='completed'",
                    )
                    .bind(workspace_id)
                    .bind(application_id)
                    .bind(&reuse.blueprint)
                    .fetch_optional(&mut **tx)
                    .await?;
                    let Some((target_id, target_code)) = installed else {
                        continue;
                    };
                    if let Some(existing) =
                        latest_published_blueprint(tx, workspace_id, &target_code).await?
                        && existing.id == target_id
                    {
                        existing_blueprints.insert(resource.key.clone(), existing);
                    }
                }
            }
            seed.prerequisites
                .insert(prerequisite.key.clone(), resolution);
        }
        Ok(seed)
    }

    /// Uploads every bundled sample file of the pack, verifying each object
    /// by reading it back. Every object key is first recorded as an ordinary
    /// upload intent with a short deadline; the plan transaction extends the
    /// deadline, so an unused upload is removed by ordinary cleanup.
    async fn stage_solution_pack_sample_files(
        &self,
        pack: &ValidatedSolutionPack,
        object_store: &dyn ObjectStore,
    ) -> Result<Vec<StagedSampleFile>, RepositoryError> {
        let Some(sample) = pack.sample_data() else {
            return Ok(Vec::new());
        };
        if sample.files.is_empty() {
            return Ok(Vec::new());
        }
        let staged = sample
            .files
            .iter()
            .map(|(path, file)| {
                let bytes = pack.file(path).expect("validated sample file exists");
                StagedSampleFile {
                    path: path.clone(),
                    file_id: Uuid::new_v4(),
                    object_key: format!("files/{}", Uuid::new_v4()),
                    filename: file.filename.clone(),
                    media_type: file.media_type.clone(),
                    byte_size: bytes.len() as i64,
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                }
            })
            .collect::<Vec<_>>();
        let keys = staged
            .iter()
            .map(|file| file.object_key.clone())
            .collect::<Vec<_>>();
        self.begin_file_uploads(
            &keys,
            Utc::now() + Duration::hours(SAMPLE_FILE_STAGING_HOURS),
        )
        .await?;
        for file in &staged {
            let bytes = pack.file(&file.path).expect("validated sample file exists");
            let stored = object_store
                .put(
                    &file.object_key,
                    StoredObject {
                        bytes: bytes::Bytes::copy_from_slice(bytes),
                        content_type: Some(file.media_type.clone()),
                    },
                )
                .await
                .is_ok();
            let verified = stored
                && get_object_for_integrity(object_store, &file.object_key, bytes.len())
                    .await
                    .is_ok_and(|stored| object_matches(&stored, bytes.len(), &file.sha256));
            if !verified {
                sqlx::query("UPDATE file_upload_intents SET cleanup_after=clock_timestamp() WHERE workspace_id=$1 AND object_key=ANY($2) AND state='pending'")
                    .bind(self.workspace_id.0)
                    .bind(&keys)
                    .execute(&self.pool)
                    .await?;
                return Err(RepositoryError::SolutionPackAssetStorageUnavailable);
            }
        }
        Ok(staged)
    }

    async fn reconcile_solution_pack_asset_cleanup(
        &self,
        object_store: &dyn ObjectStore,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let reconciliation_started_at = Utc::now();
        for _ in 0..MAX_ASSET_CLEANUP_OBJECTS_PER_PASS {
            let mut tx = self.pool.begin().await?;
            // Lock the plan and staging row in the same order as application preparation.
            // A plan with any durable application is never cleanup-eligible: applications
            // intentionally remain resumable after plan expiry.
            let row = sqlx::query_as::<_, (Uuid, String, String)>(
                "SELECT o.plan_id,o.logical_key,o.object_key FROM solution_pack_plan_asset_objects o JOIN solution_pack_plans p ON p.id=o.plan_id AND p.workspace_id=o.workspace_id WHERE o.workspace_id=$1 AND o.updated_at < $2 AND o.state IN ('uploading','staged','cleanup_pending') AND (o.state='cleanup_pending' OR p.expires_at <= now()) AND NOT EXISTS (SELECT 1 FROM solution_pack_applications a WHERE a.workspace_id=o.workspace_id AND a.plan_id=o.plan_id) ORDER BY o.updated_at FOR UPDATE OF p,o SKIP LOCKED LIMIT 1",
            )
            .bind(workspace_id)
            .bind(reconciliation_started_at)
            .fetch_optional(&mut *tx)
            .await?;
            let Some((plan_id, logical_key, object_key)) = row else {
                tx.commit().await?;
                break;
            };
            // Recheck in a new READ COMMITTED statement after acquiring the plan lock.
            // This closes the case where application insertion committed while this
            // cleanup SELECT was waiting on the same plan row.
            let has_application: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM solution_pack_applications WHERE workspace_id=$1 AND plan_id=$2)",
            )
            .bind(workspace_id)
            .bind(plan_id)
            .fetch_one(&mut *tx)
            .await?;
            if has_application {
                tx.commit().await?;
                continue;
            }
            let claimed: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM presentation_assets WHERE workspace_id=$1 AND object_key=$2)",
            )
            .bind(workspace_id)
            .bind(&object_key)
            .fetch_one(&mut *tx)
            .await?;
            if claimed {
                sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='claimed',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3 AND state <> 'claimed'")
                    .bind(workspace_id)
                    .bind(plan_id)
                    .bind(&logical_key)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                continue;
            }
            let cleanup_claim = sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='cleanup_pending',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3 AND state <> 'claimed'")
                .bind(workspace_id)
                .bind(plan_id)
                .bind(&logical_key)
                .execute(&mut *tx)
                .await?;
            if cleanup_claim.rows_affected() != 1 {
                tx.rollback().await?;
                continue;
            }
            tx.commit().await?;
            // Deletion occurs only after the cleanup claim commits. Application creation
            // cannot race this point because it must lock the plan and rejects this state.
            if object_store.delete(&object_key).await.is_ok() {
                sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='cleaned',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3 AND state='cleanup_pending'")
                    .bind(workspace_id)
                    .bind(plan_id)
                    .bind(&logical_key)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn cleanup_solution_pack_sample_staging(&self) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        loop {
            let mut tx = self.pool.begin().await?;
            set_solution_pack_transaction_timeouts(&mut tx).await?;
            // Preparation and cleanup lock the same plan row. Cleanup skips a plan
            // while preparation owns it, then rechecks application absence while
            // holding the lock, so expiry cannot strand a started application.
            let expired_plan_ids = sqlx::query_scalar::<_, Uuid>(&format!(
                "SELECT p.id FROM solution_pack_plans p WHERE p.workspace_id=$1 AND p.expires_at <= now() AND EXISTS (SELECT 1 FROM solution_pack_plan_sample_records s WHERE s.workspace_id=p.workspace_id AND s.plan_id=p.id) AND NOT EXISTS (SELECT 1 FROM solution_pack_applications a WHERE a.workspace_id=p.workspace_id AND a.plan_id=p.id) ORDER BY p.expires_at,p.id FOR UPDATE OF p SKIP LOCKED LIMIT {SAMPLE_STAGING_CLEANUP_BATCH}",
            ))
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await?;
            if expired_plan_ids.is_empty() {
                tx.commit().await?;
                break;
            }
            sqlx::query("DELETE FROM solution_pack_plan_sample_records s WHERE s.workspace_id=$1 AND s.plan_id=ANY($2) AND NOT EXISTS (SELECT 1 FROM solution_pack_applications a WHERE a.workspace_id=s.workspace_id AND a.plan_id=s.plan_id)")
                .bind(workspace_id).bind(&expired_plan_ids).execute(&mut *tx).await?;
            expire_sample_file_staging(&mut tx, workspace_id, &expired_plan_ids).await?;
            tx.commit().await?;
            if expired_plan_ids.len() < SAMPLE_STAGING_CLEANUP_BATCH {
                break;
            }
        }

        // Each application is independently locked so an active step either commits
        // before abandonment or observes the terminal state before beginning.
        loop {
            let expired = sqlx::query_scalar::<_, Uuid>(&format!(
                "SELECT id FROM solution_pack_applications WHERE workspace_id=$1 AND resumable_until <= now() AND state IN ('running','failed') ORDER BY resumable_until LIMIT {SAMPLE_STAGING_CLEANUP_BATCH}",
            )).bind(workspace_id).fetch_all(&self.pool).await?;
            if expired.is_empty() {
                break;
            }
            for application_id in expired {
                self.abandon_solution_pack_application_inner(application_id, true)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn abandon_solution_pack_application(
        &self,
        application_id: Uuid,
    ) -> Result<SolutionPackApplication, RepositoryError> {
        self.abandon_solution_pack_application_inner(application_id, false)
            .await?;
        self.get_solution_pack_application(application_id)
            .await?
            .ok_or(RepositoryError::NotFound("solution-pack application"))
    }

    async fn abandon_solution_pack_application_inner(
        &self,
        application_id: Uuid,
        require_expired: bool,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        set_solution_pack_transaction_timeouts(&mut tx).await?;
        let application = sqlx::query_as::<_, (String, Uuid, Option<DateTime<Utc>>)>(
            "SELECT state,plan_id,resumable_until FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("solution-pack application"))?;
        let state: SolutionPackApplicationState = stored_code(application.0.clone())?;
        if state == SolutionPackApplicationState::Abandoned {
            tx.commit().await?;
            return Ok(false);
        }
        if matches!(
            state,
            SolutionPackApplicationState::Completed | SolutionPackApplicationState::Invalid
        ) {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        let resumable_until = application.2.ok_or_else(|| {
            RepositoryError::InvalidSolutionPackPlan(
                "only sample-selected applications can be abandoned".into(),
            )
        })?;
        if require_expired {
            let deadline_elapsed: bool = sqlx::query_scalar("SELECT $1 <= clock_timestamp()")
                .bind(resumable_until)
                .fetch_one(&mut *tx)
                .await?;
            if !deadline_elapsed {
                tx.commit().await?;
                return Ok(false);
            }
        }

        // Reset ordinary transient failures to an unexecuted state. A sample target
        // without its atomically committed step result is never adopted.
        sqlx::query("UPDATE solution_pack_application_steps SET state='pending',diagnostic_code=NULL,diagnostic_message=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND application_id=$2 AND state='failed'")
            .bind(workspace_id).bind(application_id).execute(&mut *tx).await?;
        let pending_samples = sqlx::query_as::<_, (i64, Uuid)>(
            "SELECT position,target_id FROM solution_pack_application_steps WHERE workspace_id=$1 AND application_id=$2 AND resource_kind='sample_record' AND state='pending' ORDER BY position FOR UPDATE",
        ).bind(workspace_id).bind(application_id).fetch_all(&mut *tx).await?;
        for (position, target_id) in pending_samples {
            let unexpected_target: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(workspace_id)
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            if unexpected_target {
                sqlx::query("UPDATE solution_pack_application_steps SET state='failed',diagnostic_code='permanent_conflict',diagnostic_message='preallocated sample record exists without a committed step result',updated_at=clock_timestamp() WHERE workspace_id=$1 AND application_id=$2 AND position=$3")
                    .bind(workspace_id).bind(application_id).bind(position).execute(&mut *tx).await?;
            }
        }
        sqlx::query(
            "DELETE FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2",
        )
        .bind(workspace_id)
        .bind(application.1)
        .execute(&mut *tx)
        .await?;
        expire_sample_file_staging(&mut tx, workspace_id, &[application.1]).await?;
        let (diagnostic_code, diagnostic_message) = if require_expired {
            (
                "resumability_expired",
                "sample application resumability deadline elapsed",
            )
        } else {
            (
                "abandoned_by_administrator",
                "sample application was abandoned by an administrator",
            )
        };
        sqlx::query("UPDATE solution_pack_applications SET state='abandoned',abandoned_at=clock_timestamp(),updated_at=clock_timestamp(),diagnostic_code=$3,diagnostic_message=$4 WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id).bind(application_id).bind(diagnostic_code).bind(diagnostic_message).execute(&mut *tx).await?;
        let mut audit_repository = self.clone();
        if let Some(audit) = audit_repository.audit_context.as_mut() {
            audit.target = serde_json::json!({"type":"solution_pack_application","id":application_id,"plan_id":application.1});
        }
        audit_repository.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Uploads a plan's created presentation assets after its transaction
    /// commits, reading each back before trusting it. Any failure cleans up
    /// every upload; once all are verified the assets are staged and the plan
    /// takes its `ready` state and final evidence digest.
    async fn stage_plan_assets(
        &self,
        object_store: &dyn ObjectStore,
        pack: &ValidatedSolutionPack,
        plan_id: Uuid,
        asset_creates: &[&PlannedMapping],
        ready: bool,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        for mapping in asset_creates {
            let asset = pack
                .presentation_asset(&mapping.logical_key)
                .expect("create asset was validated");
            let key = presentation_asset_object_key(workspace_id, plan_id, mapping.target_id);
            let object = StoredObject {
                bytes: bytes::Bytes::from(asset.stored_bytes.clone()),
                content_type: Some(asset.media_type.clone()),
            };
            let preexisting = match get_object_for_integrity(
                object_store,
                &key,
                asset.stored_bytes.len(),
            )
            .await
            {
                Ok(stored) => Some(object_matches(
                    &stored,
                    asset.stored_bytes.len(),
                    &asset.stored_sha256,
                )),
                Err(
                    ObjectStoreError::Unavailable
                    | ObjectStoreError::TimedOut(_)
                    | ObjectStoreError::Operation(_),
                ) => Some(false),
                Err(ObjectStoreError::NotFound) => None,
            };
            let uploaded = match preexisting {
                Some(exact) => exact,
                None => {
                    // A successful PUT acknowledgment does not prove that the
                    // stored bytes are complete. Read back after either outcome
                    // before marking the plan asset staged.
                    let _ = object_store.put(&key, object).await;
                    get_object_for_integrity(object_store, &key, asset.stored_bytes.len())
                        .await
                        .is_ok_and(|stored| {
                            object_matches(&stored, asset.stored_bytes.len(), &asset.stored_sha256)
                        })
                }
            };
            if !uploaded {
                sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='cleanup_pending',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND state IN ('uploading','staged')")
                    .bind(workspace_id)
                    .bind(plan_id)
                    .execute(&self.pool)
                    .await?;
                for cleanup_mapping in asset_creates {
                    let cleanup_key = presentation_asset_object_key(
                        workspace_id,
                        plan_id,
                        cleanup_mapping.target_id,
                    );
                    let _ = object_store.delete(&cleanup_key).await;
                }
                return Err(RepositoryError::SolutionPackAssetStorageUnavailable);
            }
        }
        // Every upload was verified above; a failed one cleans them all up,
        // so the assets are marked staged together.
        sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='staged',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=ANY($3) AND state='uploading'")
            .bind(workspace_id)
            .bind(plan_id)
            .bind(
                asset_creates
                    .iter()
                    .map(|mapping| mapping.logical_key.as_str())
                    .collect::<Vec<_>>(),
            )
            .execute(&self.pool)
            .await?;
        let mut finalize = self.pool.begin().await?;
        sqlx::query("UPDATE solution_pack_plans SET ready=$3 WHERE workspace_id=$1 AND id=$2")
            .bind(workspace_id)
            .bind(plan_id)
            .bind(ready)
            .execute(&mut *finalize)
            .await?;
        seal_plan_resource_evidence(&mut finalize, workspace_id, plan_id).await?;
        finalize.commit().await?;
        Ok(())
    }

    pub async fn create_solution_pack_plan(
        &self,
        pack: &ValidatedSolutionPack,
        request: CreateSolutionPackPlanRequest<'_>,
        object_store: &dyn ObjectStore,
    ) -> Result<SolutionPackPlan, RepositoryError> {
        let CreateSolutionPackPlanRequest {
            prefix,
            publication,
            blueprint_mappings: requested_mappings,
            asset_mappings: requested_asset_mappings,
            context_mappings: requested_context_mappings,
            include_sample_data,
            official_extensions,
        } = request;
        if include_sample_data && pack.sample_data().is_none() {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "--include-sample-data was supplied but the pack has no sample-data resource"
                    .into(),
            ));
        }
        if include_sample_data && publication != BlueprintPublication::Publish {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "sample data requires --blueprint-publication publish".into(),
            ));
        }
        self.reconcile_solution_pack_asset_cleanup(object_store)
            .await?;
        self.cleanup_solution_pack_sample_staging().await?;
        validate_blueprint_mapping_requests(pack, requested_mappings)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
        validate_presentation_asset_mapping_requests(pack, requested_asset_mappings)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
        validate_context_mapping_requests(pack, requested_context_mappings)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
        let workspace_id = self.workspace_id.0;
        // Bundled sample files reach object storage before the plan transaction,
        // each under an ordinary upload intent, so no transaction stays open
        // during object I/O and an abandoned upload is reaped.
        let staged_sample_files = if include_sample_data {
            self.stage_solution_pack_sample_files(pack, object_store)
                .await?
        } else {
            Vec::new()
        };
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;
        let WorkspacePlanningState {
            physical_codes,
            explore_navigation,
            explore_navigation_valid,
            extension_layout,
            extension_layout_valid,
            role_codes,
            published_record_codes,
        } = workspace_planning_state(&mut tx, workspace_id).await?;
        let mut existing_blueprints = std::collections::BTreeMap::new();
        for requested in requested_mappings {
            let existing = latest_published_blueprint(&mut tx, workspace_id, &requested.code)
                .await?
                .ok_or_else(|| {
                    RepositoryError::InvalidSolutionPackPlan(format!(
                        "existing published blueprint '{}' was not found",
                        requested.code
                    ))
                })?;
            existing_blueprints.insert(requested.key.clone(), existing);
        }
        let existing_presentation_assets = existing_presentation_assets(
            &mut tx,
            object_store,
            workspace_id,
            requested_asset_mappings,
        )
        .await?;
        // An extension installed since resolution is evaluated as installed.
        let (installed_extensions, official_extensions) =
            planned_extension_snapshots(&mut tx, workspace_id, pack, official_extensions).await?;
        let seed = self
            .seed_workspace_snapshot(
                &mut tx,
                pack,
                prefix,
                requested_context_mappings,
                &mut existing_blueprints,
            )
            .await?;
        let mut draft = build_solution_pack_plan(
            pack,
            prefix,
            publication,
            &PlanningWorkspaceSnapshot {
                workspace_id,
                physical_codes,
                existing_blueprints,
                existing_presentation_assets,
                installed_extensions,
                explore_navigation,
                explore_navigation_valid,
                extension_layout,
                extension_layout_valid,
                role_codes,
                published_record_codes,
                seed,
            },
        )
        .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
        draft.ready = draft.is_applicable();

        if include_sample_data {
            let sample = pack.sample_data().expect("selected sample exists");
            for record_index in &sample.target_first_order {
                let record = &sample.declaration.records[*record_index];
                let blueprint_mapping = draft
                    .mappings
                    .iter()
                    .find(|mapping| mapping.logical_key == record.blueprint)
                    .ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(format!(
                            "sample record '{}' blueprint mapping is missing",
                            record.key
                        ))
                    })?;
                let blueprint_action = draft
                    .actions
                    .iter()
                    .find(|action| action.logical_key == record.blueprint)
                    .ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(format!(
                            "sample record '{}' blueprint action is missing",
                            record.key
                        ))
                    })?;
                let blueprint_version = blueprint_mapping.target_version.ok_or_else(|| {
                    RepositoryError::InvalidSolutionPackPlan(format!(
                        "sample record '{}' requires an exact blueprint revision",
                        record.key
                    ))
                })?;
                if !blueprint_action.action.provides_target() {
                    return Err(RepositoryError::InvalidSolutionPackPlan(format!(
                        "sample record '{}' requires a published mapped or created blueprint",
                        record.key
                    )));
                }
                for context in sample_record_contexts(record) {
                    if !draft.actions.iter().any(|action| {
                        action.resource_kind == PlanResourceKind::Context
                            && action.logical_key == context
                            && action.action.provides_target()
                    }) {
                        return Err(RepositoryError::InvalidSolutionPackPlan(format!(
                            "sample record '{}' requires context '{context}' to be created or mapped",
                            record.key
                        )));
                    }
                }
                let (_, current_digest) = canonical_sample_input(pack, record);
                draft.mappings.push(PlannedMapping {
                    resource_kind: PlanResourceKind::SampleRecord,
                    logical_key: record.key.clone(),
                    target_id: Uuid::new_v4(),
                    target_code: "sample_record".to_owned(),
                    target_version: None,
                    mapping_kind: MappingKind::Create,
                    snapshot: serde_json::json!({"blueprint_id":blueprint_mapping.target_id,"blueprint_version":blueprint_version,"canonical_declaration_sha256":current_digest}),
                });
                draft.actions.push(PlannedAction {
                    resource_kind: PlanResourceKind::SampleRecord,
                    logical_key: record.key.clone(),
                    action: PlanActionKind::Create,
                    reason_code: "sample_selected",
                    summary: serde_json::json!({"blueprint":record.blueprint,"scalar_count":record.facts.len(),"relationship_target_count":record.relationships.iter().map(|relationship| relationship.targets.len()).sum::<usize>(),"warning":crate::solution_pack_sample_data::SAMPLE_AUTOMATION_WARNING}),
                    normalized_payload: Some(serde_json::json!({"private_staging":true})),
                    preconditions: serde_json::json!([{"kind":"target_absent","resource_kind":"sample_record","code":"sample_record"}]),
                });
            }
        }
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
        let asset_creates = draft
            .mappings
            .iter()
            .filter(|mapping| {
                mapping.resource_kind == PlanResourceKind::PresentationAsset
                    && mapping.mapping_kind == MappingKind::Create
                    && draft.actions.iter().any(|action| {
                        action.logical_key == mapping.logical_key
                            && action.action == PlanActionKind::Create
                    })
            })
            .collect::<Vec<_>>();
        let initial_ready = draft.ready && asset_creates.is_empty();
        let mut plan = materialize_plan(
            (id, workspace_id),
            pack,
            prefix,
            publication,
            (created_at, expires_at),
            include_sample_data,
            &draft,
        );
        for requirement in &mut plan.extension_requirements {
            if requirement.status == RequirementStatus::Install {
                requirement.install = official_extensions
                    .iter()
                    .find(|release| release.extension_id == requirement.extension_id)
                    .map(|release| extension_install_summary(release));
            }
        }
        let response_size = serde_json::to_vec(&plan)
            .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?
            .len();
        if response_size > MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "solution-pack plan summary exceeds the size limit".to_owned(),
            ));
        }
        sqlx::query(
            "INSERT INTO solution_pack_plans (id, workspace_id, actor_user_id, actor_token_id, source_kind, source_metadata, archive_sha256, manifest_version, pack_id, pack_name, pack_version, pack_description, host_api, prefix, blueprint_publication, ready, created_at, expires_at, readme_markdown, release_notes_markdown, setup_checklist, sample_data_selected, sample_declaration_sha256, sample_record_count, sample_automation_warning) VALUES ($1, $2, $3, $4, 'local_archive', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)",
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
        .bind(&manifest.attricat.host_api)
        .bind(prefix)
        .bind(publication.as_str())
        .bind(initial_ready)
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
        .bind(include_sample_data)
        .bind(include_sample_data.then(|| pack.sample_data().expect("selected sample exists").canonical_sha256.clone()))
        .bind(if include_sample_data { pack.sample_data().expect("selected sample exists").declaration.records.len() as i32 } else { 0 })
        .bind(include_sample_data.then_some(crate::solution_pack_sample_data::SAMPLE_AUTOMATION_WARNING))
        .execute(&mut *tx)
        .await?;
        if include_sample_data {
            insert_plan_sample_staging(
                &mut tx,
                workspace_id,
                id,
                pack,
                &draft,
                &staged_sample_files,
                expires_at,
            )
            .await?;
        }
        insert_plan_rows(&mut tx, workspace_id, id, &draft).await?;
        insert_extension_installs(&mut tx, workspace_id, id, &draft, &official_extensions).await?;
        if !asset_creates.is_empty() {
            let assets: Vec<_> = asset_creates
                .iter()
                .map(|mapping| {
                    (
                        mapping,
                        pack.presentation_asset(&mapping.logical_key)
                            .expect("create asset was validated"),
                    )
                })
                .collect();
            sqlx::query("INSERT INTO solution_pack_plan_asset_objects (plan_id,workspace_id,logical_key,target_id,object_key,media_type,byte_size,sha256,state) SELECT $1,$2,a.logical_key,a.target_id,a.object_key,a.media_type,a.byte_size,a.sha256,'uploading' FROM unnest($3::text[],$4::uuid[],$5::text[],$6::text[],$7::bigint[],$8::text[]) WITH ORDINALITY AS a(logical_key,target_id,object_key,media_type,byte_size,sha256,position) ORDER BY a.position")
                .bind(id)
                .bind(workspace_id)
                .bind(assets.iter().map(|(mapping, _)| mapping.logical_key.as_str()).collect::<Vec<_>>())
                .bind(assets.iter().map(|(mapping, _)| mapping.target_id).collect::<Vec<_>>())
                .bind(
                    assets
                        .iter()
                        .map(|(mapping, _)| presentation_asset_object_key(workspace_id, id, mapping.target_id))
                        .collect::<Vec<_>>(),
                )
                .bind(assets.iter().map(|(_, asset)| asset.media_type.as_str()).collect::<Vec<_>>())
                .bind(assets.iter().map(|(_, asset)| asset.stored_bytes.len() as i64).collect::<Vec<_>>())
                .bind(assets.iter().map(|(_, asset)| asset.stored_sha256.as_str()).collect::<Vec<_>>())
                .execute(&mut *tx)
                .await?;
        }
        seal_plan_resource_evidence(&mut tx, workspace_id, id).await?;
        insert_plan_checks(&mut tx, workspace_id, id, pack.checks()).await?;
        let mut audit_repository = self.clone();
        if let Some(audit) = audit_repository.audit_context.as_mut() {
            audit.target = serde_json::json!({"type": "solution_pack_plan", "id": id});
        }
        audit_repository.write_audit_event(&mut tx).await?;
        tx.commit().await?;

        if !asset_creates.is_empty() {
            self.stage_plan_assets(object_store, pack, id, &asset_creates, draft.ready)
                .await?;
        }
        self.get_solution_pack_plan(id)
            .await?
            .ok_or(RepositoryError::NotFound("solution-pack plan"))
    }

    pub async fn get_solution_pack_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Option<SolutionPackPlan>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let Some(mut plan) = sqlx::query_as::<_, SolutionPackPlan>(&format!(
            "SELECT {PLAN_COLUMNS} FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2"
        ))
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        plan.mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            &format!("SELECT {PLAN_MAPPING_COLUMNS}, CASE WHEN mapping_kind='existing' THEN snapshot END AS snapshot FROM solution_pack_plan_mappings WHERE workspace_id = $1 AND plan_id = $2 ORDER BY position"),
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
            "SELECT r.position, r.logical_key, r.extension_id, r.version_requirement, r.required, r.configuration_template_path, r.configuration_template_sha256, r.status, r.reason_code, r.installed_release_id, r.installed_version, r.installed_state, r.configuration_matches, CASE WHEN i.plan_id IS NOT NULL THEN jsonb_build_object('version', i.version, 'repository', i.repository, 'tag_name', i.tag_name, 'grants', i.required_grants) END AS install FROM solution_pack_plan_extension_requirements r LEFT JOIN solution_pack_plan_extension_releases i ON i.plan_id = r.plan_id AND i.logical_key = r.logical_key WHERE r.workspace_id = $1 AND r.plan_id = $2 ORDER BY r.position",
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
    include_sample_data: bool,
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
            resource_kind: mapping.resource_kind,
            logical_key: mapping.logical_key.clone(),
            target_id: mapping.target_id,
            target_code: mapping.target_code.clone(),
            target_version: mapping.target_version,
            mapping_kind: mapping.mapping_kind,
            snapshot: (mapping.mapping_kind == MappingKind::Existing)
                .then(|| mapping.snapshot.clone()),
        })
        .collect();
    let actions = draft
        .actions
        .iter()
        .enumerate()
        .map(|(position, action)| SolutionPackPlanAction {
            position: position as i64,
            resource_kind: action.resource_kind,
            logical_key: action.logical_key.clone(),
            action: action.action,
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
                status: requirement.status,
                reason_code: requirement.reason_code.to_owned(),
                installed_release_id: requirement.installed_release_id,
                installed_version: requirement.installed_version.clone(),
                installed_state: requirement
                    .installed_state
                    .map(|state| state.as_str().to_owned()),
                configuration_matches: requirement.configuration_matches,
                install: None,
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
        host_api: manifest.attricat.host_api.clone(),
        prefix: prefix.to_owned(),
        blueprint_publication: publication,
        ready: draft.ready,
        sample_data_selected: include_sample_data,
        sample_declaration_sha256: include_sample_data.then(|| {
            pack.sample_data()
                .expect("selected sample exists")
                .canonical_sha256
                .clone()
        }),
        sample_record_count: if include_sample_data {
            pack.sample_data()
                .expect("selected sample exists")
                .declaration
                .records
                .len() as i32
        } else {
            0
        },
        sample_automation_warning: include_sample_data
            .then(|| crate::solution_pack_sample_data::SAMPLE_AUTOMATION_WARNING.to_owned()),
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
        .filter(|action| {
            matches!(
                action.action,
                PlanActionKind::Conflict | PlanActionKind::Blocked
            )
        })
        .map(|action| SolutionPackPlanConflict {
            position: action.position,
            resource_kind: action.resource_kind,
            logical_key: action.logical_key.clone(),
            action: action.action,
            reason_code: action.reason_code.clone(),
        })
        .collect()
}

/// Reserves a plan's sample dataset identity and records its sample records
/// and staged files. Staged files stay claimable for the plan lifetime and
/// the sample application's resumability window.
async fn insert_plan_sample_staging(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
    pack: &ValidatedSolutionPack,
    draft: &SolutionPackPlanDraft,
    staged_sample_files: &[StagedSampleFile],
    expires_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    let sample = pack.sample_data().expect("selected sample exists");
    let manifest = pack.manifest();
    let reservation = sqlx::query("INSERT INTO solution_pack_sample_dataset_reservations (workspace_id,pack_id,pack_version,archive_sha256,sample_declaration_sha256,plan_id) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(workspace_id)
        .bind(&manifest.id)
        .bind(&manifest.version)
        .bind(pack.archive_sha256())
        .bind(&sample.canonical_sha256)
        .bind(plan_id)
        .execute(&mut **tx)
        .await;
    if let Err(error) = reservation {
        if error
            .as_database_error()
            .is_some_and(|database| database.is_unique_violation())
        {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "this exact sample dataset identity is permanently reserved by its original plan; retry that plan instead".into(),
            ));
        }
        return Err(error.into());
    }
    let mut sample_rows = SampleRecordRows::default();
    for (position, record_index) in sample.target_first_order.iter().copied().enumerate() {
        let record = &sample.declaration.records[record_index];
        let mapping = draft
            .mappings
            .iter()
            .find(|mapping| mapping.logical_key == record.blueprint)
            .ok_or_else(|| {
                RepositoryError::InvalidSolutionPackPlan(format!(
                    "sample record '{}' blueprint mapping is missing",
                    record.key
                ))
            })?;
        let action = draft
            .actions
            .iter()
            .find(|action| action.logical_key == record.blueprint)
            .ok_or_else(|| {
                RepositoryError::InvalidSolutionPackPlan(format!(
                    "sample record '{}' blueprint action is missing",
                    record.key
                ))
            })?;
        if !action.action.provides_target() || mapping.target_version.is_none() {
            return Err(RepositoryError::InvalidSolutionPackPlan(format!(
                "sample record '{}' requires a published mapped or created blueprint",
                record.key
            )));
        }
        let sample_mapping = draft
            .mappings
            .iter()
            .find(|candidate| candidate.logical_key == record.key)
            .expect("selected sample mapping exists");
        let (canonical_input, record_digest) = canonical_sample_input(pack, record);
        sample_rows.positions.push(position as i32);
        sample_rows.logical_keys.push(record.key.clone());
        sample_rows.target_ids.push(sample_mapping.target_id);
        sample_rows
            .blueprint_logical_keys
            .push(record.blueprint.clone());
        sample_rows.blueprint_ids.push(mapping.target_id);
        sample_rows
            .blueprint_versions
            .push(mapping.target_version.expect("checked"));
        sample_rows.digests.push(record_digest);
        sample_rows.scalar_counts.push(record.facts.len() as i32);
        sample_rows.relationship_target_counts.push(
            record
                .relationships
                .iter()
                .map(|relationship| relationship.targets.len() as i32)
                .sum::<i32>(),
        );
        sample_rows.canonical_inputs.push(canonical_input);
    }
    sample_rows.insert(tx, workspace_id, plan_id).await?;
    if !staged_sample_files.is_empty() {
        sqlx::query("INSERT INTO solution_pack_plan_sample_files (plan_id,workspace_id,path,file_id,object_key,filename,media_type,byte_size,sha256) SELECT $1,$2,f.path,f.file_id,f.object_key,f.filename,f.media_type,f.byte_size,f.sha256 FROM unnest($3::text[],$4::uuid[],$5::text[],$6::text[],$7::text[],$8::bigint[],$9::text[]) WITH ORDINALITY AS f(path,file_id,object_key,filename,media_type,byte_size,sha256,position) ORDER BY f.position")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(staged_sample_files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.file_id).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.object_key.as_str()).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.filename.as_str()).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.media_type.as_str()).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.byte_size).collect::<Vec<_>>())
            .bind(staged_sample_files.iter().map(|file| file.sha256.as_str()).collect::<Vec<_>>())
            .execute(&mut **tx)
            .await?;
    }
    // Staged files stay claimable for the plan lifetime and the
    // sample application's resumability window.
    sqlx::query("UPDATE file_upload_intents SET cleanup_after=$3 WHERE workspace_id=$1 AND object_key=ANY($2) AND state='pending'")
        .bind(workspace_id)
        .bind(staged_sample_files.iter().map(|file| file.object_key.clone()).collect::<Vec<_>>())
        .bind(
            expires_at
                + Duration::days(SAMPLE_APPLICATION_RESUMABLE_DAYS)
                + Duration::hours(SAMPLE_FILE_CLEANUP_MARGIN_HOURS),
        )
        .execute(&mut **tx)
        .await?;
    Ok(())
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
            .bind(mapping.resource_kind.as_str())
            .bind(&mapping.logical_key)
            .bind(mapping.target_id)
            .bind(&mapping.target_code)
            .bind(mapping.target_version)
            .bind(mapping.mapping_kind.as_str())
            .bind(&mapping.snapshot)
            .execute(&mut **tx)
            .await?;
    }
    for (position, action) in draft.actions.iter().enumerate() {
        sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id, workspace_id, position, resource_kind, logical_key, action, reason_code, summary, normalized_payload, preconditions) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position as i64)
            .bind(action.resource_kind.as_str())
            .bind(&action.logical_key)
            .bind(action.action.as_str())
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
            .bind(requirement.status.as_str())
            .bind(requirement.reason_code)
            .bind(requirement.installed_release_id)
            .bind(&requirement.installed_version)
            .bind(requirement.installed_state.map(ExtensionState::as_str))
            .bind(requirement.configuration_matches)
            .bind(&requirement.evaluation_template)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// The reviewable summary of an official release a plan installs.
fn extension_install_summary(release: &ResolvedExtensionRelease) -> Value {
    serde_json::json!({
        "version": release.manifest.version,
        "repository": release.release.source,
        "tag_name": release.release.tag_name,
        "grants": extension_install_grants(release),
    })
}

fn extension_install_grants(release: &ResolvedExtensionRelease) -> Value {
    super::required_extension_grants(&release.manifest)
        .into_iter()
        .map(|(kind, id)| serde_json::json!({"kind": kind, "id": id}))
        .collect()
}

/// Pins the official release of every requirement the plan installs, in the
/// dependency order apply installs them.
async fn insert_extension_installs(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
    draft: &SolutionPackPlanDraft,
    official_extensions: &[&ResolvedExtensionRelease],
) -> Result<(), RepositoryError> {
    let mut position = 0_i64;
    for release in official_extensions {
        let Some(requirement) = draft
            .extension_requirements
            .iter()
            .find(|requirement| requirement.extension_id == release.extension_id)
            .filter(|requirement| requirement.status == RequirementStatus::Install)
        else {
            continue;
        };
        sqlx::query("INSERT INTO solution_pack_plan_extension_releases (plan_id,workspace_id,position,logical_key,extension_id,version,installed_release_id,repository,release_id,tag_name,asset_id,asset_name,download_url,archive_sha256,manifest_sha256,configuration,required_grants) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(position)
            .bind(&requirement.logical_key)
            .bind(&release.extension_id)
            .bind(&release.manifest.version)
            .bind(release.installed_release_id)
            .bind(&release.release.source)
            .bind(release.release.release_id as i64)
            .bind(&release.release.tag_name)
            .bind(release.release.asset.id as i64)
            .bind(&release.release.asset.name)
            .bind(&release.release.asset.download_url)
            .bind(&release.archive_sha256)
            .bind(&release.manifest_sha256)
            .bind(&release.configuration)
            .bind(extension_install_grants(release))
            .execute(&mut **tx)
            .await?;
        position += 1;
    }
    Ok(())
}

fn presentation_asset_object_key(workspace_id: Uuid, plan_id: Uuid, target_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"attricat.presentation-asset.object.v1\0");
    hasher.update(workspace_id.as_bytes());
    hasher.update(plan_id.as_bytes());
    hasher.update(target_id.as_bytes());
    format!("presentation-assets/{:x}", hasher.finalize())
}

async fn validate_sample_staging_integrity(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
) -> Result<(), RepositoryError> {
    let selected: bool = sqlx::query_scalar(
        "SELECT sample_data_selected FROM solution_pack_plans WHERE workspace_id=$1 AND id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let evidence: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(e) - 'workspace_id' - 'plan_id' ORDER BY position),'[]'::jsonb) FROM solution_pack_plan_sample_evidence e WHERE workspace_id=$1 AND plan_id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let staging: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(s) - 'workspace_id' - 'plan_id' - 'canonical_input' - 'created_at' ORDER BY position),'[]'::jsonb) FROM solution_pack_plan_sample_records s WHERE workspace_id=$1 AND plan_id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let stage_rows = sqlx::query_as::<_, (String, String, Value)>(
        "SELECT logical_key,canonical_declaration_sha256,canonical_input FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    if !selected {
        if evidence != serde_json::json!([]) || !stage_rows.is_empty() {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "unselected sample evidence is present".into(),
            ));
        }
        return Ok(());
    }
    if stage_rows.is_empty() {
        let terminal: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM solution_pack_applications WHERE workspace_id=$1 AND plan_id=$2 AND state IN ('completed','invalid','abandoned'))",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_one(&mut **tx)
        .await?;
        if terminal {
            return Ok(());
        }
        return Err(RepositoryError::InvalidSolutionPackPlan(
            "sample staging evidence is missing".into(),
        ));
    }
    if evidence != staging {
        return Err(RepositoryError::InvalidSolutionPackPlan(
            "sample staging metadata does not match durable evidence".into(),
        ));
    }
    for (logical_key, expected, input) in stage_rows {
        let actual = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&input)
                    .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?
            )
        );
        if actual != expected {
            return Err(RepositoryError::InvalidSolutionPackPlan(format!(
                "sample staging input for '{logical_key}' does not match its digest"
            )));
        }
    }
    Ok(())
}

/// Stores the digest of a plan's persisted evidence once its rows are written.
async fn seal_plan_resource_evidence(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
) -> Result<(), RepositoryError> {
    let evidence_sha256 = plan_resource_evidence_sha256(tx, workspace_id, plan_id).await?;
    sqlx::query(
        "UPDATE solution_pack_plans SET resource_evidence_sha256=$3 WHERE workspace_id=$1 AND id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .bind(evidence_sha256)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Digest of every persisted plan row apply trusts: the header, mappings,
/// private actions, staged asset objects, extension requirements and pinned
/// releases, and the selected sample evidence and staged sample files. Every
/// section is always present, empty when the plan has none.
async fn plan_resource_evidence_sha256(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    plan_id: Uuid,
) -> Result<String, RepositoryError> {
    let header: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('plan_id',id,'workspace_id',workspace_id,'archive_sha256',archive_sha256,'pack_id',pack_id,'pack_version',pack_version,'prefix',prefix,'blueprint_publication',blueprint_publication,'ready',ready,'sample_data_selected',sample_data_selected,'sample_declaration_sha256',sample_declaration_sha256,'sample_record_count',sample_record_count,'sample_automation_warning',sample_automation_warning) FROM solution_pack_plans WHERE workspace_id=$1 AND id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let mappings = sqlx::query_as::<_, PlanEvidenceMapping>(
        &format!("SELECT {PLAN_MAPPING_COLUMNS},snapshot FROM solution_pack_plan_mappings WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position"),
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    let actions = sqlx::query_as::<_, PlanEvidenceAction>(
        "SELECT position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions FROM solution_pack_plan_actions WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    let asset_objects = sqlx::query_as::<_, PlanEvidenceAssetObject>(
        "SELECT logical_key,target_id,object_key,media_type,byte_size,sha256 FROM solution_pack_plan_asset_objects WHERE workspace_id=$1 AND plan_id=$2 ORDER BY logical_key",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    let extension_requirements = sqlx::query_as::<_, PlanEvidenceExtensionRequirement>(
        "SELECT position,logical_key,extension_id,version_requirement,required,configuration_template_path,configuration_template_sha256,status,reason_code,installed_release_id,installed_version,installed_state,configuration_matches,evaluation_template FROM solution_pack_plan_extension_requirements WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    let installs = sqlx::query_as::<_, PlanEvidenceExtensionInstall>(
        "SELECT position,logical_key,extension_id,version,installed_release_id,repository,release_id,tag_name,asset_id,asset_name,download_url,archive_sha256,manifest_sha256,configuration,required_grants FROM solution_pack_plan_extension_releases WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?;
    let sample_records: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(jsonb_build_object('position',position,'logical_key',logical_key,'target_id',target_id,'blueprint_logical_key',blueprint_logical_key,'blueprint_id',blueprint_id,'blueprint_version',blueprint_version,'canonical_declaration_sha256',canonical_declaration_sha256,'scalar_count',scalar_count,'relationship_target_count',relationship_target_count) ORDER BY position),'[]'::jsonb) FROM solution_pack_plan_sample_evidence WHERE workspace_id=$1 AND plan_id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let sample_files: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(jsonb_build_object('path',path,'file_id',file_id,'object_key',object_key,'filename',filename,'media_type',media_type,'byte_size',byte_size,'sha256',sha256) ORDER BY path),'[]'::jsonb) FROM solution_pack_plan_sample_files WHERE workspace_id=$1 AND plan_id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    let encoded = serde_json::to_vec(&(
        header,
        mappings,
        actions,
        asset_objects,
        extension_requirements,
        installs,
        sample_records,
        sample_files,
    ))
    .map_err(|error| RepositoryError::InvalidSolutionPackPlan(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
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
    #[sqlx(try_from = "String")]
    pub blueprint_publication: BlueprintPublication,
    #[sqlx(try_from = "String")]
    pub state: SolutionPackApplicationState,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub mapping_snapshot: Value,
    pub readme_markdown: Option<String>,
    pub release_notes_markdown: Option<String>,
    pub setup_checklist: Option<Value>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub resumable_until: Option<DateTime<Utc>>,
    pub abandoned_at: Option<DateTime<Utc>>,
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
    #[sqlx(try_from = "String")]
    pub blueprint_publication: BlueprintPublication,
    #[sqlx(try_from = "String")]
    pub state: SolutionPackApplicationState,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub resumable_until: Option<DateTime<Utc>>,
    pub abandoned_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SolutionPackApplicationStep {
    pub position: i64,
    #[sqlx(try_from = "String")]
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    pub target_id: Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    #[sqlx(try_from = "String")]
    pub state: SolutionPackStepState,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
    pub result_snapshot: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct RevalidationStep {
    #[sqlx(try_from = "String")]
    resource_kind: PlanResourceKind,
    target_id: Uuid,
    target_code: String,
    #[sqlx(try_from = "String")]
    state: SolutionPackStepState,
    #[sqlx(try_from = "String")]
    action: PlanActionKind,
    normalized_payload: Option<Value>,
    preconditions: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetAbsentPrecondition {
    kind: String,
    resource_kind: PlanResourceKind,
    code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistingBlueprintPrecondition {
    kind: String,
    id: Uuid,
    code: String,
    version: i64,
    definition_hash: String,
    canonical_definition_hash: String,
    blueprint_kind: String,
    status: String,
    deleted: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExistingBlueprintMappingSnapshot {
    id: Uuid,
    code: String,
    version: i64,
    kind: String,
    canonical_definition_hash: String,
    definition_hash: String,
    status: String,
    deleted: bool,
}

#[derive(sqlx::FromRow)]
struct PendingApplicationStep {
    position: i64,
    #[sqlx(try_from = "String")]
    resource_kind: PlanResourceKind,
    logical_key: String,
    target_id: Uuid,
    target_code: String,
    target_version: Option<i64>,
    #[sqlx(try_from = "String")]
    action: PlanActionKind,
    normalized_payload: Option<Value>,
    preconditions: Value,
}

struct StepApplicationError {
    position: Option<i64>,
    error: RepositoryError,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationAssetPayload {
    purpose: String,
    media_type: String,
    byte_size: i64,
    source_byte_size: i64,
    source_sha256: String,
    stored_sha256: String,
    width: Option<i32>,
    height: Option<i32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlueprintPayload {
    definition: String,
    version: i64,
    publication: BlueprintPublication,
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

async fn invalidate_solution_pack_application(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
    plan_id: Uuid,
    diagnostic_code: &str,
    diagnostic_message: &str,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "DELETE FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2",
    )
    .bind(workspace_id)
    .bind(plan_id)
    .execute(&mut **tx)
    .await?;
    expire_sample_file_staging(tx, workspace_id, &[plan_id]).await?;
    sqlx::query("UPDATE solution_pack_applications SET state='invalid',diagnostic_code=$3,diagnostic_message=$4,updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND state <> 'completed'")
        .bind(workspace_id)
        .bind(application_id)
        .bind(diagnostic_code)
        .bind(diagnostic_message)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn set_solution_pack_transaction_timeouts(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), RepositoryError> {
    sqlx::query("SET LOCAL lock_timeout = '5min'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '5min'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl AttricatRepository {
    /// Official releases a plan installs, in installation order. Returns none
    /// once the plan's application has finished, so re-applying a completed
    /// plan never re-enables an extension an operator disabled since.
    pub async fn solution_pack_extension_installs(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<PlannedExtensionInstall>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let (ready, expires_at) = sqlx::query_as::<_, (bool, DateTime<Utc>)>(
            "SELECT ready, expires_at FROM solution_pack_plans WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("solution-pack plan"))?;
        let application_state = sqlx::query_scalar::<_, String>(
            "SELECT state FROM solution_pack_applications WHERE workspace_id=$1 AND plan_id=$2",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?;
        match application_state
            .map(stored_code::<SolutionPackApplicationState>)
            .transpose()?
        {
            // Apply reports these states itself.
            Some(
                SolutionPackApplicationState::Completed
                | SolutionPackApplicationState::Invalid
                | SolutionPackApplicationState::Abandoned,
            ) => return Ok(Vec::new()),
            Some(_) => {}
            None if !ready => return Err(RepositoryError::SolutionPackPlanNotReady),
            None if expires_at <= Utc::now() => {
                return Err(RepositoryError::SolutionPackPlanExpired);
            }
            None => {}
        }
        Ok(sqlx::query_as::<_, PlannedExtensionInstall>(
            "SELECT logical_key,extension_id,version,installed_release_id,repository,release_id,tag_name,asset_id,asset_name,download_url,archive_sha256,manifest_sha256,configuration FROM solution_pack_plan_extension_releases WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn apply_solution_pack_plan(
        &self,
        plan_id: Uuid,
        object_store: &dyn ObjectStore,
    ) -> Result<SolutionPackApplication, RepositoryError> {
        let application_id = self.prepare_solution_pack_application(plan_id).await?;
        loop {
            match self
                .apply_next_solution_pack_step(application_id, object_store)
                .await
            {
                Ok(true) => continue,
                Ok(false) => break,
                Err(failure) => {
                    if matches!(
                        failure.error,
                        RepositoryError::SolutionPackAssetStorageUnavailable
                    ) {
                        return Err(failure.error);
                    }
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

    async fn validate_persisted_solution_pack_resources(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            &format!("SELECT {PLAN_MAPPING_COLUMNS},snapshot FROM solution_pack_plan_mappings WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position"),
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&mut **tx)
        .await?;
        let actions = sqlx::query_as::<_, PersistedSolutionPackPlanAction>(
            "SELECT resource_kind,logical_key,action,reason_code,normalized_payload,preconditions FROM solution_pack_plan_actions WHERE workspace_id=$1 AND plan_id=$2 ORDER BY position",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_all(&mut **tx)
        .await?;
        let stored_evidence_sha256: Option<String> = sqlx::query_scalar(
            "SELECT resource_evidence_sha256 FROM solution_pack_plans WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .fetch_one(&mut **tx)
        .await?;
        let stored_evidence_sha256 = stored_evidence_sha256.ok_or_else(|| {
            RepositoryError::InvalidSolutionPackPlan(
                "persisted solution-pack resource evidence is missing".into(),
            )
        })?;
        // The persisted digest is authoritative even if every covered row or its
        // mapping/action discriminator was modified after review.
        if stored_evidence_sha256
            != plan_resource_evidence_sha256(tx, workspace_id, plan_id).await?
        {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "persisted solution-pack resource evidence is inconsistent".into(),
            ));
        }
        validate_sample_staging_integrity(tx, workspace_id, plan_id).await?;
        if mappings.len() != actions.len() {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "persisted resource mappings and actions do not match".into(),
            ));
        }
        let mappings_by_key = mappings
            .iter()
            .map(|mapping| (mapping.logical_key.as_str(), mapping))
            .collect::<std::collections::BTreeMap<_, _>>();
        for mapping in &mappings {
            let valid = match mapping.resource_kind {
                PlanResourceKind::Blueprint => match mapping.mapping_kind {
                    MappingKind::Create => mapping.target_version == Some(1),
                    MappingKind::Existing => {
                        mapping.target_version.is_some_and(|version| version > 0)
                    }
                    _ => false,
                },
                PlanResourceKind::PresentationAsset => {
                    matches!(
                        mapping.mapping_kind,
                        MappingKind::Create | MappingKind::Existing
                    ) && mapping.target_version.is_none()
                        && mapping.target_code == "presentation_asset"
                }
                PlanResourceKind::SampleRecord => {
                    mapping.mapping_kind == MappingKind::Create
                        && mapping.target_version.is_none()
                        && mapping.target_code == "sample_record"
                }
                PlanResourceKind::Prerequisite => {
                    mapping.mapping_kind == MappingKind::Existing
                        && mapping.target_version.is_none()
                }
                PlanResourceKind::Context | PlanResourceKind::PublicationChannel => {
                    matches!(
                        mapping.mapping_kind,
                        MappingKind::Create | MappingKind::Existing
                    ) && mapping.target_version.is_none()
                }
                PlanResourceKind::Rule | PlanResourceKind::Workflow => {
                    mapping.mapping_kind == MappingKind::Create && mapping.target_version == Some(1)
                }
                PlanResourceKind::SavedSearch => {
                    mapping.mapping_kind == MappingKind::Create
                        && mapping.target_version.is_none()
                        && mapping.target_code == "saved_search"
                }
                PlanResourceKind::WorkspaceSetting => {
                    mapping.mapping_kind == MappingKind::Workspace
                        && mapping.target_id == workspace_id
                        && mapping.target_version.is_none()
                        && matches!(
                            (mapping.logical_key.as_str(), mapping.target_code.as_str()),
                            ("workspace/explore-navigation", "explore_navigation")
                                | ("workspace/extension-layout", "extension_layout")
                                | ("workspace/lexicon", "lexicon")
                        )
                }
            };
            if !valid {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource mapping".into(),
                ));
            }
            if mapping.mapping_kind == MappingKind::Existing
                && mapping.resource_kind == PlanResourceKind::PresentationAsset
            {
                let snapshot: ExistingAssetEvidence =
                    serde_json::from_value(mapping.snapshot.clone().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "existing asset mapping is missing snapshot evidence".into(),
                        )
                    })?)
                    .map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "existing asset mapping snapshot is invalid".into(),
                        )
                    })?;
                if snapshot.id != mapping.target_id
                    || snapshot.kind.is_some()
                    || !matches!(snapshot.purpose.as_str(), "logo" | "icon" | "illustration")
                    || !matches!(
                        snapshot.media_type.as_str(),
                        "image/png" | "image/jpeg" | "image/webp" | "image/svg+xml"
                    )
                    || snapshot.byte_size <= 0
                    || snapshot.byte_size > MAX_PRESENTATION_ASSET_BYTES
                    || snapshot.sha256.len() != SHA256_HEX_LEN
                    || snapshot.source_sha256.len() != SHA256_HEX_LEN
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "existing asset mapping snapshot does not match its target".into(),
                    ));
                }
            }
            if mapping.mapping_kind == MappingKind::Existing
                && mapping.resource_kind == PlanResourceKind::Blueprint
            {
                let snapshot = mapping
                    .snapshot
                    .clone()
                    .ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "existing mapping is missing snapshot evidence".into(),
                        )
                    })
                    .and_then(|value| {
                        serde_json::from_value::<ExistingBlueprintMappingSnapshot>(value).map_err(
                            |_| {
                                RepositoryError::InvalidSolutionPackPlan(
                                    "existing mapping snapshot is invalid".into(),
                                )
                            },
                        )
                    })?;
                if snapshot.id != mapping.target_id
                    || snapshot.code != mapping.target_code
                    || Some(snapshot.version) != mapping.target_version
                    || !matches!(snapshot.kind.as_str(), "record" | "mixin")
                    || !is_lowercase_sha256_hex(&snapshot.definition_hash)
                    || !is_lowercase_sha256_hex(&snapshot.canonical_definition_hash)
                    || snapshot.status != RevisionStatus::Published.as_str()
                    || snapshot.deleted
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "existing mapping snapshot does not match its target".into(),
                    ));
                }
            }
        }
        for action in &actions {
            let Some(mapping) = mappings_by_key.get(action.logical_key.as_str()) else {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "persisted resource action has no mapping".into(),
                ));
            };
            // Mapping positions preserve manifest declaration order while action positions
            // preserve dependency execution order. The logical key is their stable join key.
            let valid = action.resource_kind == mapping.resource_kind
                && match action.resource_kind {
                    PlanResourceKind::Blueprint => {
                        matches!(
                            (action.action, mapping.mapping_kind),
                            (
                                PlanActionKind::Create
                                    | PlanActionKind::Skip
                                    | PlanActionKind::Conflict
                                    | PlanActionKind::Blocked,
                                MappingKind::Create
                            ) | (
                                PlanActionKind::Map
                                    | PlanActionKind::Conflict
                                    | PlanActionKind::Blocked,
                                MappingKind::Existing
                            )
                        )
                    }
                    PlanResourceKind::WorkspaceSetting => matches!(
                        action.action,
                        PlanActionKind::Append
                            | PlanActionKind::Satisfied
                            | PlanActionKind::Skip
                            | PlanActionKind::Conflict
                            | PlanActionKind::Blocked
                    ),
                    PlanResourceKind::PresentationAsset => matches!(
                        (action.action, mapping.mapping_kind),
                        (
                            PlanActionKind::Create
                                | PlanActionKind::Skip
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Create
                        ) | (
                            PlanActionKind::Map
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Existing
                        )
                    ),
                    PlanResourceKind::SampleRecord => {
                        action.action == PlanActionKind::Create
                            && mapping.mapping_kind == MappingKind::Create
                    }
                    PlanResourceKind::Prerequisite => {
                        matches!(action.action, PlanActionKind::Map | PlanActionKind::Blocked)
                    }
                    PlanResourceKind::Context => matches!(
                        (action.action, mapping.mapping_kind),
                        (
                            PlanActionKind::Create
                                | PlanActionKind::Skip
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Create
                        ) | (PlanActionKind::Map, MappingKind::Existing)
                    ),
                    PlanResourceKind::PublicationChannel => matches!(
                        (action.action, mapping.mapping_kind),
                        (
                            PlanActionKind::Create
                                | PlanActionKind::Skip
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Create
                        ) | (
                            PlanActionKind::Satisfied
                                | PlanActionKind::Skip
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Existing
                        )
                    ),
                    PlanResourceKind::Rule
                    | PlanResourceKind::Workflow
                    | PlanResourceKind::SavedSearch => matches!(
                        (action.action, mapping.mapping_kind),
                        (
                            PlanActionKind::Create
                                | PlanActionKind::Skip
                                | PlanActionKind::Conflict
                                | PlanActionKind::Blocked,
                            MappingKind::Create
                        )
                    ),
                };
            if !valid {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource action".into(),
                ));
            }
            if action.resource_kind == PlanResourceKind::PresentationAsset
                && action.action == PlanActionKind::Skip
                && (action.reason_code != "optional_not_selected"
                    || action.normalized_payload.is_some()
                    || action.preconditions != serde_json::json!([]))
            {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "persisted optional presentation asset skip evidence is invalid".into(),
                ));
            }
            if action.resource_kind == PlanResourceKind::PresentationAsset
                && action.action == PlanActionKind::Create
            {
                let payload: PresentationAssetPayload =
                    serde_json::from_value(action.normalized_payload.clone().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "persisted presentation asset payload is missing".into(),
                        )
                    })?)
                    .map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "persisted presentation asset payload is invalid".into(),
                        )
                    })?;
                let staged = sqlx::query_as::<_, (Uuid, String, i64, String, String)>(
                    "SELECT target_id,media_type,byte_size,sha256,state FROM solution_pack_plan_asset_objects WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3",
                )
                .bind(workspace_id)
                .bind(plan_id)
                .bind(&action.logical_key)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| RepositoryError::InvalidSolutionPackPlan(
                    "persisted presentation asset staging evidence is missing".into(),
                ))?;
                if staged.0 != mapping.target_id
                    || staged.1 != payload.media_type
                    || staged.2 != payload.byte_size
                    || staged.3 != payload.stored_sha256
                    || !matches!(staged.4.as_str(), "staged" | "claimed")
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted presentation asset staging evidence is inconsistent".into(),
                    ));
                }
            }
            if action.action == PlanActionKind::Map
                && action.resource_kind == PlanResourceKind::PresentationAsset
            {
                if action.reason_code != "exact_asset_match" || action.normalized_payload.is_some()
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted asset map action evidence is invalid".into(),
                    ));
                }
                let preconditions: Vec<ExistingAssetEvidence> =
                    serde::Deserialize::deserialize(&action.preconditions).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "persisted asset map preconditions are invalid".into(),
                        )
                    })?;
                let snapshot: ExistingAssetEvidence = serde_json::from_value(
                    mapping.snapshot.clone().expect("existing asset snapshot"),
                )
                .expect("existing asset snapshot validated");
                if preconditions.len() != 1
                    || preconditions[0].kind.as_deref() != Some("existing_presentation_asset")
                    || preconditions[0].id != snapshot.id
                    || preconditions[0].purpose != snapshot.purpose
                    || preconditions[0].media_type != snapshot.media_type
                    || preconditions[0].byte_size != snapshot.byte_size
                    || preconditions[0].sha256 != snapshot.sha256
                    || preconditions[0].source_sha256 != snapshot.source_sha256
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted asset map evidence does not match its reviewed snapshot".into(),
                    ));
                }
            }
            if action.action == PlanActionKind::Map
                && action.resource_kind == PlanResourceKind::Blueprint
            {
                if !matches!(
                    action.reason_code.as_str(),
                    "exact_blueprint_match" | "prerequisite_blueprint_match"
                ) || action.normalized_payload.is_some()
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted map action evidence is invalid".into(),
                    ));
                }
                let preconditions: Vec<ExistingBlueprintPrecondition> =
                    serde::Deserialize::deserialize(&action.preconditions).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "persisted map preconditions are invalid".into(),
                        )
                    })?;
                let snapshot: ExistingBlueprintMappingSnapshot = serde_json::from_value(
                    mapping
                        .snapshot
                        .clone()
                        .expect("existing mapping has snapshot"),
                )
                .expect("existing mapping snapshot was validated");
                if preconditions.len() != 1
                    || preconditions[0].kind != "existing_blueprint"
                    || preconditions[0].id != snapshot.id
                    || preconditions[0].code != snapshot.code
                    || preconditions[0].version != snapshot.version
                    || preconditions[0].definition_hash != snapshot.definition_hash
                    || preconditions[0].canonical_definition_hash
                        != snapshot.canonical_definition_hash
                    || preconditions[0].blueprint_kind != snapshot.kind
                    || preconditions[0].status != snapshot.status
                    || preconditions[0].deleted != snapshot.deleted
                {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted map evidence does not match its reviewed snapshot".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    async fn prepare_solution_pack_application(
        &self,
        plan_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        self.cleanup_solution_pack_sample_staging().await?;
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let plan = sqlx::query_as::<_, SolutionPackPlan>(
            &format!("SELECT {PLAN_COLUMNS} FROM solution_pack_plans WHERE workspace_id = $1 AND id = $2 FOR UPDATE"),
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
            let state: SolutionPackApplicationState = stored_code(state.clone())?;
            if state == SolutionPackApplicationState::Completed {
                tx.commit().await?;
                return Ok(*application_id);
            }
            if matches!(
                state,
                SolutionPackApplicationState::Invalid | SolutionPackApplicationState::Abandoned
            ) {
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
        self.validate_persisted_solution_pack_resources(&mut tx, plan_id)
            .await?;
        if let Err(error) = self
            .revalidate_required_extension_requirements(&mut tx, plan_id)
            .await
        {
            if matches!(error, RepositoryError::SolutionPackPlanStale)
                && let Some((application_id, _)) = existing_application
            {
                invalidate_solution_pack_application(
                    &mut tx,
                    workspace_id,
                    application_id,
                    plan_id,
                    "plan_stale",
                    "persisted extension requirements no longer match the workspace",
                )
                .await?;
                tx.commit().await?;
            }
            return Err(error);
        }
        let mappings = sqlx::query_as::<_, SolutionPackPlanMapping>(
            "SELECT m.position, m.resource_kind, m.logical_key, m.target_id, m.target_code, m.target_version, m.mapping_kind, CASE WHEN m.mapping_kind='existing' THEN m.snapshot END AS snapshot FROM solution_pack_plan_mappings m JOIN solution_pack_plan_actions a ON a.plan_id=m.plan_id AND a.logical_key=m.logical_key WHERE m.workspace_id = $1 AND m.plan_id = $2 AND a.action IN ('create','map','append','satisfied') ORDER BY m.position",
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
            "INSERT INTO solution_pack_applications (id, workspace_id, plan_id, actor_user_id, actor_token_id, request_id, correlation_id, source_kind, source_metadata, archive_sha256, pack_id, pack_version, blueprint_publication, state, mapping_snapshot, readme_markdown, release_notes_markdown, setup_checklist, resumable_until) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'running',$14,$15,$16,$17,$18) ON CONFLICT (plan_id) DO NOTHING RETURNING id",
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
        .bind(plan.blueprint_publication.as_str())
        .bind(mapping_snapshot)
        .bind(&plan.readme_markdown)
        .bind(&plan.release_notes_markdown)
        .bind(&plan.setup_checklist)
        .bind(
            plan.sample_data_selected
                .then(|| Utc::now() + Duration::days(SAMPLE_APPLICATION_RESUMABLE_DAYS)),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let application_id = match inserted {
            Some(id) => {
                if plan.sample_data_selected {
                    sqlx::query(&format!("UPDATE solution_pack_applications SET resumable_until=started_at + interval '{SAMPLE_APPLICATION_RESUMABLE_DAYS} days' WHERE workspace_id=$1 AND id=$2"))
                        .bind(workspace_id).bind(id).execute(&mut *tx).await?;
                }
                sqlx::query(
                    "INSERT INTO solution_pack_application_steps (application_id, workspace_id, position, plan_id, resource_kind, logical_key, target_id, target_code, target_version, state) SELECT $1, a.workspace_id, a.position, a.plan_id, a.resource_kind, a.logical_key, m.target_id, m.target_code, m.target_version, 'pending' FROM solution_pack_plan_actions a JOIN solution_pack_plan_mappings m ON m.plan_id = a.plan_id AND m.logical_key = a.logical_key WHERE a.workspace_id = $2 AND a.plan_id = $3 AND a.action IN ('create','map','append','satisfied') ORDER BY a.position",
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
        .await
        .map_err(RepositoryError::from)
        .and_then(stored_code::<SolutionPackApplicationState>)?;
        if state == SolutionPackApplicationState::Invalid {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        if state != SolutionPackApplicationState::Completed {
            if let Err(error) = self
                .revalidate_application_steps(&mut tx, application_id)
                .await
            {
                if matches!(error, RepositoryError::SolutionPackPlanStale) {
                    invalidate_solution_pack_application(
                        &mut tx,
                        workspace_id,
                        application_id,
                        plan_id,
                        "plan_stale",
                        "persisted target preconditions no longer match the workspace",
                    )
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
        let workspace_id = self.workspace_id.0;
        let requirements = sqlx::query_as::<_, PrivatePlanExtensionRequirement>(
            // An `install` requirement must now be satisfied by its pinned release.
            "SELECT r.logical_key, r.extension_id, r.version_requirement, r.required, r.configuration_template_path, r.configuration_template_sha256, r.status, COALESCE(r.installed_release_id, i.installed_release_id) AS installed_release_id, COALESCE(r.installed_version, i.version) AS installed_version, r.evaluation_template FROM solution_pack_plan_extension_requirements r LEFT JOIN solution_pack_plan_extension_releases i ON i.plan_id=r.plan_id AND i.logical_key=r.logical_key WHERE r.workspace_id=$1 AND r.plan_id=$2 AND r.required ORDER BY r.position",
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
        .collect::<std::collections::BTreeMap<_, _>>();
        for persisted in requirements {
            if !persisted.required
                || !matches!(
                    persisted.status,
                    RequirementStatus::Satisfied | RequirementStatus::Install
                )
            {
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
            if evaluated.status != RequirementStatus::Satisfied
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
        let workspace_id = self.workspace_id.0;
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
        .collect::<std::collections::BTreeMap<_, _>>();
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
                || snapshot.state == ExtensionState::Quarantined
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
        let workspace_id = self.workspace_id.0;
        self.revalidate_all_existing_blueprints(tx, application_id)
            .await?;
        let steps = sqlx::query_as::<_, RevalidationStep>(
            "SELECT s.resource_kind,s.target_id,s.target_code,s.state,a.action,a.normalized_payload,a.preconditions FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 ORDER BY s.position",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_all(&mut **tx)
        .await?;
        for step in steps {
            if step.action == PlanActionKind::Map
                && step.resource_kind == PlanResourceKind::PresentationAsset
            {
                let expected: Vec<ExistingAssetEvidence> =
                    serde_json::from_value(step.preconditions).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid existing-asset preconditions".into(),
                        )
                    })?;
                let actual = sqlx::query_as::<_, (String, String, i64, String)>(
                    "SELECT purpose,media_type,byte_size,sha256 FROM presentation_assets WHERE workspace_id=$1 AND id=$2 FOR SHARE",
                )
                .bind(workspace_id)
                .bind(step.target_id)
                .fetch_optional(&mut **tx)
                .await?;
                if expected.len() != 1
                    || expected[0].kind.as_deref() != Some("existing_presentation_asset")
                    || expected[0].id != step.target_id
                    || actual.as_ref().is_none_or(|actual| {
                        actual.0 != expected[0].purpose
                            || actual.1 != expected[0].media_type
                            || actual.2 != expected[0].byte_size
                            || actual.3 != expected[0].sha256
                    })
                {
                    return Err(RepositoryError::SolutionPackPlanStale);
                }
                continue;
            }
            if step.resource_kind == PlanResourceKind::Prerequisite {
                self.revalidate_prerequisite(tx, step.target_id, &step.preconditions)
                    .await?;
                continue;
            }
            if step.resource_kind == PlanResourceKind::Context && step.action == PlanActionKind::Map
            {
                self.revalidate_existing_context(tx, step.target_id, &step.target_code)
                    .await?;
                continue;
            }
            if step.resource_kind == PlanResourceKind::PublicationChannel
                && step.action == PlanActionKind::Satisfied
            {
                self.revalidate_satisfied_channel(
                    tx,
                    step.target_id,
                    step.normalized_payload.as_ref(),
                )
                .await?;
                continue;
            }
            if step.action == PlanActionKind::Map {
                continue;
            }
            if step.resource_kind == PlanResourceKind::WorkspaceSetting {
                if step.target_id != workspace_id {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workspace setting mapping is invalid".into(),
                    ));
                }
                if step.target_code == "explore_navigation" {
                    let payload = parse_explore_navigation_payload(
                        step.normalized_payload.ok_or_else(|| {
                            RepositoryError::InvalidSolutionPackPlan(
                                "missing workspace setting payload".into(),
                            )
                        })?,
                    )?;
                    let current = self
                        .lock_explore_navigation_in_transaction(tx, workspace_id)
                        .await?;
                    if step.state == SolutionPackStepState::Completed
                        || step.action == PlanActionKind::Satisfied
                    {
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
                    let valid = if step.state == SolutionPackStepState::Completed
                        || step.action == PlanActionKind::Satisfied
                    {
                        all_exact
                    } else {
                        step.action == PlanActionKind::Append && appendable
                    };
                    if !valid {
                        return Err(RepositoryError::SolutionPackPlanStale);
                    }
                } else if step.target_code == "lexicon" {
                    // Applying lexicon entries never conflicts with workspace
                    // state, so only the persisted payload is checked.
                    parse_lexicon_payload(step.normalized_payload.ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "missing workspace setting payload".into(),
                        )
                    })?)?;
                } else if step.target_code == "extension_layout" {
                    let payload = parse_extension_layout_payload(
                        step.normalized_payload.ok_or_else(|| {
                            RepositoryError::InvalidSolutionPackPlan(
                                "missing workspace setting payload".into(),
                            )
                        })?,
                    )?;
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
                    let valid = if step.state == SolutionPackStepState::Completed
                        || step.action == PlanActionKind::Satisfied
                    {
                        all_exact
                    } else {
                        step.action == PlanActionKind::Append && appendable
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
            if step.resource_kind == PlanResourceKind::SampleRecord {
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND id=$2 AND deleted_at IS NULL)",
                )
                .bind(workspace_id)
                .bind(step.target_id)
                .fetch_one(&mut **tx)
                .await?;
                if (step.state == SolutionPackStepState::Completed) != exists {
                    return Err(RepositoryError::SolutionPackPlanStale);
                }
                continue;
            }
            if step.resource_kind == PlanResourceKind::PresentationAsset {
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM presentation_assets WHERE workspace_id=$1 AND id=$2)",
                )
                .bind(workspace_id)
                .bind(step.target_id)
                .fetch_one(&mut **tx)
                .await?;
                if (step.state == SolutionPackStepState::Completed) != exists {
                    return Err(RepositoryError::SolutionPackPlanStale);
                }
                continue;
            }
            if matches!(
                step.resource_kind,
                PlanResourceKind::PublicationChannel
                    | PlanResourceKind::Rule
                    | PlanResourceKind::Workflow
                    | PlanResourceKind::SavedSearch
            ) {
                let (exists, code_taken) = self
                    .seed_target_state(tx, step.resource_kind, step.target_id, &step.target_code)
                    .await?;
                let valid = if step.state == SolutionPackStepState::Completed {
                    exists
                } else {
                    !exists && !code_taken
                };
                if !valid {
                    return Err(RepositoryError::SolutionPackPlanStale);
                }
                continue;
            }
            if !matches!(
                step.resource_kind,
                PlanResourceKind::Blueprint | PlanResourceKind::Context
            ) {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource kind".into(),
                ));
            }
            let matches =
                super::workspace_resource_code_matches(tx, workspace_id, &step.target_code).await?;
            let valid = if step.state == SolutionPackStepState::Completed {
                matches.as_slice() == [(step.resource_kind.as_str().to_owned(), step.target_id)]
            } else {
                matches.is_empty()
            };
            if !valid {
                return Err(RepositoryError::SolutionPackPlanStale);
            }
        }
        Ok(())
    }

    async fn revalidate_existing_blueprint(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        target_id: Uuid,
        target_code: &str,
        target_version: Option<i64>,
        preconditions: Value,
    ) -> Result<Value, RepositoryError> {
        let persisted: Vec<ExistingBlueprintPrecondition> = serde_json::from_value(preconditions)
            .map_err(|_| {
            RepositoryError::InvalidSolutionPackPlan(
                "invalid existing-blueprint preconditions".into(),
            )
        })?;
        if persisted.len() != 1 {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "existing-blueprint step requires one precondition".into(),
            ));
        }
        let expected = &persisted[0];
        if expected.kind != "existing_blueprint"
            || expected.id != target_id
            || expected.code != target_code
            || Some(expected.version) != target_version
            || !matches!(expected.blueprint_kind.as_str(), "record" | "mixin")
            || expected.status != RevisionStatus::Published.as_str()
            || expected.deleted
        {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "existing-blueprint mapping and preconditions do not match".into(),
            ));
        }
        let workspace_id = self.workspace_id.0;
        // Lock every revision for the selected code. Ordinary revision creation
        // locks the latest row, and publication/deletion locks its target row,
        // so none can race this step after the exact evidence is checked.
        let revisions = sqlx::query_as::<
            _,
            (
                Uuid,
                i64,
                String,
                String,
                String,
                Option<DateTime<Utc>>,
                String,
            ),
        >(
            "SELECT id,version,code,kind,status,deleted_at,definition_hash FROM blueprints WHERE workspace_id=$1 AND code=$2 ORDER BY version FOR SHARE",
        )
        .bind(workspace_id)
        .bind(&expected.code)
        .fetch_all(&mut **tx)
        .await?;
        let latest_published_version = revisions
            .iter()
            .filter(|(_, _, _, _, status, deleted_at, _)| {
                status == RevisionStatus::Published.as_str() && deleted_at.is_none()
            })
            .map(|(_, version, _, _, _, _, _)| *version)
            .max();
        let actual = revisions
            .iter()
            .find(|(id, version, _, _, _, _, _)| *id == target_id && *version == expected.version);
        let Some((_, _, code, blueprint_kind, status, deleted_at, definition_hash)) = actual else {
            return Err(RepositoryError::SolutionPackPlanStale);
        };
        if code != &expected.code
            || blueprint_kind != &expected.blueprint_kind
            || status != &expected.status
            || deleted_at.is_some()
            || definition_hash != &expected.definition_hash
            || latest_published_version != Some(expected.version)
        {
            return Err(RepositoryError::SolutionPackPlanStale);
        }
        Ok(serde_json::json!({
            "outcome": "reused",
            "id": target_id,
            "code": code,
            "version": expected.version,
            "definition_hash": definition_hash,
            "kind": blueprint_kind,
            "status": status,
        }))
    }

    async fn revalidate_all_existing_blueprints(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        application_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let rows = sqlx::query_as::<_, (Uuid, String, Option<i64>, Value)>(
            "SELECT s.target_id,s.target_code,s.target_version,a.preconditions FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 AND a.action='map' AND s.resource_kind='blueprint' ORDER BY s.target_code,s.target_id",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_all(&mut **tx)
        .await?;
        for (id, code, version, preconditions) in rows {
            self.revalidate_existing_blueprint(tx, id, &code, version, preconditions)
                .await?;
        }
        Ok(())
    }

    async fn apply_next_solution_pack_step(
        &self,
        application_id: Uuid,
        object_store: &dyn ObjectStore,
    ) -> Result<bool, StepApplicationError> {
        let mut attempted_position = None;
        self.apply_next_solution_pack_step_inner(
            application_id,
            &mut attempted_position,
            object_store,
        )
        .await
        .map_err(|error| StepApplicationError {
            position: attempted_position,
            error,
        })
    }

    /// Creates a staged sample record from its canonical plan evidence.
    async fn apply_sample_record_step(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan_id: Uuid,
        step: &PendingApplicationStep,
        object_store: &dyn ObjectStore,
    ) -> Result<(Value, Vec<crate::domain_events::NewDomainEvent>), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok({
            let (blueprint_id, blueprint_version, canonical_digest, canonical_input) = sqlx::query_as::<_, (Uuid, i64, String, Value)>(
            "SELECT blueprint_id,blueprint_version,canonical_declaration_sha256,canonical_input FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3 AND target_id=$4 FOR UPDATE",
        )
        .bind(workspace_id).bind(plan_id).bind(&step.logical_key).bind(step.target_id)
        .fetch_optional(&mut **tx).await?
        .ok_or_else(|| RepositoryError::InvalidSolutionPackPlan("sample staging evidence is missing".into()))?;
            let sample: SampleRecord = serde_json::from_value(
                canonical_input
                    .get("record")
                    .cloned()
                    .unwrap_or(Value::Null),
            )
            .map_err(|_| {
                RepositoryError::InvalidSolutionPackPlan("sample staging input is invalid".into())
            })?;
            let contexts = self
                .sample_context_ids(tx, plan_id, sample_record_contexts(&sample))
                .await?;
            let file_values = sample.files.clone();
            let mut values = sample
                .facts
                .into_iter()
                .map(|fact| NewAttributeValue::Scalar {
                    attribute_id: None,
                    attribute_code: fact.attribute.rsplit('/').next().map(str::to_owned),
                    context_id: fact.context.as_ref().map(|context| contexts[context]),
                    value: fact.value,
                })
                .collect::<Vec<_>>();
            for relationship in sample.relationships {
                let target_ids = sqlx::query_as::<_, (String, Uuid)>(
                "SELECT logical_key,target_id FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=ANY($3)",
            )
            .bind(workspace_id).bind(plan_id).bind(&relationship.targets)
            .fetch_all(&mut **tx).await?
            .into_iter().collect::<std::collections::BTreeMap<_, _>>();
                if target_ids.len() != relationship.targets.len() {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "sample relationship target evidence is missing".into(),
                    ));
                }
                for target in relationship.targets {
                    values.push(NewAttributeValue::Relationship {
                        attribute_id: None,
                        attribute_code: relationship
                            .attribute
                            .rsplit('/')
                            .next()
                            .map(str::to_owned),
                        context_id: relationship
                            .context
                            .as_ref()
                            .map(|context| contexts[context]),
                        target_record_id: target_ids[&target],
                    });
                }
            }
            let (record, changes, event) = self
                .create_record_in_transaction(
                    tx,
                    ChosenIdRecordCreate {
                        record_id: step.target_id,
                        blueprint_id,
                        blueprint_version,
                        values,
                        files: super::record_commands::CreateFileValues::None,
                        system_tags: vec![SAMPLE_SYSTEM_TAG.to_owned()],
                        system_metadata: serde_json::json!({}),
                        host_sample_marker: true,
                    },
                )
                .await
                .map_err(solution_pack_mutation_error)?;
            // Bundled files are attached in the same transaction before
            // the write is staged, and the record is validated again
            // and its preview refreshed with them, like an ordinary
            // file write.
            let file_count = self
                .attach_sample_files(
                    tx,
                    plan_id,
                    record.id,
                    &file_values,
                    &contexts,
                    object_store,
                )
                .await?;
            if file_count > 0 {
                self.revalidate_record(tx, &record)
                    .await
                    .map_err(solution_pack_mutation_error)?;
            }
            let mut record_repository = self.clone();
            if let Some(audit) = record_repository.audit_context.as_mut() {
                audit.target = serde_json::json!({"type":"record","id":record.id});
            }
            record_repository
                .stage_record_mutation(tx, changes, event)
                .await?;
            (
                {
                    let mut result = serde_json::json!({"id":record.id,"blueprint_id":record.blueprint_id,"blueprint_version":record.blueprint_version,"is_sample":true,"canonical_declaration_sha256":canonical_digest});
                    if file_count > 0 {
                        result["file_count"] = file_count.into();
                    }
                    result
                },
                Vec::new(),
            )
        })
    }

    /// Appends the pack's Explore navigation or extension layout entries to
    /// the workspace settings, or records that they are already satisfied.
    async fn apply_workspace_setting_step(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        application_id: Uuid,
        step: &PendingApplicationStep,
    ) -> Result<(Value, Vec<crate::domain_events::NewDomainEvent>), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok({
            if step.target_id != workspace_id
                || !matches!(
                    step.action,
                    PlanActionKind::Append | PlanActionKind::Satisfied
                )
            {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "persisted workspace setting step is invalid".into(),
                ));
            }
            if step.target_code == "explore_navigation" {
                let desired = parse_explore_navigation_payload(
                    step.normalized_payload.clone().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "missing workspace setting payload".into(),
                        )
                    })?,
                )?;
                let current = self
                    .lock_explore_navigation_in_transaction(tx, workspace_id)
                    .await?;
                self.validate_explore_navigation_entries_in_transaction(tx, workspace_id, &desired)
                    .await
                    .map_err(solution_pack_navigation_error)?;
                let all_exact = navigation_entries_all_exact(&current, &desired);
                let outcome = if step.action == PlanActionKind::Satisfied {
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
                    self.write_explore_navigation_in_transaction(tx, workspace_id, &merged)
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
            } else if step.target_code == "lexicon" {
                let entries =
                    parse_lexicon_payload(step.normalized_payload.clone().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "missing workspace setting payload".into(),
                        )
                    })?)?;
                let pack_id: String = sqlx::query_scalar(
                "SELECT pack_id FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2",
            )
            .bind(workspace_id)
            .bind(application_id)
            .fetch_one(&mut **tx)
            .await?;
                let written = Self::apply_solution_pack_lexicon_in_transaction(
                    tx,
                    workspace_id,
                    &pack_id,
                    &entries,
                )
                .await?;
                (
                    serde_json::json!({
                        "setting": "lexicon",
                        "entry_count": entries.len(),
                        "written_count": written,
                        "outcome": if written == 0 { "satisfied" } else { "appended" },
                    }),
                    Vec::new(),
                )
            } else if step.target_code == "extension_layout" {
                let desired = parse_extension_layout_payload(
                    step.normalized_payload.clone().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "missing workspace setting payload".into(),
                        )
                    })?,
                )?;
                self.revalidate_extension_contributions(tx, &extension_layout_snapshots(&desired))
                    .await?;
                let current = self
                    .lock_workspace_extension_layout_in_transaction(tx, workspace_id)
                    .await
                    .map_err(solution_pack_layout_error)?;
                let before_sha256 = json_sha256(&current)?;
                let all_exact = extension_layout_entries_all_exact(&current, &desired);
                let (outcome, merged) = if step.action == PlanActionKind::Satisfied {
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
                    self.write_workspace_extension_layout_in_transaction(tx, workspace_id, &merged)
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
        })
    }

    async fn apply_next_solution_pack_step_inner(
        &self,
        application_id: Uuid,
        attempted_position: &mut Option<i64>,
        object_store: &dyn ObjectStore,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        set_solution_pack_transaction_timeouts(&mut tx).await?;
        let (state, plan_id, resumable_until) = sqlx::query_as::<_, (String, Uuid, Option<DateTime<Utc>>)>(
            "SELECT state,plan_id,resumable_until FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("solution-pack application"))?;
        let state: SolutionPackApplicationState = stored_code(state)?;
        if state == SolutionPackApplicationState::Completed {
            tx.commit().await?;
            return Ok(false);
        }
        if matches!(
            state,
            SolutionPackApplicationState::Invalid | SolutionPackApplicationState::Abandoned
        ) {
            return Err(RepositoryError::SolutionPackApplicationInvalid);
        }
        if state == SolutionPackApplicationState::Failed {
            return Err(RepositoryError::SolutionPackApplicationFailed(
                "retry must revalidate the application before continuing".to_owned(),
            ));
        }
        if resumable_until.is_some_and(|deadline| deadline <= Utc::now()) {
            return Err(RepositoryError::SolutionPackApplicationFailed(
                "sample application resumability deadline has elapsed".to_owned(),
            ));
        }
        let step = sqlx::query_as::<_, PendingApplicationStep>(
            "SELECT s.position,s.resource_kind,s.logical_key,s.target_id,s.target_code,s.target_version,a.action,a.normalized_payload,a.preconditions FROM solution_pack_application_steps s JOIN solution_pack_plan_actions a ON a.plan_id=s.plan_id AND a.position=s.position WHERE s.workspace_id=$1 AND s.application_id=$2 AND s.state='pending' ORDER BY s.position LIMIT 1",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(step) = step else {
            sqlx::query("DELETE FROM solution_pack_plan_sample_records WHERE workspace_id=$1 AND plan_id=$2")
                .bind(workspace_id).bind(plan_id).execute(&mut *tx).await?;
            expire_sample_file_staging(&mut tx, workspace_id, &[plan_id]).await?;
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
        self.revalidate_all_existing_blueprints(&mut tx, application_id)
            .await?;
        *attempted_position = Some(step.position);
        if step.resource_kind != PlanResourceKind::WorkspaceSetting
            && !matches!(step.action, PlanActionKind::Map | PlanActionKind::Satisfied)
        {
            self.ensure_target_absent(&mut tx, &step).await?;
        }
        let (result_snapshot, events) = if step.action == PlanActionKind::Map {
            if step.resource_kind == PlanResourceKind::PresentationAsset {
                let expected: Vec<ExistingAssetEvidence> =
                    serde::Deserialize::deserialize(&step.preconditions).map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid existing-asset preconditions".into(),
                        )
                    })?;
                let actual = sqlx::query_as::<_, (String, String, i64, String, String)>(
                    "SELECT purpose,media_type,byte_size,sha256,object_key FROM presentation_assets WHERE workspace_id=$1 AND id=$2 FOR SHARE",
                )
                .bind(workspace_id)
                .bind(step.target_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(RepositoryError::SolutionPackPlanStale)?;
                if expected.len() != 1
                    || expected[0].purpose != actual.0
                    || expected[0].media_type != actual.1
                    || expected[0].byte_size != actual.2
                    || expected[0].sha256 != actual.3
                {
                    return Err(RepositoryError::SolutionPackPlanStale);
                }
                verify_stored_object(object_store, &actual.4, actual.2, &actual.3).await?;
                (
                    serde_json::json!({"outcome":"reused","id":step.target_id,"purpose":actual.0,"media_type":actual.1,"byte_size":actual.2,"sha256":actual.3,"source_sha256":expected[0].source_sha256}),
                    Vec::new(),
                )
            } else if step.resource_kind == PlanResourceKind::Prerequisite {
                (
                    self.revalidate_prerequisite(&mut tx, step.target_id, &step.preconditions)
                        .await?,
                    Vec::new(),
                )
            } else if step.resource_kind == PlanResourceKind::Context {
                (
                    self.revalidate_existing_context(&mut tx, step.target_id, &step.target_code)
                        .await?,
                    Vec::new(),
                )
            } else {
                (
                    self.revalidate_existing_blueprint(
                        &mut tx,
                        step.target_id,
                        &step.target_code,
                        step.target_version,
                        step.preconditions.clone(),
                    )
                    .await?,
                    Vec::new(),
                )
            }
        } else {
            match step.resource_kind {
                PlanResourceKind::Blueprint => {
                    let payload: BlueprintPayload = serde_json::from_value(
                        step.normalized_payload.clone().ok_or_else(|| {
                            RepositoryError::InvalidSolutionPackPlan(
                                "missing blueprint payload".into(),
                            )
                        })?,
                    )
                    .map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid persisted blueprint payload".into(),
                        )
                    })?;
                    if payload.version != 1 || step.target_version != Some(1) {
                        return Err(RepositoryError::InvalidSolutionPackPlan(
                            "persisted blueprint version is invalid".into(),
                        ));
                    }
                    self.revalidate_extension_contributions(
                        &mut tx,
                        &payload.extension_contributions,
                    )
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
                    let blueprint = match payload.publication {
                        BlueprintPublication::Draft => created.blueprint,
                        BlueprintPublication::Publish => {
                            let published = self
                                .publish_blueprint_in_transaction(&mut tx, step.target_id, 1)
                                .await?;
                            events.push(blueprint_event(self, BLUEPRINT_PUBLISHED_V1, &published));
                            published
                        }
                    };
                    (
                        serde_json::json!({"id":blueprint.id,"code":blueprint.code,"version":blueprint.version,"status":blueprint.status}),
                        events,
                    )
                }
                PlanResourceKind::Context
                | PlanResourceKind::PublicationChannel
                | PlanResourceKind::Rule
                | PlanResourceKind::Workflow
                | PlanResourceKind::SavedSearch => self.apply_seed_step(&mut tx, &step).await?,
                PlanResourceKind::SampleRecord => {
                    self.apply_sample_record_step(&mut tx, plan_id, &step, object_store)
                        .await?
                }
                PlanResourceKind::PresentationAsset => {
                    let payload: PresentationAssetPayload = serde_json::from_value(
                        step.normalized_payload.clone().ok_or_else(|| {
                            RepositoryError::InvalidSolutionPackPlan(
                                "missing presentation asset payload".into(),
                            )
                        })?,
                    )
                    .map_err(|_| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "invalid presentation asset payload".into(),
                        )
                    })?;
                    if payload.source_sha256.len() != SHA256_HEX_LEN
                        || payload.stored_sha256.len() != SHA256_HEX_LEN
                        || payload.byte_size <= 0
                        || payload.byte_size > MAX_PRESENTATION_ASSET_BYTES
                        || payload.source_byte_size <= 0
                        || payload.source_byte_size > MAX_PRESENTATION_ASSET_BYTES
                    {
                        return Err(RepositoryError::InvalidSolutionPackPlan(
                            "invalid presentation asset payload evidence".into(),
                        ));
                    }
                    let staged = sqlx::query_as::<_, (String, String, i64, String, String)>(
                        "SELECT object_key,media_type,byte_size,sha256,state FROM solution_pack_plan_asset_objects WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3 AND target_id=$4 FOR UPDATE",
                    )
                    .bind(workspace_id)
                    .bind(plan_id)
                    .bind(&step.logical_key)
                    .bind(step.target_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| RepositoryError::InvalidSolutionPackPlan(
                        "presentation asset staging evidence is missing".into(),
                    ))?;
                    if !matches!(staged.4.as_str(), "staged" | "claimed")
                        || staged.1 != payload.media_type
                        || staged.2 != payload.byte_size
                        || staged.3 != payload.stored_sha256
                    {
                        return Err(RepositoryError::InvalidSolutionPackPlan(
                            "presentation asset staging evidence is inconsistent".into(),
                        ));
                    }
                    verify_stored_object(
                        object_store,
                        &staged.0,
                        payload.byte_size,
                        &payload.stored_sha256,
                    )
                    .await?;
                    sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
                        .bind(step.target_id)
                        .bind(workspace_id)
                        .bind(&payload.purpose)
                        .bind(&payload.media_type)
                        .bind(payload.byte_size)
                        .bind(&payload.stored_sha256)
                        .bind(payload.width)
                        .bind(payload.height)
                        .bind(&staged.0)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='claimed',updated_at=now() WHERE workspace_id=$1 AND plan_id=$2 AND logical_key=$3")
                        .bind(workspace_id)
                        .bind(plan_id)
                        .bind(&step.logical_key)
                        .execute(&mut *tx)
                        .await?;
                    (
                        serde_json::json!({"id":step.target_id,"purpose":payload.purpose,"media_type":payload.media_type,"byte_size":payload.byte_size,"sha256":payload.stored_sha256,"source_byte_size":payload.source_byte_size,"source_sha256":payload.source_sha256}),
                        Vec::new(),
                    )
                }
                PlanResourceKind::WorkspaceSetting => {
                    self.apply_workspace_setting_step(&mut tx, application_id, &step)
                        .await?
                }
                _ => {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "unsupported persisted resource kind".into(),
                    ));
                }
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
        let workspace_id = self.workspace_id.0;
        if step.resource_kind == PlanResourceKind::SampleRecord {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(workspace_id)
            .bind(step.target_id)
            .fetch_one(&mut **tx)
            .await?;
            return if exists {
                Err(RepositoryError::SolutionPackPlanStale)
            } else {
                Ok(())
            };
        }
        if step.resource_kind == PlanResourceKind::PresentationAsset {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM presentation_assets WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(workspace_id)
            .bind(step.target_id)
            .fetch_one(&mut **tx)
            .await?;
            return if exists {
                Err(RepositoryError::SolutionPackPlanStale)
            } else {
                Ok(())
            };
        }
        if matches!(
            step.resource_kind,
            PlanResourceKind::PublicationChannel
                | PlanResourceKind::Rule
                | PlanResourceKind::Workflow
                | PlanResourceKind::SavedSearch
        ) {
            match step.resource_kind {
                PlanResourceKind::Rule => {
                    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
                        .bind(format!("rule-code:{workspace_id}:{}", step.target_code))
                        .execute(&mut **tx)
                        .await?;
                }
                PlanResourceKind::Workflow => {
                    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                        .bind(format!("workflow-code:{workspace_id}:{}", step.target_code))
                        .execute(&mut **tx)
                        .await?;
                }
                PlanResourceKind::PublicationChannel => {
                    sqlx::query("SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND id=$2 FOR UPDATE")
                        .bind(workspace_id)
                        .bind(step.target_id)
                        .execute(&mut **tx)
                        .await?;
                }
                _ => {}
            }
            let (exists, code_taken) = self
                .seed_target_state(tx, step.resource_kind, step.target_id, &step.target_code)
                .await?;
            return if exists || code_taken {
                Err(RepositoryError::SolutionPackPlanStale)
            } else {
                Ok(())
            };
        }
        if !matches!(
            step.resource_kind,
            PlanResourceKind::Blueprint | PlanResourceKind::Context
        ) {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "unsupported persisted resource kind".into(),
            ));
        }
        super::lock_workspace_resource_code(tx, workspace_id, &step.target_code).await?;
        // The workspace row comes before the table lock below, the order
        // context creation uses (see the lock order in `mod.rs`); both steps
        // advance a workspace generation later anyway.
        sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(workspace_id)
            .execute(&mut **tx)
            .await?;
        // Attribute contexts share the physical code namespace. PostgreSQL has
        // no row lock for an absent key, so hold a short table lock while the
        // blueprint insert and its cross-kind absence check commit atomically.
        sqlx::query("LOCK TABLE attribute_contexts IN SHARE MODE")
            .execute(&mut **tx)
            .await?;
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
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let (application_state, plan_id) = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT state,plan_id FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_one(&mut *tx)
        .await?;
        if stored_code::<SolutionPackApplicationState>(application_state)?
            == SolutionPackApplicationState::Completed
        {
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
        if step_state.map(stored_code).transpose()? == Some(SolutionPackStepState::Completed) {
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
            RepositoryError::SolutionPackPlanStale => {
                (SolutionPackApplicationState::Invalid, "plan_stale")
            }
            RepositoryError::SolutionPackApplicationInvalid => {
                (SolutionPackApplicationState::Invalid, "application_invalid")
            }
            RepositoryError::SolutionPackAssetObjectIntegrityFailed => (
                SolutionPackApplicationState::Invalid,
                "asset_object_integrity_failed",
            ),
            _ => (SolutionPackApplicationState::Failed, "step_failed"),
        };
        let message = bounded_diagnostic(&error.to_string());
        if state == SolutionPackApplicationState::Invalid {
            invalidate_solution_pack_application(
                &mut tx,
                workspace_id,
                application_id,
                plan_id,
                code,
                &message,
            )
            .await?;
        } else {
            sqlx::query("UPDATE solution_pack_applications SET state=$3, diagnostic_code=$4, diagnostic_message=$5, updated_at=clock_timestamp() WHERE workspace_id=$1 AND id=$2 AND state <> 'completed'")
                .bind(workspace_id).bind(application_id).bind(state.as_str()).bind(code).bind(&message).execute(&mut *tx).await?;
        }
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
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let Some(mut application) = sqlx::query_as::<_, SolutionPackApplication>(
            "SELECT id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot,readme_markdown,release_notes_markdown,setup_checklist,started_at,updated_at,completed_at,resumable_until,abandoned_at FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2",
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
        let workspace_id = self.workspace_id.0;
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
        let workspace_id = self.workspace_id.0;
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
        let workspace_id = self.workspace_id.0;
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
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let application = sqlx::query_as::<_, (Uuid, String)>("SELECT plan_id,state FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR UPDATE")
            .bind(workspace_id).bind(application_id).fetch_optional(&mut *tx).await?
            .ok_or(RepositoryError::NotFound("solution-pack application"))?;
        if stored_code::<SolutionPackApplicationState>(application.1.clone())?
            != SolutionPackApplicationState::Completed
        {
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
        if !run.results.is_empty() {
            let results = &run.results;
            sqlx::query("INSERT INTO solution_pack_check_results (run_id,workspace_id,position,check_key,title,predicate_type,passed,reason_code,summary,evidence,evaluated_at) SELECT $1,$2,r.position,r.check_key,r.title,r.predicate_type,r.passed,r.reason_code,r.summary,r.evidence,r.evaluated_at FROM unnest($3::bigint[],$4::text[],$5::text[],$6::text[],$7::bool[],$8::text[],$9::text[],$10::jsonb[],$11::timestamptz[]) WITH ORDINALITY AS r(position,check_key,title,predicate_type,passed,reason_code,summary,evidence,evaluated_at,ordinal) ORDER BY r.ordinal")
                .bind(run_id)
                .bind(workspace_id)
                .bind(results.iter().map(|result| result.position).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.key.as_str()).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.title.as_str()).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.predicate_type.as_str()).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.passed).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.reason_code.as_str()).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.summary.as_str()).collect::<Vec<_>>())
                .bind(results.iter().map(|result| &result.evidence).collect::<Vec<_>>())
                .bind(results.iter().map(|result| result.evaluated_at).collect::<Vec<_>>())
                .execute(&mut *tx)
                .await?;
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
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, SolutionPackApplicationSummary>(
            "SELECT id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,started_at,updated_at,completed_at,resumable_until,abandoned_at FROM solution_pack_applications WHERE workspace_id=$1 ORDER BY started_at DESC,id DESC LIMIT $2 OFFSET $3",
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
            let published: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM blueprints WHERE workspace_id=$1 AND code=$2 AND kind='record' AND status='published' AND deleted_at IS NULL)")
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
            let installed_ok = version_matches && state != ExtensionState::Quarantined.as_str();
            let evidence =
                serde_json::json!({"extension_id":extension_id,"version":version,"state":state});
            match predicate {
                SolutionPackCheckPredicate::ExtensionInstalled { .. } => Ok(check_outcome(
                    installed_ok,
                    "installed",
                    if !version_matches {
                        "incompatible_version"
                    } else if state == ExtensionState::Quarantined.as_str() {
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
                .unwrap_or_else(empty_workspace_extension_layout);
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
    } else if state != ExtensionState::Enabled.as_str() {
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

/// Re-validates persisted lexicon entries; plans are data, not trusted code.
fn parse_lexicon_payload(payload: Value) -> Result<Vec<attricat_lexicon::Entry>, RepositoryError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LexiconPayload {
        entries: Vec<attricat_lexicon::Entry>,
    }
    let payload: LexiconPayload = serde_json::from_value(payload).map_err(|_| {
        RepositoryError::InvalidSolutionPackPlan("persisted lexicon payload is invalid".into())
    })?;
    if payload.entries.is_empty() || payload.entries.len() > attricat_lexicon::MAX_FILE_ENTRIES {
        return Err(RepositoryError::InvalidSolutionPackPlan(
            "persisted lexicon payload is invalid".into(),
        ));
    }
    payload
        .entries
        .into_iter()
        .map(|entry| {
            entry.validated().map_err(|_| {
                RepositoryError::InvalidSolutionPackPlan(
                    "persisted lexicon payload is invalid".into(),
                )
            })
        })
        .collect()
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
        RepositoryError::AttricatCodeTaken => RepositoryError::SolutionPackPlanStale,
        RepositoryError::Database(sqlx::Error::Database(database_error))
            if database_error.is_unique_violation() =>
        {
            RepositoryError::SolutionPackPlanStale
        }
        error => error,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextPayload {
    code: String,
    data: Value,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationChannelPayload {
    context_id: Uuid,
    context_code: String,
    enabled: bool,
    // Plans made before channels carried their checks omit these.
    #[serde(default)]
    required_rule_codes: Vec<String>,
    #[serde(default)]
    require_valid_record: bool,
}

impl PublicationChannelPayload {
    fn settings(&self) -> ExistingPublicationChannel {
        ExistingPublicationChannel {
            enabled: self.enabled,
            required_rule_codes: self.required_rule_codes.clone(),
            require_valid_record: self.require_valid_record,
        }
    }

    fn result(&self) -> Value {
        serde_json::json!({
            "context_id": self.context_id,
            "context_code": self.context_code,
            "enabled": self.enabled,
            "required_rule_codes": self.required_rule_codes,
            "require_valid_record": self.require_valid_record,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulePayload {
    definition: String,
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowPayload {
    definition: String,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSearchPayload {
    name: String,
    description: Option<String>,
    visibility: String,
    state: Value,
}

fn seed_payload<T: serde::de::DeserializeOwned>(
    payload: Option<&Value>,
    label: &str,
) -> Result<T, RepositoryError> {
    payload
        .cloned()
        .and_then(|payload| serde_json::from_value(payload).ok())
        .ok_or_else(|| {
            RepositoryError::InvalidSolutionPackPlan(format!("invalid persisted {label} payload"))
        })
}

impl AttricatRepository {
    /// Workspace context ids of the pack contexts a sample record uses, from
    /// the plan's reviewed context mappings.
    async fn sample_context_ids(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan_id: Uuid,
        keys: std::collections::BTreeSet<&str>,
    ) -> Result<std::collections::BTreeMap<String, Uuid>, RepositoryError> {
        if keys.is_empty() {
            return Ok(Default::default());
        }
        let keys = keys.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let contexts = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT m.logical_key,m.target_id FROM solution_pack_plan_mappings m JOIN attribute_contexts c ON c.workspace_id=m.workspace_id AND c.id=m.target_id AND c.code=m.target_code WHERE m.workspace_id=$1 AND m.plan_id=$2 AND m.resource_kind='context' AND m.logical_key=ANY($3)",
        )
        .bind(self.workspace_id.0)
        .bind(plan_id)
        .bind(&keys)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        if contexts.len() != keys.len() {
            return Err(RepositoryError::SolutionPackPlanStale);
        }
        Ok(contexts)
    }

    /// Attaches bundled sample files to a just-created sample record through
    /// ordinary file storage. Each bundled path becomes one ordinary file the
    /// first time a sample record attaches it; later references link it.
    async fn attach_sample_files(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        plan_id: Uuid,
        record_id: Uuid,
        values: &[crate::solution_pack_sample_data::SampleFileValue],
        contexts: &std::collections::BTreeMap<String, Uuid>,
        object_store: &dyn ObjectStore,
    ) -> Result<usize, RepositoryError> {
        if values.is_empty() {
            return Ok(0);
        }
        let workspace_id = self.workspace_id.0;
        let record = self.lock_record(tx, record_id).await?;
        let default_context: Uuid = sqlx::query_scalar(
            "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND code='default'",
        )
        .bind(workspace_id)
        .fetch_one(&mut **tx)
        .await?;
        let paths = values
            .iter()
            .flat_map(|value| &value.files)
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        let staged = sqlx::query_as::<_, (String, Uuid, String, String, String, i64, String)>(
            "SELECT path,file_id,object_key,filename,media_type,byte_size,sha256 FROM solution_pack_plan_sample_files WHERE workspace_id=$1 AND plan_id=$2 AND path=ANY($3)",
        )
        .bind(workspace_id)
        .bind(plan_id)
        .bind(&paths)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|(path, file_id, object_key, filename, media_type, byte_size, sha256)| {
            (path, (file_id, object_key, filename, media_type, byte_size, sha256))
        })
        .collect::<std::collections::HashMap<_, _>>();
        let staged_ids = staged
            .values()
            .map(|(file_id, ..)| *file_id)
            .collect::<Vec<_>>();
        // Files an earlier sample record of this application already created.
        let mut created = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM files WHERE workspace_id=$1 AND id=ANY($2)",
        )
        .bind(workspace_id)
        .bind(&staged_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
        let mut attached = 0;
        for value in values {
            let code = value.attribute.rsplit('/').next().unwrap_or_default();
            let (attribute_id, _, context_editable) =
                self.file_upload_attribute(tx, &record, code).await?;
            let context_id = value
                .context
                .as_ref()
                .map_or(default_context, |context| contexts[context]);
            self.validate_context_editable(tx, Some(context_id), &context_editable)
                .await?;
            self.ensure_attribute_unlocked(tx, &record, code, context_id)
                .await?;
            let value_id: Uuid = sqlx::query_scalar(
                "INSERT INTO attribute_values (id, workspace_id, record_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(record_id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut **tx)
            .await?;
            for (position, file) in value.files.iter().enumerate() {
                let (file_id, object_key, filename, media_type, byte_size, sha256) =
                    staged.get(&file.path).cloned().ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "sample file staging evidence is missing".into(),
                        )
                    })?;
                if filename != file.filename || media_type != file.media_type {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "sample file staging evidence is inconsistent".into(),
                    ));
                }
                if created.insert(file_id) {
                    verify_stored_object(object_store, &object_key, byte_size, &sha256).await?;
                    self.insert_file_in_transaction(
                        tx,
                        file_id,
                        crate::repository::NewUploadedFile {
                            original_filename: filename.clone(),
                            display_filename: filename,
                            mime_type: media_type,
                            byte_size: byte_size as u64,
                            sha256,
                            object_key,
                        },
                        super::files::UploadClaim::Immediate,
                    )
                    .await?;
                }
                self.insert_attribute_file_reference_in_transaction(
                    tx,
                    value_id,
                    file_id,
                    position as i32,
                )
                .await?;
                attached += 1;
            }
        }
        Ok(attached)
    }

    /// A prerequisite application is completed and immutable, but it must
    /// still exist with the reviewed pack identity.
    async fn revalidate_prerequisite(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        application_id: Uuid,
        preconditions: &Value,
    ) -> Result<Value, RepositoryError> {
        let expected = preconditions
            .as_array()
            .filter(|values| values.len() == 1)
            .and_then(|values| values[0].as_object())
            .filter(|value| {
                value.get("kind").and_then(Value::as_str) == Some("prerequisite_application")
                    && value.get("application_id").and_then(Value::as_str)
                        == Some(application_id.to_string().as_str())
            })
            .ok_or_else(|| {
                RepositoryError::InvalidSolutionPackPlan(
                    "invalid prerequisite preconditions".into(),
                )
            })?;
        let actual = sqlx::query_as::<_, (String, String, String)>(
            "SELECT state,pack_id,pack_version FROM solution_pack_applications WHERE workspace_id=$1 AND id=$2 FOR SHARE",
        )
        .bind(self.workspace_id.0)
        .bind(application_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((state, pack_id, pack_version)) = actual else {
            return Err(RepositoryError::SolutionPackPlanStale);
        };
        if state != SolutionPackApplicationState::Completed.as_str()
            || expected.get("pack_id").and_then(Value::as_str) != Some(pack_id.as_str())
            || expected.get("pack_version").and_then(Value::as_str) != Some(pack_version.as_str())
        {
            return Err(RepositoryError::SolutionPackPlanStale);
        }
        Ok(serde_json::json!({
            "outcome": "reused",
            "application_id": application_id,
            "pack_id": pack_id,
            "pack_version": pack_version,
        }))
    }

    async fn revalidate_existing_context(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
        code: &str,
    ) -> Result<Value, RepositoryError> {
        let current: Option<String> = sqlx::query_scalar(
            "SELECT code FROM attribute_contexts WHERE workspace_id=$1 AND id=$2 FOR SHARE",
        )
        .bind(self.workspace_id.0)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?;
        if current.as_deref() != Some(code) {
            return Err(RepositoryError::SolutionPackPlanStale);
        }
        Ok(serde_json::json!({"outcome": "reused", "id": context_id, "code": code}))
    }

    async fn revalidate_satisfied_channel(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
        payload: Option<&Value>,
    ) -> Result<Value, RepositoryError> {
        let payload: PublicationChannelPayload = seed_payload(payload, "publication channel")?;
        if payload.context_id != context_id {
            return Err(RepositoryError::InvalidSolutionPackPlan(
                "persisted publication channel mapping does not match its payload".into(),
            ));
        }
        let current = existing_publication_channel(tx, self.workspace_id.0, context_id).await?;
        if !current.is_some_and(|current| current.same_settings(&payload.settings())) {
            return Err(RepositoryError::SolutionPackPlanStale);
        }
        let mut result = payload.result();
        result["outcome"] = "satisfied".into();
        Ok(result)
    }

    /// Whether a seed resource target exists by its planned id, and whether
    /// its planned code is taken by any resource.
    async fn seed_target_state(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        resource_kind: PlanResourceKind,
        target_id: Uuid,
        target_code: &str,
    ) -> Result<(bool, bool), RepositoryError> {
        let query = match resource_kind {
            PlanResourceKind::Rule => {
                "SELECT EXISTS(SELECT 1 FROM rules WHERE workspace_id=$1 AND id=$2), EXISTS(SELECT 1 FROM rules WHERE workspace_id=$1 AND code=$3)"
            }
            PlanResourceKind::Workflow => {
                "SELECT EXISTS(SELECT 1 FROM workflows WHERE workspace_id=$1 AND id=$2), EXISTS(SELECT 1 FROM workflows WHERE workspace_id=$1 AND code=$3)"
            }
            PlanResourceKind::SavedSearch => {
                "SELECT EXISTS(SELECT 1 FROM saved_views WHERE workspace_id=$1 AND id=$2), false AND $3 <> ''"
            }
            PlanResourceKind::PublicationChannel => {
                "SELECT EXISTS(SELECT 1 FROM publication_channels WHERE workspace_id=$1 AND context_id=$2), false AND $3 <> ''"
            }
            _ => {
                return Err(RepositoryError::InvalidSolutionPackPlan(
                    "unsupported persisted resource kind".into(),
                ));
            }
        };
        Ok(sqlx::query_as::<_, (bool, bool)>(query)
            .bind(self.workspace_id.0)
            .bind(target_id)
            .bind(target_code)
            .fetch_one(&mut **tx)
            .await?)
    }

    /// Creates one seed resource through its ordinary in-transaction path.
    async fn apply_seed_step(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        step: &PendingApplicationStep,
    ) -> Result<(Value, Vec<crate::domain_events::NewDomainEvent>), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let payload = step.normalized_payload.as_ref();
        match (step.resource_kind, step.action) {
            (PlanResourceKind::Context, PlanActionKind::Create) => {
                let payload: ContextPayload = seed_payload(payload, "context")?;
                if payload.code != step.target_code {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted context mapping does not match its payload".into(),
                    ));
                }
                let context = self
                    .create_context_in_transaction(
                        tx,
                        step.target_id,
                        CreateAttributeContext {
                            code: payload.code,
                            data: payload.data,
                            parent_id: payload.parent_id,
                        },
                    )
                    .await
                    .map_err(solution_pack_mutation_error)?;
                Ok((
                    serde_json::json!({"id": context.id, "code": context.code, "parent_id": context.parent_id}),
                    vec![super::contexts::context_event(
                        self,
                        crate::domain_events::CONTEXT_CREATED_V1,
                        &context,
                    )],
                ))
            }
            (PlanResourceKind::PublicationChannel, PlanActionKind::Satisfied) => Ok((
                self.revalidate_satisfied_channel(tx, step.target_id, payload)
                    .await?,
                Vec::new(),
            )),
            (PlanResourceKind::PublicationChannel, PlanActionKind::Create) => {
                let payload: PublicationChannelPayload =
                    seed_payload(payload, "publication channel")?;
                if payload.context_id != step.target_id {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted publication channel mapping does not match its payload".into(),
                    ));
                }
                // The plan created this channel's step only when no channel
                // existed; the precondition check rejected one created since.
                self.upsert_publication_channel_in_transaction(
                    tx,
                    payload.context_id,
                    crate::model::UpdatePublicationChannel {
                        enabled: payload.enabled,
                        required_rule_codes: Some(payload.required_rule_codes.clone()),
                        require_valid_record: Some(payload.require_valid_record),
                    },
                )
                .await
                .map_err(solution_pack_mutation_error)?;
                Ok((payload.result(), Vec::new()))
            }
            (PlanResourceKind::Rule, PlanActionKind::Create) => {
                let payload: RulePayload = seed_payload(payload, "rule")?;
                let id = self
                    .create_rule_in_transaction(
                        tx,
                        step.target_id,
                        CreateRule {
                            blueprint_id: payload.blueprint_id,
                            blueprint_version: payload.blueprint_version,
                            context_id: payload.context_id,
                            definition: payload.definition,
                        },
                    )
                    .await
                    .map_err(solution_pack_mutation_error)?;
                let code: String = sqlx::query_scalar(
                    "SELECT code FROM rules WHERE workspace_id=$1 AND id=$2 AND version=1",
                )
                .bind(workspace_id)
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
                if code != step.target_code {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted rule mapping does not match its payload".into(),
                    ));
                }
                self.publish_rule_in_transaction(tx, id, 1).await?;
                if payload.enabled {
                    // The ordinary enable gate: planning defers enforcing
                    // rules on blueprints that may already have records.
                    self.ensure_rule_enable_allowed(tx, id, 1, false)
                        .await
                        .map_err(solution_pack_mutation_error)?;
                    self.enable_rule_in_transaction(tx, id, 1).await?;
                }
                Ok((
                    serde_json::json!({"id": id, "code": code, "version": 1, "status": "published", "enabled": payload.enabled}),
                    Vec::new(),
                ))
            }
            (PlanResourceKind::Workflow, PlanActionKind::Create) => {
                let payload: WorkflowPayload = seed_payload(payload, "workflow")?;
                let id = self
                    .create_workflow_in_transaction(
                        tx,
                        step.target_id,
                        CreateWorkflow {
                            definition: payload.definition,
                        },
                    )
                    .await
                    .map_err(solution_pack_mutation_error)?;
                let code: String = sqlx::query_scalar(
                    "SELECT code FROM workflows WHERE workspace_id=$1 AND id=$2 AND version=1",
                )
                .bind(workspace_id)
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
                if code != step.target_code {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "persisted workflow mapping does not match its payload".into(),
                    ));
                }
                self.publish_workflow_revision_in_transaction(tx, id, 1)
                    .await?;
                if payload.enabled {
                    self.enable_workflow_revision_in_transaction(tx, id, 1)
                        .await?;
                }
                Ok((
                    serde_json::json!({"id": id, "code": code, "version": 1, "status": "published", "enabled": payload.enabled}),
                    Vec::new(),
                ))
            }
            (PlanResourceKind::SavedSearch, PlanActionKind::Create) => {
                let payload: SavedSearchPayload = seed_payload(payload, "saved search")?;
                if payload.visibility != "workspace" || !payload.state.is_object() {
                    return Err(RepositoryError::InvalidSolutionPackPlan(
                        "invalid persisted saved search payload".into(),
                    ));
                }
                // A workspace saved search is owned by the person applying the
                // seed, like any search they share.
                let owner = self
                    .audit_context
                    .as_ref()
                    .and_then(|context| context.actor_user_id)
                    .ok_or_else(|| {
                        RepositoryError::InvalidSolutionPackPlan(
                            "a saved search can only be seeded by a signed-in user or personal token".into(),
                        )
                    })?;
                // Validated and normalized like every other saved-search write.
                attricat_validation::saved_search::validate_state(
                    attricat_validation::saved_search::EXPLORER_SEARCH_KIND,
                    &payload.state,
                )
                .map_err(|reason| {
                    RepositoryError::InvalidSolutionPackPlan(format!(
                        "invalid persisted saved search state: {reason}"
                    ))
                })?;
                let state = attricat_validation::saved_search::normalize_state(&payload.state);
                self.create_saved_view_in_transaction(
                    tx,
                    step.target_id,
                    &super::saved_views::NewSavedView {
                        owner_user_id: owner,
                        name: Some(&payload.name),
                        description: payload.description.as_deref(),
                        visibility: "workspace",
                        state: &state,
                    },
                )
                .await?;
                Ok((
                    serde_json::json!({"id": step.target_id, "name": payload.name, "visibility": "workspace", "owner_user_id": owner}),
                    Vec::new(),
                ))
            }
            _ => Err(RepositoryError::InvalidSolutionPackPlan(
                "unsupported persisted seed step".into(),
            )),
        }
    }
}

fn bounded_diagnostic(message: &str) -> String {
    message.chars().take(MAX_STEP_DIAGNOSTIC_CHARS).collect()
}

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == SHA256_HEX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    pub async fn solution_pack_housekeeping_workspaces(
        &self,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT id FROM workspaces WHERE deleted_at IS NULL ORDER BY id")
                .fetch_all(&self.pool)
                .await?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

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

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn failed_navigation_step_revalidates_and_resumes(pool: sqlx::PgPool) {
        let workspace_id = AttricatRepository::DEFAULT_WORKSPACE_ID;
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let blueprint_id = Uuid::new_v4();
        publish_name_blueprint(
            &repository,
            &pool,
            blueprint_id,
            "retry_product",
            "Retry product",
        )
        .await;

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
        let mut seal = pool.begin().await.unwrap();
        seal_plan_resource_evidence(&mut seal, workspace_id, plan_id)
            .await
            .unwrap();
        seal.commit().await.unwrap();
        let request_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,diagnostic_code,diagnostic_message,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,'attricat.retry-navigation','1.0.0','publish','failed','step_failed','synthetic transient failure','[]'::jsonb)")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(request_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,state,diagnostic_code,diagnostic_message) VALUES ($1,$2,0,$3,'workspace_setting',$4,$2,'explore_navigation','failed','step_failed','synthetic transient failure')")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(logical_key).execute(&pool).await.unwrap();

        let object_store = crate::storage::FakeObjectStore::available();
        let application = repository
            .apply_solution_pack_plan(plan_id, &object_store)
            .await
            .unwrap();
        assert_eq!(application.state, SolutionPackApplicationState::Completed);
        assert_eq!(application.steps[0].state, SolutionPackStepState::Completed);
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

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn application_insertion_plan_lock_wins_against_expired_staging_cleanup(
        pool: sqlx::PgPool,
    ) {
        let workspace_id = AttricatRepository::DEFAULT_WORKSPACE_ID;
        let plan_id = Uuid::new_v4();
        let object_key = format!("presentation-assets/{plan_id}");
        sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,created_at,expires_at) VALUES ($1,$2,'local_archive','{\"side_loaded\":true}'::jsonb,$3,1,$4,'Cleanup race','1.0.0','Cleanup race','^1.0','cleanup_race','draft',true,clock_timestamp()-interval '2 hours',clock_timestamp()-interval '1 hour')")
            .bind(plan_id)
            .bind(workspace_id)
            .bind("0".repeat(64))
            .bind(format!("attricat.cleanup-race.{plan_id}"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO solution_pack_plan_asset_objects (plan_id,workspace_id,logical_key,target_id,object_key,media_type,byte_size,sha256,state) VALUES ($1,$2,'assets/logo',$3,$4,'image/svg+xml',1,$5,'staged')")
            .bind(plan_id)
            .bind(workspace_id)
            .bind(Uuid::new_v4())
            .bind(&object_key)
            .bind(format!("{:x}", Sha256::digest(b"x")))
            .execute(&pool)
            .await
            .unwrap();
        let object_store = std::sync::Arc::new(crate::storage::FakeObjectStore::available());
        object_store
            .put(
                &object_key,
                StoredObject {
                    bytes: bytes::Bytes::from_static(b"x"),
                    content_type: Some("image/svg+xml".to_owned()),
                },
            )
            .await
            .unwrap();

        // Model application preparation precisely: acquire the plan lock first, then
        // allow cleanup to issue its competing plan/staging lock query.
        let mut application_tx = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM solution_pack_plans WHERE id=$1 FOR UPDATE")
            .bind(plan_id)
            .fetch_one(&mut *application_tx)
            .await
            .unwrap();
        let cleanup_repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let cleanup_store = object_store.clone();
        let cleanup = tokio::spawn(async move {
            cleanup_repository
                .reconcile_solution_pack_asset_cleanup(cleanup_store.as_ref())
                .await
        });

        // `SKIP LOCKED` makes reconciliation complete while the preparation
        // transaction still owns the plan lock. Awaiting it here is the deterministic
        // barrier: cleanup has inspected candidates before application insertion continues.
        cleanup.await.unwrap().unwrap();
        assert_eq!(object_store.object_count().await, 1);
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM solution_pack_plan_asset_objects WHERE plan_id=$1"
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "staged"
        );

        let application_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot) VALUES ($1,$2,$3,$4,$4,'local_archive','{\"side_loaded\":true}'::jsonb,$5,$6,'1.0.0','draft','running','[]'::jsonb)")
            .bind(application_id)
            .bind(workspace_id)
            .bind(plan_id)
            .bind(request_id)
            .bind("0".repeat(64))
            .bind(format!("attricat.cleanup-race.{plan_id}"))
            .execute(&mut *application_tx)
            .await
            .unwrap();
        application_tx.commit().await.unwrap();

        assert_eq!(object_store.object_count().await, 1);
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM solution_pack_plan_asset_objects WHERE plan_id=$1"
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "staged"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_applications WHERE plan_id=$1"
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn ambiguous_commit_reconciliation_never_fails_a_later_pending_step(pool: sqlx::PgPool) {
        let workspace_id = AttricatRepository::DEFAULT_WORKSPACE_ID;
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
            let logical_key = format!("blueprints/{position}");
            let target_id = Uuid::new_v4();
            let target_code = format!("reconcile_{position}");
            sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,$3,'blueprint',$4,$5,$6,'create','{}'::jsonb)")
                .bind(plan_id).bind(workspace_id).bind(position).bind(&logical_key).bind(target_id).bind(&target_code).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,$3,'blueprint',$4,'create','target_absent','{}'::jsonb,'{}'::jsonb,$5)")
                .bind(plan_id).bind(workspace_id).bind(position).bind(&logical_key).bind(serde_json::json!([{"kind":"target_absent","resource_kind":"blueprint","code":target_code}])).execute(&pool).await.unwrap();
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
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
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

    /// Creates and publishes version 1 of a record blueprint with one string
    /// `name` attribute.
    async fn publish_name_blueprint(
        repository: &AttricatRepository,
        pool: &PgPool,
        blueprint_id: Uuid,
        code: &str,
        name: &str,
    ) {
        let mut tx = pool.begin().await.unwrap();
        repository
            .create_blueprint_in_transaction(
                &mut tx,
                blueprint_id,
                CreateBlueprint {
                    definition: format!(
                        r#"
format_version = 1
code = "{code}"
name = "{name}"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
                    ),
                },
            )
            .await
            .unwrap();
        repository
            .publish_blueprint_in_transaction(&mut tx, blueprint_id, 1)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    async fn insert_sample_lifecycle_fixture(pool: &PgPool, expired: bool) -> (Uuid, Uuid, Uuid) {
        let workspace_id = AttricatRepository::DEFAULT_WORKSPACE_ID;
        let plan_id = Uuid::new_v4();
        let application_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let blueprint_id = Uuid::new_v4();
        let blueprint_code = format!("lifecycle_{}", plan_id.simple());
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        publish_name_blueprint(
            &repository,
            pool,
            blueprint_id,
            &blueprint_code,
            "Lifecycle fixture",
        )
        .await;
        let started_at = if expired {
            Utc::now() - Duration::days(31)
        } else {
            Utc::now()
        };
        let resumable_until = started_at + Duration::days(30);
        sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at,sample_data_selected,sample_declaration_sha256,sample_record_count,sample_automation_warning) VALUES ($1,$2,'local_archive','{}'::jsonb,$3,1,$4,'Lifecycle','1.0.0','Lifecycle fixture','^1.0','lifecycle','publish',true,clock_timestamp()+interval '24 hours',true,$5,1,$6)")
            .bind(plan_id).bind(workspace_id).bind("0".repeat(64)).bind(format!("attricat.lifecycle.{plan_id}")).bind("1".repeat(64)).bind(crate::solution_pack_sample_data::SAMPLE_AUTOMATION_WARNING).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_plan_mappings (plan_id,workspace_id,position,resource_kind,logical_key,target_id,target_code,mapping_kind,snapshot) VALUES ($1,$2,0,'sample_record','sample-records/item',$3,'sample_record','create','{}'::jsonb)")
            .bind(plan_id).bind(workspace_id).bind(target_id).execute(pool).await.unwrap();
        sqlx::query(r#"INSERT INTO solution_pack_plan_actions (plan_id,workspace_id,position,resource_kind,logical_key,action,reason_code,summary,normalized_payload,preconditions) VALUES ($1,$2,0,'sample_record','sample-records/item','create','sample_selected','{}'::jsonb,'{}'::jsonb,'[{"kind":"target_absent","resource_kind":"sample_record","code":"sample_record"}]'::jsonb)"#)
            .bind(plan_id).bind(workspace_id).execute(pool).await.unwrap();
        let canonical_input = serde_json::json!({
            "record": {
                "key": "sample-records/item",
                "blueprint": "blueprints/item",
                "facts": [{"attribute":"blueprints/item/attributes/name","value":"Sample lifecycle value"}],
                "relationships": []
            },
            "effective_defaults": []
        });
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&canonical_input).unwrap())
        );
        let mut tx = pool.begin().await.unwrap();
        SampleRecordRows {
            positions: vec![0],
            logical_keys: vec!["sample-records/item".to_owned()],
            target_ids: vec![target_id],
            blueprint_logical_keys: vec!["blueprints/item".to_owned()],
            blueprint_ids: vec![blueprint_id],
            blueprint_versions: vec![1],
            digests: vec![digest],
            scalar_counts: vec![1],
            relationship_target_counts: vec![0],
            canonical_inputs: vec![canonical_input],
        }
        .insert(&mut tx, workspace_id, plan_id)
        .await
        .unwrap();
        seal_plan_resource_evidence(&mut tx, workspace_id, plan_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot,started_at,resumable_until) VALUES ($1,$2,$3,$4,$4,'local_archive','{}'::jsonb,$5,$6,'1.0.0','publish','running','[]'::jsonb,$7,$8)")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(Uuid::new_v4()).bind("0".repeat(64)).bind(format!("attricat.lifecycle.{plan_id}")).bind(started_at).bind(resumable_until).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO solution_pack_application_steps (application_id,workspace_id,position,plan_id,resource_kind,logical_key,target_id,target_code,state) VALUES ($1,$2,0,$3,'sample_record','sample-records/item',$4,'sample_record','pending')")
            .bind(application_id).bind(workspace_id).bind(plan_id).bind(target_id).execute(pool).await.unwrap();
        (plan_id, application_id, target_id)
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn sample_retryable_failure_retains_staging_and_resumes_exactly_once(pool: PgPool) {
        let (plan_id, application_id, target_id) =
            insert_sample_lifecycle_fixture(&pool, false).await;
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        assert!(
            !repository
                .record_solution_pack_failure(
                    application_id,
                    Some(0),
                    &RepositoryError::Database(sqlx::Error::PoolClosed),
                )
                .await
                .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM solution_pack_applications WHERE id=$1",
            )
            .bind(application_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            "failed"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );

        let object_store = crate::storage::FakeObjectStore::available();
        let application = repository
            .apply_solution_pack_plan(plan_id, &object_store)
            .await
            .unwrap();
        assert_eq!(application.state, SolutionPackApplicationState::Completed);
        let retried = repository
            .apply_solution_pack_plan(plan_id, &object_store)
            .await
            .unwrap();
        assert_eq!(retried.state, SolutionPackApplicationState::Completed);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM records WHERE id=$1")
                .bind(target_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM domain_events WHERE aggregate_id=$1 AND event_type='record.created.v1'",
            )
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn sample_ambiguous_commit_reconciles_without_duplicate_side_effects(pool: PgPool) {
        let (plan_id, application_id, target_id) =
            insert_sample_lifecycle_fixture(&pool, false).await;
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let object_store = crate::storage::FakeObjectStore::available();
        let mut attempted_position = None;
        assert!(
            repository
                .apply_next_solution_pack_step_inner(
                    application_id,
                    &mut attempted_position,
                    &object_store,
                )
                .await
                .unwrap()
        );
        assert_eq!(attempted_position, Some(0));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1,
            "nonterminal committed step must retain staging for reconciliation"
        );
        assert!(
            repository
                .record_solution_pack_failure(
                    application_id,
                    Some(0),
                    &RepositoryError::Database(sqlx::Error::PoolClosed),
                )
                .await
                .unwrap()
        );
        let application = repository
            .apply_solution_pack_plan(plan_id, &object_store)
            .await
            .unwrap();
        assert_eq!(application.state, SolutionPackApplicationState::Completed);
        assert_eq!(application.steps[0].state, SolutionPackStepState::Completed);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM records WHERE id=$1")
                .bind(target_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM domain_events WHERE aggregate_id=$1 AND event_type='record.created.v1'",
            )
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn manual_abandonment_is_terminal_and_scrubs_staging(pool: PgPool) {
        let (plan_id, application_id, _) = insert_sample_lifecycle_fixture(&pool, false).await;
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let application = repository
            .abandon_solution_pack_application(application_id)
            .await
            .unwrap();
        assert_eq!(application.state, SolutionPackApplicationState::Abandoned);
        assert_eq!(application.steps[0].state, SolutionPackStepState::Pending);
        assert!(application.abandoned_at.is_some());
        let staged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(staged, 0);
        assert!(matches!(
            repository.prepare_solution_pack_application(plan_id).await,
            Err(RepositoryError::SolutionPackApplicationInvalid)
        ));
        assert_eq!(
            repository
                .abandon_solution_pack_application(application_id)
                .await
                .unwrap()
                .state,
            SolutionPackApplicationState::Abandoned
        );
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn hourly_cleanup_auto_abandons_and_purges_expired_staging(pool: PgPool) {
        let (plan_id, application_id, _) = insert_sample_lifecycle_fixture(&pool, true).await;
        let (unstarted_plan_id, unstarted_application_id, _) =
            insert_sample_lifecycle_fixture(&pool, false).await;
        sqlx::query("DELETE FROM solution_pack_application_steps WHERE application_id=$1")
            .bind(unstarted_application_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM solution_pack_applications WHERE id=$1")
            .bind(unstarted_application_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE solution_pack_plans SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 hour' WHERE id=$1")
            .bind(unstarted_plan_id).execute(&pool).await.unwrap();
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        repository
            .cleanup_solution_pack_sample_staging()
            .await
            .unwrap();
        let (state, diagnostic, abandoned_at) = sqlx::query_as::<
            _,
            (String, Option<String>, Option<DateTime<Utc>>),
        >(
            "SELECT state,diagnostic_code,abandoned_at FROM solution_pack_applications WHERE id=$1",
        )
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, "abandoned");
        assert_eq!(diagnostic.as_deref(), Some("resumability_expired"));
        assert!(abandoned_at.is_some());
        let staged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(staged, 0);
        let unstarted_staged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
        )
        .bind(unstarted_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unstarted_staged, 0);
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn retry_staleness_atomically_invalidates_and_scrubs_sample_staging(pool: PgPool) {
        for extension_drift in [false, true] {
            let (plan_id, application_id, _) = insert_sample_lifecycle_fixture(&pool, false).await;
            if extension_drift {
                sqlx::query("INSERT INTO solution_pack_plan_extension_requirements (plan_id,workspace_id,position,logical_key,extension_id,version_requirement,required,status,reason_code,installed_release_id,installed_version,installed_state,configuration_matches,evaluation_template) VALUES ($1,$2,0,'extensions/drift','example.drift','^1.0',true,'satisfied','satisfied',$3,'1.0.0','enabled',true,'{}'::jsonb)")
                    .bind(plan_id)
                    .bind(AttricatRepository::DEFAULT_WORKSPACE_ID)
                    .bind(Uuid::new_v4())
                    .execute(&pool)
                    .await
                    .unwrap();
                // Reviewed evidence: the drift is in the workspace, not the plan.
                let mut seal = pool.begin().await.unwrap();
                seal_plan_resource_evidence(
                    &mut seal,
                    AttricatRepository::DEFAULT_WORKSPACE_ID,
                    plan_id,
                )
                .await
                .unwrap();
                seal.commit().await.unwrap();
            } else {
                sqlx::query("UPDATE solution_pack_application_steps SET state='completed',result_snapshot='{}'::jsonb,completed_at=clock_timestamp() WHERE application_id=$1")
                    .bind(application_id).execute(&pool).await.unwrap();
            }
            let repository =
                AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
            let result = repository.prepare_solution_pack_application(plan_id).await;
            assert!(
                matches!(result, Err(RepositoryError::SolutionPackPlanStale)),
                "unexpected retry result: {result:?}"
            );
            let state: String =
                sqlx::query_scalar("SELECT state FROM solution_pack_applications WHERE id=$1")
                    .bind(application_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(state, "invalid");
            let staged: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
            )
            .bind(plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(staged, 0);
        }
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn sample_expiry_cleanup_skips_a_plan_locked_for_application_start(pool: PgPool) {
        let (plan_id, application_id, _) = insert_sample_lifecycle_fixture(&pool, false).await;
        sqlx::query("DELETE FROM solution_pack_application_steps WHERE application_id=$1")
            .bind(application_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM solution_pack_applications WHERE id=$1")
            .bind(application_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE solution_pack_plans SET created_at=clock_timestamp()-interval '2 hours',expires_at=clock_timestamp()-interval '1 second' WHERE id=$1")
            .bind(plan_id).execute(&pool).await.unwrap();

        let mut prepare = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM solution_pack_plans WHERE id=$1 FOR UPDATE")
            .bind(plan_id)
            .execute(&mut *prepare)
            .await
            .unwrap();
        let cleanup_repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let cleanup = tokio::spawn(async move {
            cleanup_repository
                .cleanup_solution_pack_sample_staging()
                .await
        });
        cleanup.await.unwrap().unwrap();
        let request_id = Uuid::new_v4();
        sqlx::query("INSERT INTO solution_pack_applications (id,workspace_id,plan_id,request_id,correlation_id,source_kind,source_metadata,archive_sha256,pack_id,pack_version,blueprint_publication,state,mapping_snapshot,resumable_until) VALUES ($1,$2,$3,$4,$4,'local_archive','{}'::jsonb,$5,$6,'1.0.0','publish','running','[]'::jsonb,clock_timestamp()+interval '30 days')")
            .bind(application_id).bind(AttricatRepository::DEFAULT_WORKSPACE_ID).bind(plan_id).bind(request_id).bind("0".repeat(64)).bind(format!("attricat.lifecycle.{plan_id}")).execute(&mut *prepare).await.unwrap();
        prepare.commit().await.unwrap();
        let staged: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM solution_pack_plan_sample_records WHERE plan_id=$1",
        )
        .bind(plan_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(staged, 1);
    }

    #[sqlx::test(migrations = "../../apps/api/migrations")]
    async fn abandonment_uses_application_lock_and_step_timeouts(pool: PgPool) {
        let (_, application_id, _) = insert_sample_lifecycle_fixture(&pool, false).await;
        let mut timeout_tx = pool.begin().await.unwrap();
        set_solution_pack_transaction_timeouts(&mut timeout_tx)
            .await
            .unwrap();
        let lock_timeout: String = sqlx::query_scalar("SHOW lock_timeout")
            .fetch_one(&mut *timeout_tx)
            .await
            .unwrap();
        let statement_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
            .fetch_one(&mut *timeout_tx)
            .await
            .unwrap();
        assert_eq!(lock_timeout, "5min");
        assert_eq!(statement_timeout, "5min");
        timeout_tx.rollback().await.unwrap();

        let mut active_step = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM solution_pack_applications WHERE id=$1 FOR UPDATE")
            .bind(application_id)
            .execute(&mut *active_step)
            .await
            .unwrap();
        let repository =
            AttricatRepository::new(pool.clone(), AttricatRepository::DEFAULT_WORKSPACE_ID);
        let abandon = tokio::spawn(async move {
            repository
                .abandon_solution_pack_application(application_id)
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !abandon.is_finished(),
            "abandonment bypassed the application lock"
        );
        active_step.rollback().await.unwrap();
        assert_eq!(
            abandon.await.unwrap().unwrap().state,
            SolutionPackApplicationState::Abandoned
        );
    }
}
