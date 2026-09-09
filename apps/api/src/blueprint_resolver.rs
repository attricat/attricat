use std::collections::HashSet;

use crate::extensions::Manifest;
use async_recursion::async_recursion;
use catalog_blueprint::{
    BlueprintKind, CompiledBlueprint, ResolvedInclude, ViewDefinition, compile, parse,
    validate_table_renderer,
};
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

#[async_recursion]
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

        let compiled_include = compile_source(
            transaction,
            workspace_id,
            &included_source.definition,
            resolving,
        )
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

    let compiled = compile(definition, &resolved_includes, source)
        .map_err(RepositoryError::invalid_blueprint_definition)?;
    validate_table_columns(transaction, workspace_id, &compiled).await?;
    Ok(compiled)
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
            let value_type = if let Some((relationship, target_field)) =
                column.field.split_once('.')
            {
                let source = compiled
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == relationship)
                    .expect("compiler validated table relationship");
                if source.relationship_cardinality.as_deref() != Some("one_to_one") {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' requires a one_to_one relationship",
                        column.field
                    )));
                }
                let target = source.target_blueprint.as_deref().ok_or_else(|| {
                    RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' relationship has no target blueprint",
                        column.field
                    ))
                })?;
                let target_type = sqlx::query_scalar::<_, String>(
                    "SELECT a.value_type FROM blueprints b JOIN attributes a ON a.blueprint_id = b.id AND a.blueprint_version = b.version WHERE b.code = $1 AND b.workspace_id = $2 AND b.deleted_at IS NULL AND a.code = $3 AND a.deleted_at IS NULL ORDER BY b.version DESC LIMIT 1",
                )
                .bind(target)
                .bind(workspace_id)
                .bind(target_field)
                .fetch_optional(&mut **transaction)
                .await?
                .ok_or_else(|| RepositoryError::InvalidBlueprintDefinition(format!(
                    "table column '{}' target field was not found", column.field
                )))?;
                if target_type == "relationship" || target_type == "file" {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' target field must be scalar",
                        column.field
                    )));
                }
                target_type
            } else {
                compiled
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == column.field)
                    .expect("compiler validated table field")
                    .value_type
                    .clone()
            };
            if let Some(renderer) = &column.renderer {
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
                && candidate.version == renderer.version as u32
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
