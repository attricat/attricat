//! Blueprint resources: pack-local references are rewritten, includes are
//! resolved within the pack, and every blueprint is compiled offline.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use catalog_blueprint::{
    BlueprintDefinition, BlueprintError, BlueprintKind, CompiledBlueprint, ResolvedInclude,
    ViewDefinition, ViewNode,
};
use serde_json::Value;

use super::{
    BlueprintExtensionLayoutEntry, MAX_IDENTIFIER_BYTES, MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES,
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
            SolutionPackError::Invalid(format!("blueprint '{}' must be UTF-8 TOML", resource.key))
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
    let blueprints = prepared
        .into_iter()
        .map(|(key, mut blueprint)| {
            blueprint.portable.table_path_dependencies = table_path_dependencies[&key].clone();
            blueprint.portable.effective_attributes = compiled[&key].attributes.clone();
            blueprint.portable.unique_keys = compiled[&key].unique_keys.clone();
            (key, blueprint.portable)
        })
        .collect();

    Ok(blueprints)
}

/// Visits the blueprint codes named by predicates embedded in a blueprint
/// definition: its `[[rules]]`, entity checks (`x-attricat-checks`) and status
/// transition conditions. Packs name these blueprints by pack-local code.
pub(super) fn visit_embedded_predicate_blueprint_codes(
    table: &mut toml::Table,
    visit: &mut dyn FnMut(&mut String) -> Result<(), SolutionPackError>,
) -> Result<(), SolutionPackError> {
    fn json_string(
        schema: &mut toml::Value,
        visit: &mut dyn FnMut(&mut String) -> Result<(), SolutionPackError>,
    ) -> Result<(), SolutionPackError> {
        let toml::Value::String(source) = schema else {
            return Ok(());
        };
        // Malformed JSON is reported by the ordinary blueprint parser.
        let Ok(original) = serde_json::from_str::<Value>(source) else {
            return Ok(());
        };
        let mut rewritten = original.clone();
        catalog_validation::predicate::visit_schema_blueprint_codes(&mut rewritten, visit)?;
        if rewritten != original {
            *source = serde_json::to_string(&rewritten).expect("JSON value serializes");
        }
        Ok(())
    }
    if let Some(rules) = table.get_mut("rules").and_then(toml::Value::as_array_mut) {
        for rule in rules {
            if let Some(predicate) = rule.get_mut("predicate") {
                crate::solution_pack_seeds::visit_toml_predicate_blueprint_codes(predicate, visit)?;
            }
        }
    }
    if let Some(schema) = table.get_mut("entity_schema") {
        json_string(schema, visit)?;
    }
    if let Some(attributes) = table
        .get_mut("attributes")
        .and_then(toml::Value::as_array_mut)
    {
        for attribute in attributes {
            if let Some(schema) = attribute.get_mut("value_schema") {
                json_string(schema, visit)?;
            }
        }
    }
    Ok(())
}

fn table_of(value: &mut toml::Value) -> &mut toml::Table {
    value.as_table_mut().expect("blueprint source is a table")
}

fn prepare_blueprint(
    key: &str,
    source: &str,
    blueprint_codes: &HashMap<&str, &str>,
) -> Result<PreparedBlueprint, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(source).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    let table = value.as_table_mut().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    if table.contains_key("extensions") {
        return invalid(format!(
            "blueprint '{key}' cannot contain extension metadata in solution-pack v1"
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
            let include = include.as_table_mut().ok_or_else(|| {
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
                .and_then(toml::Value::as_str)
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
            include.insert(
                "code".into(),
                toml::Value::String((*dependency_code).into()),
            );
            include.insert("version".into(), toml::Value::Integer(1));
            portable_includes.push(SolutionPackBlueprintInclude {
                alias,
                key: dependency.clone(),
            });
            dependencies.insert(dependency.clone());
            include_keys.push(dependency);
        }
    }

    if let Some(attributes) = table
        .get_mut("attributes")
        .and_then(toml::Value::as_array_mut)
    {
        for attribute in attributes {
            if let Some(attribute) = attribute.as_table_mut() {
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
    if let Some(views) = table.get_mut("views").and_then(toml::Value::as_table_mut) {
        for (_, view) in views.iter_mut() {
            if let Some(view) = view.as_table_mut() {
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
    let native_source = toml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    let definition = catalog_blueprint::parse(&native_source).map_err(|error| {
        let message = match error {
            BlueprintError::Toml(_) => format!("blueprint '{key}' is invalid"),
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

    let mut extension_layout = definition
        .views
        .get("extension_layout")
        .and_then(|view| match view {
            ViewDefinition::ExtensionLayout { outlets, .. } => Some(outlets),
            _ => None,
        })
        .into_iter()
        .flat_map(|outlets| outlets.iter())
        .flat_map(|(outlet, layout)| {
            layout
                .order
                .iter()
                .chain(&layout.hidden)
                .map(move |contribution| BlueprintExtensionLayoutEntry {
                    contribution: contribution.clone(),
                    outlet: outlet.clone(),
                })
        })
        .collect::<Vec<_>>();
    extension_layout.sort_by(|left, right| {
        left.outlet
            .cmp(&right.outlet)
            .then_with(|| left.contribution.cmp(&right.contribution))
    });

    Ok(PreparedBlueprint {
        portable: SolutionPackBlueprint {
            key: key.to_owned(),
            code: definition.code.clone(),
            source: source.to_owned(),
            includes: portable_includes,
            dependencies,
            table_path_dependencies: BTreeSet::new(),
            kind: definition.kind.clone(),
            effective_attributes: Vec::new(),
            unique_keys: Vec::new(),
            extension_layout,
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
                    .is_some_and(|renderer| !renderer.id.starts_with("catalog."))
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
    table: &mut toml::map::Map<String, toml::Value>,
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
    *reference = toml::Value::String((*code).to_owned());
    Ok(())
}

fn rewrite_blueprint_reference_list(
    owner_key: &str,
    table: &mut toml::map::Map<String, toml::Value>,
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
        let mut entry = toml::map::Map::new();
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
    node: &mut toml::map::Map<String, toml::Value>,
    blueprint_codes: &HashMap<&str, &str>,
    mut dependencies: Option<&mut BTreeSet<String>>,
) -> Result<(), SolutionPackError> {
    if node.get("type").and_then(toml::Value::as_str) == Some("incoming_relationship_list")
        && let Some(relationships) = node
            .get_mut("relationships")
            .and_then(toml::Value::as_array_mut)
    {
        for relationship in relationships {
            if let Some(relationship) = relationship.as_table_mut() {
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
        if let Some(children) = node.get_mut(collection).and_then(toml::Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_table_mut() {
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
    let definition = catalog_blueprint::compile(
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
                let mut current = owner;
                for segment in &parts[..parts.len() - 1] {
                    let attribute = current
                        .attributes
                        .iter()
                        .find(|attribute| attribute.code == *segment)
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' table column '{}' path segment '{segment}' was not found",
                                column.field
                            ))
                        })?;
                    if attribute.value_type != "relationship" {
                        return invalid(format!(
                            "blueprint '{owner_key}' table column '{}' segment '{segment}' must be a relationship",
                            column.field
                        ));
                    }
                    let target_code = attribute.target_blueprint.as_deref().ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' segment '{segment}' has no target blueprint",
                            column.field
                        ))
                    })?;
                    let target_key = by_code.get(target_code).ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' target '{target_code}' is not declared by the pack",
                            column.field
                        ))
                    })?;
                    owner_dependencies.insert((*target_key).to_owned());
                    current = &blueprints[*target_key];
                }
                let leaf = parts.last().expect("multi-part table path has a leaf");
                let leaf_attribute = current
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == *leaf)
                    .ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' path segment '{leaf}' was not found",
                            column.field
                        ))
                    })?;
                if matches!(leaf_attribute.value_type.as_str(), "relationship" | "file") {
                    return invalid(format!(
                        "blueprint '{owner_key}' table column '{}' leaf must be scalar",
                        column.field
                    ));
                }
            }
        }
        dependencies.insert(owner_key.clone(), owner_dependencies);
    }
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
        | ViewDefinition::Table { .. }
        | ViewDefinition::ExtensionLayout { .. } => Ok(()),
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
                    if attribute.value_type != "relationship" {
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
