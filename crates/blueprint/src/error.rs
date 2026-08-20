use thiserror::Error;

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
    #[error("entity blueprints must define views.dropdown_option")]
    MissingDropdownOptionView,
    #[error("views.dropdown_option must define at least one field")]
    EmptyDropdownOptionFields,
    #[error("views.dropdown_option contains duplicate field '{0}'")]
    DuplicateDropdownOptionField(String),
    #[error("attribute '{0}' has an invalid tag")]
    InvalidAttributeTag(String),
    #[error("attribute '{code}' has unsupported value type '{value_type}'")]
    UnsupportedValueType { code: String, value_type: String },
    #[error("attribute '{code}' cannot define a value schema for relationship values")]
    RelationshipValueSchema { code: String },
    #[error("{field} is not valid JSON Schema: {message}")]
    InvalidJsonSchema { field: String, message: String },
    #[error("only entity blueprints can define an entity schema")]
    EntitySchemaOnMixin,
    #[error("entity schema {keyword} references unknown attribute '{attribute}'")]
    EntitySchemaUnknownAttribute {
        keyword: &'static str,
        attribute: String,
    },
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
    #[error(
        "component references are only allowed on table, field, relationship_list, incoming_relationship_list, and stack blocks"
    )]
    ComponentOnNonDataBlock,
    #[error("stack component in view '{view}' can only be used in views.detail")]
    StackComponentOutsideDetail { view: String },
    #[error("stack component in view '{view}' must have a scalar field as its first child")]
    StackComponentInvalidFirstChild { view: String },
    #[error(
        "stack component in view '{view}' only allows text and scalar field children after the first child"
    )]
    StackComponentInvalidChild { view: String },
    #[error("view '{view}' references unknown attribute '{field}'")]
    UnknownViewField { view: String, field: String },
    #[error("view '{view}' field '{field}' must be scalar")]
    NonScalarViewField { view: String, field: String },
    #[error("view '{view}' relationship_list '{field}' must be a relationship")]
    NonRelationshipViewField { view: String, field: String },
    #[error("view '{view}' hierarchy field '{field}' must target its own blueprint")]
    HierarchyFieldMustTargetOwnBlueprint { view: String, field: String },
    #[error("view '{view}' incoming_relationship_list label must not be empty")]
    EmptyIncomingRelationshipLabel { view: String },
    #[error("view '{view}' incoming_relationship_list must specify at least one relationship")]
    EmptyIncomingRelationships { view: String },
    #[error("view '{view}' incoming_relationship_list page_size must be greater than zero")]
    InvalidIncomingRelationshipPageSize { view: String },
    #[error("incoming_relationship_list can only be used in views.detail")]
    IncomingRelationshipListOutsideDetail,
}
