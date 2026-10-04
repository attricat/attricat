//! Seed resources beyond blueprints: rules, workflows, saved searches,
//! contexts with publication channels, and prerequisite seeds.
//!
//! Like blueprints, every resource is validated offline against the pack's
//! own logical keys. Workspace identities are only chosen by a plan, and a
//! seed only provisions a new installation: there is no update, drift, or
//! removal handling for any of these resources.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use catalog_blueprint::BlueprintKind;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::solution_packs::{
    BlueprintPublication, MAX_IDENTIFIER_BYTES, MAX_VERSION_REQUIREMENT_BYTES, PlannedAction,
    PlannedMapping, SolutionPackBlueprint, SolutionPackError, SolutionPackManifest,
    SolutionPackResource, ValidatedSolutionPack, deterministic_target_id, invalid,
    is_valid_stable_code, parse_sha256, parse_version_req, resource_code, safe_archive_path,
    validate_acyclic, validate_pack_id,
};

pub const MAX_SOLUTION_PACK_RULES: usize = 64;
pub const MAX_SOLUTION_PACK_WORKFLOWS: usize = 64;
pub const MAX_SOLUTION_PACK_SAVED_SEARCHES: usize = 64;
pub const MAX_SOLUTION_PACK_CONTEXTS: usize = 32;
pub const MAX_SOLUTION_PACK_PREREQUISITES: usize = 16;
const MAX_SEED_SOURCE_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_DATA_BYTES: usize = 4 * 1024;
const MAX_SAVED_SEARCH_STATE_BYTES: usize = 32 * 1024;
const MAX_SAVED_SEARCH_NAME_BYTES: usize = 120;
const MAX_SAVED_SEARCH_DESCRIPTION_BYTES: usize = 500;
const MAX_SAVED_SEARCH_FILTERS: usize = 20;
/// Explorer sort fields that are not attribute paths.
const SYSTEM_SORT_FIELDS: &[&str] = &["blueprint_version", "publication_status"];
const SAVED_SEARCH_FILTER_OPERATORS: &[&str] =
    &["eq", "contains", "starts_with", "gt", "gte", "lt", "lte"];

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
    /// Whether the context is a publication channel, and if so whether that
    /// channel is enabled.
    pub publication_channel: Option<bool>,
}

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
        return invalid(format!("resource '{key}' exceeds the 64 KiB size limit"));
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

/// Collects every `attribute_code` string in a serialized predicate, so
/// attribute checks keep working when the predicate set grows.
fn predicate_attribute_codes(value: &Value, codes: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (field, value) in object {
                if field == "attribute_code"
                    && let Some(code) = value.as_str()
                {
                    codes.insert(code.to_owned());
                } else {
                    predicate_attribute_codes(value, codes);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                predicate_attribute_codes(value, codes);
            }
        }
        _ => {}
    }
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
    let mut attribute_codes = BTreeSet::new();
    predicate_attribute_codes(
        &serde_json::to_value(&compiled.predicate).expect("rule predicate serializes"),
        &mut attribute_codes,
    );
    for code in attribute_codes {
        if !target
            .effective_attributes()
            .iter()
            .any(|attribute| attribute.code == code)
        {
            return invalid(format!(
                "rule '{key}' references unknown attribute '{code}' of '{blueprint}'"
            ));
        }
    }
    Ok(SeedRule {
        key: key.to_owned(),
        code: compiled.code.clone(),
        blueprint,
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
    for action in &compiled.actions {
        if let catalog_workflow::Action::AttributeWrite { attribute_code, .. } = action
            && !blueprints.values().any(|blueprint| {
                blueprint
                    .effective_attributes()
                    .iter()
                    .any(|attribute| &attribute.code == attribute_code)
            })
        {
            return invalid(format!(
                "workflow '{key}' writes attribute '{attribute_code}' that no pack blueprint declares"
            ));
        }
    }
    Ok(SeedWorkflow {
        key: key.to_owned(),
        code: compiled.code.clone(),
        enabled,
        definition,
        compiled,
    })
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
    if serde_json::to_vec(&file.state)
        .expect("JSON value serializes")
        .len()
        > MAX_SAVED_SEARCH_STATE_BYTES
    {
        return Err(invalid_state("state exceeds 32 KiB"));
    }
    let state = file
        .state
        .as_object()
        .ok_or_else(|| invalid_state("state must be an object"))?;
    const KEYS: &[&str] = &[
        "blueprint",
        "query",
        "context",
        "allVersions",
        "locked",
        "sort",
        "attributeFilters",
        "relationshipFacets",
    ];
    if let Some(unknown) = state.keys().find(|field| !KEYS.contains(&field.as_str())) {
        return Err(invalid_state(&format!(
            "state field '{unknown}' cannot be seeded"
        )));
    }
    let blueprint = state
        .get("blueprint")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_state("state must reference a blueprint"))?
        .to_owned();
    let target = entity_blueprint(key, &blueprint, blueprints)?;
    let mut referenced_blueprints = BTreeSet::from([blueprint.clone()]);
    let context = state
        .get("context")
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_state("context must be a logical key"))
        })
        .transpose()?;
    validate_context_reference(key, context.as_deref(), contexts)?;
    if state
        .get("query")
        .is_some_and(|value| value.as_str().is_none_or(|query| query.trim().is_empty()))
    {
        return Err(invalid_state("query must be a non-empty string"));
    }
    for flag in ["allVersions", "locked"] {
        if state.get(flag).is_some_and(|value| !value.is_boolean()) {
            return Err(invalid_state(&format!("{flag} must be a boolean")));
        }
    }
    let by_code = blueprints
        .values()
        .map(|blueprint| (blueprint.code(), blueprint))
        .collect::<HashMap<_, _>>();
    if let Some(sort) = state.get("sort") {
        let sort = sort
            .as_object()
            .filter(|sort| sort.len() == 2)
            .ok_or_else(|| invalid_state("sort must contain only field and direction"))?;
        let field = sort
            .get("field")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_state("sort field is missing"))?;
        if !matches!(
            sort.get("direction").and_then(Value::as_str),
            Some("asc" | "desc")
        ) {
            return Err(invalid_state("sort direction must be asc or desc"));
        }
        if !SYSTEM_SORT_FIELDS.contains(&field) {
            validate_attribute_path(target, field, &by_code, &mut referenced_blueprints)
                .map_err(|reason| invalid_state(&reason))?;
        }
    }
    if let Some(filters) = state.get("attributeFilters") {
        let filters = filters
            .as_array()
            .filter(|filters| filters.len() <= MAX_SAVED_SEARCH_FILTERS)
            .ok_or_else(|| invalid_state("attributeFilters must be an array of at most 20"))?;
        for filter in filters {
            let filter = filter
                .as_object()
                .filter(|filter| {
                    filter
                        .keys()
                        .all(|field| matches!(field.as_str(), "field" | "operator" | "value"))
                })
                .ok_or_else(|| invalid_state("attribute filter is invalid"))?;
            let field = filter
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_state("attribute filter field is missing"))?;
            if !filter
                .get("operator")
                .and_then(Value::as_str)
                .is_some_and(|operator| SAVED_SEARCH_FILTER_OPERATORS.contains(&operator))
                || !filter.get("value").is_some_and(|value| {
                    value.is_string() || value.is_number() || value.is_boolean()
                })
            {
                return Err(invalid_state(
                    "attribute filter operator or value is invalid",
                ));
            }
            validate_attribute_path(target, field, &by_code, &mut referenced_blueprints)
                .map_err(|reason| invalid_state(&reason))?;
        }
    }
    if let Some(facets) = state.get("relationshipFacets") {
        let facets = facets
            .as_array()
            .filter(|facets| facets.len() <= MAX_SAVED_SEARCH_FILTERS)
            .ok_or_else(|| invalid_state("relationshipFacets must be an array of at most 20"))?;
        for facet in facets {
            let facet = facet
                .as_object()
                .filter(|facet| {
                    facet
                        .keys()
                        .all(|field| matches!(field.as_str(), "field" | "targetBlueprint"))
                })
                .ok_or_else(|| {
                    invalid_state("relationship facets cannot seed selected entity IDs")
                })?;
            let field = facet
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_state("relationship facet field is missing"))?;
            if field.is_empty()
                || field.len() > 512
                || !field.split('.').all(catalog_validation::is_valid_code)
            {
                return Err(invalid_state("relationship facet field is invalid"));
            }
            if let Some(target) = facet.get("targetBlueprint") {
                let target = target
                    .as_str()
                    .ok_or_else(|| invalid_state("targetBlueprint must be a logical key"))?;
                entity_blueprint(key, target, blueprints)?;
                referenced_blueprints.insert(target.to_owned());
            }
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

/// Validates an Explorer attribute path: relationship segments through pack
/// blueprints ending in an attribute of the last blueprint.
fn validate_attribute_path(
    start: &SolutionPackBlueprint,
    path: &str,
    by_code: &HashMap<&str, &SolutionPackBlueprint>,
    referenced: &mut BTreeSet<String>,
) -> Result<(), String> {
    let segments = path.split('.').collect::<Vec<_>>();
    let mut current = start;
    for (index, segment) in segments.iter().enumerate() {
        let attribute = current
            .effective_attributes()
            .iter()
            .find(|attribute| attribute.code == *segment)
            .ok_or_else(|| format!("field '{path}' references unknown attribute '{segment}'"))?;
        if index + 1 == segments.len() {
            return Ok(());
        }
        let target = attribute
            .target_blueprint
            .as_deref()
            .filter(|_| attribute.value_type == "relationship")
            .and_then(|code| by_code.get(code))
            .ok_or_else(|| {
                format!("field '{path}' segment '{segment}' is not a pack relationship")
            })?;
        referenced.insert(target.key().to_owned());
        current = target;
    }
    Err(format!("field '{path}' is empty"))
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
            "context '{key}' data must be a JSON object of at most 4 KiB"
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
            .map(|declaration| declaration.enabled),
    })
}

/// Logical keys of the plan actions this module adds for contexts.
pub fn publication_channel_key(context_key: &str) -> String {
    format!("channels/{}", resource_code(context_key))
}

#[derive(Clone, Debug)]
pub struct ExistingContextSnapshot {
    pub id: uuid::Uuid,
    pub code: String,
    /// `Some(enabled)` when the context is already a publication channel.
    pub publication_channel_enabled: Option<bool>,
}

/// How a prerequisite seed is satisfied by the workspace's completed
/// applications. Resolved by the repository; the planner never searches.
#[derive(Clone, Debug)]
pub enum PrerequisiteResolution {
    Satisfied {
        application_id: uuid::Uuid,
        pack_version: String,
    },
    /// No completed application of the prerequisite pack exists.
    Missing,
    /// Completed applications exist, but none has a version in range.
    Incompatible { versions: Vec<String> },
}

/// Workspace facts for seed resources, collected by the repository in the
/// planning transaction.
#[derive(Clone, Debug, Default)]
pub struct SeedWorkspaceSnapshot {
    /// Explicit context selections keyed by pack-local context key.
    pub existing_contexts: BTreeMap<String, ExistingContextSnapshot>,
    pub rule_codes: BTreeSet<String>,
    pub workflow_codes: BTreeSet<String>,
    pub prerequisites: BTreeMap<String, PrerequisiteResolution>,
}

pub(crate) type Outcome = (&'static str, &'static str);

fn optional_outcome(required: bool, outcome: Outcome) -> Outcome {
    if !required && matches!(outcome.0, "conflict" | "blocked") {
        ("skip", outcome.1)
    } else {
        outcome
    }
}

fn available(outcome: Option<&Outcome>) -> bool {
    outcome.is_some_and(|(action, _)| matches!(*action, "create" | "map"))
}

fn physical_code(prefix: &str, key: &str) -> Result<String, SolutionPackError> {
    let code = format!("{prefix}_{}", resource_code(key));
    if code.len() > MAX_IDENTIFIER_BYTES || !is_valid_stable_code(&code) {
        return invalid(format!(
            "prefix produces an invalid physical code for '{key}'"
        ));
    }
    Ok(code)
}

fn target_absent(resource_kind: &str, code: &str) -> Value {
    serde_json::json!([{"kind": "target_absent", "resource_kind": resource_kind, "code": code}])
}

/// Plans prerequisite seeds. These actions are evidence that the reused
/// blueprints come from a reviewed, completed application.
pub(crate) fn plan_prerequisites(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace_id: uuid::Uuid,
    seed: &SeedWorkspaceSnapshot,
) -> (
    Vec<PlannedMapping>,
    Vec<PlannedAction>,
    BTreeMap<String, Outcome>,
) {
    let mut mappings = Vec::new();
    let mut actions = Vec::new();
    let mut outcomes = BTreeMap::new();
    for prerequisite in &pack.manifest().prerequisites {
        let resolution = seed
            .prerequisites
            .get(&prerequisite.key)
            .cloned()
            .unwrap_or(PrerequisiteResolution::Missing);
        let (target_id, snapshot, outcome, preconditions) = match &resolution {
            PrerequisiteResolution::Satisfied {
                application_id,
                pack_version,
            } => (
                *application_id,
                serde_json::json!({
                    "application_id": application_id,
                    "pack_id": prerequisite.id,
                    "pack_version": pack_version,
                }),
                ("map", "prerequisite_satisfied"),
                serde_json::json!([{
                    "kind": "prerequisite_application",
                    "application_id": application_id,
                    "pack_id": prerequisite.id,
                    "pack_version": pack_version,
                }]),
            ),
            PrerequisiteResolution::Missing => (
                deterministic_target_id(workspace_id, pack, prefix, publication, &prerequisite.key),
                serde_json::json!({"pack_id": prerequisite.id, "available_versions": []}),
                ("blocked", "prerequisite_missing"),
                serde_json::json!([]),
            ),
            PrerequisiteResolution::Incompatible { versions } => (
                deterministic_target_id(workspace_id, pack, prefix, publication, &prerequisite.key),
                serde_json::json!({"pack_id": prerequisite.id, "available_versions": versions}),
                ("blocked", "prerequisite_incompatible"),
                serde_json::json!([]),
            ),
        };
        outcomes.insert(prerequisite.key.clone(), outcome);
        mappings.push(PlannedMapping {
            resource_kind: "prerequisite",
            logical_key: prerequisite.key.clone(),
            target_id,
            target_code: prerequisite.id.clone(),
            target_version: None,
            mapping_kind: "existing",
            snapshot: snapshot.clone(),
        });
        actions.push(PlannedAction {
            resource_kind: "prerequisite",
            logical_key: prerequisite.key.clone(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "pack_id": prerequisite.id,
                "version_requirement": prerequisite.version,
                "resolution": snapshot,
            }),
            normalized_payload: None,
            preconditions,
        });
    }
    (mappings, actions, outcomes)
}

/// Context mappings and actions, plus each context's outcome and mapping by key.
pub(crate) type PlannedContexts = (
    Vec<PlannedMapping>,
    Vec<PlannedAction>,
    BTreeMap<String, (Outcome, PlannedMapping)>,
);

/// Plans contexts (parents first) and their publication channels.
pub(crate) fn plan_contexts(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace_id: uuid::Uuid,
    physical_codes: &BTreeSet<String>,
    blueprint_codes: &BTreeSet<String>,
    seed: &SeedWorkspaceSnapshot,
) -> Result<PlannedContexts, SolutionPackError> {
    let resources = pack
        .manifest()
        .resources
        .contexts
        .iter()
        .map(|resource| (resource.key.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = Vec::new();
    let mut visited = BTreeSet::new();
    fn visit<'a>(
        key: &'a str,
        pack: &'a ValidatedSolutionPack,
        visited: &mut BTreeSet<&'a str>,
        ordered: &mut Vec<&'a str>,
    ) {
        if !visited.insert(key) {
            return;
        }
        if let Some(parent) = pack
            .context(key)
            .and_then(|context| context.parent.as_deref())
        {
            visit(parent, pack, visited, ordered);
        }
        ordered.push(key);
    }
    for key in resources.keys() {
        visit(key, pack, &mut visited, &mut ordered);
    }

    let mut planned = BTreeMap::<String, (Outcome, PlannedMapping)>::new();
    let mut mappings = Vec::new();
    let mut actions = Vec::new();
    let mut generated = HashMap::<String, usize>::new();
    for key in &ordered {
        if !seed.existing_contexts.contains_key(*key) {
            *generated.entry(physical_code(prefix, key)?).or_default() += 1;
        }
    }
    for key in ordered {
        let resource = resources[key];
        let context = pack.context(key).expect("validated context exists");
        let parent = context
            .parent
            .as_ref()
            .map(|parent| planned.get(parent).expect("parents are planned first"));
        let (mapping, outcome, payload, preconditions) = if let Some(existing) =
            seed.existing_contexts.get(key)
        {
            (
                PlannedMapping {
                    resource_kind: "context",
                    logical_key: key.to_owned(),
                    target_id: existing.id,
                    target_code: existing.code.clone(),
                    target_version: None,
                    mapping_kind: "existing",
                    snapshot: serde_json::json!({"id": existing.id, "code": existing.code}),
                },
                ("map", "existing_context_selected"),
                None,
                serde_json::json!([{"kind": "existing_context", "id": existing.id, "code": existing.code}]),
            )
        } else {
            let code = physical_code(prefix, key)?;
            let mapping = PlannedMapping {
                resource_kind: "context",
                logical_key: key.to_owned(),
                target_id: deterministic_target_id(workspace_id, pack, prefix, publication, key),
                target_code: code.clone(),
                target_version: None,
                mapping_kind: "create",
                snapshot: serde_json::json!({"code": code}),
            };
            let outcome = if generated[&code] > 1 || blueprint_codes.contains(&code) {
                ("conflict", "duplicate_target_code")
            } else if physical_codes.contains(&code) {
                ("conflict", "target_code_exists")
            } else if parent.is_some_and(|(outcome, _)| !available(Some(outcome))) {
                ("blocked", "dependency_not_creatable")
            } else {
                ("create", "target_absent")
            };
            let payload = serde_json::json!({
                "code": code,
                "data": context.data,
                "parent_id": parent.map(|(_, mapping)| mapping.target_id),
            });
            let preconditions = target_absent("context", &code);
            (mapping, outcome, Some(payload), preconditions)
        };
        let outcome = optional_outcome(resource.required, outcome);
        actions.push(PlannedAction {
            resource_kind: "context",
            logical_key: key.to_owned(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "target_code": mapping.target_code,
                "required": resource.required,
                "parent": context.parent,
                "publication_channel": context.publication_channel,
            }),
            normalized_payload: payload.filter(|_| outcome.0 == "create"),
            preconditions: if matches!(outcome.0, "create" | "conflict" | "map") {
                preconditions
            } else {
                serde_json::json!([])
            },
        });
        mappings.push(mapping.clone());
        planned.insert(key.to_owned(), (outcome, mapping));
    }

    for context in pack.contexts() {
        let Some(enabled) = context.publication_channel else {
            continue;
        };
        let required = resources[context.key.as_str()].required;
        let (context_outcome, context_mapping) = &planned[&context.key];
        let existing = seed
            .existing_contexts
            .get(&context.key)
            .and_then(|existing| existing.publication_channel_enabled);
        let outcome = if !available(Some(context_outcome)) {
            ("blocked", "dependency_not_creatable")
        } else if existing == Some(enabled) {
            ("satisfied", "exact_match")
        } else if existing.is_some() {
            ("conflict", "publication_channel_mismatch")
        } else {
            ("create", "target_absent")
        };
        let outcome = optional_outcome(required, outcome);
        let key = publication_channel_key(&context.key);
        let payload = serde_json::json!({
            "context_id": context_mapping.target_id,
            "context_code": context_mapping.target_code,
            "enabled": enabled,
        });
        mappings.push(PlannedMapping {
            resource_kind: "publication_channel",
            logical_key: key.clone(),
            target_id: context_mapping.target_id,
            target_code: context_mapping.target_code.clone(),
            target_version: None,
            mapping_kind: if existing.is_some() {
                "existing"
            } else {
                "create"
            },
            snapshot: serde_json::json!({"context": context.key, "enabled": existing}),
        });
        actions.push(PlannedAction {
            resource_kind: "publication_channel",
            logical_key: key,
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "context": context.key,
                "context_code": context_mapping.target_code,
                "enabled": enabled,
                "current_enabled": existing,
                "required": required,
            }),
            normalized_payload: matches!(outcome.0, "create" | "satisfied").then_some(payload),
            preconditions: if matches!(outcome.0, "create" | "conflict") {
                target_absent("publication_channel", &context_mapping.target_code)
            } else {
                serde_json::json!([])
            },
        });
    }
    Ok((mappings, actions, planned))
}

/// Inputs shared by the rule, workflow, and saved-search planners.
pub(crate) struct DependentPlanInput<'a> {
    pub pack: &'a ValidatedSolutionPack,
    pub prefix: &'a str,
    pub publication: BlueprintPublication,
    pub workspace_id: uuid::Uuid,
    pub seed: &'a SeedWorkspaceSnapshot,
    pub blueprint_outcomes: &'a HashMap<String, Outcome>,
    pub blueprint_mappings: &'a BTreeMap<String, PlannedMapping>,
    pub contexts: &'a BTreeMap<String, (Outcome, PlannedMapping)>,
}

impl DependentPlanInput<'_> {
    fn blueprint_available(&self, key: &str) -> bool {
        available(self.blueprint_outcomes.get(key))
    }

    fn context_available(&self, key: Option<&str>) -> bool {
        key.is_none_or(|key| available(self.contexts.get(key).map(|(outcome, _)| outcome)))
    }

    fn context_mapping(&self, key: Option<&str>) -> Option<&PlannedMapping> {
        key.map(|key| &self.contexts[key].1)
    }

    fn target_id(&self, key: &str) -> uuid::Uuid {
        deterministic_target_id(
            self.workspace_id,
            self.pack,
            self.prefix,
            self.publication,
            key,
        )
    }
}

fn replace_code(definition: &str, code: &str) -> Result<String, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(definition)
        .map_err(|_| SolutionPackError::Invalid("validated definition is invalid".into()))?;
    value
        .as_table_mut()
        .expect("validated definition is a table")
        .insert("code".into(), toml::Value::String(code.to_owned()));
    toml::to_string(&value)
        .map_err(|_| SolutionPackError::Invalid("validated definition is invalid".into()))
}

pub(crate) fn plan_dependents(
    input: &DependentPlanInput<'_>,
) -> Result<(Vec<PlannedMapping>, Vec<PlannedAction>), SolutionPackError> {
    let mut mappings = Vec::new();
    let mut actions = Vec::new();
    let manifest = input.pack.manifest();

    for resource in &manifest.resources.rules {
        let rule = input.pack.rule(&resource.key).expect("validated rule");
        let code = physical_code(input.prefix, &resource.key)?;
        let blueprint_outcome = input.blueprint_outcomes.get(&rule.blueprint);
        let outcome = if !input.blueprint_available(&rule.blueprint)
            || !input.context_available(rule.context.as_deref())
        {
            ("blocked", "dependency_not_creatable")
        } else if blueprint_outcome.is_some_and(|(action, _)| *action == "create")
            && input.publication != BlueprintPublication::Publish
        {
            ("blocked", "blueprint_not_published")
        } else if input.seed.rule_codes.contains(&code) {
            ("conflict", "target_code_exists")
        } else {
            ("create", "target_absent")
        };
        let outcome = optional_outcome(resource.required, outcome);
        let blueprint = &input.blueprint_mappings[&rule.blueprint];
        let context = input.context_mapping(rule.context.as_deref());
        let payload = (outcome.0 == "create")
            .then(|| -> Result<Value, SolutionPackError> {
                Ok(serde_json::json!({
                    "definition": replace_code(&rule.definition, &code)?,
                    "blueprint_id": blueprint.target_id,
                    "blueprint_version": blueprint.target_version,
                    "context_id": context.map(|context| context.target_id),
                    "enabled": rule.enabled,
                }))
            })
            .transpose()?;
        mappings.push(PlannedMapping {
            resource_kind: "rule",
            logical_key: resource.key.clone(),
            target_id: input.target_id(&resource.key),
            target_code: code.clone(),
            target_version: Some(1),
            mapping_kind: "create",
            snapshot: serde_json::json!({"code": code, "version": 1}),
        });
        actions.push(PlannedAction {
            resource_kind: "rule",
            logical_key: resource.key.clone(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "target_code": code,
                "required": resource.required,
                "blueprint": rule.blueprint,
                "blueprint_code": blueprint.target_code,
                "context": rule.context,
                "context_code": context.map(|context| context.target_code.clone()),
                "enabled": rule.enabled,
                "severity": rule.compiled.severity,
                "predicate": serde_json::to_value(&rule.compiled.predicate)
                    .expect("rule predicate serializes")["type"],
            }),
            normalized_payload: payload,
            preconditions: if matches!(outcome.0, "create" | "conflict") {
                target_absent("rule", &code)
            } else {
                serde_json::json!([])
            },
        });
    }

    for resource in &manifest.resources.workflows {
        let workflow = input
            .pack
            .workflow(&resource.key)
            .expect("validated workflow");
        let code = physical_code(input.prefix, &resource.key)?;
        let outcome = if input.seed.workflow_codes.contains(&code) {
            ("conflict", "target_code_exists")
        } else {
            ("create", "target_absent")
        };
        let outcome = optional_outcome(resource.required, outcome);
        let payload = (outcome.0 == "create")
            .then(|| -> Result<Value, SolutionPackError> {
                Ok(serde_json::json!({
                    "definition": replace_code(&workflow.definition, &code)?,
                    "enabled": workflow.enabled,
                }))
            })
            .transpose()?;
        mappings.push(PlannedMapping {
            resource_kind: "workflow",
            logical_key: resource.key.clone(),
            target_id: input.target_id(&resource.key),
            target_code: code.clone(),
            target_version: Some(1),
            mapping_kind: "create",
            snapshot: serde_json::json!({"code": code, "version": 1}),
        });
        actions.push(PlannedAction {
            resource_kind: "workflow",
            logical_key: resource.key.clone(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "target_code": code,
                "required": resource.required,
                "enabled": workflow.enabled,
                "triggers": workflow.compiled.triggers.iter().map(|trigger| {
                    serde_json::to_value(trigger).expect("workflow trigger serializes")["type"].clone()
                }).collect::<Vec<_>>(),
                "action_count": workflow.compiled.actions.len(),
            }),
            normalized_payload: payload,
            preconditions: if matches!(outcome.0, "create" | "conflict") {
                target_absent("workflow", &code)
            } else {
                serde_json::json!([])
            },
        });
    }

    for resource in &manifest.resources.saved_searches {
        let search = input
            .pack
            .saved_search(&resource.key)
            .expect("validated saved search");
        let outcome = if !search
            .referenced_blueprints
            .iter()
            .all(|key| input.blueprint_available(key))
            || !input.context_available(search.context.as_deref())
        {
            ("blocked", "dependency_not_creatable")
        } else {
            ("create", "target_absent")
        };
        let outcome = optional_outcome(resource.required, outcome);
        let blueprint = &input.blueprint_mappings[&search.blueprint];
        let context = input.context_mapping(search.context.as_deref());
        let payload = (outcome.0 == "create").then(|| {
            serde_json::json!({
                "name": search.name,
                "description": search.description,
                "visibility": "workspace",
                "state": physical_search_state(search, input.blueprint_mappings, context),
            })
        });
        mappings.push(PlannedMapping {
            resource_kind: "saved_search",
            logical_key: resource.key.clone(),
            target_id: input.target_id(&resource.key),
            target_code: "saved_search".to_owned(),
            target_version: None,
            mapping_kind: "create",
            snapshot: serde_json::json!({}),
        });
        actions.push(PlannedAction {
            resource_kind: "saved_search",
            logical_key: resource.key.clone(),
            action: outcome.0,
            reason_code: outcome.1,
            summary: serde_json::json!({
                "required": resource.required,
                "name": search.name,
                "blueprint": search.blueprint,
                "blueprint_code": blueprint.target_code,
                "context": search.context,
                "visibility": "workspace",
            }),
            normalized_payload: payload,
            preconditions: if matches!(outcome.0, "create" | "conflict") {
                target_absent("saved_search", "saved_search")
            } else {
                serde_json::json!([])
            },
        });
    }
    Ok((mappings, actions))
}

/// Explorer state with physical blueprint and context codes, normalized like
/// ordinary saved-search writes.
fn physical_search_state(
    search: &SeedSavedSearch,
    blueprints: &BTreeMap<String, PlannedMapping>,
    context: Option<&PlannedMapping>,
) -> Value {
    let mut state = search.state.clone();
    let object = state.as_object_mut().expect("validated state is an object");
    object.insert(
        "blueprint".into(),
        Value::String(blueprints[&search.blueprint].target_code.clone()),
    );
    match context {
        Some(context) if context.target_code != "default" => {
            object.insert("context".into(), Value::String(context.target_code.clone()));
        }
        _ => {
            object.remove("context");
        }
    }
    if let Some(Value::Array(facets)) = object.get_mut("relationshipFacets") {
        for facet in facets {
            if let Some(Value::String(target)) = facet.get_mut("targetBlueprint") {
                *target = blueprints[target.as_str()].target_code.clone();
            }
        }
    }
    if let Some(Value::String(query)) = object.get_mut("query") {
        *query = query.trim().to_owned();
    }
    for flag in ["allVersions", "locked"] {
        if object.get(flag) == Some(&Value::Bool(false)) {
            object.remove(flag);
        }
    }
    for list in ["attributeFilters", "relationshipFacets"] {
        if object
            .get(list)
            .is_some_and(|value| value.as_array().is_some_and(Vec::is_empty))
        {
            object.remove(list);
        }
    }
    state
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
            "publication_channel": context.publication_channel.map(|enabled| serde_json::json!({"enabled": enabled})),
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
