use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::{
    AttributeDeclaration, BlueprintDefinition, BlueprintError, BlueprintKind, CompiledBlueprint,
    EffectiveAttribute, ResolvedInclude,
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
            value_type,
            value_schema,
            default_value,
            file_policy,
            target_blueprint,
            tags,
            context_fallback,
            context_editable,
            readonly,
        ) = match declaration {
            AttributeDeclaration::Local(local) => (
                local.code.clone(),
                local.value_type.clone(),
                local.value_schema.clone(),
                local.default_value.clone(),
                local.file_policy.clone(),
                local.target_blueprint.clone(),
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
                    attribute.value_type.clone(),
                    attribute.value_schema.clone(),
                    attribute.default_value.clone(),
                    attribute.file_policy.clone(),
                    attribute.target_blueprint.clone(),
                    attribute.tags.clone(),
                    attribute.context_fallback.clone(),
                    attribute.context_editable.clone(),
                    attribute.readonly,
                )
            }
        };
        attributes.push(EffectiveAttribute {
            code,
            value_type,
            value_schema,
            default_value,
            file_policy,
            target_blueprint,
            tags,
            context_fallback,
            context_editable,
            readonly,
            position: position as i64,
        });
    }
    if definition.kind == BlueprintKind::Entity
        && let Some(schema) = &definition.entity_schema
    {
        validate_entity_schema_attributes(schema, &attributes)?;
    }
    if definition.kind == BlueprintKind::Entity && !definition.views.contains_key("dropdown_option")
    {
        return Err(BlueprintError::MissingDropdownOptionView);
    }
    for (name, view) in &definition.views {
        crate::view_validation::validate_view(name, view, &attributes, &definition.code)?;
    }

    Ok(CompiledBlueprint {
        code: definition.code,
        name: definition.name,
        kind: definition.kind,
        raw_definition_hash: raw_hash(source),
        includes: definition.includes,
        views: definition.views,
        entity_schema: definition.entity_schema,
        attributes,
    })
}

fn validate_entity_schema_attributes(
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
            Err(BlueprintError::EntitySchemaUnknownAttribute {
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
