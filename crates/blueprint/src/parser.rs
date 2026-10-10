use std::collections::{HashMap, HashSet};

use attricat_validation::{CODE_PATTERN, is_valid_code, validate_json_schema_definition};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde::Deserialize;

use crate::{
    AttributeDeclaration, BlueprintDefinition, BlueprintError, BlueprintKind,
    CONNECTOR_JOB_DIRECTIONS, ConnectorJobDefinition, FilePolicy, IncludeRef, KNOWN_VIEW_NAMES,
    LocalAttributeDeclaration, MAX_UNIQUE_KEY_ATTRIBUTES, PublicationPolicy, UNIQUE_KEY_SCOPES,
    UniqueKeyDefinition, ViewDefinition,
};

pub const ATTRIBUTE_VALUE_TYPES: &[&str] = &[
    "string",
    "number",
    "integer",
    "boolean",
    "date",
    "datetime",
    "time",
    "json",
    "relationship",
    "file",
];
/// `one_to_one` is relationship shorthand for both directions set to `one`.
pub const ATTRIBUTE_CARDINALITIES: &[&str] = &["one", "many", "one_to_one"];
/// Per-direction relationship cardinalities, also the file cardinalities.
pub const DIRECTIONAL_CARDINALITIES: &[&str] = &["one", "many"];
pub const CONTEXT_FALLBACKS: &[&str] = &["default", "none"];
pub const CONTEXT_EDITABLE_SCOPES: &[&str] = &["all", "default"];
/// Longest blueprint or attribute description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;

/// An Attricat blueprint: a versioned record type or an includable mixin.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Attricat blueprint definition v1")]
struct RawBlueprintDefinition {
    /// Definition format version. Only `1` is supported.
    #[schemars(extend("const" = 1))]
    format_version: u32,
    /// Stable blueprint identifier, using ASCII letters, numbers, hyphens, and underscores.
    #[schemars(regex(pattern = CODE_PATTERN))]
    code: String,
    /// Human-readable blueprint name. `{{…}}` references resolve from the
    /// workspace lexicon.
    #[schemars(length(min = 1))]
    name: String,
    /// What the blueprint's records are, for people and agents, such as
    /// "Product groupings, such as Basic tools". `{{…}}` references resolve
    /// from the workspace lexicon.
    #[schemars(length(min = 1, max = MAX_DESCRIPTION_CHARS))]
    description: Option<String>,
    kind: BlueprintKind,
    /// Exact, version-pinned mixins whose attributes can be selected with `from`.
    #[serde(default)]
    includes: Vec<IncludeRef>,
    /// Named views: `dropdown_option`, `table`, `detail`, and `extension_layout`.
    /// The `detail` layout also drives editing; a legacy `edit` view is ignored.
    #[serde(default)]
    #[schemars(extend("x-attricat-key-suggestions" = KNOWN_VIEW_NAMES))]
    views: HashMap<String, ViewDefinition>,
    /// JSON Schema (Draft 2020-12) source for cross-field record validation.
    /// Record blueprints only.
    record_schema: Option<String>,
    /// Channel publication behavior when records are edited.
    #[serde(default)]
    publication: PublicationPolicy,
    /// Scheduled extension imports and exports. Record blueprints only.
    #[serde(default)]
    connector_jobs: Vec<ConnectorJobDefinition>,
    /// Namespaced extension metadata is preserved in the immutable source
    /// definition and intentionally ignored by the core blueprint compiler.
    #[serde(default)]
    #[schemars(with = "HashMap<String, serde_json::Value>")]
    extensions: HashMap<String, toml::Value>,
    /// Data-health rules evaluated for this blueprint's records.
    #[serde(default)]
    #[schemars(schema_with = "attricat_rules::embedded_rules_schema")]
    rules: Vec<toml::Value>,
    /// Business keys whose normalized values must be unique across the
    /// blueprint family. Record blueprints only.
    #[serde(default)]
    unique_keys: Vec<UniqueKeyDefinition>,
    /// Attributes in display order. Each declares exactly one of `value_type`,
    /// `extension_type`, or `from`.
    #[schemars(length(min = 1))]
    attributes: Vec<RawAttributeDeclaration>,
}

/// An attribute declared by the blueprint or selected from an include.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawAttributeDeclaration {
    /// Attribute identifier, unique within the blueprint.
    #[schemars(regex(pattern = CODE_PATTERN))]
    code: String,
    /// Human-readable label shown in forms, filters, and table headers.
    /// Defaults to the humanized `code`. Attributes selected with `from` use
    /// the mixin's name and cannot set their own. `{{…}}` references resolve
    /// from the workspace lexicon.
    #[schemars(length(min = 1))]
    name: Option<String>,
    /// What the attribute holds, for people and agents. Attributes selected
    /// with `from` use the mixin's description and cannot set their own.
    /// `{{…}}` references resolve from the workspace lexicon.
    #[schemars(length(min = 1, max = MAX_DESCRIPTION_CHARS))]
    description: Option<String>,
    /// Stored value type.
    #[schemars(extend("enum" = ATTRIBUTE_VALUE_TYPES))]
    value_type: Option<String>,
    /// A `provider:type@semver-range` reference resolved by the host using an
    /// enabled workspace installation.
    extension_type: Option<String>,
    /// JSON Schema (Draft 2020-12) source for scalar values.
    value_schema: Option<String>,
    /// JSON configuration passed to the extension type.
    extension_configuration: Option<String>,
    /// Value stored in the default context when a record is created.
    /// Not supported for relationships and files.
    default_value: Option<serde_json::Value>,
    /// Relationships: `one`, `many` (default), or `one_to_one`.
    /// Files: `one` (default) or `many`.
    #[schemars(extend(
        "enum" = ATTRIBUTE_CARDINALITIES,
        "x-attricat-value-types" = ["relationship", "file"]
    ))]
    cardinality: Option<String>,
    /// How many sources may link to one target. Defaults to `many`.
    #[schemars(extend(
        "enum" = DIRECTIONAL_CARDINALITIES,
        "x-attricat-value-types" = ["relationship"]
    ))]
    target_cardinality: Option<String>,
    /// Whether multiple files keep their order. Defaults to `true` for `many`.
    #[schemars(extend("x-attricat-value-types" = ["file"]))]
    ordered: Option<bool>,
    /// Accepted MIME groups, such as `image`.
    #[serde(default)]
    #[schemars(extend("x-attricat-value-types" = ["file"]))]
    allowed_mime_groups: Vec<String>,
    /// Accepted file extensions without the leading dot.
    #[serde(default)]
    #[schemars(extend("x-attricat-value-types" = ["file"]))]
    allowed_extensions: Vec<String>,
    /// Maximum upload size in bytes.
    #[schemars(range(min = 1), extend("x-attricat-value-types" = ["file"]))]
    max_bytes: Option<u64>,
    /// File purpose codes, such as `product_image`.
    #[serde(default)]
    #[schemars(extend("x-attricat-value-types" = ["file"]))]
    purposes: Vec<String>,
    /// Accept only images and generate thumbnails.
    #[schemars(extend("x-attricat-value-types" = ["file"]))]
    image_only: Option<bool>,
    /// Blueprint that relationship targets must use.
    #[schemars(
        regex(pattern = CODE_PATTERN),
        extend(
            "x-attricat-reference" = "blueprint",
            "x-attricat-value-types" = ["relationship"]
        )
    )]
    target_blueprint: Option<String>,
    /// Several blueprints that relationship targets may use. Cannot be
    /// combined with `target_blueprint`.
    #[serde(default)]
    #[schemars(
        length(min = 1),
        extend(
            "x-attricat-reference" = "blueprint",
            "x-attricat-value-types" = ["relationship"]
        )
    )]
    target_blueprints: Vec<String>,
    /// Reject relationship writes that would form a cycle through this
    /// self-referencing attribute. Requires `context_editable = "default"`.
    #[schemars(extend("x-attricat-value-types" = ["relationship"]))]
    acyclic: Option<bool>,
    /// An acyclic hierarchy in which every record has at most one target
    /// (parent). Implies `acyclic` and `cardinality = "one"`.
    #[schemars(extend("x-attricat-value-types" = ["relationship"]))]
    tree: Option<bool>,
    /// Free-form metadata. `hidden`, `hidden:form`, `hidden:detail`,
    /// `hidden:explorer`, and `hidden:metadata` hide the attribute from default UI.
    #[serde(default)]
    #[schemars(extend("x-attricat-suggestions" = [
        "hidden",
        "hidden:form",
        "hidden:detail",
        "hidden:explorer",
        "hidden:metadata"
    ]))]
    tags: Vec<String>,
    /// Missing values in a non-default context: `default` inherits from the
    /// nearest ancestor context; `none` leaves the attribute absent.
    #[serde(default = "default_context_fallback")]
    #[schemars(extend("enum" = CONTEXT_FALLBACKS))]
    context_fallback: String,
    /// Contexts that accept writes: `all`, or only the `default` context.
    #[serde(default = "default_context_editable")]
    #[schemars(extend("enum" = CONTEXT_EDITABLE_SCOPES))]
    context_editable: String,
    /// Preview-only in the Attricat web app. API writes remain allowed.
    #[serde(default)]
    readonly: bool,
    /// Selects `<include alias>.<attribute code>` from an include. The
    /// attribute code must match `code`.
    #[schemars(extend("x-attricat-reference" = "include_attribute"))]
    from: Option<String>,
}

/// The JSON Schema for blueprint definition TOML, published as
/// `contracts/blueprint-definition-v1.schema.json`. It describes structure
/// and allowed values; cross-field rules remain enforced by [`parse`].
pub fn definition_json_schema() -> serde_json::Value {
    SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<RawBlueprintDefinition>()
        .to_value()
}

fn default_context_fallback() -> String {
    "default".to_owned()
}

fn default_context_editable() -> String {
    "all".to_owned()
}

pub fn parse(source: &str) -> Result<BlueprintDefinition, BlueprintError> {
    let raw: RawBlueprintDefinition = toml::from_str(source)?;
    // Parsing this field here makes `[extensions.<extension-id>]` a supported,
    // namespaced escape hatch while retaining strict validation for every core
    // blueprint field. Extension runtimes read their own data from the saved
    // immutable `definition` text.
    let _extension_metadata = &raw.extensions;
    if raw.format_version != 1 {
        return Err(BlueprintError::UnsupportedFormatVersion(raw.format_version));
    }
    validate_code(&raw.code, "blueprint code")?;
    validate_non_empty(&raw.name, "blueprint name")?;
    if let Some(description) = &raw.description {
        validate_description(description, "blueprint description")?;
    }
    if raw.attributes.is_empty() {
        return Err(BlueprintError::EmptyAttributes);
    }
    let record_schema = parse_json_schema(raw.record_schema, "record_schema")?;
    if record_schema.is_some() && raw.kind != BlueprintKind::Record {
        return Err(BlueprintError::RecordSchemaOnMixin);
    }

    let mut aliases = HashSet::new();
    for include in &raw.includes {
        validate_code(&include.alias, "include alias")?;
        validate_code(&include.code, "include code")?;
        if include.version <= 0 {
            return Err(BlueprintError::EmptyField("include version"));
        }
        if !aliases.insert(include.alias.clone()) {
            return Err(BlueprintError::DuplicateIncludeAlias(include.alias.clone()));
        }
    }

    let mut publication_roles = HashSet::new();
    for role in &raw.publication.retain_on_edit_roles {
        validate_code(role, "publication retain_on_edit_roles")?;
        if !publication_roles.insert(role) {
            return Err(BlueprintError::DuplicatePublicationRole(role.clone()));
        }
    }

    let mut job_codes = HashSet::new();
    if !raw.connector_jobs.is_empty() && raw.kind != BlueprintKind::Record {
        return Err(BlueprintError::InvalidConnectorJob(
            "only record blueprints can define connector jobs".into(),
        ));
    }
    for job in &raw.connector_jobs {
        validate_code(&job.code, "connector job code")?;
        if !job_codes.insert(&job.code) {
            return Err(BlueprintError::InvalidConnectorJob(format!(
                "duplicate connector job code '{}'",
                job.code
            )));
        }
        if !CONNECTOR_JOB_DIRECTIONS.contains(&job.direction.as_str())
            || (job.direction == "import") != job.context.is_some()
            || (job.direction == "export" && job.input_file_id.is_some())
            || job
                .interval_seconds
                .is_some_and(|seconds| !(60..=2_592_000).contains(&seconds))
            || !job.input.is_table()
        {
            return Err(BlueprintError::InvalidConnectorJob(format!(
                "invalid connector job '{}'",
                job.code
            )));
        }
        if let Some(context) = &job.context {
            validate_code(context, "connector job context")?;
        }
    }

    let mut codes = HashSet::new();
    let mut attributes = Vec::with_capacity(raw.attributes.len());
    for attribute in raw.attributes {
        validate_code(&attribute.code, "attribute code")?;
        if !codes.insert(attribute.code.clone()) {
            return Err(BlueprintError::DuplicateAttributeCode(attribute.code));
        }
        if let Some(name) = &attribute.name {
            validate_non_empty(name, "attribute name")?;
        }
        if let Some(description) = &attribute.description {
            validate_description(description, "attribute description")?;
        }

        attributes.push(
            match (
                attribute.value_type.clone(),
                attribute.extension_type.clone(),
                attribute.from.clone(),
            ) {
                (Some(value_type), None, None) => {
                    validate_non_empty(&value_type, "attribute value_type")?;
                    if !ATTRIBUTE_VALUE_TYPES.contains(&value_type.as_str()) {
                        return Err(BlueprintError::UnsupportedValueType {
                            code: attribute.code,
                            value_type,
                        });
                    }
                    if attribute.declares_relationship_targets() && value_type != "relationship" {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    let (target_blueprint, target_blueprints) =
                        parse_relationship_targets(&attribute)?;
                    let hierarchy = parse_relationship_hierarchy(
                        &attribute,
                        &raw.code,
                        &raw.kind,
                        &target_blueprints,
                    )?;
                    if value_type == "relationship" && attribute.value_schema.is_some() {
                        return Err(BlueprintError::RelationshipValueSchema {
                            code: attribute.code,
                        });
                    }
                    if attribute.default_value.is_some()
                        && matches!(value_type.as_str(), "relationship" | "file")
                    {
                        return Err(BlueprintError::DefaultValueOnNonScalarAttribute {
                            code: attribute.code,
                        });
                    }
                    let has_file_policy = (attribute.cardinality.is_some()
                        && value_type != "relationship")
                        || attribute.ordered.is_some()
                        || !attribute.allowed_mime_groups.is_empty()
                        || !attribute.allowed_extensions.is_empty()
                        || attribute.max_bytes.is_some()
                        || !attribute.purposes.is_empty()
                        || attribute.image_only.is_some();
                    if value_type != "file" && has_file_policy {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    let (cardinality, target_cardinality) = if value_type == "relationship" {
                        // A tree gives every record at most one parent.
                        let cardinality = attribute.cardinality.clone().unwrap_or_else(|| {
                            if hierarchy.as_deref() == Some("tree") {
                                "one".to_owned()
                            } else {
                                "many".to_owned()
                            }
                        });
                        let (cardinality, target_cardinality) = if cardinality == "one_to_one" {
                            if attribute.target_cardinality.is_some() {
                                return Err(BlueprintError::InvalidRelationshipCardinality(
                                    attribute.code,
                                ));
                            }
                            ("one".to_owned(), "one".to_owned())
                        } else {
                            (
                                cardinality,
                                attribute
                                    .target_cardinality
                                    .clone()
                                    .unwrap_or_else(|| "many".to_owned()),
                            )
                        };
                        if !DIRECTIONAL_CARDINALITIES.contains(&cardinality.as_str())
                            || !DIRECTIONAL_CARDINALITIES.contains(&target_cardinality.as_str())
                        {
                            return Err(BlueprintError::InvalidRelationshipCardinality(
                                attribute.code,
                            ));
                        }
                        if hierarchy.as_deref() == Some("tree") && cardinality != "one" {
                            return Err(BlueprintError::InvalidRelationshipHierarchy {
                                code: attribute.code,
                                message: "a tree allows at most one target per record; use cardinality = \"one\"".into(),
                            });
                        }
                        (Some(cardinality), Some(target_cardinality))
                    } else {
                        if attribute.target_cardinality.is_some() {
                            return Err(BlueprintError::InvalidAttributeDeclaration(
                                attribute.code,
                            ));
                        }
                        (None, None)
                    };
                    if value_type == "file"
                        && (attribute.value_schema.is_some()
                            || attribute.target_blueprint.is_some())
                    {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    let file_policy = if value_type == "file" {
                        Some(parse_file_policy(&attribute, &attribute.code)?)
                    } else {
                        None
                    };
                    let value_schema = parse_json_schema(
                        attribute.value_schema,
                        &format!("attribute '{}'.value_schema", attribute.code),
                    )?;
                    if value_schema.as_ref().is_some_and(|schema| {
                        schema.get(attricat_validation::status::STATUS_KEY).is_some()
                    }) && value_type != "string"
                    {
                        return Err(BlueprintError::InvalidJsonSchema {
                            field: format!("attribute '{}'.value_schema", attribute.code),
                            message: "status is only supported on string attributes".into(),
                        });
                    }
                    if value_schema.as_ref().is_some_and(|schema| {
                        schema
                            .get(attricat_validation::principal::PRINCIPAL_KEY)
                            .is_some()
                    }) && (value_type != "string" || attribute.default_value.is_some())
                    {
                        return Err(BlueprintError::InvalidJsonSchema {
                            field: format!("attribute '{}'.value_schema", attribute.code),
                            message: "user or team assignment requires a string attribute without a default_value".into(),
                        });
                    }
                    if let (Some(schema), Some(default)) = (&value_schema, &attribute.default_value)
                    {
                        attricat_validation::status::validate_status_transition(
                            schema,
                            &serde_json::Value::Null,
                            default,
                        )
                        .map_err(|message| {
                            BlueprintError::InvalidJsonSchema {
                                field: format!("attribute '{}'.default_value", attribute.code),
                                message,
                            }
                        })?;
                    }
                    if !CONTEXT_FALLBACKS.contains(&attribute.context_fallback.as_str()) {
                        return Err(BlueprintError::InvalidContextFallback {
                            code: attribute.code,
                            context_fallback: attribute.context_fallback,
                        });
                    }
                    if !CONTEXT_EDITABLE_SCOPES.contains(&attribute.context_editable.as_str()) {
                        return Err(BlueprintError::InvalidContextEditable {
                            code: attribute.code,
                            context_editable: attribute.context_editable,
                        });
                    }
                    let mut tags = HashSet::new();
                    for tag in &attribute.tags {
                        if tag.trim().is_empty() || !tags.insert(tag.as_str()) {
                            return Err(BlueprintError::InvalidAttributeTag(attribute.code));
                        }
                    }
                    AttributeDeclaration::Local(Box::new(LocalAttributeDeclaration {
                        code: attribute.code,
                        name: attribute.name,
                        description: attribute.description,
                        value_type,
                        value_schema,
                        extension_type: None,
                        extension_configuration: None,
                        default_value: attribute.default_value,
                        file_policy,
                        target_blueprint,
                        target_blueprints,
                        cardinality,
                        target_cardinality,
                        hierarchy,
                        tags: attribute.tags,
                        context_fallback: attribute.context_fallback,
                        context_editable: attribute.context_editable,
                        readonly: attribute.readonly,
                    }))
                }
                (None, None, Some(source))
                    if !attribute.declares_relationship_targets()
                        && attribute.name.is_none()
                        && attribute.description.is_none() =>
                {
                    let (include_alias, attribute_code) =
                        parse_selection(&attribute.code, &source)?;
                    AttributeDeclaration::Selection {
                        code: attribute.code,
                        include_alias,
                        attribute_code,
                    }
                }
                (None, Some(extension_type), None) => {
                    if extension_type.trim().is_empty()
                        || attribute.value_schema.is_some()
                        || attribute.declares_relationship_targets()
                        || attribute.cardinality.is_some()
                        || attribute.target_cardinality.is_some()
                        || attribute.ordered.is_some()
                        || !attribute.allowed_mime_groups.is_empty()
                        || !attribute.allowed_extensions.is_empty()
                        || attribute.max_bytes.is_some()
                        || !attribute.purposes.is_empty()
                        || attribute.image_only.is_some()
                    {
                        return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                    }
                    if !CONTEXT_FALLBACKS.contains(&attribute.context_fallback.as_str()) {
                        return Err(BlueprintError::InvalidContextFallback {
                            code: attribute.code,
                            context_fallback: attribute.context_fallback,
                        });
                    }
                    if !CONTEXT_EDITABLE_SCOPES.contains(&attribute.context_editable.as_str()) {
                        return Err(BlueprintError::InvalidContextEditable {
                            code: attribute.code,
                            context_editable: attribute.context_editable,
                        });
                    }
                    let extension_configuration = parse_json_value(
                        attribute.extension_configuration,
                        &format!("attribute '{}'.extension_configuration", attribute.code),
                    )?;
                    AttributeDeclaration::Local(Box::new(LocalAttributeDeclaration {
                        code: attribute.code,
                        name: attribute.name,
                        description: attribute.description,
                        // Resolved before persistence; this keeps the pure compiler
                        // useful without giving extensions storage control.
                        value_type: "string".to_owned(),
                        value_schema: None,
                        extension_type: Some(extension_type),
                        extension_configuration,
                        default_value: attribute.default_value,
                        file_policy: None,
                        target_blueprint: None,
                        target_blueprints: Vec::new(),
                        cardinality: None,
                        target_cardinality: None,
                        hierarchy: None,
                        tags: attribute.tags,
                        context_fallback: attribute.context_fallback,
                        context_editable: attribute.context_editable,
                        readonly: attribute.readonly,
                    }))
                }
                _ => return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code)),
            },
        );
    }

    let rules = raw
        .rules
        .into_iter()
        .map(attricat_rules::compile_embedded)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| BlueprintError::InvalidRule(error.to_string()))?;
    let mut rule_codes = HashSet::new();
    for rule in &rules {
        if !rule_codes.insert(rule.code.clone()) {
            return Err(BlueprintError::InvalidRule(format!(
                "duplicate rule code '{}'",
                rule.code
            )));
        }
        let types: HashMap<String, Option<String>> = attributes
            .iter()
            .map(|attribute| match attribute {
                AttributeDeclaration::Local(local) => {
                    (local.code.clone(), Some(local.value_type.clone()))
                }
                AttributeDeclaration::Selection { code, .. } => (code.clone(), None),
            })
            .collect();
        attricat_rules::validate_against_attributes(rule, &types).map_err(|error| match error {
            attricat_rules::RuleError::UnknownAttribute(attribute) => {
                BlueprintError::RuleUnknownAttribute {
                    rule: rule.code.clone(),
                    attribute,
                }
            }
            error => BlueprintError::InvalidRule(format!("rule '{}': {error}", rule.code)),
        })?;
    }

    validate_unique_keys(&raw.unique_keys, &raw.kind, &codes)?;

    let definition = BlueprintDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        description: raw.description,
        kind: raw.kind,
        includes: raw.includes,
        views: raw.views,
        record_schema,
        publication: raw.publication,
        connector_jobs: raw.connector_jobs,
        rules,
        unique_keys: raw.unique_keys,
        attributes,
    };
    crate::lexicon_text::validate_lexicon_references(&definition)?;
    Ok(definition)
}

impl RawAttributeDeclaration {
    /// Whether the declaration sets target blueprints or a hierarchy, which
    /// only local relationship attributes may declare.
    fn declares_relationship_targets(&self) -> bool {
        self.target_blueprint.is_some()
            || !self.target_blueprints.is_empty()
            || self.acyclic.is_some()
            || self.tree.is_some()
    }
}

/// Normalizes `target_blueprint`/`target_blueprints` into the single-target
/// field (kept for single-target consumers) and the complete allowed set.
fn parse_relationship_targets(
    attribute: &RawAttributeDeclaration,
) -> Result<(Option<String>, Vec<String>), BlueprintError> {
    let invalid = |message: &str| BlueprintError::InvalidTargetBlueprints {
        code: attribute.code.clone(),
        message: message.to_owned(),
    };
    if attribute.target_blueprint.is_some() && !attribute.target_blueprints.is_empty() {
        return Err(invalid(
            "declare either target_blueprint or target_blueprints, not both",
        ));
    }
    if let Some(target) = &attribute.target_blueprint {
        validate_code(target, "attribute target_blueprint")?;
        return Ok((Some(target.clone()), vec![target.clone()]));
    }
    let mut seen = HashSet::new();
    for target in &attribute.target_blueprints {
        validate_code(target, "attribute target_blueprints")?;
        if !seen.insert(target.as_str()) {
            return Err(invalid(&format!("'{target}' is listed more than once")));
        }
    }
    Ok(match attribute.target_blueprints.as_slice() {
        [] => (None, Vec::new()),
        [single] => (Some(single.clone()), vec![single.clone()]),
        targets => (None, targets.to_vec()),
    })
}

fn parse_relationship_hierarchy(
    attribute: &RawAttributeDeclaration,
    blueprint_code: &str,
    kind: &BlueprintKind,
    targets: &[String],
) -> Result<Option<String>, BlueprintError> {
    let invalid = |message: &str| BlueprintError::InvalidRelationshipHierarchy {
        code: attribute.code.clone(),
        message: message.to_owned(),
    };
    let hierarchy = match (attribute.acyclic, attribute.tree) {
        (_, Some(true)) if attribute.acyclic == Some(false) => {
            return Err(invalid("a tree is always acyclic; remove acyclic = false"));
        }
        (_, Some(true)) => "tree",
        (Some(true), _) => "acyclic",
        _ => return Ok(None),
    };
    // Cycle checks follow direct default-context edges. Contextual overrides
    // would let inherited values combine into a cycle no single write closes.
    if attribute.context_editable != "default" {
        return Err(invalid(
            "hierarchies are shared by every context; set context_editable = \"default\"",
        ));
    }
    if *kind == BlueprintKind::Record
        && !targets.is_empty()
        && !targets.iter().any(|target| target == blueprint_code)
    {
        return Err(invalid(&format!(
            "a hierarchy must be able to target its own blueprint '{blueprint_code}'"
        )));
    }
    Ok(Some(hierarchy.to_owned()))
}

fn validate_unique_keys(
    keys: &[UniqueKeyDefinition],
    kind: &BlueprintKind,
    attribute_codes: &HashSet<String>,
) -> Result<(), BlueprintError> {
    let mut key_codes = HashSet::new();
    for key in keys {
        let invalid = |message: String| BlueprintError::InvalidUniqueKey {
            key: key.code.clone(),
            message,
        };
        validate_code(&key.code, "unique key code")?;
        if *kind != BlueprintKind::Record {
            return Err(invalid(
                "only record blueprints can define unique keys".into(),
            ));
        }
        if !key_codes.insert(key.code.as_str()) {
            return Err(invalid("the key code is duplicated".into()));
        }
        if key.attributes.is_empty() || key.attributes.len() > MAX_UNIQUE_KEY_ATTRIBUTES {
            return Err(invalid(format!(
                "a key combines 1 to {MAX_UNIQUE_KEY_ATTRIBUTES} attributes"
            )));
        }
        let mut seen = HashSet::new();
        for attribute in &key.attributes {
            if !attribute_codes.contains(attribute) {
                return Err(invalid(format!("unknown attribute '{attribute}'")));
            }
            if !seen.insert(attribute.as_str()) {
                return Err(invalid(format!("attribute '{attribute}' is listed twice")));
            }
        }
        if !UNIQUE_KEY_SCOPES.contains(&key.scope.as_str()) {
            return Err(invalid(format!("unsupported scope '{}'", key.scope)));
        }
    }
    Ok(())
}

fn parse_file_policy(
    attribute: &RawAttributeDeclaration,
    code: &str,
) -> Result<FilePolicy, BlueprintError> {
    let cardinality = attribute
        .cardinality
        .clone()
        .unwrap_or_else(|| "one".to_owned());
    if !DIRECTIONAL_CARDINALITIES.contains(&cardinality.as_str()) {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    let ordered = attribute.ordered.unwrap_or(cardinality == "many");
    if cardinality == "one" && ordered {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    let validate_unique_non_empty = |values: &[String]| {
        let mut seen = HashSet::new();
        values
            .iter()
            .all(|value| !value.trim().is_empty() && seen.insert(value))
    };
    if !validate_unique_non_empty(&attribute.allowed_mime_groups)
        || !validate_unique_non_empty(&attribute.allowed_extensions)
        || !validate_unique_non_empty(&attribute.purposes)
        || attribute.max_bytes == Some(0)
        || attribute.allowed_extensions.iter().any(|extension| {
            let normalized = extension.trim_start_matches('.');
            normalized.is_empty()
                || !normalized
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        || attribute
            .purposes
            .iter()
            .any(|purpose| !is_valid_code(purpose))
    {
        return Err(BlueprintError::InvalidFilePolicy(code.to_owned()));
    }
    Ok(FilePolicy {
        cardinality,
        ordered,
        allowed_mime_groups: attribute.allowed_mime_groups.clone(),
        allowed_extensions: attribute.allowed_extensions.clone(),
        max_bytes: attribute.max_bytes,
        purposes: attribute.purposes.clone(),
        image_only: attribute.image_only.unwrap_or(false),
    })
}

fn parse_json_value(
    source: Option<String>,
    field: &str,
) -> Result<Option<serde_json::Value>, BlueprintError> {
    source
        .map(|source| {
            serde_json::from_str(&source).map_err(|error| BlueprintError::InvalidJsonSchema {
                field: field.to_owned(),
                message: error.to_string(),
            })
        })
        .transpose()
}

fn parse_json_schema(
    source: Option<String>,
    field: &str,
) -> Result<Option<serde_json::Value>, BlueprintError> {
    let Some(source) = source else {
        return Ok(None);
    };
    let schema =
        serde_json::from_str(&source).map_err(|error| BlueprintError::InvalidJsonSchema {
            field: field.to_owned(),
            message: error.to_string(),
        })?;
    validate_json_schema_definition(&schema).map_err(|message| {
        BlueprintError::InvalidJsonSchema {
            field: field.to_owned(),
            message,
        }
    })?;
    Ok(Some(schema))
}

pub(crate) fn validate_code(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if !is_valid_code(value) {
        return Err(BlueprintError::InvalidCode { field });
    }
    Ok(())
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if value.trim().is_empty() {
        return Err(BlueprintError::EmptyField(field));
    }
    Ok(())
}

fn validate_description(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    validate_non_empty(value, field)?;
    if value.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(BlueprintError::FieldTooLong {
            field,
            max: MAX_DESCRIPTION_CHARS,
        });
    }
    Ok(())
}

fn parse_selection(code: &str, source: &str) -> Result<(String, String), BlueprintError> {
    let Some((include_alias, attribute_code)) = source.split_once('.') else {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    };
    if include_alias.is_empty() || attribute_code.is_empty() || attribute_code.contains('.') {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    }
    if code != attribute_code {
        return Err(BlueprintError::InvalidSelection {
            code: code.to_owned(),
            selection: source.to_owned(),
        });
    }

    Ok((include_alias.to_owned(), attribute_code.to_owned()))
}
