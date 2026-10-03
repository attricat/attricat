use std::collections::HashSet;

use crate::extensions::Manifest;
use catalog_blueprint::{
    BlueprintKind, CompiledBlueprint, ResolvedInclude, ViewDefinition, compile, parse,
    validate_table_renderer,
};
use catalog_validation::validate_json_schema;
use semver::VersionReq;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::repository::RepositoryError;

pub(crate) async fn compile_definition(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    source: &str,
) -> Result<CompiledBlueprint, RepositoryError> {
    // Track the active branch, not every visited include: reusing one pinned
    // mixin is valid, while revisiting it before unwinding is a cycle.
    let mut resolving = HashSet::new();
    compile_source(transaction, workspace_id, source, &mut resolving).await
}

async fn compile_source(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    source: &str,
    resolving: &mut HashSet<(String, i64)>,
) -> Result<CompiledBlueprint, RepositoryError> {
    let definition = parse(source).map_err(RepositoryError::invalid_blueprint_definition)?;
    let mut resolved_includes = Vec::with_capacity(definition.includes.len());

    for include in &definition.includes {
        if !resolving.insert((include.code.clone(), include.version)) {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "recursive include '{}@{}'",
                include.code, include.version
            )));
        }

        // Includes name an exact revision so compiling a new mixin revision
        // cannot change the effective schema of an existing blueprint.
        let included_source = sqlx::query_as::<_, IncludedBlueprint>(
            r#"SELECT kind, definition
               FROM blueprints
               WHERE code = $1 AND version = $2 AND workspace_id = $3 AND deleted_at IS NULL"#,
        )
        .bind(&include.code)
        .bind(include.version)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "include '{}@{}' was not found",
                include.code, include.version
            ))
        })?;

        if included_source.kind != "mixin" {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "include '{}@{}' is not a mixin",
                include.code, include.version
            )));
        }

        let compiled_include = Box::pin(compile_source(
            transaction,
            workspace_id,
            &included_source.definition,
            resolving,
        ))
        .await?;
        resolving.remove(&(include.code.clone(), include.version));
        if compiled_include.kind != BlueprintKind::Mixin || compiled_include.code != include.code {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "include '{}@{}' does not match its stored definition",
                include.code, include.version
            )));
        }

        resolved_includes.push(ResolvedInclude {
            alias: include.alias.clone(),
            code: include.code.clone(),
            version: include.version,
            attributes: compiled_include.attributes,
        });
    }

    let mut compiled = compile(definition, &resolved_includes, source)
        .map_err(RepositoryError::invalid_blueprint_definition)?;
    resolve_extension_attribute_types(transaction, workspace_id, &mut compiled).await?;
    validate_table_columns(transaction, workspace_id, &compiled).await?;
    validate_status_coverage(&compiled)?;
    Ok(compiled)
}

/// Status locks and approvals must name attributes of the effective entity
/// blueprint, so a typo cannot silently leave a finalized field editable.
/// Qualified `namespace:code` reusable attributes are attached per entity and
/// cannot be checked here.
fn validate_status_coverage(compiled: &CompiledBlueprint) -> Result<(), RepositoryError> {
    if compiled.kind != BlueprintKind::Entity {
        return Ok(());
    }
    let codes: HashSet<&str> = compiled
        .attributes
        .iter()
        .map(|attribute| attribute.code.as_str())
        .collect();
    for attribute in &compiled.attributes {
        let Some(options) = attribute
            .value_schema
            .as_ref()
            .and_then(|schema| schema.get(catalog_validation::status::STATUS_KEY))
            .and_then(|config| config.get("options"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for option in options {
            let lists = [
                option.get("lock"),
                option
                    .get("approval")
                    .and_then(|approval| approval.get("covers")),
            ];
            for code in lists
                .into_iter()
                .flatten()
                .filter_map(Value::as_array)
                .flatten()
                .filter_map(Value::as_str)
            {
                if !code.contains(':') && !codes.contains(code) {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "status attribute '{}' covers unknown attribute '{code}'",
                        attribute.code
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Resolve only while authoring/publishing. Persisted attributes contain the
/// complete declaration, so all ordinary reads remain independent of extension
/// lifecycle state.
async fn resolve_extension_attribute_types(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &mut CompiledBlueprint,
) -> Result<(), RepositoryError> {
    let installations = sqlx::query_as::<_, (String, Uuid, String, Value)>(
        "SELECT i.extension_id, i.installed_release_id, r.version, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.state = 'enabled'",
    )
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?;

    for attribute in &mut compiled.attributes {
        let Some(reference) = attribute.extension_type.as_ref() else {
            continue;
        };
        // Include attributes already compiled from a pinned revision carry the
        // final object rather than a source reference.
        let Some(reference) = reference.get("reference").and_then(Value::as_str) else {
            continue;
        };
        let (provider, type_id, requirement) = parse_extension_type_reference(reference)?;
        let Some((_, installed_release_id, release_version, manifest)) = installations
            .iter()
            .find(|(extension_id, _, _, _)| extension_id == provider)
        else {
            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "extension attribute type '{reference}' has no enabled provider"
            )));
        };
        let manifest: Manifest = serde_json::from_value(manifest.clone()).map_err(|_| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "extension attribute type '{reference}' provider manifest is invalid"
            ))
        })?;
        let declaration = manifest
            .attribute_types
            .iter()
            .filter(|item| {
                item.id == type_id
                    && requirement.matches(
                        &semver::Version::parse(&item.version)
                            .expect("manifest validated at install"),
                    )
            })
            .max_by_key(|item| {
                semver::Version::parse(&item.version).expect("manifest validated at install")
            })
            .ok_or_else(|| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "extension attribute type '{reference}' has no compatible declaration"
                ))
            })?;
        let configuration = attribute
            .extension_type
            .as_ref()
            .and_then(|value| value.get("configuration"))
            .cloned()
            .unwrap_or(Value::Null);
        match &declaration.configuration_schema {
            Some(schema) => {
                if let Some(error) = validate_json_schema(schema, &configuration)
                    .map_err(|message| RepositoryError::InvalidBlueprintDefinition(format!(
                        "extension attribute type '{reference}' has invalid configuration schema: {message}"
                    )))?
                    .into_iter()
                    .next()
                {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "extension attribute type '{reference}' configuration {}: {}",
                        error.instance_path, error.message
                    )));
                }
            }
            None if !configuration.is_null() => {
                return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                    "extension attribute type '{reference}' does not accept configuration"
                )));
            }
            None => {}
        }
        attribute.value_type = declaration.primitive.clone();
        attribute.value_schema = declaration.value_schema.clone();
        attribute.extension_type = Some(json!({
            "provider": provider,
            "type": type_id,
            "version": declaration.version,
            "primitive": declaration.primitive,
            "value_schema": declaration.value_schema,
            "configuration_schema": declaration.configuration_schema,
            "configuration": configuration,
            "installed_release_id": installed_release_id,
            "release_version": release_version,
        }));
    }
    Ok(())
}

fn parse_extension_type_reference(
    reference: &str,
) -> Result<(&str, &str, VersionReq), RepositoryError> {
    let Some((provider, type_and_requirement)) = reference.split_once(':') else {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "extension_type '{reference}' must use provider:type@semver-range"
        )));
    };
    let Some((type_id, requirement)) = type_and_requirement.rsplit_once('@') else {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "extension_type '{reference}' must use provider:type@semver-range"
        )));
    };
    if provider.is_empty() || type_id.is_empty() || requirement.is_empty() {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "extension_type '{reference}' must use provider:type@semver-range"
        )));
    }
    let requirement = VersionReq::parse(requirement).map_err(|_| {
        RepositoryError::InvalidBlueprintDefinition(format!(
            "extension_type '{reference}' has an invalid SemVer range"
        ))
    })?;
    Ok((provider, type_id, requirement))
}

async fn validate_table_columns(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    for view in compiled.views.values() {
        let ViewDefinition::Table {
            columns: Some(columns),
            ..
        } = view
        else {
            continue;
        };
        for column in columns {
            let parts: Vec<_> = column.field.split('.').collect();
            let value_type = if parts.len() == 1 {
                compiled
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == column.field)
                    .expect("compiler validated table field")
                    .value_type
                    .clone()
            } else {
                let source = compiled
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == parts[0])
                    .expect("compiler validated table relationship");
                let mut target = source.target_blueprint.clone().ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' relationship has no target blueprint",
                        column.field
                    ))
                })?;
                let mut leaf_type = None;
                for (index, field) in parts[1..].iter().enumerate() {
                    let (value_type, next_target) = sqlx::query_as::<_, (String, Option<String>)>(
                        "SELECT a.value_type, a.target_blueprint_code FROM blueprints b JOIN attributes a ON a.blueprint_id = b.id AND a.blueprint_version = b.version WHERE b.code = $1 AND b.workspace_id = $2 AND b.status = 'published' AND b.deleted_at IS NULL AND a.code = $3 AND a.deleted_at IS NULL ORDER BY b.version DESC LIMIT 1",
                    )
                    .bind(&target)
                    .bind(workspace_id)
                    .bind(field)
                    .fetch_optional(&mut **transaction)
                    .await?
                    .ok_or_else(|| RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' path segment '{}' was not found", column.field, field
                    )))?;
                    let is_leaf = index + 1 == parts.len() - 1;
                    if is_leaf {
                        if value_type == "relationship" || value_type == "file" {
                            return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                                "table column '{}' leaf must be scalar",
                                column.field
                            )));
                        }
                        leaf_type = Some(value_type);
                    } else if value_type != "relationship" {
                        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                            "table column '{}' segment '{}' must be a relationship",
                            column.field, field
                        )));
                    } else {
                        target = next_target.ok_or_else(|| {
                            RepositoryError::InvalidBlueprintDefinition(format!(
                                "table column '{}' segment '{}' has no target blueprint",
                                column.field, field
                            ))
                        })?;
                    }
                }
                leaf_type.expect("relationship table path has a leaf")
            };
            if let Some(renderer) = &column.renderer {
                if renderer.id == "catalog.table_image" {
                    let image_attribute = (parts.len() == 1)
                        .then(|| {
                            compiled
                                .attributes
                                .iter()
                                .find(|attribute| attribute.code == column.field)
                        })
                        .flatten();
                    let valid_image_column = image_attribute.is_some_and(|attribute| {
                        attribute.value_type == "file"
                            && attribute.file_policy.as_ref().is_some_and(|policy| {
                                policy.image_only && policy.cardinality == "one"
                            })
                    });
                    if !valid_image_column {
                        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                            "table image renderer requires a direct image-only single-file attribute ('{}')",
                            column.field
                        )));
                    }
                }
                validate_renderer(transaction, workspace_id, renderer, &value_type).await?;
            }
        }
    }
    Ok(())
}

async fn validate_renderer(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    renderer: &catalog_blueprint::ComponentReference,
    value_type: &str,
) -> Result<(), RepositoryError> {
    if renderer.id.starts_with("catalog.") {
        return validate_table_renderer(renderer, value_type)
            .map_err(RepositoryError::invalid_blueprint_definition);
    }
    let manifests = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.state = 'enabled'",
    )
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?;
    let valid = manifests
        .into_iter()
        .filter_map(|raw| serde_json::from_value::<Manifest>(raw).ok())
        .flat_map(|manifest| manifest.cell_renderers)
        .any(|candidate| {
            candidate.id == renderer.id
                && candidate.version == renderer.version
                && candidate.value_types.iter().any(|item| item == value_type)
                && (renderer.props.is_null()
                    || renderer.props.as_object().is_some_and(|props| {
                        props
                            .keys()
                            .all(|key| candidate.allowed_props.iter().any(|prop| prop == key))
                    }))
        });
    if valid {
        Ok(())
    } else {
        Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "table column renderer '{}@{}' is not declared for {value_type}",
            renderer.id, renderer.version
        )))
    }
}

#[derive(sqlx::FromRow)]
struct IncludedBlueprint {
    kind: String,
    definition: String,
}
