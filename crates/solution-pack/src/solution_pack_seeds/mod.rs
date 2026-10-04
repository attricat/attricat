//! Seed resources beyond blueprints: rules, workflows, saved searches,
//! contexts with publication channels, and prerequisite seeds.
//!
//! Like blueprints, every resource is validated offline against the pack's
//! own logical keys. Workspace identities are only chosen by a plan, and a
//! seed only provisions a new installation: there is no update, drift, or
//! removal handling for any of these resources.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use catalog_blueprint::BlueprintKind;
use catalog_validation::saved_search;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::solution_packs::{
    MAX_IDENTIFIER_BYTES, MAX_VERSION_REQUIREMENT_BYTES, SolutionPackBlueprint, SolutionPackError,
    SolutionPackManifest, SolutionPackResource, ValidatedSolutionPack, invalid,
    is_valid_stable_code, parse_sha256, parse_version_req, resource_code, safe_archive_path,
    validate_acyclic, validate_pack_id,
};

mod plan;

pub(crate) use plan::{
    DependentPlanInput, physical_blueprint_code, plan_contexts, plan_dependents,
    plan_prerequisites, visit_toml_predicate_blueprint_codes,
};
pub use plan::{
    ExistingContextSnapshot, ExistingPublicationChannel, PrerequisiteResolution,
    SeedWorkspaceSnapshot,
};

pub const MAX_SOLUTION_PACK_RULES: usize = 64;
pub const MAX_SOLUTION_PACK_WORKFLOWS: usize = 64;
pub const MAX_SOLUTION_PACK_SAVED_SEARCHES: usize = 64;
pub const MAX_SOLUTION_PACK_CONTEXTS: usize = 32;
pub const MAX_SOLUTION_PACK_PREREQUISITES: usize = 16;
const MAX_SEED_SOURCE_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_DATA_BYTES: usize = 4 * 1024;
const MAX_SAVED_SEARCH_NAME_BYTES: usize = 120;
const MAX_SAVED_SEARCH_DESCRIPTION_BYTES: usize = 500;

/// Another seed that must already be applied to the workspace. Planning never
/// installs a prerequisite; it only reuses what a completed application of it
/// created.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackPrerequisite {
    pub key: String,
    pub id: String,
    pub version: String,
}

/// Declares that a pack blueprint is the exact blueprint a prerequisite seed
/// installed under its own logical key, so the plan reuses it instead of
/// creating a copy.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackBlueprintReuse {
    pub prerequisite: String,
    pub blueprint: String,
}

#[derive(Clone, Debug)]
pub struct SeedRule {
    pub key: String,
    pub code: String,
    pub blueprint: String,
    /// Other pack blueprints the predicate names, such as `referenced_by`
    /// sources, by logical key.
    pub referenced_blueprints: BTreeSet<String>,
    pub context: Option<String>,
    pub enabled: bool,
    /// Native rule TOML without pack-only keys, still using the pack-local code.
    pub definition: String,
    pub compiled: catalog_rules::CompiledRule,
}

#[derive(Clone, Debug)]
pub struct SeedWorkflow {
    pub key: String,
    pub code: String,
    pub enabled: bool,
    /// Native workflow TOML without pack-only keys, still using the pack-local code.
    pub definition: String,
    pub compiled: catalog_workflow::CompiledWorkflow,
}

#[derive(Clone, Debug)]
pub struct SeedSavedSearch {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    /// Explorer search state whose blueprint and context references are still
    /// pack logical keys.
    pub state: Value,
    pub blueprint: String,
    pub context: Option<String>,
    pub referenced_blueprints: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct SeedContext {
    pub key: String,
    pub code: String,
    pub data: Value,
    pub parent: Option<String>,
    /// The publication channel the context provides, if any.
    pub publication_channel: Option<SeedPublicationChannel>,
}

/// A publication channel declared by a pack context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedPublicationChannel {
    pub enabled: bool,
    /// Pack rules, by logical key, that must hold before publication.
    pub required_rules: Vec<String>,
    pub require_valid_entity: bool,
}

/// Rules a channel may require, as for an ordinary channel update.
const MAX_CHANNEL_REQUIRED_RULES: usize = 32;

#[derive(Debug, Default)]
pub(crate) struct ValidatedSeeds {
    pub rules: BTreeMap<String, SeedRule>,
    pub workflows: BTreeMap<String, SeedWorkflow>,
    pub saved_searches: BTreeMap<String, SeedSavedSearch>,
    pub contexts: BTreeMap<String, SeedContext>,
}

/// Manifest declarations of the seed resource kinds handled by this module,
/// as `(namespace, file extension, resource)`.
pub(crate) fn seed_resources(
    manifest: &SolutionPackManifest,
) -> impl Iterator<Item = (&'static str, &'static str, &SolutionPackResource)> {
    let resources = &manifest.resources;
    resources
        .rules
        .iter()
        .map(|resource| ("rules", ".toml", resource))
        .chain(
            resources
                .workflows
                .iter()
                .map(|resource| ("workflows", ".toml", resource)),
        )
        .chain(
            resources
                .saved_searches
                .iter()
                .map(|resource| ("saved-searches", ".json", resource)),
        )
        .chain(
            resources
                .contexts
                .iter()
                .map(|resource| ("contexts", ".json", resource)),
        )
}

pub(crate) fn validate_seed_manifest(
    manifest: &SolutionPackManifest,
) -> Result<(), SolutionPackError> {
    for (label, count, limit) in [
        (
            "rules",
            manifest.resources.rules.len(),
            MAX_SOLUTION_PACK_RULES,
        ),
        (
            "workflows",
            manifest.resources.workflows.len(),
            MAX_SOLUTION_PACK_WORKFLOWS,
        ),
        (
            "saved searches",
            manifest.resources.saved_searches.len(),
            MAX_SOLUTION_PACK_SAVED_SEARCHES,
        ),
        (
            "contexts",
            manifest.resources.contexts.len(),
            MAX_SOLUTION_PACK_CONTEXTS,
        ),
        (
            "prerequisites",
            manifest.prerequisites.len(),
            MAX_SOLUTION_PACK_PREREQUISITES,
        ),
    ] {
        if count > limit {
            return invalid(format!("solution-pack manifest declares too many {label}"));
        }
    }
    for (namespace, extension, resource) in seed_resources(manifest) {
        validate_seed_resource(namespace, extension, resource)?;
    }

    let mut prerequisite_ids = BTreeSet::new();
    let mut prerequisite_keys = BTreeSet::new();
    for prerequisite in &manifest.prerequisites {
        validate_namespaced_key(&prerequisite.key, "prerequisites")?;
        validate_pack_id(&prerequisite.id).map_err(|_| {
            SolutionPackError::Invalid(format!(
                "prerequisite '{}' id must be a solution-pack id",
                prerequisite.key
            ))
        })?;
        if prerequisite.id == manifest.id {
            return invalid(format!(
                "prerequisite '{}' cannot reference the pack itself",
                prerequisite.key
            ));
        }
        if prerequisite.version.is_empty()
            || prerequisite.version.len() > MAX_VERSION_REQUIREMENT_BYTES
            || prerequisite.version.trim() != prerequisite.version
            || parse_version_req(&prerequisite.version).is_err()
        {
            return invalid(format!(
                "prerequisite '{}' version must be a trimmed SemVer range",
                prerequisite.key
            ));
        }
        if !prerequisite_ids.insert(prerequisite.id.as_str()) {
            return invalid(format!("duplicate prerequisite id '{}'", prerequisite.id));
        }
        prerequisite_keys.insert(prerequisite.key.as_str());
    }

    let mut reused = BTreeSet::new();
    for resource in &manifest.resources.blueprints {
        let Some(reuse) = &resource.reuse else {
            continue;
        };
        if !prerequisite_keys.contains(reuse.prerequisite.as_str()) {
            return invalid(format!(
                "blueprint '{}' reuses undeclared prerequisite '{}'",
                resource.key, reuse.prerequisite
            ));
        }
        validate_namespaced_key(&reuse.blueprint, "blueprints")?;
        reused.insert(reuse.prerequisite.as_str());
    }
    if let Some(unused) = prerequisite_keys.difference(&reused).next() {
        return invalid(format!(
            "prerequisite '{unused}' is not reused by any blueprint"
        ));
    }
    Ok(())
}

fn validate_seed_resource(
    namespace: &str,
    extension: &str,
    resource: &SolutionPackResource,
) -> Result<(), SolutionPackError> {
    validate_namespaced_key(&resource.key, namespace)?;
    if resource.reuse.is_some() {
        return invalid(format!(
            "resource '{}' cannot declare blueprint reuse",
            resource.key
        ));
    }
    if !safe_archive_path(&resource.path)
        || !resource.path.starts_with(&format!("{namespace}/"))
        || !resource.path.ends_with(extension)
    {
        return invalid(format!(
            "resource path '{}' must be a safe {namespace}/ path ending in {extension}",
            resource.path
        ));
    }
    parse_sha256(&resource.sha256).map_err(|()| {
        SolutionPackError::Invalid(format!(
            "resource '{}' sha256 must be 64 lowercase hexadecimal characters",
            resource.key
        ))
    })?;
    Ok(())
}

fn validate_namespaced_key(key: &str, namespace: &str) -> Result<(), SolutionPackError> {
    let valid = key.len() <= MAX_IDENTIFIER_BYTES
        && key
            .strip_prefix(namespace)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|code| !code.contains('/') && is_valid_stable_code(code));
    if valid {
        Ok(())
    } else {
        invalid(format!(
            "key '{key}' must be a stable code in the {namespace}/ namespace"
        ))
    }
}

pub(crate) fn validate_seed_content(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
) -> Result<ValidatedSeeds, SolutionPackError> {
    let mut seeds = ValidatedSeeds::default();
    for resource in &manifest.resources.contexts {
        let context = validate_context(resource, &files[&resource.path])?;
        seeds.contexts.insert(resource.key.clone(), context);
    }
    for context in seeds.contexts.values() {
        if let Some(parent) = &context.parent
            && !seeds.contexts.contains_key(parent)
        {
            return invalid(format!(
                "context '{}' references undeclared parent '{parent}'",
                context.key
            ));
        }
    }
    let parents = seeds
        .contexts
        .values()
        .map(|context| {
            (
                context.key.as_str(),
                context.parent.iter().map(String::as_str).collect(),
            )
        })
        .collect::<HashMap<_, _>>();
    validate_acyclic(&parents, "context parent")?;

    let context_keys = seeds.contexts.keys().cloned().collect::<BTreeSet<_>>();
    for resource in &manifest.resources.rules {
        let rule = validate_rule(resource, &files[&resource.path], blueprints, &context_keys)?;
        seeds.rules.insert(resource.key.clone(), rule);
    }
    for context in seeds.contexts.values() {
        for rule in context
            .publication_channel
            .iter()
            .flat_map(|channel| &channel.required_rules)
        {
            if !seeds.rules.contains_key(rule) {
                return invalid(format!(
                    "context '{}' channel requires undeclared rule '{rule}'",
                    context.key
                ));
            }
        }
    }
    for resource in &manifest.resources.workflows {
        let workflow = validate_workflow(resource, &files[&resource.path], blueprints)?;
        seeds.workflows.insert(resource.key.clone(), workflow);
    }
    for resource in &manifest.resources.saved_searches {
        let search =
            validate_saved_search(resource, &files[&resource.path], blueprints, &context_keys)?;
        seeds.saved_searches.insert(resource.key.clone(), search);
    }
    Ok(seeds)
}

fn utf8_source<'a>(key: &str, bytes: &'a [u8]) -> Result<&'a str, SolutionPackError> {
    if bytes.len() > MAX_SEED_SOURCE_BYTES {
        return invalid(format!(
            "resource '{key}' exceeds the {} KiB size limit",
            MAX_SEED_SOURCE_BYTES / 1024
        ));
    }
    std::str::from_utf8(bytes)
        .map_err(|_| SolutionPackError::Invalid(format!("resource '{key}' must be UTF-8")))
}

/// Removes pack-only keys from a native TOML definition. The remainder must
/// be accepted by the ordinary native parser.
fn split_pack_keys(
    key: &str,
    source: &str,
    pack_keys: &[&str],
) -> Result<(BTreeMap<String, toml::Value>, String), SolutionPackError> {
    let mut value: toml::Value = toml::from_str(source)
        .map_err(|_| SolutionPackError::Invalid(format!("resource '{key}' is not valid TOML")))?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| SolutionPackError::Invalid(format!("resource '{key}' is not valid TOML")))?;
    let mut extracted = BTreeMap::new();
    for pack_key in pack_keys {
        if let Some(value) = table.remove(*pack_key) {
            extracted.insert((*pack_key).to_owned(), value);
        }
    }
    let native = toml::to_string(&value)
        .map_err(|_| SolutionPackError::Invalid(format!("resource '{key}' is not valid TOML")))?;
    Ok((extracted, native))
}

fn required_bool(
    key: &str,
    extracted: &BTreeMap<String, toml::Value>,
    field: &str,
) -> Result<bool, SolutionPackError> {
    extracted
        .get(field)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| {
            SolutionPackError::Invalid(format!("resource '{key}' must declare boolean '{field}'"))
        })
}

fn optional_key(
    key: &str,
    extracted: &BTreeMap<String, toml::Value>,
    field: &str,
) -> Result<Option<String>, SolutionPackError> {
    extracted
        .get(field)
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "resource '{key}' field '{field}' must be a logical key"
                ))
            })
        })
        .transpose()
}

fn entity_blueprint<'a>(
    owner: &str,
    reference: &str,
    blueprints: &'a BTreeMap<String, SolutionPackBlueprint>,
) -> Result<&'a SolutionPackBlueprint, SolutionPackError> {
    let blueprint = blueprints.get(reference).ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "resource '{owner}' references undeclared blueprint '{reference}'"
        ))
    })?;
    if blueprint.kind() != BlueprintKind::Entity {
        return invalid(format!(
            "resource '{owner}' must reference an entity blueprint"
        ));
    }
    Ok(blueprint)
}

fn validate_context_reference(
    owner: &str,
    context: Option<&str>,
    contexts: &BTreeSet<String>,
) -> Result<(), SolutionPackError> {
    if let Some(context) = context
        && !contexts.contains(context)
    {
        return invalid(format!(
            "resource '{owner}' references undeclared context '{context}'"
        ));
    }
    Ok(())
}

/// Resolves the blueprint codes a predicate names (the source blueprint of
/// `referenced_by`) to pack entity blueprints, by their pack-local code, and
/// checks the referencing relationship. Returns the blueprints' logical keys.
pub(crate) fn predicate_blueprint_keys(
    owner: &str,
    predicate: &catalog_validation::predicate::Predicate,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
) -> Result<BTreeSet<String>, SolutionPackError> {
    use catalog_validation::predicate::Predicate;
    fn walk(
        owner: &str,
        predicate: &Predicate,
        blueprints: &BTreeMap<String, SolutionPackBlueprint>,
        keys: &mut BTreeSet<String>,
    ) -> Result<(), SolutionPackError> {
        match predicate {
            Predicate::ReferencedBy {
                blueprint_code,
                relationship_code,
                predicate,
                ..
            } => {
                let source = blueprints
                    .values()
                    .find(|blueprint| blueprint.code() == blueprint_code)
                    .filter(|blueprint| blueprint.kind() == BlueprintKind::Entity)
                    .ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "'{owner}' references blueprint '{blueprint_code}' that is not a pack entity blueprint"
                        ))
                    })?;
                if !declares_relationship(source, relationship_code) {
                    return invalid(format!(
                        "'{owner}' references unknown relationship '{relationship_code}' of '{}'",
                        source.key()
                    ));
                }
                keys.insert(source.key().to_owned());
                if let Some(predicate) = predicate {
                    walk(owner, predicate, blueprints, keys)?;
                }
            }
            Predicate::Linked { predicate, .. } => walk(owner, predicate, blueprints, keys)?,
            Predicate::AllOf { predicates } | Predicate::AnyOf { predicates } => {
                for predicate in predicates {
                    walk(owner, predicate, blueprints, keys)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut keys = BTreeSet::new();
    walk(owner, predicate, blueprints, &mut keys)?;
    Ok(keys)
}

fn validate_rule(
    resource: &SolutionPackResource,
    bytes: &[u8],
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
    contexts: &BTreeSet<String>,
) -> Result<SeedRule, SolutionPackError> {
    let key = resource.key.as_str();
    let source = utf8_source(key, bytes)?;
    let (extracted, definition) =
        split_pack_keys(key, source, &["blueprint", "context", "enabled"])?;
    let enabled = required_bool(key, &extracted, "enabled")?;
    let blueprint = optional_key(key, &extracted, "blueprint")?.ok_or_else(|| {
        SolutionPackError::Invalid(format!("rule '{key}' must declare its blueprint"))
    })?;
    let context = optional_key(key, &extracted, "context")?;
    let compiled = catalog_rules::compile(&definition)
        .map_err(|error| SolutionPackError::Invalid(format!("rule '{key}' is invalid: {error}")))?;
    let expected_code = resource_code(key);
    if compiled.code != expected_code {
        return invalid(format!("rule '{key}' code must be '{expected_code}'"));
    }
    let target = entity_blueprint(key, &blueprint, blueprints)?;
    validate_context_reference(key, context.as_deref(), contexts)?;
    // The same type check as an ordinary rule write against its blueprint.
    let types = target
        .effective_attributes()
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type.clone()))
        .collect::<HashMap<_, _>>();
    catalog_rules::validate_against_attributes(&compiled, &types).map_err(|error| {
        SolutionPackError::Invalid(format!(
            "rule '{key}' is invalid for '{blueprint}': {error}"
        ))
    })?;
    let referenced_blueprints = predicate_blueprint_keys(key, &compiled.predicate, blueprints)?;
    Ok(SeedRule {
        key: key.to_owned(),
        code: compiled.code.clone(),
        blueprint,
        referenced_blueprints,
        context,
        enabled,
        definition,
        compiled,
    })
}

fn validate_workflow(
    resource: &SolutionPackResource,
    bytes: &[u8],
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
) -> Result<SeedWorkflow, SolutionPackError> {
    let key = resource.key.as_str();
    let source = utf8_source(key, bytes)?;
    let (extracted, definition) = split_pack_keys(key, source, &["enabled"])?;
    let enabled = required_bool(key, &extracted, "enabled")?;
    let compiled = catalog_workflow::compile(&definition).map_err(|error| {
        SolutionPackError::Invalid(format!("workflow '{key}' is invalid: {error}"))
    })?;
    let expected_code = resource_code(key);
    if compiled.code != expected_code {
        return invalid(format!("workflow '{key}' code must be '{expected_code}'"));
    }
    if compiled
        .triggers
        .iter()
        .any(|trigger| matches!(trigger, catalog_workflow::Trigger::Schedule { .. }))
    {
        return invalid(format!(
            "workflow '{key}' cannot seed a schedule trigger because it targets a workspace entity"
        ));
    }
    let all = blueprints.values().collect::<Vec<_>>();
    for trigger in &compiled.triggers {
        if let catalog_workflow::Trigger::Event { attributes, .. } = trigger
            && let Some(code) = attributes.iter().find(|code| {
                !all.iter()
                    .any(|blueprint| declares_attribute(blueprint, code))
            })
        {
            return invalid(format!(
                "workflow '{key}' trigger filters attribute '{code}' that no pack blueprint declares"
            ));
        }
    }
    validate_workflow_actions(key, &compiled.actions, &all)?;
    Ok(SeedWorkflow {
        key: key.to_owned(),
        code: compiled.code.clone(),
        enabled,
        definition,
        compiled,
    })
}

fn declares_attribute(blueprint: &SolutionPackBlueprint, code: &str) -> bool {
    blueprint
        .effective_attributes()
        .iter()
        .any(|attribute| attribute.code == code)
}

fn declares_relationship(blueprint: &SolutionPackBlueprint, code: &str) -> bool {
    blueprint
        .effective_attributes()
        .iter()
        .any(|attribute| attribute.code == code && attribute.value_type == "relationship")
}

/// Checks every write, including those nested in a referencing-entities
/// update, against the pack blueprints that can hold the written entity.
fn validate_workflow_actions(
    key: &str,
    actions: &[catalog_workflow::Action],
    candidates: &[&SolutionPackBlueprint],
) -> Result<(), SolutionPackError> {
    for action in actions {
        match action {
            catalog_workflow::Action::AttributeWrite { attribute_code, .. }
                if !candidates
                    .iter()
                    .any(|blueprint| declares_attribute(blueprint, attribute_code)) =>
            {
                return invalid(format!(
                    "workflow '{key}' writes attribute '{attribute_code}' that no pack blueprint declares"
                ));
            }
            catalog_workflow::Action::ReferencingEntitiesUpdate {
                relationship_attribute,
                actions,
                ..
            } => {
                let referencing = candidates
                    .iter()
                    .copied()
                    .filter(|blueprint| declares_relationship(blueprint, relationship_attribute))
                    .collect::<Vec<_>>();
                if referencing.is_empty() {
                    return invalid(format!(
                        "workflow '{key}' follows relationship '{relationship_attribute}' that no pack blueprint declares"
                    ));
                }
                validate_workflow_actions(key, actions, &referencing)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSearchFile {
    format_version: u32,
    kind: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    state: Value,
}

fn validate_saved_search(
    resource: &SolutionPackResource,
    bytes: &[u8],
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
    contexts: &BTreeSet<String>,
) -> Result<SeedSavedSearch, SolutionPackError> {
    let key = resource.key.as_str();
    utf8_source(key, bytes)?;
    let file: SavedSearchFile = serde_json::from_slice(bytes).map_err(|_| {
        SolutionPackError::Invalid(format!("saved search '{key}' is not strict JSON"))
    })?;
    if file.format_version != 1 || file.kind != "solution_pack_saved_search" {
        return invalid(format!(
            "saved search '{key}' format_version and kind are unsupported"
        ));
    }
    if file.name.trim() != file.name
        || file.name.is_empty()
        || file.name.len() > MAX_SAVED_SEARCH_NAME_BYTES
    {
        return invalid(format!(
            "saved search '{key}' name must be 1-{MAX_SAVED_SEARCH_NAME_BYTES} trimmed bytes"
        ));
    }
    if file.description.as_ref().is_some_and(|description| {
        description.is_empty() || description.len() > MAX_SAVED_SEARCH_DESCRIPTION_BYTES
    }) {
        return invalid(format!(
            "saved search '{key}' description must be 1-{MAX_SAVED_SEARCH_DESCRIPTION_BYTES} bytes"
        ));
    }
    let invalid_state =
        |reason: &str| SolutionPackError::Invalid(format!("saved search '{key}' {reason}"));
    // The same shape every saved-search writer accepts, then pack-only rules.
    saved_search::validate_state(saved_search::EXPLORER_SEARCH_KIND, &file.state)
        .map_err(|reason| invalid_state(&reason))?;
    let state = file
        .state
        .as_object()
        .expect("validated state is an object");
    if state.contains_key("version") {
        return Err(invalid_state(
            "state field 'version' cannot be seeded because it identifies workspace data",
        ));
    }
    let blueprint = state["blueprint"]
        .as_str()
        .expect("validated blueprint")
        .to_owned();
    let target = entity_blueprint(key, &blueprint, blueprints)?;
    let mut referenced_blueprints = BTreeSet::from([blueprint.clone()]);
    let context = state
        .get("context")
        .map(|value| value.as_str().expect("validated context").to_owned());
    validate_context_reference(key, context.as_deref(), contexts)?;
    let by_code = blueprints
        .values()
        .map(|blueprint| (blueprint.code(), blueprint))
        .collect::<HashMap<_, _>>();
    if let Some(field) = state
        .get("sort")
        .and_then(|sort| sort["field"].as_str())
        .filter(|field| !saved_search::SYSTEM_SORT_FIELDS.contains(field))
    {
        validate_attribute_path(target, field, &by_code, &mut referenced_blueprints)
            .map_err(|reason| invalid_state(&reason))?;
    }
    for filter in state
        .get("attributeFilters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = filter["field"].as_str().expect("validated filter field");
        validate_attribute_path(target, field, &by_code, &mut referenced_blueprints)
            .map_err(|reason| invalid_state(&reason))?;
    }
    for facet in state
        .get("relationshipFacets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if facet.get("selectedIds").is_some() {
            return Err(invalid_state(
                "relationship facets cannot seed selected entity IDs",
            ));
        }
        let field = facet["field"].as_str().expect("validated facet field");
        let relationship =
            validate_relationship_path(target, field, &by_code, &mut referenced_blueprints)
                .map_err(|reason| invalid_state(&reason))?;
        if let Some(target) = facet.get("targetBlueprint") {
            let target = target.as_str().expect("validated targetBlueprint");
            let target_blueprint = entity_blueprint(key, target, blueprints)?;
            if !relationship.target_blueprints.is_empty()
                && !relationship
                    .target_blueprints
                    .iter()
                    .any(|code| code == target_blueprint.code())
            {
                return Err(invalid_state(&format!(
                    "relationship facet '{field}' cannot target '{target}'"
                )));
            }
            referenced_blueprints.insert(target.to_owned());
        }
    }
    Ok(SeedSavedSearch {
        key: key.to_owned(),
        name: file.name,
        description: file.description,
        state: file.state,
        blueprint,
        context,
        referenced_blueprints,
    })
}

/// Follows the relationship hops of an Explorer field path through pack
/// blueprints, within Explorer's hop limit, and returns the last attribute.
fn walk_attribute_path<'a>(
    start: &'a SolutionPackBlueprint,
    path: &str,
    by_code: &HashMap<&str, &'a SolutionPackBlueprint>,
    referenced: &mut BTreeSet<String>,
) -> Result<&'a catalog_blueprint::EffectiveAttribute, String> {
    let segments = saved_search::field_path(path)?;
    let mut current = start;
    for (index, segment) in segments.iter().enumerate() {
        let attribute = current
            .effective_attributes()
            .iter()
            .find(|attribute| attribute.code == *segment)
            .ok_or_else(|| format!("field '{path}' references unknown attribute '{segment}'"))?;
        if index + 1 == segments.len() {
            return Ok(attribute);
        }
        if attribute.value_type != "relationship" {
            return Err(format!(
                "field '{path}' segment '{segment}' is not a relationship"
            ));
        }
        // Explorer follows a relationship only to its single target blueprint.
        let target = attribute
            .target_blueprint
            .as_deref()
            .and_then(|code| by_code.get(code))
            .ok_or_else(|| {
                format!("field '{path}' segment '{segment}' does not lead to one pack blueprint")
            })?;
        referenced.insert(target.key().to_owned());
        current = target;
    }
    Err(format!("field '{path}' is empty"))
}

/// Validates an Explorer attribute path: relationship hops through pack
/// blueprints ending in an attribute of the last blueprint.
fn validate_attribute_path(
    start: &SolutionPackBlueprint,
    path: &str,
    by_code: &HashMap<&str, &SolutionPackBlueprint>,
    referenced: &mut BTreeSet<String>,
) -> Result<(), String> {
    walk_attribute_path(start, path, by_code, referenced).map(|_| ())
}

/// Validates a relationship facet path, which must end in a relationship.
fn validate_relationship_path<'a>(
    start: &'a SolutionPackBlueprint,
    path: &str,
    by_code: &HashMap<&str, &'a SolutionPackBlueprint>,
    referenced: &mut BTreeSet<String>,
) -> Result<&'a catalog_blueprint::EffectiveAttribute, String> {
    if path.split('.').count() > saved_search::MAX_RELATIONSHIP_HOPS {
        return Err(format!(
            "relationship facet '{path}' follows more than {} relationship hops",
            saved_search::MAX_RELATIONSHIP_HOPS
        ));
    }
    let attribute = walk_attribute_path(start, path, by_code, referenced)?;
    if attribute.value_type == "relationship" {
        Ok(attribute)
    } else {
        Err(format!("relationship facet '{path}' is not a relationship"))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextFile {
    format_version: u32,
    kind: String,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    publication_channel: Option<PublicationChannelDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationChannelDeclaration {
    enabled: bool,
    #[serde(default)]
    required_rules: Vec<String>,
    #[serde(default)]
    require_valid_entity: bool,
}

fn validate_context(
    resource: &SolutionPackResource,
    bytes: &[u8],
) -> Result<SeedContext, SolutionPackError> {
    let key = resource.key.as_str();
    utf8_source(key, bytes)?;
    let file: ContextFile = serde_json::from_slice(bytes)
        .map_err(|_| SolutionPackError::Invalid(format!("context '{key}' is not strict JSON")))?;
    if file.format_version != 1 || file.kind != "solution_pack_context" {
        return invalid(format!(
            "context '{key}' format_version and kind are unsupported"
        ));
    }
    let data = file.data.unwrap_or_else(|| serde_json::json!({}));
    if !data.is_object()
        || serde_json::to_vec(&data)
            .expect("JSON value serializes")
            .len()
            > MAX_CONTEXT_DATA_BYTES
    {
        return invalid(format!(
            "context '{key}' data must be a JSON object of at most {} KiB",
            MAX_CONTEXT_DATA_BYTES / 1024
        ));
    }
    if let Some(parent) = &file.parent {
        validate_namespaced_key(parent, "contexts")?;
        if parent == key {
            return invalid(format!("context '{key}' cannot be its own parent"));
        }
    }
    Ok(SeedContext {
        key: key.to_owned(),
        code: resource_code(key).to_owned(),
        data,
        parent: file.parent,
        publication_channel: file
            .publication_channel
            .map(|declaration| -> Result<_, SolutionPackError> {
                let unique = declaration.required_rules.iter().collect::<BTreeSet<_>>();
                if declaration.required_rules.len() > MAX_CHANNEL_REQUIRED_RULES
                    || unique.len() != declaration.required_rules.len()
                {
                    return invalid(format!(
                        "context '{key}' channel must require at most {MAX_CHANNEL_REQUIRED_RULES} unique rules"
                    ));
                }
                for rule in &declaration.required_rules {
                    validate_namespaced_key(rule, "rules")?;
                }
                Ok(SeedPublicationChannel {
                    enabled: declaration.enabled,
                    required_rules: declaration.required_rules,
                    require_valid_entity: declaration.require_valid_entity,
                })
            })
            .transpose()?,
    })
}

/// Summaries of a pack's seed resources for inspection.
pub fn seed_inspection_summary(pack: &ValidatedSolutionPack) -> Value {
    let manifest = pack.manifest();
    serde_json::json!({
        "prerequisites": manifest.prerequisites.iter().map(|prerequisite| serde_json::json!({
            "key": prerequisite.key,
            "id": prerequisite.id,
            "version": prerequisite.version,
            "blueprints": manifest.resources.blueprints.iter()
                .filter(|resource| resource.reuse.as_ref().is_some_and(|reuse| reuse.prerequisite == prerequisite.key))
                .map(|resource| resource.key.clone())
                .collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "contexts": pack.contexts().map(|context| serde_json::json!({
            "key": context.key,
            "parent": context.parent,
            "publication_channel": context.publication_channel.as_ref().map(|channel| serde_json::json!({
                "enabled": channel.enabled,
                "required_rules": channel.required_rules,
                "require_valid_entity": channel.require_valid_entity,
            })),
        })).collect::<Vec<_>>(),
        "rules": manifest.resources.rules.iter().map(|resource| {
            let rule = pack.rule(&resource.key).expect("validated rule");
            serde_json::json!({
                "key": resource.key,
                "required": resource.required,
                "blueprint": rule.blueprint,
                "context": rule.context,
                "enabled": rule.enabled,
                "severity": rule.compiled.severity,
            })
        }).collect::<Vec<_>>(),
        "workflows": manifest.resources.workflows.iter().map(|resource| {
            let workflow = pack.workflow(&resource.key).expect("validated workflow");
            serde_json::json!({
                "key": resource.key,
                "required": resource.required,
                "enabled": workflow.enabled,
            })
        }).collect::<Vec<_>>(),
        "saved_searches": manifest.resources.saved_searches.iter().map(|resource| {
            let search = pack.saved_search(&resource.key).expect("validated saved search");
            serde_json::json!({
                "key": resource.key,
                "required": resource.required,
                "name": search.name,
                "blueprint": search.blueprint,
                "context": search.context,
            })
        }).collect::<Vec<_>>(),
    })
}

/// The version requirement of a prerequisite, already validated.
pub fn prerequisite_version_req(prerequisite: &SolutionPackPrerequisite) -> VersionReq {
    parse_version_req(&prerequisite.version).expect("validated prerequisite version")
}

/// An installer's explicit choice to use an existing workspace context for a
/// pack context instead of creating one.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextMappingRequest {
    pub key: String,
    pub code: String,
}

pub fn validate_context_mapping_requests(
    pack: &ValidatedSolutionPack,
    mappings: &[ContextMappingRequest],
) -> Result<(), SolutionPackError> {
    if mappings.len() > MAX_SOLUTION_PACK_CONTEXTS {
        return invalid("too many context mappings");
    }
    let mut seen = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for mapping in mappings {
        if !codes.insert(mapping.code.as_str()) {
            return invalid(format!(
                "context '{}' is mapped to more than one pack context",
                mapping.code
            ));
        }
        if pack.context(&mapping.key).is_none() {
            return invalid(format!("unknown context mapping key '{}'", mapping.key));
        }
        if !seen.insert(mapping.key.as_str()) {
            return invalid(format!("duplicate context mapping key '{}'", mapping.key));
        }
        if mapping.code.is_empty()
            || mapping.code.len() > MAX_IDENTIFIER_BYTES
            || !catalog_validation::is_valid_code(&mapping.code)
        {
            return invalid(format!(
                "invalid existing context code for '{}'",
                mapping.key
            ));
        }
    }
    Ok(())
}

/// Logical keys of pack blueprints that a prerequisite seed provides.
pub fn reused_blueprints(
    pack: &ValidatedSolutionPack,
) -> impl Iterator<Item = (&SolutionPackResource, &SolutionPackBlueprintReuse)> {
    pack.manifest()
        .resources
        .blueprints
        .iter()
        .filter_map(|resource| resource.reuse.as_ref().map(|reuse| (resource, reuse)))
}
