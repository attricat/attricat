use attricat_domain::attribute_options::Cardinality;
use attricat_domain::value_type::ValueType;
use std::collections::HashSet;

use crate::extensions::{BUILT_IN_CELL_PREFIX, Manifest, UiContributionKind, UiOutlet};
use attricat_blueprint::{
    BlueprintKind, BrowseBy, CompiledBlueprint, ResolvedInclude, ViewDefinition, ViewNode, compile,
    parse, validate_table_renderer,
};
use attricat_validation::saved_search::browse_path_hop;
use attricat_validation::validate_json_schema;
use semver::VersionReq;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Postgres, Transaction};
use uuid::Uuid;

use crate::persistence_rows::{Db, IntoDomain};
use crate::repository::relationship_paths::{
    LoadedBlueprints, OnceLoader, PathBlueprint, PathLoader, walk_relationship_path,
};
use crate::repository::{ATTRIBUTE_COLUMNS, LATEST_PUBLISHED_BY_CODE, RepositoryError};
use attricat_domain::model::Attribute;

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
    validate_default_from_paths(transaction, workspace_id, &compiled).await?;
    validate_field_editors(transaction, workspace_id, &compiled).await?;
    validate_form_assists(transaction, workspace_id, &compiled).await?;
    validate_browse_by(transaction, workspace_id, &compiled).await?;
    validate_status_coverage(&compiled)?;
    Ok(compiled)
}

/// Status locks and approvals must name attributes of the effective record
/// blueprint, so a typo cannot silently leave a finalized field editable.
/// Qualified `namespace:code` reusable attributes are attached per record and
/// cannot be checked here.
fn validate_status_coverage(compiled: &CompiledBlueprint) -> Result<(), RepositoryError> {
    if compiled.kind != BlueprintKind::Record {
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
            .and_then(|schema| schema.get(attricat_validation::status::STATUS_KEY))
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
        let manifest: Manifest = serde::Deserialize::deserialize(manifest).map_err(|_| {
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
        attribute.value_type = ValueType::parse(&declaration.primitive).ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "extension attribute type '{reference}' has unsupported primitive '{}'",
                declaration.primitive
            ))
        })?;
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

/// An attribute of the latest published revision of a blueprint, as a
/// relationship path following it from another blueprint sees it.
#[derive(sqlx::FromRow)]
struct PathSegment {
    #[sqlx(try_from = "String")]
    value_type: ValueType,
    target_blueprint: Option<String>,
    cardinality: Option<String>,
}

async fn published_path_segment(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    blueprint_code: &str,
    field: &str,
) -> Result<Option<PathSegment>, RepositoryError> {
    Ok(sqlx::query_as::<_, PathSegment>(
        "SELECT a.value_type, a.target_blueprint_code AS target_blueprint, a.cardinality FROM blueprints b JOIN attributes a ON a.blueprint_id = b.id AND a.blueprint_version = b.version WHERE b.code = $1 AND b.workspace_id = $2 AND b.status = 'published' AND b.deleted_at IS NULL AND a.code = $3 AND a.deleted_at IS NULL ORDER BY b.version DESC LIMIT 1",
    )
    .bind(blueprint_code)
    .bind(workspace_id)
    .bind(field)
    .fetch_optional(&mut **transaction)
    .await?)
}

/// Loads the blueprints a table column path reaches inside the transaction
/// that saves the blueprint, which may have published them.
struct TransactionPathLoader<'t, 'c> {
    transaction: &'t mut Transaction<'c, Postgres>,
    workspace_id: Uuid,
}

impl PathLoader for TransactionPathLoader<'_, '_> {
    fn load(
        &mut self,
        code: &str,
    ) -> impl Future<Output = Result<Option<PathBlueprint>, RepositoryError>> + Send {
        published_path_blueprint(self.transaction, self.workspace_id, code)
    }
}

/// The latest published revision of blueprint `code` with its attributes,
/// as a table column path reaching it sees it inside this transaction.
async fn published_path_blueprint(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    code: &str,
) -> Result<Option<PathBlueprint>, RepositoryError> {
    let Some((id, version)) = sqlx::query_as::<_, (Uuid, i64)>(LATEST_PUBLISHED_BY_CODE)
        .bind(code)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?
    else {
        return Ok(None);
    };
    let attributes: Vec<Attribute> = sqlx::query_as::<_, Db<Attribute>>(AssertSqlSafe(format!(
        r#"SELECT {ATTRIBUTE_COLUMNS}
             FROM attributes
            WHERE blueprint_id = $1 AND blueprint_version = $2 AND workspace_id = $3 AND deleted_at IS NULL
            ORDER BY position"#
    )))
    .bind(id)
    .bind(version)
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_domain();
    Ok(Some(PathBlueprint {
        id,
        code: code.to_owned(),
        attributes: attributes.into(),
    }))
}

/// Follows the hops of a `views.explorer.browse_by` path past the first,
/// which the compiler checked, through the published target blueprints.
async fn validate_browse_by(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    let Some(ViewDefinition::Explorer {
        browse_by: Some(BrowseBy::Field(path)),
    }) = compiled.views.get("explorer")
    else {
        return Ok(());
    };
    let hops: Vec<_> = path.split('.').collect();
    // A single field, which may link to several blueprints, has no later hops.
    if hops.len() == 1 {
        return Ok(());
    }
    let mut target = compiled
        .attributes
        .iter()
        .find(|attribute| attribute.code == hops[0])
        .and_then(|attribute| attribute.target_blueprint.clone())
        .expect("compiler validated the first browse_by hop");
    for hop in &hops[1..] {
        let segment = published_path_segment(transaction, workspace_id, &target, hop)
            .await?
            .filter(|segment| {
                browse_path_hop(
                    segment.value_type,
                    segment.target_blueprint.as_deref(),
                    segment.cardinality.as_deref().and_then(Cardinality::parse),
                    hops.len(),
                )
            })
            .ok_or_else(|| {
                RepositoryError::InvalidBlueprintDefinition(format!(
                    "views.explorer browse_by '{path}' segment '{hop}' must be a relationship of published blueprint '{target}' with a single target blueprint and cardinality: one"
                ))
            })?;
        target = segment
            .target_blueprint
            .expect("browse_by hop has a target");
    }
    Ok(())
}

async fn validate_table_columns(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    // Columns through the same relationships share their targets' loads.
    let mut loaded = LoadedBlueprints::new();
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
            } else {
                let resolved = walk_relationship_path(
                    &column.field,
                    &parts,
                    (&compiled.code, &compiled.attributes),
                    OnceLoader {
                        inner: TransactionPathLoader {
                            transaction: &mut *transaction,
                            workspace_id,
                        },
                        loaded: &mut loaded,
                    },
                )
                .await
                .map_err(|error| {
                    error.into_repository(|error| {
                        RepositoryError::InvalidBlueprintDefinition(format!(
                            "table column '{}' {error}",
                            column.field
                        ))
                    })
                })?;
                if matches!(
                    resolved.leaf.value_type,
                    ValueType::Relationship | ValueType::File
                ) {
                    return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                        "table column '{}' leaf must be scalar",
                        column.field
                    )));
                }
                resolved.leaf.value_type
            };
            if let Some(renderer) = &column.renderer {
                if renderer.id == "attricat.table_image" {
                    let image_attribute = (parts.len() == 1)
                        .then(|| {
                            compiled
                                .attributes
                                .iter()
                                .find(|attribute| attribute.code == column.field)
                        })
                        .flatten();
                    let valid_image_column = image_attribute.is_some_and(|attribute| {
                        attribute.value_type == ValueType::File
                            && attribute.file_policy.as_ref().is_some_and(|policy| {
                                policy.image_only && policy.cardinality == Cardinality::One
                            })
                    });
                    if !valid_image_column {
                        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
                            "table image renderer requires a direct image-only single-file attribute ('{}')",
                            column.field
                        )));
                    }
                }
                validate_renderer(transaction, workspace_id, renderer, value_type).await?;
            }
        }
    }
    Ok(())
}

/// A `default_from` path resolves like a table column, through the latest
/// published revisions of the blueprints it reaches, to one source value:
/// every hop holds a single target, and the leaf is a scalar of the
/// attribute's value type (an `integer` may fill a `number`). A leaf that only
/// some targets of a multi-target hop declare is allowed; the others yield
/// nothing.
async fn validate_default_from_paths(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    if compiled
        .attributes
        .iter()
        .all(|attribute| attribute.default_from.is_none())
    {
        return Ok(());
    }
    // A path back to this blueprint, such as a variant's `base.price`, sees
    // the revision being saved rather than the last published one.
    let mut loaded =
        LoadedBlueprints::from([(compiled.code.clone(), Some(draft_path_blueprint(compiled)))]);
    for attribute in &compiled.attributes {
        let Some(path) = &attribute.default_from else {
            continue;
        };
        let invalid = |message: String| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "attribute '{}' has an invalid default_from: '{path}' {message}",
                attribute.code
            ))
        };
        let parts: Vec<_> = path.split('.').collect();
        let resolved = walk_relationship_path(
            path,
            &parts,
            (&compiled.code, &compiled.attributes),
            OnceLoader {
                inner: TransactionPathLoader {
                    transaction: &mut *transaction,
                    workspace_id,
                },
                loaded: &mut loaded,
            },
        )
        .await
        .map_err(|error| error.into_repository(|error| invalid(error.to_string())))?;
        if !resolved.hops.single_valued {
            return Err(invalid(
                "must follow relationships with cardinality: one".into(),
            ));
        }
        let leaf = resolved.leaf.value_type;
        if !leaf.copies_into(attribute.value_type) {
            return Err(invalid(format!(
                "ends on a {leaf} attribute, which cannot fill a {} attribute",
                attribute.value_type
            )));
        }
    }
    Ok(())
}

/// The blueprint being saved as a path walk sees it: only the fields a walk
/// reads (code, value type, targets and cardinality) are meaningful.
fn draft_path_blueprint(compiled: &CompiledBlueprint) -> PathBlueprint {
    let now = chrono::Utc::now();
    PathBlueprint {
        id: Uuid::nil(),
        code: compiled.code.clone(),
        attributes: compiled
            .attributes
            .iter()
            .map(|attribute| Attribute {
                id: Uuid::nil(),
                blueprint_id: Uuid::nil(),
                blueprint_version: 0,
                code: attribute.code.clone(),
                name: attribute.name.clone(),
                description: None,
                value_type: attribute.value_type,
                value_schema: attribute.value_schema.clone(),
                extension_type: attribute.extension_type.clone(),
                default_value: attribute.default_value.clone(),
                default_from: attribute.default_from.clone(),
                file_policy: None,
                target_blueprint_code: attribute.target_blueprint.clone(),
                target_blueprint_codes: attribute.target_blueprints.clone(),
                cardinality: attribute.cardinality,
                target_cardinality: attribute.target_cardinality,
                hierarchy: attribute.hierarchy.clone(),
                tags: json!(attribute.tags),
                context_fallback: attribute.context_fallback,
                context_editable: attribute.context_editable,
                readonly: attribute.readonly,
                generated: None,
                balance: None,
                target_scope: None,
                position: attribute.position,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            })
            .collect(),
    }
}

/// Collects the `views.detail` fields that name an extension editor.
fn field_editors<'a>(
    nodes: &'a [ViewNode],
    found: &mut Vec<(&'a str, &'a attricat_blueprint::EditorReference)>,
) {
    for node in nodes {
        match node {
            ViewNode::Field {
                field,
                editor: Some(editor),
                ..
            } => found.push((field, editor)),
            ViewNode::Stack { children, .. }
            | ViewNode::Grid { children, .. }
            | ViewNode::Section { children, .. } => field_editors(children, found),
            ViewNode::Tabs { tabs, .. } => {
                for tab in tabs {
                    field_editors(&tab.children, found);
                }
            }
            ViewNode::Accordion { sections, .. } => {
                for section in sections {
                    field_editors(&section.children, found);
                }
            }
            _ => {}
        }
    }
}

/// A detail field's editor must be an enabled extension's `editor` with that
/// id and version that edits the field's value type, given props within their
/// declared types, as a table column's renderer is checked against a cell.
async fn validate_field_editors(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    let mut editors = Vec::new();
    match compiled.views.get("detail") {
        Some(
            ViewDefinition::Stack { children, .. }
            | ViewDefinition::Grid { children, .. }
            | ViewDefinition::Section { children, .. },
        ) => field_editors(children, &mut editors),
        Some(ViewDefinition::Tabs { tabs, .. }) => {
            for tab in tabs {
                field_editors(&tab.children, &mut editors);
            }
        }
        Some(ViewDefinition::Accordion { sections, .. }) => {
            for section in sections {
                field_editors(&section.children, &mut editors);
            }
        }
        _ => {}
    }
    if editors.is_empty() {
        return Ok(());
    }
    let declared = enabled_manifests(transaction, workspace_id)
        .await?
        .into_iter()
        .flat_map(|manifest| {
            let extension_id = manifest.attricat.id;
            manifest
                .ui
                .into_iter()
                .map(move |contribution| (extension_id.clone(), contribution))
        })
        .filter(|(_, contribution)| {
            contribution.kind == UiContributionKind::Editor
                && contribution.outlet == Some(UiOutlet::AttributeEditor)
        })
        .collect::<Vec<_>>();
    for (field, editor) in editors {
        let value_type = compiled
            .attributes
            .iter()
            .find(|attribute| attribute.code == field)
            .expect("compiler validated detail field")
            .value_type;
        let mut problem = None;
        let valid = declared
            .iter()
            .filter(|(extension_id, contribution)| {
                *extension_id == editor.extension_id
                    && contribution.id == editor.id
                    && contribution.version == editor.version
            })
            .filter_map(|(_, contribution)| contribution.editor.as_ref())
            .filter(|declaration| {
                declaration
                    .value_types
                    .iter()
                    .any(|item| item == value_type.as_str())
            })
            .any(
                |declaration| match declaration.validate_props(&editor.props) {
                    Ok(()) => true,
                    Err(error) => {
                        problem = Some(error);
                        false
                    }
                },
            );
        if !valid {
            return Err(RepositoryError::InvalidBlueprintDefinition(match problem {
                Some(problem) => format!(
                    "field '{field}' editor '{}:{}@{}' {problem}",
                    editor.extension_id, editor.id, editor.version
                ),
                None => format!(
                    "field '{field}' editor '{}:{}@{}' is not declared for {value_type} by an enabled extension",
                    editor.extension_id, editor.id, editor.version
                ),
            }));
        }
    }
    Ok(())
}

/// Each `views.detail.extensions` entry that names an enabled extension's
/// `form_assist` must fit the blueprint: its watched fields are fields of the
/// blueprint and its proposed fields are scalars the form can edit. Entries
/// for other contributions, or for extensions that are not enabled, are not
/// checked here; the host ignores what it cannot offer.
async fn validate_form_assists(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    compiled: &CompiledBlueprint,
) -> Result<(), RepositoryError> {
    let Some(detail) = compiled.views.get("detail") else {
        return Ok(());
    };
    if detail.extensions().is_empty() {
        return Ok(());
    }
    let manifests = enabled_manifests(transaction, workspace_id).await?;
    for entry in detail.extensions() {
        let Some((extension_id, contribution_id)) = attricat_blueprint::view_extension_key(entry)
        else {
            continue;
        };
        let invalid = |message: String| {
            RepositoryError::InvalidBlueprintDefinition(format!("form assist '{entry}' {message}"))
        };
        let Some(declaration) = manifests
            .iter()
            .filter(|manifest| manifest.attricat.id == extension_id)
            .flat_map(|manifest| &manifest.ui)
            .find(|contribution| {
                contribution.kind == UiContributionKind::FormAssist
                    && contribution.outlet == Some(UiOutlet::RecordForm)
                    && contribution.id == contribution_id
            })
            .and_then(|contribution| contribution.form_assist.as_ref())
        else {
            continue;
        };
        let attribute = |code: &str| {
            compiled
                .attributes
                .iter()
                .find(|attribute| attribute.code == code)
        };
        for code in &declaration.watch {
            if attribute(code).is_none_or(|attribute| attribute.value_type == ValueType::File) {
                return Err(invalid(format!(
                    "watches '{code}', which is not a scalar or relationship field of the blueprint"
                )));
            }
        }
        for code in &declaration.proposes {
            let editable = attribute(code).is_some_and(|attribute| {
                !matches!(
                    attribute.value_type,
                    ValueType::Relationship | ValueType::File
                ) && !attribute.readonly
                    && attribute.generated.is_none()
                    && attribute.balance.is_none()
            });
            if !editable {
                return Err(invalid(format!(
                    "proposes '{code}', which is not an editable scalar field of the blueprint"
                )));
            }
        }
    }
    Ok(())
}

/// The manifests of the workspace's enabled extension releases.
async fn enabled_manifests(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
) -> Result<Vec<Manifest>, RepositoryError> {
    let manifests = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id WHERE i.workspace_id = $1 AND i.state = 'enabled'",
    )
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(manifests
        .into_iter()
        .filter_map(|raw| serde_json::from_value::<Manifest>(raw).ok())
        .collect())
}

async fn validate_renderer(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    renderer: &attricat_blueprint::ComponentReference,
    value_type: ValueType,
) -> Result<(), RepositoryError> {
    if renderer.id.starts_with(BUILT_IN_CELL_PREFIX) {
        return validate_table_renderer(renderer, value_type)
            .map_err(RepositoryError::invalid_blueprint_definition);
    }
    // An enabled extension's cell with this id and version that shows the
    // column's value type, given props within their declared types.
    let mut problem = None;
    let valid = enabled_manifests(transaction, workspace_id)
        .await?
        .into_iter()
        .flat_map(|manifest| manifest.ui)
        .filter(|contribution| {
            contribution.kind == UiContributionKind::Cell
                && contribution.outlet == Some(UiOutlet::ExplorerTableCell)
                && contribution.id == renderer.id
                && contribution.version == renderer.version
        })
        .filter_map(|contribution| contribution.cell)
        .filter(|cell| {
            cell.value_types
                .iter()
                .any(|item| item == value_type.as_str())
        })
        .any(|cell| match cell.validate_props(&renderer.props) {
            Ok(()) => true,
            Err(error) => {
                problem = Some(error);
                false
            }
        });
    if valid {
        Ok(())
    } else if let Some(problem) = problem {
        Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "table column renderer '{}@{}' {problem}",
            renderer.id, renderer.version
        )))
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
