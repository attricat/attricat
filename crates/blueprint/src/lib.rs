use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use catalog_validation::is_valid_code;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct BlueprintDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub includes: Vec<IncludeRef>,
    pub display: HashMap<String, DisplayDefinition>,
    pub views: HashMap<String, ViewDefinition>,
    pub attributes: Vec<AttributeDeclaration>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentReference {
    pub id: String,
    pub version: i64,
    #[serde(default)]
    pub props: serde_json::Value,
}

#[derive(Deserialize)]
struct ComponentManifest {
    components: Vec<ComponentManifestEntry>,
}

#[derive(Deserialize)]
struct ComponentManifestEntry {
    id: String,
    version: i64,
    capabilities: Vec<String>,
    placements: Vec<String>,
    value_types: Vec<String>,
    allowed_props: Vec<String>,
}

static COMPONENT_MANIFEST: LazyLock<ComponentManifest> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../../apps/catalog-web/src/features/views/component-contract.json"
    ))
    .expect("component manifest must be valid JSON")
});

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewTab {
    pub label: String,
    pub children: Vec<ViewNode>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSection {
    pub label: String,
    pub children: Vec<ViewNode>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewDefinition {
    Table {
        fields: Vec<String>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Stack {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Grid {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Section {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Tabs {
        tabs: Vec<ViewTab>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Accordion {
        sections: Vec<ViewSection>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewNode {
    Stack {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Grid {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Section {
        children: Vec<ViewNode>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Tabs {
        tabs: Vec<ViewTab>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Accordion {
        sections: Vec<ViewSection>,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Heading {
        text: String,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Text {
        text: String,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Divider {
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    Field {
        field: String,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    RelationshipList {
        field: String,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayDefinition {
    pub fields: Vec<String>,
    #[serde(default = "default_display_separator")]
    pub separator: String,
}

fn default_display_separator() -> String {
    " · ".to_owned()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlueprintKind {
    Entity,
    Mixin,
}

impl BlueprintKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Mixin => "mixin",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncludeRef {
    pub alias: String,
    pub code: String,
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttributeDeclaration {
    Local {
        code: String,
        value_type: String,
        target_blueprint: Option<String>,
        tags: Vec<String>,
        context_fallback: String,
        context_editable: String,
    },
    Selection {
        code: String,
        include_alias: String,
        attribute_code: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedInclude {
    pub alias: String,
    pub code: String,
    pub version: i64,
    pub attributes: Vec<EffectiveAttribute>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EffectiveAttribute {
    pub code: String,
    pub value_type: String,
    pub target_blueprint: Option<String>,
    pub tags: Vec<String>,
    pub context_fallback: String,
    pub context_editable: String,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBlueprint {
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub raw_definition_hash: String,
    pub includes: Vec<IncludeRef>,
    pub display: HashMap<String, DisplayDefinition>,
    pub views: HashMap<String, ViewDefinition>,
    pub attributes: Vec<EffectiveAttribute>,
}

#[derive(Debug, Error)]
pub enum BlueprintError {
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported format version {0}")]
    UnsupportedFormatVersion(u32),
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    #[error("blueprints must define at least one attribute")]
    EmptyAttributes,
    #[error("include alias '{0}' is duplicated")]
    DuplicateIncludeAlias(String),
    #[error("attribute code '{0}' is duplicated")]
    DuplicateAttributeCode(String),
    #[error("{field} must contain only ASCII letters, numbers, hyphens, and underscores")]
    InvalidCode { field: &'static str },
    #[error("attribute '{0}' must define exactly one of value_type or from")]
    InvalidAttributeDeclaration(String),
    #[error("attribute '{code}' selects invalid source '{selection}'")]
    InvalidSelection { code: String, selection: String },
    #[error("attribute '{0}' references an unknown include alias")]
    UnknownIncludeAlias(String),
    #[error("attribute '{attribute}' is not exposed by include '{alias}'")]
    UnknownIncludedAttribute { alias: String, attribute: String },
    #[error("resolved include '{0}' is missing or does not match its declaration")]
    InvalidResolvedInclude(String),
    #[error("entity blueprints must define display.dropdown_option")]
    MissingDropdownOptionDisplay,
    #[error("display '{0}' must define at least one field")]
    EmptyDisplayFields(String),
    #[error("display '{display}' contains duplicate field '{field}'")]
    DuplicateDisplayField { display: String, field: String },
    #[error("display '{display}' references unknown or non-scalar attribute '{field}'")]
    InvalidDisplayField { display: String, field: String },
    #[error("attribute '{0}' has an invalid tag")]
    InvalidAttributeTag(String),
    #[error("attribute '{code}' has unsupported value type '{value_type}'")]
    UnsupportedValueType { code: String, value_type: String },
    #[error("attribute '{code}' has invalid context_fallback '{context_fallback}'")]
    InvalidContextFallback {
        code: String,
        context_fallback: String,
    },
    #[error("attribute '{code}' has invalid context_editable '{context_editable}'")]
    InvalidContextEditable {
        code: String,
        context_editable: String,
    },
    #[error(
        "component id '{0}' must contain lowercase underscore-separated segments joined by dots"
    )]
    InvalidComponentId(String),
    #[error("component '{id}' has invalid version '{version}'")]
    InvalidComponentVersion { id: String, version: i64 },
    #[error("component '{id}' version '{version}' is not in the manifest")]
    UnknownComponent { id: String, version: i64 },
    #[error("component '{id}' cannot be used in {placement}")]
    InvalidComponentPlacement { id: String, placement: &'static str },
    #[error("component '{id}' does not support attribute value type '{value_type}'")]
    InvalidComponentValueType { id: String, value_type: String },
    #[error("component '{id}' does not support the '{capability}' capability")]
    MissingComponentCapability {
        id: String,
        capability: &'static str,
    },
    #[error("component '{id}' has unsupported prop '{prop}'")]
    InvalidComponentProp { id: String, prop: String },
    #[error("component references are only allowed on table, field, relationship_list, and stack blocks")]
    ComponentOnNonDataBlock,
    #[error("stack component in view '{view}' can only be used in views.detail")]
    StackComponentOutsideDetail { view: String },
    #[error("stack component in view '{view}' must have a scalar field as its first child")]
    StackComponentInvalidFirstChild { view: String },
    #[error("stack component in view '{view}' only allows text and scalar field children after the first child")]
    StackComponentInvalidChild { view: String },
    #[error("view '{view}' references unknown attribute '{field}'")]
    UnknownViewField { view: String, field: String },
    #[error("view '{view}' field '{field}' must be scalar")]
    NonScalarViewField { view: String, field: String },
    #[error("view '{view}' relationship_list '{field}' must be a relationship")]
    NonRelationshipViewField { view: String, field: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlueprintDefinition {
    format_version: u32,
    code: String,
    name: String,
    kind: BlueprintKind,
    #[serde(default)]
    includes: Vec<IncludeRef>,
    #[serde(default)]
    display: HashMap<String, DisplayDefinition>,
    #[serde(default)]
    views: HashMap<String, ViewDefinition>,
    attributes: Vec<RawAttributeDeclaration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttributeDeclaration {
    code: String,
    value_type: Option<String>,
    target_blueprint: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_context_fallback")]
    context_fallback: String,
    #[serde(default = "default_context_editable")]
    context_editable: String,
    from: Option<String>,
}

fn default_context_fallback() -> String {
    "default".to_owned()
}

fn default_context_editable() -> String {
    "all".to_owned()
}

pub fn parse(source: &str) -> Result<BlueprintDefinition, BlueprintError> {
    let raw: RawBlueprintDefinition = toml::from_str(source)?;
    if raw.format_version != 1 {
        return Err(BlueprintError::UnsupportedFormatVersion(raw.format_version));
    }
    validate_code(&raw.code, "blueprint code")?;
    validate_non_empty(&raw.name, "blueprint name")?;
    if raw.attributes.is_empty() {
        return Err(BlueprintError::EmptyAttributes);
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

    let mut codes = HashSet::new();
    let mut attributes = Vec::with_capacity(raw.attributes.len());
    for attribute in raw.attributes {
        validate_code(&attribute.code, "attribute code")?;
        if !codes.insert(attribute.code.clone()) {
            return Err(BlueprintError::DuplicateAttributeCode(attribute.code));
        }

        attributes.push(match (attribute.value_type, attribute.from) {
            (Some(value_type), None) => {
                validate_non_empty(&value_type, "attribute value_type")?;
                if !matches!(
                    value_type.as_str(),
                    "string"
                        | "number"
                        | "integer"
                        | "boolean"
                        | "date"
                        | "datetime"
                        | "time"
                        | "relationship"
                ) {
                    return Err(BlueprintError::UnsupportedValueType {
                        code: attribute.code,
                        value_type,
                    });
                }
                if attribute.target_blueprint.is_some() && value_type != "relationship" {
                    return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code));
                }
                if let Some(target_blueprint) = &attribute.target_blueprint {
                    validate_code(target_blueprint, "attribute target_blueprint")?;
                }
                if !matches!(attribute.context_fallback.as_str(), "default" | "none") {
                    return Err(BlueprintError::InvalidContextFallback {
                        code: attribute.code,
                        context_fallback: attribute.context_fallback,
                    });
                }
                if !matches!(attribute.context_editable.as_str(), "all" | "default") {
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
                AttributeDeclaration::Local {
                    code: attribute.code,
                    value_type,
                    target_blueprint: attribute.target_blueprint,
                    tags: attribute.tags,
                    context_fallback: attribute.context_fallback,
                    context_editable: attribute.context_editable,
                }
            }
            (None, Some(source)) if attribute.target_blueprint.is_none() => {
                let (include_alias, attribute_code) = parse_selection(&attribute.code, &source)?;
                AttributeDeclaration::Selection {
                    code: attribute.code,
                    include_alias,
                    attribute_code,
                }
            }
            _ => return Err(BlueprintError::InvalidAttributeDeclaration(attribute.code)),
        });
    }

    Ok(BlueprintDefinition {
        format_version: raw.format_version,
        code: raw.code,
        name: raw.name,
        kind: raw.kind,
        includes: raw.includes,
        display: raw.display,
        views: raw.views,
        attributes,
    })
}

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
        let (code, value_type, target_blueprint, tags, context_fallback, context_editable) =
            match declaration {
                AttributeDeclaration::Local {
                    code,
                    value_type,
                    target_blueprint,
                    tags,
                    context_fallback,
                    context_editable,
                } => (
                    code.clone(),
                    value_type.clone(),
                    target_blueprint.clone(),
                    tags.clone(),
                    context_fallback.clone(),
                    context_editable.clone(),
                ),
                AttributeDeclaration::Selection {
                    code,
                    include_alias,
                    attribute_code,
                } => {
                    let include =
                        resolved_by_alias
                            .get(include_alias.as_str())
                            .ok_or_else(|| {
                                BlueprintError::UnknownIncludeAlias(include_alias.clone())
                            })?;
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
                        attribute.target_blueprint.clone(),
                        attribute.tags.clone(),
                        attribute.context_fallback.clone(),
                        attribute.context_editable.clone(),
                    )
                }
            };
        attributes.push(EffectiveAttribute {
            code,
            value_type,
            target_blueprint,
            tags,
            context_fallback,
            context_editable,
            position: position as i64,
        });
    }
    if definition.kind == BlueprintKind::Entity
        && !definition.display.contains_key("dropdown_option")
    {
        return Err(BlueprintError::MissingDropdownOptionDisplay);
    }
    for (name, display) in &definition.display {
        if display.fields.is_empty() {
            return Err(BlueprintError::EmptyDisplayFields(name.clone()));
        }
        let mut fields = HashSet::new();
        for field in &display.fields {
            if field.trim().is_empty() || !fields.insert(field) {
                return Err(BlueprintError::DuplicateDisplayField {
                    display: name.clone(),
                    field: field.clone(),
                });
            }
            if !attributes
                .iter()
                .any(|attribute| attribute.code == *field && attribute.value_type != "relationship")
            {
                return Err(BlueprintError::InvalidDisplayField {
                    display: name.clone(),
                    field: field.clone(),
                });
            }
        }
    }
    for (name, view) in &definition.views {
        validate_view(name, view, &attributes)?;
    }

    Ok(CompiledBlueprint {
        code: definition.code,
        name: definition.name,
        kind: definition.kind,
        raw_definition_hash: raw_hash(source),
        includes: definition.includes,
        display: definition.display,
        views: definition.views,
        attributes,
    })
}

fn validate_view(
    view: &str,
    definition: &ViewDefinition,
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    match definition {
        ViewDefinition::Table { fields, component } => {
            for field in fields {
                let attribute = validate_view_field(view, field, attributes, false)?;
                validate_component(component.as_ref(), view, "table", Some(&attribute.value_type))?;
            }
        }
        ViewDefinition::Stack {
            children,
            component,
        } => {
            validate_stack_component(component.as_ref(), view, children, attributes)?;
            validate_view_nodes(view, children, attributes)?;
        }
        ViewDefinition::Grid {
            children,
            component,
        }
        | ViewDefinition::Section {
            children,
            component,
        } => {
            validate_non_data_component(component.as_ref())?;
            validate_view_nodes(view, children, attributes)?;
        }
        ViewDefinition::Tabs { tabs, component } => {
            validate_non_data_component(component.as_ref())?;
            for tab in tabs {
                validate_view_nodes(view, &tab.children, attributes)?;
            }
        }
        ViewDefinition::Accordion {
            sections,
            component,
        } => {
            validate_non_data_component(component.as_ref())?;
            for section in sections {
                validate_view_nodes(view, &section.children, attributes)?;
            }
        }
    }
    Ok(())
}

fn validate_view_nodes(
    view: &str,
    nodes: &[ViewNode],
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    for node in nodes {
        match node {
            ViewNode::Stack {
                children,
                component,
            } => {
                validate_stack_component(component.as_ref(), view, children, attributes)?;
                validate_view_nodes(view, children, attributes)?;
            }
            ViewNode::Grid {
                children,
                component,
            }
            | ViewNode::Section {
                children,
                component,
            } => {
                validate_non_data_component(component.as_ref())?;
                validate_view_nodes(view, children, attributes)?;
            }
            ViewNode::Tabs { tabs, component } => {
                validate_non_data_component(component.as_ref())?;
                for tab in tabs {
                    validate_view_nodes(view, &tab.children, attributes)?;
                }
            }
            ViewNode::Accordion {
                sections,
                component,
            } => {
                validate_non_data_component(component.as_ref())?;
                for section in sections {
                    validate_view_nodes(view, &section.children, attributes)?;
                }
            }
            ViewNode::Heading { component, .. }
            | ViewNode::Text { component, .. }
            | ViewNode::Divider { component } => validate_non_data_component(component.as_ref())?,
            ViewNode::Field { .. } | ViewNode::RelationshipList { .. } => {}
        }
        match node {
            ViewNode::Field { field, component } => {
                let attribute = validate_view_field(view, field, attributes, false)?;
                validate_component(component.as_ref(), view, "field", Some(&attribute.value_type))?;
            }
            ViewNode::RelationshipList { field, component } => {
                let attribute = validate_view_field(view, field, attributes, true)?;
                validate_component(
                    component.as_ref(),
                    view,
                    "relationship_list",
                    Some(&attribute.value_type),
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_view_field<'a>(
    view: &str,
    field: &str,
    attributes: &'a [EffectiveAttribute],
    relationship: bool,
) -> Result<&'a EffectiveAttribute, BlueprintError> {
    let attribute = attributes
        .iter()
        .find(|attribute| attribute.code == field)
        .ok_or_else(|| BlueprintError::UnknownViewField {
            view: view.to_owned(),
            field: field.to_owned(),
        })?;
    if relationship && attribute.value_type != "relationship" {
        return Err(BlueprintError::NonRelationshipViewField {
            view: view.to_owned(),
            field: field.to_owned(),
        });
    }
    if !relationship && attribute.value_type == "relationship" {
        return Err(BlueprintError::NonScalarViewField {
            view: view.to_owned(),
            field: field.to_owned(),
        });
    }
    Ok(attribute)
}

fn validate_non_data_component(
    component: Option<&ComponentReference>,
) -> Result<(), BlueprintError> {
    if component.is_some() {
        return Err(BlueprintError::ComponentOnNonDataBlock);
    }
    Ok(())
}

fn validate_stack_component(
    component: Option<&ComponentReference>,
    view: &str,
    children: &[ViewNode],
    attributes: &[EffectiveAttribute],
) -> Result<(), BlueprintError> {
    validate_component(component, view, "stack", None)?;
    if component.is_none() {
        return Ok(());
    }
    if view != "detail" {
        return Err(BlueprintError::StackComponentOutsideDetail {
            view: view.to_owned(),
        });
    }
    let Some(ViewNode::Field { field, .. }) = children.first() else {
        return Err(BlueprintError::StackComponentInvalidFirstChild {
            view: view.to_owned(),
        });
    };
    validate_view_field(view, field, attributes, false)?;
    if children[1..]
        .iter()
        .any(|child| !matches!(child, ViewNode::Text { .. } | ViewNode::Field { .. }))
    {
        return Err(BlueprintError::StackComponentInvalidChild {
            view: view.to_owned(),
        });
    }
    Ok(())
}

fn validate_component(
    component: Option<&ComponentReference>,
    view: &str,
    placement: &'static str,
    value_type: Option<&str>,
) -> Result<(), BlueprintError> {
    let Some(component) = component else {
        return Ok(());
    };
    if !is_valid_component_id(&component.id) {
        return Err(BlueprintError::InvalidComponentId(component.id.clone()));
    }
    if component.version <= 0 {
        return Err(BlueprintError::InvalidComponentVersion {
            id: component.id.clone(),
            version: component.version,
        });
    }
    let manifest_component = COMPONENT_MANIFEST
        .components
        .iter()
        .find(|entry| entry.id == component.id && entry.version == component.version)
        .ok_or_else(|| BlueprintError::UnknownComponent {
            id: component.id.clone(),
            version: component.version,
        })?;
    if !manifest_component
        .placements
        .iter()
        .any(|item| item == placement)
    {
        return Err(BlueprintError::InvalidComponentPlacement {
            id: component.id.clone(),
            placement,
        });
    }
    if let Some(value_type) = value_type {
        if !manifest_component
            .value_types
            .iter()
            .any(|item| item == value_type)
        {
            return Err(BlueprintError::InvalidComponentValueType {
                id: component.id.clone(),
                value_type: value_type.to_owned(),
            });
        }
    }
    if let Some(capability) = required_view_capability(view)
        && !manifest_component
            .capabilities
            .iter()
            .any(|item| item == capability)
    {
        return Err(BlueprintError::MissingComponentCapability {
            id: component.id.clone(),
            capability,
        });
    }
    if let Some(props) = component.props.as_object() {
        for prop in props.keys() {
            if !manifest_component
                .allowed_props
                .iter()
                .any(|item| item == prop)
            {
                return Err(BlueprintError::InvalidComponentProp {
                    id: component.id.clone(),
                    prop: prop.clone(),
                });
            }
        }
    }
    Ok(())
}

fn required_view_capability(view: &str) -> Option<&'static str> {
    match view {
        "detail" | "table" => Some("display"),
        "edit" => Some("edit"),
        _ => None,
    }
}

fn is_valid_component_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.split('_').all(|word| {
                    !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase())
                })
        })
}

pub fn raw_hash(source: &str) -> String {
    let hash = Sha256::digest(source.as_bytes());
    format!("{hash:x}")
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if value.trim().is_empty() {
        return Err(BlueprintError::EmptyField(field));
    }
    Ok(())
}

fn validate_code(value: &str, field: &'static str) -> Result<(), BlueprintError> {
    if !is_valid_code(value) {
        return Err(BlueprintError::InvalidCode { field });
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
