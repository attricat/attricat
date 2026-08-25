use std::collections::HashSet;

use async_recursion::async_recursion;
use catalog_blueprint::{BlueprintKind, CompiledBlueprint, ResolvedInclude, compile, parse};
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

    compile(definition, &resolved_includes, source)
        .map_err(RepositoryError::invalid_blueprint_definition)
}

#[derive(sqlx::FromRow)]
struct IncludedBlueprint {
    kind: String,
    definition: String,
}
