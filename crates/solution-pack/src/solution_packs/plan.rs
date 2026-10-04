//! Planning: a validated pack and a snapshot of workspace facts become a
//! reviewable plan of mappings and actions. Planning never reads or writes the
//! workspace itself.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use catalog_blueprint::BlueprintKind;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    MAX_IDENTIFIER_BYTES, MAX_SOLUTION_PACK_BLUEPRINTS, MAX_SOLUTION_PACK_PREFIX_BYTES,
    MAX_SOLUTION_PACK_PRESENTATION_ASSETS, SolutionPackBlueprint, SolutionPackError,
    SolutionPackExploreNavigation, SolutionPackExtensionLayout, SolutionPackExtensionRequirement,
    SolutionPackManifest, SolutionPackResource, ValidatedSolutionPack,
    blueprints::visit_embedded_predicate_blueprint_codes, guidance::json_deep_contains, invalid,
    is_valid_stable_code, parse_version_req, resource_code,
};
use crate::{
    extensions::{ExtensionLayoutPlacement, classify_extension_layout_placement},
    solution_pack_seeds::{
        DependentPlanInput, SeedWorkspaceSnapshot, plan_contexts, plan_dependents,
        plan_prerequisites,
    },
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintPublication {
    Draft,
    Publish,
}

impl BlueprintPublication {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Publish => "publish",
        }
    }
}

#[derive(Clone, Debug)]
pub struct InstalledExtensionSnapshot {
    pub installed_release_id: uuid::Uuid,
    pub version: String,
    pub state: String,
    pub configuration: Value,
    pub policy_compatible: bool,
    /// Stable contribution key to its manifest-declared outlet.
    pub contributions: BTreeMap<String, String>,
    /// An official-registry release that applying the plan will install,
    /// configure, grant, and enable. Its `installed_release_id` is reserved for
    /// that installation and `configuration` is the configuration it receives.
    pub pending_install: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanningExploreNavigationEntry {
    pub blueprint_code: String,
    pub visible_to_role_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlueprintMappingRequest {
    pub key: String,
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentationAssetMappingRequest {
    pub key: String,
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct ExistingPresentationAssetSnapshot {
    pub id: uuid::Uuid,
    pub purpose: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct ExistingBlueprintSnapshot {
    pub id: uuid::Uuid,
    pub code: String,
    pub version: i64,
    pub kind: String,
    /// Hash of the stored definition after the same TOML parse/serialize
    /// canonicalization used for normalized pack definitions.
    pub canonical_definition_hash: String,
    /// Raw stored-source hash retained as immutable stale evidence.
    pub definition_hash: String,
}

#[derive(Debug)]
pub struct PlanningWorkspaceSnapshot {
    pub workspace_id: uuid::Uuid,
    pub physical_codes: BTreeSet<String>,
    /// Explicit selections keyed by pack-local blueprint key. Missing targets
    /// are rejected before the planner is called; the planner never searches.
    pub existing_blueprints: BTreeMap<String, ExistingBlueprintSnapshot>,
    pub existing_presentation_assets: BTreeMap<String, ExistingPresentationAssetSnapshot>,
    pub installed_extensions: BTreeMap<String, InstalledExtensionSnapshot>,
    pub explore_navigation: Vec<PlanningExploreNavigationEntry>,
    pub explore_navigation_valid: bool,
    pub extension_layout: Value,
    pub extension_layout_valid: bool,
    pub role_codes: BTreeSet<String>,
    pub published_entity_codes: BTreeSet<String>,
    pub seed: SeedWorkspaceSnapshot,
}

/// The kind of resource a plan maps or acts on. Persisted as the
/// `resource_kind` of plan mappings, plan actions and application steps.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PlanResourceKind {
    Blueprint,
    PresentationAsset,
    WorkspaceSetting,
    Prerequisite,
    Context,
    PublicationChannel,
    Rule,
    Workflow,
    SavedSearch,
    SampleEntity,
}

impl PlanResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blueprint => "blueprint",
            Self::PresentationAsset => "presentation_asset",
            Self::WorkspaceSetting => "workspace_setting",
            Self::Prerequisite => "prerequisite",
            Self::Context => "context",
            Self::PublicationChannel => "publication_channel",
            Self::Rule => "rule",
            Self::Workflow => "workflow",
            Self::SavedSearch => "saved_search",
            Self::SampleEntity => "sample_entity",
        }
    }
}

/// How a plan mapping chooses its workspace target.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MappingKind {
    /// An existing workspace resource selected explicitly or by a prerequisite.
    Existing,
    /// A resource the plan creates under a generated identity.
    Create,
    /// A workspace-wide setting.
    Workspace,
    /// A sample entity an earlier application created.
    Reuse,
}

impl MappingKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Create => "create",
            Self::Workspace => "workspace",
            Self::Reuse => "reuse",
        }
    }
}

/// What applying a plan does with one resource. Persisted as the `action` of
/// plan actions.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanActionKind {
    Create,
    Map,
    Append,
    Satisfied,
    Skip,
    Conflict,
    Blocked,
}

impl PlanActionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Map => "map",
            Self::Append => "append",
            Self::Satisfied => "satisfied",
            Self::Skip => "skip",
            Self::Conflict => "conflict",
            Self::Blocked => "blocked",
        }
    }

    /// Whether a plan containing this action can be applied.
    pub const fn is_applicable(self) -> bool {
        !matches!(self, Self::Conflict | Self::Blocked)
    }

    /// Whether the resource's target exists once the plan is applied, so
    /// other resources can depend on it.
    pub const fn provides_target(self) -> bool {
        matches!(self, Self::Create | Self::Map)
    }
}

/// A planned action and its reason code.
pub(crate) type Outcome = (PlanActionKind, &'static str);

#[derive(Clone, Debug)]
pub struct PlannedMapping {
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    pub target_id: uuid::Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    pub mapping_kind: MappingKind,
    pub snapshot: Value,
}

#[derive(Clone, Debug)]
pub struct PlannedAction {
    pub resource_kind: PlanResourceKind,
    pub logical_key: String,
    pub action: PlanActionKind,
    pub reason_code: &'static str,
    pub summary: Value,
    pub normalized_payload: Option<Value>,
    pub preconditions: Value,
}

#[derive(Clone, Debug)]
pub struct PlannedExtensionRequirement {
    pub logical_key: String,
    pub extension_id: String,
    pub version_requirement: String,
    pub required: bool,
    pub configuration_template_path: Option<String>,
    pub configuration_template_sha256: Option<String>,
    pub status: &'static str,
    pub reason_code: &'static str,
    pub installed_release_id: Option<uuid::Uuid>,
    pub installed_version: Option<String>,
    pub installed_state: Option<String>,
    pub configuration_matches: Option<bool>,
    pub evaluation_template: Value,
}

#[derive(Debug)]
pub struct SolutionPackPlanDraft {
    pub ready: bool,
    pub mappings: Vec<PlannedMapping>,
    pub actions: Vec<PlannedAction>,
    pub extension_requirements: Vec<PlannedExtensionRequirement>,
    /// Canonical physical definitions computed from the current archive and
    /// this plan's exact logical-key mappings. This is repository-only release
    /// evidence and is not itself persisted as an executable payload.
    pub blueprint_canonical_definition_hashes: BTreeMap<String, String>,
}

impl SolutionPackPlanDraft {
    /// Whether every action and required extension allows applying the plan.
    pub fn is_applicable(&self) -> bool {
        self.actions
            .iter()
            .all(|action| action.action.is_applicable())
            && self
                .extension_requirements
                .iter()
                .all(|requirement| requirement.status != "blocked")
    }
}

pub fn evaluate_extension_requirement(
    requirement: &SolutionPackExtensionRequirement,
    template: Option<&Value>,
    installed: Option<&InstalledExtensionSnapshot>,
) -> PlannedExtensionRequirement {
    let configuration_matches = installed.map(|installed| {
        template.is_none_or(|template| json_deep_contains(&installed.configuration, template))
    });
    let reason_code = match installed {
        None => "missing",
        Some(installed)
            if Version::parse(&installed.version).map_or(true, |version| {
                !parse_version_req(&requirement.version)
                    .expect("requirement version validated")
                    .matches(&version)
            }) =>
        {
            "incompatible_version"
        }
        Some(installed) if installed.state == "quarantined" => "quarantined",
        Some(installed) if !installed.policy_compatible => "policy_incompatible",
        Some(_) if configuration_matches != Some(true) => "configuration_mismatch",
        Some(_) => "satisfied",
    };
    let pending_install = installed.is_some_and(|installed| installed.pending_install);
    let reason_code = if pending_install && reason_code == "satisfied" {
        "install"
    } else {
        reason_code
    };
    // A pending release is not installed yet, so it reports no installed state.
    let installed = installed.filter(|installed| !installed.pending_install);
    let configuration_matches = if pending_install {
        None
    } else {
        configuration_matches
    };
    let status = if matches!(reason_code, "satisfied" | "install") {
        reason_code
    } else if requirement.required {
        "blocked"
    } else {
        "skipped"
    };
    PlannedExtensionRequirement {
        logical_key: requirement.key.clone(),
        extension_id: requirement.id.clone(),
        version_requirement: requirement.version.clone(),
        required: requirement.required,
        configuration_template_path: requirement
            .configuration_template
            .as_ref()
            .map(|reference| reference.path.clone()),
        configuration_template_sha256: requirement
            .configuration_template
            .as_ref()
            .map(|reference| reference.sha256.clone()),
        status,
        reason_code,
        installed_release_id: installed.map(|installed| installed.installed_release_id),
        installed_version: installed.map(|installed| installed.version.clone()),
        installed_state: installed.map(|installed| installed.state.clone()),
        configuration_matches,
        evaluation_template: template.cloned().unwrap_or_else(|| serde_json::json!({})),
    }
}

fn extension_requirement_for_contribution<'a>(
    manifest: &'a SolutionPackManifest,
    contribution: &str,
) -> &'a SolutionPackExtensionRequirement {
    let extension_id = contribution
        .split_once(':')
        .map(|(extension_id, _)| extension_id)
        .expect("validated contribution key");
    manifest
        .extensions
        .iter()
        .find(|requirement| requirement.id == extension_id)
        .expect("validated contribution has an extension requirement")
}

fn contribution_unmet_reason(
    requirement: &SolutionPackExtensionRequirement,
    contribution: &str,
    outlet: &str,
    installed: Option<&InstalledExtensionSnapshot>,
) -> Option<&'static str> {
    let installed = installed?;
    let version_matches = Version::parse(&installed.version).is_ok_and(|version| {
        parse_version_req(&requirement.version)
            .expect("validated extension requirement")
            .matches(&version)
    });
    if !version_matches {
        return Some("incompatible_version");
    }
    if installed.state == "quarantined" {
        return Some("quarantined");
    }
    if !installed.policy_compatible {
        return Some("policy_incompatible");
    }
    match installed.contributions.get(contribution) {
        None => Some("contribution_missing"),
        Some(declared_outlet) if declared_outlet != outlet => Some("outlet_mismatch"),
        Some(_) => None,
    }
}

fn contribution_availability_reason(
    requirement: &SolutionPackExtensionRequirement,
    contribution: &str,
    outlet: &str,
    installed: Option<&InstalledExtensionSnapshot>,
) -> Option<&'static str> {
    if installed.is_none() {
        Some("missing")
    } else {
        contribution_unmet_reason(requirement, contribution, outlet, installed)
    }
}

pub fn validate_presentation_asset_mapping_requests(
    pack: &ValidatedSolutionPack,
    mappings: &[PresentationAssetMappingRequest],
) -> Result<(), SolutionPackError> {
    if mappings.len() > MAX_SOLUTION_PACK_PRESENTATION_ASSETS {
        return invalid("too many presentation asset mappings");
    }
    let declared = pack
        .manifest()
        .resources
        .presentation_assets
        .iter()
        .map(|resource| resource.key.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for mapping in mappings {
        if !declared.contains(mapping.key.as_str()) {
            return invalid(format!(
                "unknown presentation asset mapping key '{}'",
                mapping.key
            ));
        }
        if !seen.insert(mapping.key.as_str()) {
            return invalid(format!(
                "duplicate presentation asset mapping key '{}'",
                mapping.key
            ));
        }
    }
    Ok(())
}

pub fn validate_blueprint_mapping_requests(
    pack: &ValidatedSolutionPack,
    mappings: &[BlueprintMappingRequest],
) -> Result<(), SolutionPackError> {
    if mappings.len() > MAX_SOLUTION_PACK_BLUEPRINTS {
        return invalid("too many blueprint mappings");
    }
    let declared = pack
        .manifest()
        .resources
        .blueprints
        .iter()
        .map(|resource| resource.key.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for mapping in mappings {
        if !declared.contains(mapping.key.as_str()) {
            return invalid(format!("unknown blueprint mapping key '{}'", mapping.key));
        }
        if !seen.insert(mapping.key.as_str()) {
            return invalid(format!("duplicate blueprint mapping key '{}'", mapping.key));
        }
        if pack
            .manifest()
            .resources
            .blueprints
            .iter()
            .any(|resource| resource.key == mapping.key && resource.reuse.is_some())
        {
            return invalid(format!(
                "blueprint '{}' is reused from a prerequisite and cannot be mapped explicitly",
                mapping.key
            ));
        }
        if mapping.code.len() > MAX_IDENTIFIER_BYTES || !is_valid_stable_code(&mapping.code) {
            return invalid(format!(
                "invalid existing blueprint code for '{}'",
                mapping.key
            ));
        }
    }
    Ok(())
}

/// Builds a plan from an already validated local archive and an explicit set
/// of previously resolved existing-blueprint selections. No selection is ever
/// inferred from a code collision.
pub fn build_solution_pack_plan(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
) -> Result<SolutionPackPlanDraft, SolutionPackError> {
    validate_plan_prefix(prefix)?;
    let manifest = pack.manifest();

    let (prerequisite_mappings, prerequisite_actions, prerequisite_outcomes) = plan_prerequisites(
        pack,
        prefix,
        publication,
        workspace.workspace_id,
        &workspace.seed,
    );
    let mut mappings_by_key = plan_resource_mappings(pack, prefix, publication, workspace)?;
    let layouts = plan_blueprint_extension_layouts(pack, workspace);
    let mut blueprints = plan_blueprint_outcomes(
        pack,
        publication,
        workspace,
        &mappings_by_key,
        &layouts,
        &prerequisite_outcomes,
    )?;

    let created_blueprint_codes = mappings_by_key
        .values()
        .filter(|mapping| {
            mapping.resource_kind == PlanResourceKind::Blueprint
                && mapping.mapping_kind == MappingKind::Create
        })
        .map(|mapping| mapping.target_code.clone())
        .collect::<BTreeSet<_>>();
    let (context_mappings, context_actions, planned_contexts) = plan_contexts(
        pack,
        prefix,
        publication,
        workspace.workspace_id,
        &workspace.physical_codes,
        &created_blueprint_codes,
        &workspace.seed,
    )?;
    let (dependent_mappings, dependent_actions) = plan_dependents(&DependentPlanInput {
        pack,
        prefix,
        publication,
        workspace_id: workspace.workspace_id,
        seed: &workspace.seed,
        blueprint_outcomes: &blueprints.outcomes,
        blueprint_mappings: &mappings_by_key,
        contexts: &planned_contexts,
    })?;
    for mapping in prerequisite_mappings
        .into_iter()
        .chain(context_mappings)
        .chain(dependent_mappings)
    {
        mappings_by_key.insert(mapping.logical_key.clone(), mapping);
    }

    // Prerequisites and contexts come first: blueprints, samples, rules and
    // saved searches may depend on them.
    let mut actions = prerequisite_actions;
    actions.extend(context_actions);
    actions.extend(plan_blueprint_actions(
        pack,
        publication,
        workspace,
        &mappings_by_key,
        &layouts,
        &mut blueprints,
    )?);
    actions.extend(plan_presentation_asset_actions(
        pack,
        workspace,
        &mappings_by_key,
    ));

    let extension_requirements = manifest
        .extensions
        .iter()
        .map(|requirement| {
            evaluate_extension_requirement(
                requirement,
                pack.configuration_template(&requirement.key),
                workspace.installed_extensions.get(&requirement.id),
            )
        })
        .collect::<Vec<_>>();
    if let Some(navigation) = pack.explore_navigation() {
        let resource = workspace_setting_resource(manifest, EXPLORE_NAVIGATION_KEY);
        if !workspace.explore_navigation_valid {
            // The current navigation must be repaired before anything else
            // about workspace settings can be planned.
            actions.push(invalid_workspace_setting_action(
                resource,
                "explore_navigation",
                "invalid_current_navigation",
            ));
            actions.extend(dependent_actions);
            return Ok(SolutionPackPlanDraft {
                ready: false,
                mappings: mappings_by_key.into_values().collect(),
                actions,
                extension_requirements,
                blueprint_canonical_definition_hashes: blueprints.canonical_definition_hashes,
            });
        }
        actions.push(plan_explore_navigation_action(
            navigation,
            resource,
            publication,
            workspace,
            &mappings_by_key,
            &blueprints.outcomes,
        ));
    }
    if let Some(entries) = pack.lexicon() {
        actions.push(plan_lexicon_action(
            entries,
            workspace_setting_resource(manifest, LEXICON_KEY),
        ));
    }
    if let Some(layout) = pack.extension_layout() {
        let resource = workspace_setting_resource(manifest, EXTENSION_LAYOUT_KEY);
        actions.push(if workspace.extension_layout_valid {
            plan_extension_layout_action(layout, resource, manifest, workspace)
        } else {
            invalid_workspace_setting_action(
                resource,
                "extension_layout",
                "invalid_current_extension_layout",
            )
        });
    }
    actions.extend(dependent_actions);

    let mut draft = SolutionPackPlanDraft {
        ready: false,
        mappings: mappings_by_key.into_values().collect(),
        actions,
        extension_requirements,
        blueprint_canonical_definition_hashes: blueprints.canonical_definition_hashes,
    };
    draft.ready = draft.is_applicable();
    Ok(draft)
}

const EXPLORE_NAVIGATION_KEY: &str = "workspace/explore-navigation";
const LEXICON_KEY: &str = "workspace/lexicon";
const EXTENSION_LAYOUT_KEY: &str = "workspace/extension-layout";

fn validated_blueprint<'a>(
    pack: &'a ValidatedSolutionPack,
    key: &str,
) -> &'a SolutionPackBlueprint {
    pack.blueprint(key).expect("validated blueprint exists")
}

fn workspace_setting_resource<'a>(
    manifest: &'a SolutionPackManifest,
    key: &str,
) -> &'a SolutionPackResource {
    manifest
        .resources
        .workspace_settings
        .iter()
        .find(|resource| resource.key == key)
        .expect("validated workspace setting has a manifest resource")
}

/// Maps every blueprint, presentation asset and workspace setting to its
/// workspace target: an explicit selection or a generated identity.
fn plan_resource_mappings(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
) -> Result<BTreeMap<String, PlannedMapping>, SolutionPackError> {
    let manifest = pack.manifest();
    let target_id =
        |key: &str| deterministic_target_id(workspace.workspace_id, pack, prefix, publication, key);
    let mut mappings = BTreeMap::new();
    for resource in &manifest.resources.blueprints {
        let logical_key = &resource.key;
        let mapping = if let Some(existing) = workspace.existing_blueprints.get(logical_key) {
            PlannedMapping {
                resource_kind: PlanResourceKind::Blueprint,
                logical_key: logical_key.clone(),
                target_id: existing.id,
                target_code: existing.code.clone(),
                target_version: Some(existing.version),
                mapping_kind: MappingKind::Existing,
                snapshot: serde_json::json!({
                    "id": existing.id,
                    "code": existing.code,
                    "version": existing.version,
                    "kind": existing.kind,
                    "canonical_definition_hash": existing.canonical_definition_hash,
                    "definition_hash": existing.definition_hash,
                    "status": "published",
                    "deleted": false,
                }),
            }
        } else {
            let generated_code = format!("{prefix}_{}", resource_code(logical_key));
            if generated_code.len() > MAX_IDENTIFIER_BYTES || !is_valid_stable_code(&generated_code)
            {
                return invalid(format!(
                    "prefix produces an invalid physical code for '{logical_key}'"
                ));
            }
            PlannedMapping {
                resource_kind: PlanResourceKind::Blueprint,
                logical_key: logical_key.clone(),
                target_id: target_id(logical_key),
                snapshot: serde_json::json!({"code": generated_code, "version": 1}),
                target_code: generated_code,
                target_version: Some(1),
                mapping_kind: MappingKind::Create,
            }
        };
        mappings.insert(logical_key.clone(), mapping);
    }
    for resource in &manifest.resources.presentation_assets {
        let validated_asset = pack
            .presentation_asset(&resource.key)
            .expect("validated presentation asset exists");
        let existing = workspace.existing_presentation_assets.get(&resource.key);
        let (target_id, mapping_kind, snapshot) = match existing {
            Some(existing) => (
                existing.id,
                MappingKind::Existing,
                serde_json::json!({
                    "id": existing.id,
                    "purpose": existing.purpose,
                    "media_type": existing.media_type,
                    "byte_size": existing.byte_size,
                    "sha256": existing.sha256,
                    "source_sha256": validated_asset.source_sha256,
                }),
            ),
            None => (
                target_id(&resource.key),
                MappingKind::Create,
                serde_json::json!({}),
            ),
        };
        mappings.insert(
            resource.key.clone(),
            PlannedMapping {
                resource_kind: PlanResourceKind::PresentationAsset,
                logical_key: resource.key.clone(),
                target_id,
                target_code: "presentation_asset".to_owned(),
                target_version: None,
                mapping_kind,
                snapshot,
            },
        );
    }
    let settings = [
        (
            pack.explore_navigation().is_some(),
            EXPLORE_NAVIGATION_KEY,
            "explore_navigation",
        ),
        (pack.lexicon().is_some(), LEXICON_KEY, "lexicon"),
        (
            pack.extension_layout().is_some(),
            EXTENSION_LAYOUT_KEY,
            "extension_layout",
        ),
    ];
    for (_, key, setting) in settings.into_iter().filter(|(declared, ..)| *declared) {
        mappings.insert(
            key.to_owned(),
            PlannedMapping {
                resource_kind: PlanResourceKind::WorkspaceSetting,
                logical_key: key.to_owned(),
                target_id: workspace.workspace_id,
                target_code: setting.to_owned(),
                target_version: None,
                mapping_kind: MappingKind::Workspace,
                snapshot: serde_json::json!({ "setting": setting }),
            },
        );
    }
    Ok(mappings)
}

/// Per-blueprint extension contributions placed in its views.
#[derive(Default)]
struct BlueprintExtensionLayouts {
    /// Contributions available in the workspace, by blueprint key.
    allowed: HashMap<String, HashSet<String>>,
    /// Per-contribution evidence for the action summary, by blueprint key.
    evidence: HashMap<String, Vec<Value>>,
    /// The reason a required contribution blocks the blueprint, by key.
    blocked: HashMap<String, &'static str>,
}

fn plan_blueprint_extension_layouts(
    pack: &ValidatedSolutionPack,
    workspace: &PlanningWorkspaceSnapshot,
) -> BlueprintExtensionLayouts {
    let manifest = pack.manifest();
    let mut layouts = BlueprintExtensionLayouts::default();
    for resource in &manifest.resources.blueprints {
        let mut allowed = HashSet::new();
        let mut evidence = Vec::new();
        for entry in &validated_blueprint(pack, &resource.key).extension_layout {
            let requirement = extension_requirement_for_contribution(manifest, &entry.contribution);
            let reason = contribution_availability_reason(
                requirement,
                &entry.contribution,
                &entry.outlet,
                workspace.installed_extensions.get(&requirement.id),
            );
            let outcome = match reason {
                None => {
                    allowed.insert(entry.contribution.clone());
                    PlanActionKind::Satisfied
                }
                Some(_) if requirement.required => {
                    layouts
                        .blocked
                        .entry(resource.key.clone())
                        .or_insert("extension_contribution_unavailable");
                    PlanActionKind::Blocked
                }
                Some(_) => PlanActionKind::Skip,
            };
            evidence.push(serde_json::json!({
                "contribution": entry.contribution,
                "outlet": entry.outlet,
                "required": requirement.required,
                "outcome": outcome,
                "reason_code": reason.unwrap_or("satisfied"),
            }));
        }
        layouts.allowed.insert(resource.key.clone(), allowed);
        layouts.evidence.insert(resource.key.clone(), evidence);
    }
    layouts
}

/// Blueprint outcomes and the normalized definitions they were decided from.
struct PlannedBlueprints {
    outcomes: HashMap<String, Outcome>,
    /// Normalized create payloads, by blueprint key.
    payloads: HashMap<String, Value>,
    canonical_definition_hashes: BTreeMap<String, String>,
}

fn plan_blueprint_outcomes(
    pack: &ValidatedSolutionPack,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
    mappings: &BTreeMap<String, PlannedMapping>,
    layouts: &BlueprintExtensionLayouts,
    prerequisite_outcomes: &BTreeMap<String, Outcome>,
) -> Result<PlannedBlueprints, SolutionPackError> {
    let manifest = pack.manifest();
    let mut generated_code_counts = HashMap::<&str, usize>::new();
    for resource in manifest
        .resources
        .blueprints
        .iter()
        .filter(|resource| resource.required)
    {
        *generated_code_counts
            .entry(&mappings[&resource.key].target_code)
            .or_default() += 1;
    }

    let mut planned = PlannedBlueprints {
        outcomes: HashMap::new(),
        payloads: HashMap::new(),
        canonical_definition_hashes: BTreeMap::new(),
    };
    for resource in &manifest.resources.blueprints {
        let logical_key = &resource.key;
        let blueprint = validated_blueprint(pack, logical_key);
        let mapping = &mappings[logical_key];
        let normalized = normalized_blueprint_payload(
            blueprint,
            mappings,
            publication,
            &layouts.allowed[logical_key],
            workspace,
            manifest,
        )?;
        let canonical_definition_hash = catalog_blueprint::raw_hash(
            normalized["definition"]
                .as_str()
                .expect("normalized blueprint definition is a string"),
        );
        let explicitly_mapped = mapping.mapping_kind == MappingKind::Existing;
        let mapping_compatible = explicitly_mapped && {
            let existing = &workspace.existing_blueprints[logical_key];
            existing.canonical_definition_hash == canonical_definition_hash
                && existing.kind == blueprint_kind_name(blueprint.kind())
        };
        planned
            .canonical_definition_hashes
            .insert(logical_key.clone(), canonical_definition_hash);
        planned.payloads.insert(logical_key.clone(), normalized);

        let prerequisite_available = resource.reuse.as_ref().map(|reuse| {
            prerequisite_outcomes
                .get(&reuse.prerequisite)
                .is_some_and(|(action, _)| *action == PlanActionKind::Map)
        });
        let outcome = if prerequisite_available == Some(false) {
            (PlanActionKind::Blocked, "prerequisite_unavailable")
        } else if prerequisite_available == Some(true) && !explicitly_mapped {
            (
                PlanActionKind::Blocked,
                "prerequisite_blueprint_unavailable",
            )
        } else if prerequisite_available == Some(true) && !mapping_compatible {
            (
                PlanActionKind::Conflict,
                "prerequisite_blueprint_incompatible",
            )
        } else if prerequisite_available == Some(true) && !layouts.blocked.contains_key(logical_key)
        {
            (PlanActionKind::Map, "prerequisite_blueprint_match")
        } else if !resource.required && !explicitly_mapped {
            (PlanActionKind::Skip, "optional_not_selected")
        } else if let Some(reason) = layouts.blocked.get(logical_key) {
            (PlanActionKind::Blocked, *reason)
        } else if explicitly_mapped && !mapping_compatible {
            (PlanActionKind::Conflict, "existing_blueprint_incompatible")
        } else if explicitly_mapped {
            (PlanActionKind::Map, "exact_blueprint_match")
        } else if generated_code_counts[mapping.target_code.as_str()] > 1 {
            (PlanActionKind::Conflict, "duplicate_target_code")
        } else if workspace.physical_codes.contains(&mapping.target_code) {
            (PlanActionKind::Conflict, "target_code_exists")
        } else if publication == BlueprintPublication::Draft
            && !blueprint.table_path_dependencies().is_empty()
        {
            (
                PlanActionKind::Blocked,
                "draft_table_path_target_unpublished",
            )
        } else {
            (PlanActionKind::Create, "target_absent")
        };
        planned.outcomes.insert(logical_key.clone(), outcome);
    }

    // A blueprint whose dependency is not created or mapped cannot be either.
    loop {
        let newly_blocked = planned
            .outcomes
            .iter()
            .filter(|(key, (action, _))| {
                action.provides_target()
                    && validated_blueprint(pack, key)
                        .dependencies()
                        .iter()
                        .any(|dependency| !planned.outcomes[dependency].0.provides_target())
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        if newly_blocked.is_empty() {
            break;
        }
        for key in newly_blocked {
            planned
                .outcomes
                .insert(key, (PlanActionKind::Blocked, "dependency_not_creatable"));
        }
    }
    Ok(planned)
}

/// Blueprint actions in apply order.
fn plan_blueprint_actions(
    pack: &ValidatedSolutionPack,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
    mappings: &BTreeMap<String, PlannedMapping>,
    layouts: &BlueprintExtensionLayouts,
    blueprints: &mut PlannedBlueprints,
) -> Result<Vec<PlannedAction>, SolutionPackError> {
    let resources = pack
        .manifest()
        .resources
        .blueprints
        .iter()
        .map(|resource| (resource.key.as_str(), resource))
        .collect::<BTreeMap<_, _>>();

    // Includes constrain apply order. A published target is also required
    // before ordinary validation can resolve a relationship table path. Other
    // relationship references may legitimately be cyclic. Blueprints that are
    // not created already exist, so their own outbound edges impose no order.
    let ordering_dependencies = resources
        .keys()
        .map(|key| {
            let blueprint = validated_blueprint(pack, key);
            let mut dependencies = BTreeSet::new();
            if blueprints.outcomes[*key].0 == PlanActionKind::Create {
                dependencies.extend(
                    blueprint
                        .includes()
                        .iter()
                        .map(|include| include.key().to_owned()),
                );
                if publication == BlueprintPublication::Publish {
                    dependencies.extend(blueprint.table_path_dependencies().iter().cloned());
                }
            }
            ((*key).to_owned(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();

    let mut actions = Vec::with_capacity(resources.len());
    for logical_key in topological_resource_order(&ordering_dependencies)? {
        let resource = resources
            .get(logical_key.as_str())
            .expect("dependency graph contains declared resources");
        let mapping = &mappings[&logical_key];
        let (action, reason_code) = blueprints.outcomes[&logical_key];
        let normalized_payload = if action == PlanActionKind::Create {
            blueprints.payloads.remove(&logical_key)
        } else {
            None
        };
        let preconditions = if action == PlanActionKind::Map {
            let existing = &workspace.existing_blueprints[&logical_key];
            serde_json::json!([{
                "kind": "existing_blueprint",
                "id": existing.id,
                "code": existing.code,
                "version": existing.version,
                "definition_hash": existing.definition_hash,
                "canonical_definition_hash": existing.canonical_definition_hash,
                "blueprint_kind": existing.kind,
                "status": "published",
                "deleted": false
            }])
        } else if matches!(action, PlanActionKind::Create | PlanActionKind::Conflict)
            && mapping.mapping_kind == MappingKind::Create
        {
            serde_json::json!([{
                "kind": "target_absent",
                "resource_kind": PlanResourceKind::Blueprint,
                "code": mapping.target_code,
            }])
        } else {
            serde_json::json!([])
        };
        let mut summary = serde_json::json!({
            "target_code": mapping.target_code,
            "target_version": mapping.target_version,
            "required": resource.required,
            "dependencies": validated_blueprint(pack, &logical_key).dependencies(),
            "extension_layout": layouts.evidence.get(&logical_key).cloned().unwrap_or_default(),
        });
        if let Some(reuse) = &resource.reuse {
            summary["reuse"] = serde_json::json!(reuse);
        }
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::Blueprint,
            logical_key,
            action,
            reason_code,
            summary,
            normalized_payload,
            preconditions,
        });
    }
    Ok(actions)
}

fn plan_presentation_asset_actions(
    pack: &ValidatedSolutionPack,
    workspace: &PlanningWorkspaceSnapshot,
    mappings: &BTreeMap<String, PlannedMapping>,
) -> Vec<PlannedAction> {
    let mut actions = Vec::new();
    for resource in &pack.manifest().resources.presentation_assets {
        let asset = pack
            .presentation_asset(&resource.key)
            .expect("validated presentation asset exists");
        let mapping = &mappings[&resource.key];
        let existing = workspace.existing_presentation_assets.get(&resource.key);
        let (action, reason_code) = match existing {
            Some(existing)
                if existing.purpose == asset.purpose
                    && existing.media_type == asset.media_type
                    && existing.byte_size == asset.stored_bytes.len() as i64
                    && existing.sha256 == asset.stored_sha256 =>
            {
                (PlanActionKind::Map, "exact_asset_match")
            }
            Some(_) => (PlanActionKind::Conflict, "existing_asset_incompatible"),
            None if resource.required => (PlanActionKind::Create, "target_absent"),
            None => (PlanActionKind::Skip, "optional_not_selected"),
        };
        let normalized_payload = (action == PlanActionKind::Create).then(|| {
            serde_json::json!({
                "purpose": asset.purpose,
                "media_type": asset.media_type,
                "byte_size": asset.stored_bytes.len(),
                "source_byte_size": asset.source_byte_size,
                "source_sha256": asset.source_sha256,
                "stored_sha256": asset.stored_sha256,
                "width": asset.width,
                "height": asset.height,
            })
        });
        let preconditions = match (action, existing) {
            (PlanActionKind::Map, Some(existing)) => serde_json::json!([{
                "kind": "existing_presentation_asset",
                "id": existing.id,
                "purpose": existing.purpose,
                "media_type": existing.media_type,
                "byte_size": existing.byte_size,
                "sha256": existing.sha256,
                "source_sha256": asset.source_sha256,
            }]),
            (PlanActionKind::Create | PlanActionKind::Conflict, _) => serde_json::json!([{
                "kind": "target_absent",
                "resource_kind": PlanResourceKind::PresentationAsset,
                "code": "presentation_asset",
            }]),
            _ => serde_json::json!([]),
        };
        actions.push(PlannedAction {
            resource_kind: PlanResourceKind::PresentationAsset,
            logical_key: resource.key.clone(),
            action,
            reason_code,
            summary: serde_json::json!({
                "target_id": mapping.target_id,
                "required": resource.required,
                "purpose": asset.purpose,
                "media_type": asset.media_type,
                "byte_size": asset.stored_bytes.len(),
                "source_byte_size": asset.source_byte_size,
                "source_sha256": asset.source_sha256,
                "stored_sha256": asset.stored_sha256,
                "width": asset.width,
                "height": asset.height,
            }),
            normalized_payload,
            preconditions,
        });
    }
    actions
}

/// A conflict for a workspace setting whose current value cannot be read.
fn invalid_workspace_setting_action(
    resource: &SolutionPackResource,
    setting: &str,
    reason_code: &'static str,
) -> PlannedAction {
    PlannedAction {
        resource_kind: PlanResourceKind::WorkspaceSetting,
        logical_key: resource.key.clone(),
        action: PlanActionKind::Conflict,
        reason_code,
        summary: serde_json::json!({
            "setting": setting,
            "required": resource.required,
            "entries": [],
        }),
        normalized_payload: None,
        preconditions: serde_json::json!([]),
    }
}

/// The action for a workspace setting built from per-entry outcomes.
fn workspace_setting_action(
    resource: &SolutionPackResource,
    setting: &str,
    (action, reason_code): Outcome,
    evidence: Vec<Value>,
    payload_entries: Vec<Value>,
) -> PlannedAction {
    PlannedAction {
        resource_kind: PlanResourceKind::WorkspaceSetting,
        logical_key: resource.key.clone(),
        action,
        reason_code,
        summary: serde_json::json!({
            "setting": setting,
            "required": resource.required,
            "entries": evidence,
        }),
        normalized_payload: matches!(action, PlanActionKind::Append | PlanActionKind::Satisfied)
            .then(|| serde_json::json!({ "entries": payload_entries })),
        preconditions: serde_json::json!([]),
    }
}

fn plan_explore_navigation_action(
    navigation: &SolutionPackExploreNavigation,
    resource: &SolutionPackResource,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
    mappings: &BTreeMap<String, PlannedMapping>,
    blueprint_outcomes: &HashMap<String, Outcome>,
) -> PlannedAction {
    let mut evidence = Vec::with_capacity(navigation.entries.len());
    let mut payload_entries = Vec::with_capacity(navigation.entries.len());
    let mut unmet_reason = None;
    let mut visibility_conflict = false;
    let current_by_code = workspace
        .explore_navigation
        .iter()
        .map(|entry| {
            let mut roles = entry.visible_to_role_codes.clone();
            roles.sort();
            (entry.blueprint_code.as_str(), roles)
        })
        .collect::<HashMap<_, _>>();
    for entry in &navigation.entries {
        let mapping = &mappings[&entry.blueprint];
        let roles_available = entry
            .visible_to_role_codes
            .iter()
            .all(|role| workspace.role_codes.contains(role));
        let blueprint_available =
            blueprint_outcomes
                .get(&entry.blueprint)
                .is_some_and(|(action, _)| match action {
                    PlanActionKind::Map => true,
                    PlanActionKind::Create => publication == BlueprintPublication::Publish,
                    _ => false,
                });
        let (outcome, reason) = if !roles_available {
            unmet_reason.get_or_insert("unknown_role_code");
            ("unmet", "unknown_role_code")
        } else if let Some(current_roles) = current_by_code.get(mapping.target_code.as_str()) {
            if *current_roles != entry.visible_to_role_codes {
                visibility_conflict = true;
                ("conflict", "visibility_mismatch")
            } else if workspace
                .published_entity_codes
                .contains(&mapping.target_code)
            {
                ("satisfied", "exact_match")
            } else {
                unmet_reason.get_or_insert("blueprint_not_published");
                ("unmet", "blueprint_not_published")
            }
        } else if !blueprint_available {
            let reason = if publication != BlueprintPublication::Publish {
                "blueprint_not_published"
            } else {
                "blueprint_not_creatable"
            };
            unmet_reason.get_or_insert(reason);
            ("unmet", reason)
        } else {
            ("append", "target_absent")
        };
        let evidence_outcome = if !resource.required && matches!(outcome, "unmet" | "conflict") {
            "skip"
        } else {
            outcome
        };
        evidence.push(serde_json::json!({
            "blueprint": entry.blueprint,
            "blueprint_code": mapping.target_code,
            "visible_to_role_codes": entry.visible_to_role_codes,
            "outcome": evidence_outcome,
            "reason_code": reason,
        }));
        if matches!(outcome, "append" | "satisfied") {
            payload_entries.push(serde_json::json!({
                "blueprint_key": entry.blueprint,
                "blueprint_code": mapping.target_code,
                "visible_to_role_codes": entry.visible_to_role_codes,
            }));
        }
    }
    let has_append = evidence.iter().any(|entry| entry["outcome"] == "append");
    let outcome = if resource.required && visibility_conflict {
        (PlanActionKind::Conflict, "visibility_mismatch")
    } else if resource.required
        && let Some(reason) = unmet_reason
    {
        (PlanActionKind::Blocked, reason)
    } else if payload_entries.is_empty() {
        (
            PlanActionKind::Skip,
            unmet_reason.unwrap_or("visibility_mismatch"),
        )
    } else if has_append {
        (PlanActionKind::Append, "target_absent")
    } else {
        (PlanActionKind::Satisfied, "exact_match")
    };
    workspace_setting_action(
        resource,
        "explore_navigation",
        outcome,
        evidence,
        payload_entries,
    )
}

/// Lexicon entries never conflict: apply adds missing entries, updates ones a
/// pack supplied, and keeps entries the workspace wrote.
fn plan_lexicon_action(
    entries: &[catalog_lexicon::Entry],
    resource: &SolutionPackResource,
) -> PlannedAction {
    let languages: BTreeSet<&str> = entries
        .iter()
        .map(|entry| entry.language.as_str())
        .collect();
    PlannedAction {
        resource_kind: PlanResourceKind::WorkspaceSetting,
        logical_key: resource.key.clone(),
        action: PlanActionKind::Append,
        reason_code: "workspace_entries_preserved",
        summary: serde_json::json!({
            "setting": "lexicon",
            "required": resource.required,
            "languages": languages,
            "entry_count": entries.len(),
        }),
        normalized_payload: Some(serde_json::json!({"entries": entries})),
        preconditions: serde_json::json!([]),
    }
}

fn plan_extension_layout_action(
    layout: &SolutionPackExtensionLayout,
    resource: &SolutionPackResource,
    manifest: &SolutionPackManifest,
    workspace: &PlanningWorkspaceSnapshot,
) -> PlannedAction {
    let mut evidence = Vec::with_capacity(layout.entries.len());
    let mut payload_entries = Vec::new();
    let mut required_unmet = None;
    let mut required_conflict = false;
    let mut has_append = false;
    for entry in &layout.entries {
        let requirement = extension_requirement_for_contribution(manifest, &entry.contribution);
        let installed = workspace.installed_extensions.get(&requirement.id);
        let availability = contribution_availability_reason(
            requirement,
            &entry.contribution,
            &entry.outlet,
            installed,
        );
        let (outcome, reason) = match (availability, installed) {
            (Some(reason), _) if entry.required => {
                required_unmet.get_or_insert(reason);
                (PlanActionKind::Blocked, reason)
            }
            (Some(reason), _) => (PlanActionKind::Skip, reason),
            (None, installed) => {
                let installed = installed.expect("available contribution is installed");
                let outcome = match classify_extension_layout_placement(
                    &workspace.extension_layout,
                    &entry.contribution,
                    &entry.outlet,
                    entry.hidden,
                    entry.promoted,
                ) {
                    ExtensionLayoutPlacement::Exact => (PlanActionKind::Satisfied, "exact_match"),
                    ExtensionLayoutPlacement::Absent => {
                        has_append = true;
                        (PlanActionKind::Append, "target_absent")
                    }
                    ExtensionLayoutPlacement::Conflict if entry.required => {
                        required_conflict = true;
                        (PlanActionKind::Conflict, "placement_mismatch")
                    }
                    ExtensionLayoutPlacement::Conflict => {
                        (PlanActionKind::Skip, "placement_mismatch")
                    }
                };
                if matches!(
                    outcome.0,
                    PlanActionKind::Append | PlanActionKind::Satisfied
                ) {
                    payload_entries.push(serde_json::json!({
                        "contribution": entry.contribution,
                        "outlet": entry.outlet,
                        "hidden": entry.hidden,
                        "promoted": entry.promoted,
                        "installed_release_id": installed.installed_release_id,
                        "installed_version": installed.version,
                    }));
                }
                outcome
            }
        };
        evidence.push(serde_json::json!({
            "contribution": entry.contribution,
            "outlet": entry.outlet,
            "hidden": entry.hidden,
            "promoted": entry.promoted,
            "required": entry.required,
            "outcome": outcome,
            "reason_code": reason,
        }));
    }
    let outcome = if required_conflict {
        (PlanActionKind::Conflict, "placement_mismatch")
    } else if let Some(reason) = required_unmet {
        (PlanActionKind::Blocked, reason)
    } else if payload_entries.is_empty() {
        (PlanActionKind::Skip, "optional_contributions_unmet")
    } else if has_append {
        (PlanActionKind::Append, "target_absent")
    } else {
        (PlanActionKind::Satisfied, "exact_match")
    };
    workspace_setting_action(
        resource,
        "extension_layout",
        outcome,
        evidence,
        payload_entries,
    )
}

fn blueprint_kind_name(kind: BlueprintKind) -> &'static str {
    match kind {
        BlueprintKind::Entity => "entity",
        BlueprintKind::Mixin => "mixin",
    }
}

pub(crate) fn deterministic_target_id(
    workspace_id: uuid::Uuid,
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    logical_key: &str,
) -> uuid::Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"attricat.solution-pack.target-id.v1\0");
    hasher.update(workspace_id.as_bytes());
    for value in [
        pack.archive_sha256(),
        prefix,
        publication.as_str(),
        logical_key,
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub fn validate_plan_prefix(prefix: &str) -> Result<(), SolutionPackError> {
    if prefix.is_empty()
        || prefix.len() > MAX_SOLUTION_PACK_PREFIX_BYTES
        || !prefix
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || prefix.ends_with('_')
    {
        return invalid(format!(
            "prefix must be 1-{MAX_SOLUTION_PACK_PREFIX_BYTES} lowercase ASCII letters, digits, or underscores, start with a letter, and not end with an underscore"
        ));
    }
    Ok(())
}

fn topological_resource_order(
    dependencies: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, SolutionPackError> {
    fn visit(
        key: &str,
        dependencies: &BTreeMap<String, BTreeSet<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        ordered: &mut Vec<String>,
    ) -> Result<(), SolutionPackError> {
        if visited.contains(key) {
            return Ok(());
        }
        if !visiting.insert(key.to_owned()) {
            return invalid(format!("resource dependencies contain a cycle at '{key}'"));
        }
        for dependency in &dependencies[key] {
            visit(dependency, dependencies, visiting, visited, ordered)?;
        }
        visiting.remove(key);
        visited.insert(key.to_owned());
        ordered.push(key.to_owned());
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::with_capacity(dependencies.len());
    for key in dependencies.keys() {
        visit(key, dependencies, &mut visiting, &mut visited, &mut ordered)?;
    }
    Ok(ordered)
}

pub(super) fn normalized_blueprint_payload(
    blueprint: &SolutionPackBlueprint,
    mappings: &BTreeMap<String, PlannedMapping>,
    publication: BlueprintPublication,
    allowed_contributions: &HashSet<String>,
    workspace: &PlanningWorkspaceSnapshot,
    manifest: &SolutionPackManifest,
) -> Result<Value, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(blueprint.source()).map_err(|_| {
        SolutionPackError::Invalid(format!("blueprint '{}' is invalid", blueprint.key()))
    })?;
    let table = value
        .as_table_mut()
        .expect("validated blueprint is a table");
    table.insert(
        "code".to_owned(),
        toml::Value::String(mappings[blueprint.key()].target_code.clone()),
    );
    if let Some(includes) = table
        .get_mut("includes")
        .and_then(toml::Value::as_array_mut)
    {
        for include in includes {
            let include = include
                .as_table_mut()
                .expect("validated include is a table");
            let key = include
                .remove("key")
                .and_then(|value| value.as_str().map(str::to_owned))
                .expect("validated include has a key");
            include.insert(
                "code".to_owned(),
                toml::Value::String(mappings[&key].target_code.clone()),
            );
            include.insert(
                "version".to_owned(),
                toml::Value::Integer(
                    mappings[&key]
                        .target_version
                        .expect("blueprint mappings have a revision"),
                ),
            );
        }
    }
    if let Some(attributes) = table
        .get_mut("attributes")
        .and_then(toml::Value::as_array_mut)
    {
        for attribute in attributes {
            if let Some(attribute) = attribute.as_table_mut() {
                normalize_reference(attribute, "target_blueprint", mappings);
                if let Some(targets) = attribute
                    .get_mut("target_blueprints")
                    .and_then(toml::Value::as_array_mut)
                {
                    for target in targets {
                        let key = target
                            .as_str()
                            .expect("validated reference is a string")
                            .to_owned();
                        *target = toml::Value::String(mappings[&key].target_code.clone());
                    }
                }
            }
        }
    }
    visit_embedded_predicate_blueprint_codes(table, &mut |code| {
        *code = crate::solution_pack_seeds::physical_blueprint_code(mappings, code)?;
        Ok(())
    })?;
    if let Some(views) = table.get_mut("views").and_then(toml::Value::as_table_mut) {
        for (name, view) in views.iter_mut() {
            if let Some(view) = view.as_table_mut() {
                normalize_view_references(view, mappings);
                if name == "extension_layout"
                    && let Some(outlets) =
                        view.get_mut("outlets").and_then(toml::Value::as_table_mut)
                {
                    for (_, outlet) in outlets.iter_mut() {
                        if let Some(outlet) = outlet.as_table_mut() {
                            for list in ["order", "hidden"] {
                                if let Some(entries) =
                                    outlet.get_mut(list).and_then(toml::Value::as_array_mut)
                                {
                                    entries.retain(|entry| {
                                        entry.as_str().is_some_and(|entry| {
                                            allowed_contributions.contains(entry)
                                        })
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let extension_contributions = blueprint
        .extension_layout
        .iter()
        .filter(|entry| allowed_contributions.contains(&entry.contribution))
        .map(|entry| {
            let requirement = extension_requirement_for_contribution(manifest, &entry.contribution);
            let installed = workspace
                .installed_extensions
                .get(&requirement.id)
                .expect("allowed contribution is installed");
            serde_json::json!({
                "contribution": entry.contribution,
                "outlet": entry.outlet,
                "installed_release_id": installed.installed_release_id,
                "installed_version": installed.version,
            })
        })
        .collect::<Vec<_>>();
    let definition = toml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!("blueprint '{}' is invalid", blueprint.key()))
    })?;
    Ok(serde_json::json!({
        "definition": definition,
        "version": 1,
        "publication": publication.as_str(),
        "extension_contributions": extension_contributions,
    }))
}

fn normalize_reference(
    table: &mut toml::map::Map<String, toml::Value>,
    field: &str,
    mappings: &BTreeMap<String, PlannedMapping>,
) {
    if let Some(reference) = table.get_mut(field) {
        let key = reference
            .as_str()
            .expect("validated reference is a string")
            .to_owned();
        *reference = toml::Value::String(mappings[&key].target_code.clone());
    }
}

fn normalize_view_references(
    node: &mut toml::map::Map<String, toml::Value>,
    mappings: &BTreeMap<String, PlannedMapping>,
) {
    if node.get("type").and_then(toml::Value::as_str) == Some("incoming_relationship_list")
        && let Some(relationships) = node
            .get_mut("relationships")
            .and_then(toml::Value::as_array_mut)
    {
        for relationship in relationships {
            if let Some(relationship) = relationship.as_table_mut() {
                normalize_reference(relationship, "source_blueprint", mappings);
            }
        }
    }
    for collection in ["children", "tabs", "sections"] {
        if let Some(children) = node.get_mut(collection).and_then(toml::Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_table_mut() {
                    normalize_view_references(child, mappings);
                }
            }
        }
    }
}
