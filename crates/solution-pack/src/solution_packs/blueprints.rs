//! Blueprint resources: pack-local references are rewritten, includes are
//! resolved within the pack, and every blueprint is compiled offline.

use attricat_domain::value_type::ValueType;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use attricat_blueprint::{
    BlueprintDefinition, BlueprintError, BlueprintKind, BrowseBy, CompiledBlueprint,
    ResolvedInclude, ViewDefinition, ViewNode,
};
use attricat_validation::relationship_path::{CodeMatch, WalkError, resolve_in, walk_path};
use attricat_validation::saved_search::browse_hop;
use serde_json::Value;

use super::{
    MAX_IDENTIFIER_BYTES, MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES,
    MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES, MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES,
    MAX_SOLUTION_PACK_TOTAL_BLUEPRINT_COMPLEXITY, SolutionPackBlueprint,
    SolutionPackBlueprintInclude, SolutionPackError, SolutionPackManifest, invalid, resource_code,
    validate_acyclic,
};

type ValidatedContent = BTreeMap<String, SolutionPackBlueprint>;

struct PreparedBlueprint {
    portable: SolutionPackBlueprint,
    native_definition: BlueprintDefinition,
    native_source: String,
    include_keys: Vec<String>,
}

pub(super) fn validate_content(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedContent, SolutionPackError> {
    let blueprint_codes = manifest
        .resources
        .blueprints
        .iter()
        .map(|resource| (resource.key.as_str(), resource_code(&resource.key)))
        .collect::<HashMap<_, _>>();

    let mut prepared = BTreeMap::new();
    for resource in &manifest.resources.blueprints {
        let source = std::str::from_utf8(&files[&resource.path]).map_err(|_| {
            SolutionPackError::Invalid(format!("blueprint '{}' must be UTF-8 YAML", resource.key))
        })?;
        let blueprint = prepare_blueprint(&resource.key, source, &blueprint_codes)?;
        prepared.insert(resource.key.clone(), blueprint);
    }

    let total_blueprint_complexity = prepared.values().try_fold(0usize, |total, blueprint| {
        total
            .checked_add(blueprint.native_definition.attributes.len())
            .and_then(|total| total.checked_add(blueprint.native_definition.includes.len()))
            .ok_or_else(|| {
                SolutionPackError::Invalid("solution-pack blueprint complexity is too large".into())
            })
    })?;
    if total_blueprint_complexity > MAX_SOLUTION_PACK_TOTAL_BLUEPRINT_COMPLEXITY {
        return invalid("solution-pack blueprint complexity exceeds the total limit");
    }

    let blueprint_dependencies = prepared
        .iter()
        .map(|(key, blueprint)| {
            (
                key.as_str(),
                blueprint
                    .include_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    validate_acyclic(&blueprint_dependencies, "blueprint include")?;

    let mut compiled = HashMap::new();
    for key in prepared.keys() {
        compile_pack_blueprint(key, &prepared, &mut compiled)?;
    }
    validate_incoming_relationships(&compiled)?;
    let table_path_dependencies = validate_table_paths(&compiled)?;
    let default_from_dependencies = validate_default_from_paths(&compiled)?;
    let blueprints = prepared
        .into_iter()
        .map(|(key, mut blueprint)| {
            blueprint.portable.table_path_dependencies = table_path_dependencies[&key].clone();
            blueprint.portable.default_from_dependencies = default_from_dependencies[&key].clone();
            blueprint.portable.effective_attributes = compiled[&key].attributes.clone();
            blueprint.portable.unique_keys = compiled[&key].unique_keys.clone();
            (key, blueprint.portable)
        })
        .collect();

    Ok(blueprints)
}

/// Visits the blueprint codes named by predicates embedded in a blueprint
/// definition: its record checks (`x-attricat-checks`) and status transition
/// conditions. Packs name these blueprints by pack-local code.
pub(super) fn visit_embedded_predicate_blueprint_codes(
    table: &mut serde_json::Map<String, Value>,
    visit: &mut dyn FnMut(&mut String) -> Result<(), SolutionPackError>,
) -> Result<(), SolutionPackError> {
    fn json_string(
        schema: &mut Value,
        visit: &mut dyn FnMut(&mut String) -> Result<(), SolutionPackError>,
    ) -> Result<(), SolutionPackError> {
        let Value::String(source) = schema else {
            return Ok(());
        };
        // Malformed JSON is reported by the ordinary blueprint parser.
        let Ok(original) = serde_json::from_str::<Value>(source) else {
            return Ok(());
        };
        let mut rewritten = original.clone();
        attricat_validation::predicate::visit_schema_blueprint_codes(&mut rewritten, visit)?;
        if rewritten != original {
            *source = serde_json::to_string(&rewritten).expect("JSON value serializes");
        }
        Ok(())
    }
    if let Some(schema) = table.get_mut("record_schema") {
        json_string(schema, visit)?;
    }
    if let Some(attributes) = table.get_mut("attributes").and_then(Value::as_array_mut) {
        for attribute in attributes {
            if let Some(schema) = attribute.get_mut("value_schema") {
                json_string(schema, visit)?;
            }
        }
    }
    Ok(())
}

fn table_of(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    value.as_object_mut().expect("blueprint source is a table")
}

fn prepare_blueprint(
    key: &str,
    source: &str,
    blueprint_codes: &HashMap<&str, &str>,
) -> Result<PreparedBlueprint, SolutionPackError> {
    let mut value: Value = attricat_yaml::from_str(source).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint YAML"
        ))
    })?;
    let table = value.as_object_mut().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint YAML"
        ))
    })?;
    if table.contains_key("extensions") {
        return invalid(format!(
            "blueprint '{key}' cannot contain extension metadata in solution-pack v1"
        ));
    }
    if table.contains_key("rules") {
        return invalid(format!(
            "blueprint '{key}' cannot declare rules; declare them as pack rules"
        ));
    }

    let mut portable_includes = Vec::new();
    let mut include_keys = Vec::new();
    let mut dependencies = BTreeSet::new();
    if let Some(includes) = table.get_mut("includes") {
        let includes = includes.as_array_mut().ok_or_else(|| {
            SolutionPackError::Invalid(format!(
                "blueprint '{key}' includes must use portable logical references"
            ))
        })?;
        for include in includes {
            let include = include.as_object_mut().ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' includes must use portable logical references"
                ))
            })?;
            if include.contains_key("code") || include.contains_key("version") {
                return invalid(format!(
                    "blueprint '{key}' include cannot contain native code or revision fields"
                ));
            }
            let alias = include
                .get("alias")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "blueprint '{key}' include must declare an alias"
                    ))
                })?
                .to_owned();
            let dependency = include
                .remove("key")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "blueprint '{key}' include must reference a logical key"
                    ))
                })?;
            let dependency_code = blueprint_codes.get(dependency.as_str()).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' references undeclared include '{dependency}'"
                ))
            })?;
            include.insert("code".into(), Value::String((*dependency_code).into()));
            include.insert("version".into(), Value::from(1));
            portable_includes.push(SolutionPackBlueprintInclude {
                alias,
                key: dependency.clone(),
            });
            dependencies.insert(dependency.clone());
            include_keys.push(dependency);
        }
    }

    // A movement's target and factor blueprints are published before the
    // blueprint, by logical key; the factor's is its relationship's target.
    let logical_targets: HashMap<String, String> = table
        .get("attributes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|attribute| {
            Some((
                attribute.get("code")?.as_str()?.to_owned(),
                attribute.get("target_blueprint")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    let mut movement_dependencies = BTreeSet::new();
    if let Some(movements) = table.get_mut("movements").and_then(Value::as_array_mut) {
        for movement in movements {
            let Some(movement) = movement.as_object_mut() else {
                continue;
            };
            if let Some(factor) = movement
                .get("factor")
                .and_then(|factor| factor.get("relationship"))
                .and_then(Value::as_str)
                .and_then(|relationship| logical_targets.get(relationship))
            {
                movement_dependencies.insert(factor.clone());
            }
            if let Some(target) = movement.get_mut("target").and_then(Value::as_object_mut) {
                if let Some(logical_key) = target.get("blueprint").and_then(Value::as_str) {
                    movement_dependencies.insert(logical_key.to_owned());
                }
                rewrite_blueprint_reference(
                    key,
                    target,
                    "blueprint",
                    "movement target",
                    blueprint_codes,
                    Some(&mut dependencies),
                )?;
            }
        }
    }
    // A movement never depends on its own blueprint for ordering.
    movement_dependencies.remove(key);

    if let Some(attributes) = table.get_mut("attributes").and_then(Value::as_array_mut) {
        for attribute in attributes {
            if let Some(attribute) = attribute.as_object_mut() {
                rewrite_blueprint_reference(
                    key,
                    attribute,
                    "target_blueprint",
                    "relationship target",
                    blueprint_codes,
                    Some(&mut dependencies),
                )?;
                rewrite_blueprint_reference_list(
                    key,
                    attribute,
                    "target_blueprints",
                    "relationship target",
                    blueprint_codes,
                    &mut dependencies,
                )?;
            }
        }
    }
    if let Some(views) = table.get_mut("views").and_then(Value::as_object_mut) {
        for (_, view) in views.iter_mut() {
            if let Some(view) = view.as_object_mut() {
                rewrite_view_references(key, view, blueprint_codes, Some(&mut dependencies))?;
            }
        }
    }

    let pack_codes = blueprint_codes.values().copied().collect::<HashSet<_>>();
    visit_embedded_predicate_blueprint_codes(table_of(&mut value), &mut |code| {
        if pack_codes.contains(code.as_str()) {
            Ok(())
        } else {
            invalid(format!(
                "blueprint '{key}' predicate references blueprint '{code}' that the pack does not declare"
            ))
        }
    })?;
    let native_source = attricat_yaml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint YAML"
        ))
    })?;
    let definition = attricat_blueprint::parse(&native_source).map_err(|error| {
        let message = match error {
            BlueprintError::Yaml(_) => format!("blueprint '{key}' is invalid"),
            error => format!("blueprint '{key}' is invalid: {error}"),
        };
        SolutionPackError::Invalid(message)
    })?;
    reject_workspace_dependent_blueprint_constructs(key, &definition)?;
    if definition.includes.len() > MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES {
        return invalid(format!(
            "blueprint '{key}' exceeds the include limit of {MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES}"
        ));
    }
    if definition.attributes.len() > MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
        return invalid(format!(
            "blueprint '{key}' exceeds the attribute limit of {MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES}"
        ));
    }
    for include in &definition.includes {
        if include.alias.len() > MAX_IDENTIFIER_BYTES {
            return invalid(format!(
                "blueprint '{key}' include alias '{}' is too long",
                include.alias
            ));
        }
    }
    let expected_code = resource_code(key);
    if definition.code != expected_code {
        return invalid(format!("blueprint '{key}' code must be '{expected_code}'"));
    }

    // Entries naming extensions the workspace lacks are kept: the host
    // ignores them until such an extension is installed and enabled.
    let view_extensions = attricat_blueprint::EXTENSION_VIEWS
        .iter()
        .filter_map(|view| {
            definition
                .views
                .get(*view)
                .map(|definition| (*view, definition))
        })
        .flat_map(|(view, definition)| {
            definition
                .extensions()
                .iter()
                .map(move |entry| ((*view).to_owned(), entry.clone()))
        })
        .collect::<Vec<_>>();

    Ok(PreparedBlueprint {
        portable: SolutionPackBlueprint {
            key: key.to_owned(),
            code: definition.code.clone(),
            source: source.to_owned(),
            includes: portable_includes,
            dependencies,
            table_path_dependencies: BTreeSet::new(),
            default_from_dependencies: BTreeSet::new(),
            movement_dependencies,
            kind: definition.kind.clone(),
            effective_attributes: Vec::new(),
            unique_keys: Vec::new(),
            view_extensions,
        },
        native_definition: definition,
        native_source,
        include_keys,
    })
}

fn reject_workspace_dependent_blueprint_constructs(
    key: &str,
    definition: &BlueprintDefinition,
) -> Result<(), SolutionPackError> {
    if !definition.publication.retain_on_edit_roles.is_empty() {
        return invalid(format!(
            "blueprint '{key}' cannot declare workspace roles in solution-pack v1"
        ));
    }
    for view in definition.views.values() {
        match view {
            ViewDefinition::Table {
                columns: Some(columns),
                ..
            } if columns.iter().any(|column| {
                column
                    .renderer
                    .as_ref()
                    .is_some_and(|renderer| !renderer.id.starts_with("attricat."))
            }) =>
            {
                return invalid(format!(
                    "blueprint '{key}' cannot declare extension table renderers in solution-pack v1"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn rewrite_blueprint_reference(
    owner_key: &str,
    table: &mut serde_json::Map<String, Value>,
    field: &str,
    label: &str,
    blueprint_codes: &HashMap<&str, &str>,
    dependencies: Option<&mut BTreeSet<String>>,
) -> Result<(), SolutionPackError> {
    let Some(reference) = table.get_mut(field) else {
        return Ok(());
    };
    let logical_key = reference.as_str().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{owner_key}' {label} must be a logical key"
        ))
    })?;
    let code = blueprint_codes.get(logical_key).ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{owner_key}' references undeclared {label} '{logical_key}'"
        ))
    })?;
    if let Some(dependencies) = dependencies {
        dependencies.insert(logical_key.to_owned());
    }
    *reference = Value::String((*code).to_owned());
    Ok(())
}

fn rewrite_blueprint_reference_list(
    owner_key: &str,
    table: &mut serde_json::Map<String, Value>,
    field: &str,
    label: &str,
    blueprint_codes: &HashMap<&str, &str>,
    dependencies: &mut BTreeSet<String>,
) -> Result<(), SolutionPackError> {
    let Some(references) = table.get_mut(field) else {
        return Ok(());
    };
    let references = references.as_array_mut().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{owner_key}' {label}s must be a list of logical keys"
        ))
    })?;
    for reference in references {
        let mut entry = serde_json::Map::new();
        entry.insert(field.to_owned(), reference.clone());
        rewrite_blueprint_reference(
            owner_key,
            &mut entry,
            field,
            label,
            blueprint_codes,
            Some(dependencies),
        )?;
        *reference = entry.remove(field).expect("rewritten reference");
    }
    Ok(())
}

fn rewrite_view_references(
    owner_key: &str,
    node: &mut serde_json::Map<String, Value>,
    blueprint_codes: &HashMap<&str, &str>,
    mut dependencies: Option<&mut BTreeSet<String>>,
) -> Result<(), SolutionPackError> {
    if node.get("type").and_then(Value::as_str) == Some("incoming_relationship_list")
        && let Some(relationships) = node.get_mut("relationships").and_then(Value::as_array_mut)
    {
        for relationship in relationships {
            if let Some(relationship) = relationship.as_object_mut() {
                rewrite_blueprint_reference(
                    owner_key,
                    relationship,
                    "source_blueprint",
                    "view source blueprint",
                    blueprint_codes,
                    dependencies.as_deref_mut(),
                )?;
            }
        }
    }

    for collection in ["children", "tabs", "sections"] {
        if let Some(children) = node.get_mut(collection).and_then(Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_object_mut() {
                    rewrite_view_references(
                        owner_key,
                        child,
                        blueprint_codes,
                        dependencies.as_deref_mut(),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn compile_pack_blueprint(
    key: &str,
    blueprints: &BTreeMap<String, PreparedBlueprint>,
    compiled: &mut HashMap<String, CompiledBlueprint>,
) -> Result<(), SolutionPackError> {
    if compiled.contains_key(key) {
        return Ok(());
    }
    let blueprint = &blueprints[key];
    let mut resolved = Vec::with_capacity(blueprint.native_definition.includes.len());
    let mut resolved_attribute_count = 0usize;
    for (include, dependency_key) in blueprint
        .native_definition
        .includes
        .iter()
        .zip(&blueprint.include_keys)
    {
        compile_pack_blueprint(dependency_key, blueprints, compiled)?;
        let dependency = &compiled[dependency_key];
        if dependency.kind != BlueprintKind::Mixin {
            return invalid(format!(
                "blueprint '{key}' include '{dependency_key}' must reference a mixin"
            ));
        }
        resolved_attribute_count = resolved_attribute_count
            .checked_add(dependency.attributes.len())
            .ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' resolved include attributes are too large"
                ))
            })?;
        if resolved_attribute_count > MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES {
            return invalid(format!(
                "blueprint '{key}' exceeds the resolved include attribute limit of {MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES}"
            ));
        }
        resolved.push(ResolvedInclude {
            alias: include.alias.clone(),
            code: include.code.clone(),
            version: include.version,
            attributes: dependency.attributes.clone(),
        });
    }
    let definition = attricat_blueprint::compile(
        blueprint.native_definition.clone(),
        &resolved,
        &blueprint.native_source,
    )
    .map_err(|error| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' fails compiler validation: {error}"
        ))
    })?;
    compiled.insert(key.to_owned(), definition);
    Ok(())
}

/// Resolves table column and `views.explorer.browse_by` paths within the
/// pack, returning the blueprints each one's later hops are read from: those
/// must be published before it.
fn validate_table_paths(
    blueprints: &HashMap<String, CompiledBlueprint>,
) -> Result<BTreeMap<String, BTreeSet<String>>, SolutionPackError> {
    let by_code = blueprints
        .iter()
        .map(|(key, blueprint)| (blueprint.code.as_str(), key.as_str()))
        .collect::<HashMap<_, _>>();
    let mut dependencies = BTreeMap::new();

    for (owner_key, owner) in blueprints {
        let mut owner_dependencies = BTreeSet::new();
        for view in owner.views.values() {
            let ViewDefinition::Table {
                columns: Some(columns),
                ..
            } = view
            else {
                continue;
            };
            for column in columns {
                let parts = column.field.split('.').collect::<Vec<_>>();
                if parts.len() == 1 {
                    continue;
                }
                // Every blueprint a hop links to must be in the pack, and is
                // published before the owner.
                let leaf = walk_path(
                    &column.field,
                    &parts,
                    (owner.code.as_str(), owner.attributes.as_slice()),
                    |target_code| {
                        let target_key = by_code.get(target_code).ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' table column '{}' target '{target_code}' is not declared by the pack",
                                column.field
                            ))
                        })?;
                        owner_dependencies.insert((*target_key).to_owned());
                        let target = &blueprints[*target_key];
                        Ok(Some((target.code.as_str(), target.attributes.as_slice())))
                    },
                )
                .map_err(|error| match error {
                    WalkError::Load(error) => error,
                    WalkError::Path(error) => SolutionPackError::Invalid(format!(
                        "blueprint '{owner_key}' table column '{}' {error}",
                        column.field
                    )),
                })?;
                if matches!(leaf.value_type, ValueType::Relationship | ValueType::File) {
                    return invalid(format!(
                        "blueprint '{owner_key}' table column '{}' leaf must be scalar",
                        column.field
                    ));
                }
            }
        }
        if let Some(ViewDefinition::Explorer {
            browse_by: Some(BrowseBy::Field(path)),
        }) = owner.views.get("explorer")
        {
            let hops = path.split('.').collect::<Vec<_>>();
            let mut current = owner;
            for (index, hop) in hops.iter().enumerate() {
                let attribute = current
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == *hop)
                    .filter(|attribute| {
                        browse_hop(
                            attribute.value_type,
                            attribute.target_blueprint.as_deref(),
                            &attribute.target_blueprints,
                            attribute.cardinality,
                            hops.len(),
                        )
                    })
                    .ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' views.explorer browse_by '{path}' segment '{hop}' must be a relationship with a single target blueprint and cardinality: one"
                        ))
                    })?;
                if index + 1 == hops.len() {
                    break;
                }
                let target_code = attribute
                    .target_blueprint
                    .as_deref()
                    .expect("browse_by hop has a target");
                let target_key = by_code.get(target_code).ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "blueprint '{owner_key}' views.explorer browse_by '{path}' target '{target_code}' is not declared by the pack"
                    ))
                })?;
                owner_dependencies.insert((*target_key).to_owned());
                current = &blueprints[*target_key];
            }
        }
        dependencies.insert(owner_key.clone(), owner_dependencies);
    }
    Ok(dependencies)
}

/// Resolves `default_from` paths within the pack as the repository does when
/// the blueprint is saved: every hop holds a single target, and the leaf is a
/// scalar the attribute's value type can copy (an `integer` may fill a
/// `number`). A leaf that only some targets of a multi-target hop declare is
/// allowed. Returns the other blueprints each path reads from, which must be
/// published before the owner; a path back to the owner reads the revision
/// being saved, so it adds no dependency. The edges must be acyclic, since
/// neither end of a cycle could be saved first.
fn validate_default_from_paths(
    blueprints: &HashMap<String, CompiledBlueprint>,
) -> Result<BTreeMap<String, BTreeSet<String>>, SolutionPackError> {
    let by_code = blueprints
        .iter()
        .map(|(key, blueprint)| (blueprint.code.as_str(), key.as_str()))
        .collect::<HashMap<_, _>>();
    let mut dependencies = BTreeMap::new();

    for (owner_key, owner) in blueprints {
        let mut owner_dependencies = BTreeSet::new();
        for attribute in &owner.attributes {
            let Some(path) = &attribute.default_from else {
                continue;
            };
            let path_error = |message: String| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{owner_key}' attribute '{}' default_from '{path}' {message}",
                    attribute.code
                ))
            };
            let parts = path.split('.').collect::<Vec<_>>();
            let (leaf, hops) = parts.split_last().expect("a path has a leaf");
            let mut frontier = vec![(owner.code.as_str(), owner.attributes.as_slice())];
            for hop in hops {
                let segment = resolve_in(path, hop, frontier.iter().copied(), CodeMatch::Exact)
                    .map_err(|error| path_error(error.to_string()))?;
                if !segment.single_valued() {
                    return Err(path_error(
                        "must follow relationships with cardinality: one".into(),
                    ));
                }
                let targets = segment
                    .next_frontier(path)
                    .map_err(|error| path_error(error.to_string()))?;
                frontier = Vec::with_capacity(targets.len());
                for target_code in &targets {
                    let target_key = by_code.get(target_code.as_str()).ok_or_else(|| {
                        path_error(format!(
                            "target '{target_code}' is not declared by the pack"
                        ))
                    })?;
                    if *target_key != owner_key.as_str() {
                        owner_dependencies.insert((*target_key).to_owned());
                    }
                    let target = &blueprints[*target_key];
                    frontier.push((target.code.as_str(), target.attributes.as_slice()));
                }
            }
            let leaf = resolve_in(path, leaf, frontier, CodeMatch::Exact)
                .map_err(|error| path_error(error.to_string()))?;
            if !leaf.value_type.copies_into(attribute.value_type) {
                return Err(path_error(format!(
                    "ends on a {} attribute, which cannot fill a {} attribute",
                    leaf.value_type, attribute.value_type
                )));
            }
        }
        dependencies.insert(owner_key.clone(), owner_dependencies);
    }

    let graph = dependencies
        .iter()
        .map(|(key, targets)| {
            (
                key.as_str(),
                targets.iter().map(String::as_str).collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    validate_acyclic(&graph, "blueprint default_from")?;
    Ok(dependencies)
}

fn validate_incoming_relationships(
    blueprints: &HashMap<String, CompiledBlueprint>,
) -> Result<(), SolutionPackError> {
    let by_code = blueprints
        .iter()
        .map(|(key, blueprint)| (blueprint.code.as_str(), (key.as_str(), blueprint)))
        .collect::<HashMap<_, _>>();

    for (owner_key, owner) in blueprints {
        for view in owner.views.values() {
            validate_incoming_relationships_in_view(owner_key, owner, view, &by_code)?;
        }
    }
    Ok(())
}

fn validate_incoming_relationships_in_view(
    owner_key: &str,
    owner: &CompiledBlueprint,
    view: &ViewDefinition,
    blueprints: &HashMap<&str, (&str, &CompiledBlueprint)>,
) -> Result<(), SolutionPackError> {
    match view {
        ViewDefinition::Stack { children, .. }
        | ViewDefinition::Grid { children, .. }
        | ViewDefinition::Section { children, .. } => {
            validate_incoming_relationships_in_nodes(owner_key, owner, children, blueprints)
        }
        ViewDefinition::Tabs { tabs, .. } => {
            for tab in tabs {
                validate_incoming_relationships_in_nodes(
                    owner_key,
                    owner,
                    &tab.children,
                    blueprints,
                )?;
            }
            Ok(())
        }
        ViewDefinition::Accordion { sections, .. } => {
            for section in sections {
                validate_incoming_relationships_in_nodes(
                    owner_key,
                    owner,
                    &section.children,
                    blueprints,
                )?;
            }
            Ok(())
        }
        ViewDefinition::DropdownOption { .. }
        | ViewDefinition::PageTitle { .. }
        | ViewDefinition::Table { .. }
        | ViewDefinition::Explorer { .. } => Ok(()),
    }
}

fn validate_incoming_relationships_in_nodes(
    owner_key: &str,
    owner: &CompiledBlueprint,
    nodes: &[ViewNode],
    blueprints: &HashMap<&str, (&str, &CompiledBlueprint)>,
) -> Result<(), SolutionPackError> {
    for node in nodes {
        match node {
            ViewNode::Stack { children, .. }
            | ViewNode::Grid { children, .. }
            | ViewNode::Section { children, .. } => {
                validate_incoming_relationships_in_nodes(owner_key, owner, children, blueprints)?
            }
            ViewNode::Tabs { tabs, .. } => {
                for tab in tabs {
                    validate_incoming_relationships_in_nodes(
                        owner_key,
                        owner,
                        &tab.children,
                        blueprints,
                    )?;
                }
            }
            ViewNode::Accordion { sections, .. } => {
                for section in sections {
                    validate_incoming_relationships_in_nodes(
                        owner_key,
                        owner,
                        &section.children,
                        blueprints,
                    )?;
                }
            }
            ViewNode::IncomingRelationshipList { relationships, .. } => {
                for relationship in relationships {
                    let (source_key, source) = blueprints
                        .get(relationship.source_blueprint.as_str())
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' incoming relationship references unavailable source blueprint '{}'",
                                relationship.source_blueprint
                            ))
                        })?;
                    let attribute = source
                        .attributes
                        .iter()
                        .find(|attribute| attribute.code == relationship.field)
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' incoming relationship source '{source_key}' has no field '{}'",
                                relationship.field
                            ))
                        })?;
                    if attribute.value_type != ValueType::Relationship {
                        return invalid(format!(
                            "blueprint '{owner_key}' incoming relationship source '{source_key}' field '{}' must be a relationship",
                            relationship.field
                        ));
                    }
                    if !attribute.target_blueprints.contains(&owner.code) {
                        return invalid(format!(
                            "blueprint '{owner_key}' incoming relationship source '{source_key}' field '{}' must target '{}'",
                            relationship.field, owner.code
                        ));
                    }
                }
            }
            ViewNode::Heading { .. }
            | ViewNode::Text { .. }
            | ViewNode::Divider { .. }
            | ViewNode::Field { .. }
            | ViewNode::RelationshipList { .. } => {}
        }
    }
    Ok(())
}
