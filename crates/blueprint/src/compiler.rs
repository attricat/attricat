use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::{
    AttributeDeclaration, BlueprintDefinition, BlueprintError, BlueprintKind, CompiledBlueprint,
    EffectiveAttribute, ResolvedInclude, UniqueKeyDefinition,
};

pub fn compile(
    definition: BlueprintDefinition,
    resolved_includes: &[ResolvedInclude],
    source: &str,
) -> Result<CompiledBlueprint, BlueprintError> {
    let resolved_by_alias: HashMap<_, _> = resolved_includes
        .iter()
        .map(|include| (include.alias.as_str(), include))
        .collect();

    for include in &definition.includes {
        let resolved = resolved_by_alias
            .get(include.alias.as_str())
            .ok_or_else(|| BlueprintError::InvalidResolvedInclude(include.alias.clone()))?;
        if resolved.code != include.code || resolved.version != include.version {
            return Err(BlueprintError::InvalidResolvedInclude(
                include.alias.clone(),
            ));
        }
    }
    if resolved_by_alias.len() != definition.includes.len() {
        return Err(BlueprintError::InvalidResolvedInclude(
            "unexpected include".to_owned(),
        ));
    }

    let mut attributes = Vec::with_capacity(definition.attributes.len());
    for (position, declaration) in definition.attributes.iter().enumerate() {
        let (
            code,
            name,
            value_type,
            value_schema,
            extension_type,
            default_value,
            file_policy,
            target_blueprint,
            target_blueprints,
            cardinality,
            target_cardinality,
            hierarchy,
            tags,
            context_fallback,
            context_editable,
            readonly,
        ) = match declaration {
            AttributeDeclaration::Local(local) => (
                local.code.clone(),
                local.name.clone(),
                local.value_type.clone(),
                local.value_schema.clone(),
                local.extension_type.as_ref().map(|reference| {
                    serde_json::json!({
                        "reference": reference,
                        "configuration": local.extension_configuration,
                    })
                }),
                local.default_value.clone(),
                local.file_policy.clone(),
                local.target_blueprint.clone(),
                local.target_blueprints.clone(),
                local.cardinality.clone(),
                local.target_cardinality.clone(),
                local.hierarchy.clone(),
                local.tags.clone(),
                local.context_fallback.clone(),
                local.context_editable.clone(),
                local.readonly,
            ),
            AttributeDeclaration::Selection {
                code,
                include_alias,
                attribute_code,
            } => {
                let include = resolved_by_alias
                    .get(include_alias.as_str())
                    .ok_or_else(|| BlueprintError::UnknownIncludeAlias(include_alias.clone()))?;
                let attribute = include
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == *attribute_code)
                    .ok_or_else(|| BlueprintError::UnknownIncludedAttribute {
                        alias: include_alias.clone(),
                        attribute: attribute_code.clone(),
                    })?;
                (
                    code.clone(),
                    attribute.name.clone(),
                    attribute.value_type.clone(),
                    attribute.value_schema.clone(),
                    attribute.extension_type.clone(),
                    attribute.default_value.clone(),
                    attribute.file_policy.clone(),
                    attribute.target_blueprint.clone(),
                    attribute.target_blueprints.clone(),
                    attribute.cardinality.clone(),
                    attribute.target_cardinality.clone(),
                    attribute.hierarchy.clone(),
                    attribute.tags.clone(),
                    attribute.context_fallback.clone(),
                    attribute.context_editable.clone(),
                    attribute.readonly,
                )
            }
        };
        attributes.push(EffectiveAttribute {
            code,
            name,
            value_type,
            value_schema,
            extension_type,
            default_value,
            file_policy,
            target_blueprint,
            target_blueprints,
            cardinality,
            target_cardinality,
            hierarchy,
            tags,
            context_fallback,
            context_editable,
            readonly,
            position: position as i64,
        });
    }
    if definition.kind == BlueprintKind::Record
        && let Some(schema) = &definition.record_schema
    {
        validate_record_schema_attributes(schema, &attributes)?;
    }
    validate_declarative_checks(&definition, &attributes)?;
    validate_unique_key_attributes(&definition.unique_keys, &attributes)?;
    validate_selected_hierarchies(&definition.code, &definition.kind, &attributes)?;
    if definition.kind == BlueprintKind::Record && !definition.views.contains_key("dropdown_option")
    {
        return Err(BlueprintError::MissingDropdownOptionView);
    }
    for (name, view) in &definition.views {
        crate::view_validation::validate_view(
            name,
            view,
            &attributes,
            &definition.code,
            &definition.kind,
        )?;
    }
    Ok(CompiledBlueprint {
        code: definition.code,
        name: definition.name,
        description: definition.description,
        kind: definition.kind,
        raw_definition_hash: raw_hash(source),
        includes: definition.includes,
        views: definition.views,
        record_schema: definition.record_schema,
        rules: definition.rules,
        unique_keys: definition.unique_keys,
        attributes,
    })
}

/// Type-checks record-schema checks, status transition conditions and rules
/// against the effective attributes. They share one predicate engine.
fn validate_declarative_checks(
    definition: &BlueprintDefinition,
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    use catalog_validation::predicate::{
        MAX_RECORD_CHECKS, MAX_TRANSITION_CONDITIONS, Usage, record_checks, validate_checks,
    };
    use catalog_validation::status::{STATUS_KEY, transition_edges};
    let types: HashMap<String, String> = attributes
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type.clone()))
        .collect();
    if let Some(schema) = &definition.record_schema {
        let invalid = |message: String| BlueprintError::InvalidJsonSchema {
            field: "record_schema".to_owned(),
            message,
        };
        let checks = record_checks(schema).map_err(invalid)?;
        validate_checks(&checks, Some(&types), Usage::Enforced, MAX_RECORD_CHECKS)
            .map_err(|message| invalid(format!("invalid x-attricat-checks: {message}")))?;
    }
    for attribute in attributes {
        let Some(edges) = attribute.value_schema.as_ref().and_then(transition_edges) else {
            continue;
        };
        for edge in edges {
            let invalid = |message: String| BlueprintError::InvalidJsonSchema {
                field: format!("attributes.{}.value_schema", attribute.code),
                message,
            };
            let conditions = edge.conditions.map_err(invalid)?;
            validate_checks(
                &conditions,
                Some(&types),
                Usage::Enforced,
                MAX_TRANSITION_CONDITIONS,
            )
            .map_err(|message| {
                invalid(format!("invalid status transition conditions: {message}"))
            })?;
        }
    }
    for rule in &definition.rules {
        catalog_rules::validate_against_attributes(rule, &types).map_err(|error| {
            BlueprintError::InvalidRule(format!("rule '{}': {error}", rule.code))
        })?;
        for selector in rule
            .enforcement
            .iter()
            .flat_map(|enforcement| &enforcement.transitions)
        {
            let options = attributes
                .iter()
                .find(|attribute| attribute.code == selector.attribute_code)
                .and_then(|attribute| attribute.value_schema.as_ref())
                .filter(|schema| schema.get(STATUS_KEY).is_some())
                .and_then(|schema| schema.get("enum"))
                .and_then(serde_json::Value::as_array);
            let Some(options) = options else {
                return Err(BlueprintError::InvalidRule(format!(
                    "rule '{}': enforcement attribute '{}' is not a status attribute",
                    rule.code, selector.attribute_code
                )));
            };
            for code in std::iter::once(&selector.to).chain(selector.from.as_ref()) {
                if !options.iter().any(|option| option.as_str() == Some(code)) {
                    return Err(BlueprintError::InvalidRule(format!(
                        "rule '{}': unknown status '{code}' for '{}'",
                        rule.code, selector.attribute_code
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Key attributes must have one comparable value per context: scalars other
/// than JSON, or relationships limited to one target.
fn validate_unique_key_attributes(
    keys: &[UniqueKeyDefinition],
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    for key in keys {
        for code in &key.attributes {
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.code == *code)
                .ok_or_else(|| BlueprintError::InvalidUniqueKey {
                    key: key.code.clone(),
                    message: format!("unknown attribute '{code}'"),
                })?;
            let supported = match attribute.value_type.as_str() {
                "json" | "file" => false,
                "relationship" => attribute.cardinality.as_deref() == Some("one"),
                _ => true,
            };
            if !supported {
                return Err(BlueprintError::InvalidUniqueKey {
                    key: key.code.clone(),
                    message: format!(
                        "attribute '{code}' must be a scalar other than json, or a relationship with cardinality = \"one\""
                    ),
                });
            }
        }
    }
    Ok(())
}

/// A hierarchy selected from a mixin must still be able to target the
/// consuming blueprint.
fn validate_selected_hierarchies(
    blueprint_code: &str,
    kind: &BlueprintKind,
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    if *kind != BlueprintKind::Record {
        return Ok(());
    }
    if let Some(attribute) = attributes.iter().find(|attribute| {
        attribute.hierarchy.is_some()
            && !attribute.target_blueprints.is_empty()
            && !attribute
                .target_blueprints
                .iter()
                .any(|target| target == blueprint_code)
    }) {
        return Err(BlueprintError::InvalidRelationshipHierarchy {
            code: attribute.code.clone(),
            message: format!(
                "a hierarchy must be able to target its own blueprint '{blueprint_code}'"
            ),
        });
    }
    Ok(())
}

fn validate_record_schema_attributes(
    schema: &serde_json::Value,
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    let Some(schema) = schema.as_object() else {
        return Ok(());
    };
    let attribute_codes: HashSet<_> = attributes
        .iter()
        .map(|attribute| attribute.code.as_str())
        .collect();
    let validate_attribute = |keyword: &'static str, attribute: &str| {
        if attribute_codes.contains(attribute) {
            Ok(())
        } else {
            Err(BlueprintError::RecordSchemaUnknownAttribute {
                keyword,
                attribute: attribute.to_owned(),
            })
        }
    };

    if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) {
        for attribute in required.iter().filter_map(serde_json::Value::as_str) {
            validate_attribute("required", attribute)?;
        }
    }
    if let Some(properties) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        for attribute in properties.keys() {
            validate_attribute("properties", attribute)?;
        }
    }
    if let Some(dependent_required) = schema
        .get("dependentRequired")
        .and_then(serde_json::Value::as_object)
    {
        for (attribute, dependencies) in dependent_required {
            validate_attribute("dependentRequired", attribute)?;
            for dependency in dependencies
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
            {
                validate_attribute("dependentRequired", dependency)?;
            }
        }
    }
    if let Some(dependent_schemas) = schema
        .get("dependentSchemas")
        .and_then(serde_json::Value::as_object)
    {
        for attribute in dependent_schemas.keys() {
            validate_attribute("dependentSchemas", attribute)?;
        }
    }
    Ok(())
}

pub fn raw_hash(source: &str) -> String {
    let hash = Sha256::digest(source.as_bytes());
    format!("{hash:x}")
}
