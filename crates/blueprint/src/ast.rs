use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct BlueprintDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub includes: Vec<IncludeRef>,
    pub views: HashMap<String, ViewDefinition>,
    pub entity_schema: Option<serde_json::Value>,
    pub publication: PublicationPolicy,
    pub connector_jobs: Vec<ConnectorJobDefinition>,
    pub rules: Vec<catalog_rules::CompiledRule>,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub attributes: Vec<AttributeDeclaration>,
}

/// Unique-key scopes: `workspace` compares default-context values across the
/// blueprint family; `context` compares resolved values within each context.
pub const UNIQUE_KEY_SCOPES: &[&str] = &["workspace", "context"];
/// Hierarchy constraints a self-referencing relationship can declare.
pub const RELATIONSHIP_HIERARCHIES: &[&str] = &["acyclic", "tree"];
/// Attributes a unique key may combine.
pub const MAX_UNIQUE_KEY_ATTRIBUTES: usize = 8;

/// A business key: no two entities of the blueprint family may share the
/// normalized values of these attributes.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniqueKeyDefinition {
    /// Stable key identifier, unique within the blueprint.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    pub code: String,
    /// One to eight scalar attributes, or single-target relationships, whose
    /// combined values must be unique.
    #[schemars(
        length(min = 1, max = MAX_UNIQUE_KEY_ATTRIBUTES),
        extend("x-attricat-reference" = "attribute")
    )]
    pub attributes: Vec<String>,
    /// `workspace` (default) compares default-context values; `context`
    /// compares resolved values separately in every context.
    #[serde(default = "default_unique_key_scope")]
    #[schemars(extend("enum" = UNIQUE_KEY_SCOPES))]
    pub scope: String,
    /// Compare strings exactly. By default strings are compared after
    /// trimming, collapsing whitespace, and lowercasing.
    #[serde(default)]
    pub case_sensitive: bool,
}

fn default_unique_key_scope() -> String {
    "workspace".to_owned()
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorJobDefinition {
    /// Stable job identifier, unique within the blueprint.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    pub code: String,
    /// `import` requires `context`; `export` cannot set `input_file_id`.
    #[schemars(extend("enum" = CONNECTOR_JOB_DIRECTIONS))]
    pub direction: String,
    /// Installed extension that provides the connector operation.
    pub extension_id: String,
    /// Connector operation declared by the extension.
    pub operation_id: String,
    /// Operation input table passed to the extension.
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub input: toml::Value,
    /// Context code that receives imported values. Required for imports.
    #[schemars(extend("x-attricat-reference" = "context"))]
    pub context: Option<String>,
    /// Uploaded file supplied to an import.
    pub input_file_id: Option<String>,
    /// Run interval in seconds, from 60 to 2592000 (30 days).
    #[schemars(range(min = 60, max = 2_592_000))]
    pub interval_seconds: Option<i32>,
    /// Whether the job is scheduled. Defaults to `true`.
    #[serde(default = "default_connector_enabled")]
    pub enabled: bool,
}

/// Entity-owned outlets whose placement a blueprint `extension_layout` may override.
pub const EXTENSION_LAYOUT_OUTLETS: &[&str] = &[
    "entity_preview_panel",
    "entity_attribute_decoration",
    "entity_action",
];
/// View names with platform meaning, offered to editors as suggestions.
pub const KNOWN_VIEW_NAMES: &[&str] = &[
    "dropdown_option",
    "detail",
    "edit",
    "table",
    "extension_layout",
];

pub const CONNECTOR_JOB_DIRECTIONS: &[&str] = &["import", "export"];

fn default_connector_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationPolicy {
    /// Workspace roles whose edits keep entities published on their channels.
    #[serde(default)]
    #[schemars(extend("x-attricat-reference" = "role"))]
    pub retain_on_edit_roles: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentReference {
    /// Registered component ID, such as `catalog.field_edit`.
    #[schemars(extend("x-attricat-reference" = "view_component"))]
    pub id: String,
    /// Component manifest versions are non-negative 32-bit protocol values.
    pub version: u32,
    /// Component-specific properties.
    #[serde(default)]
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub props: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewTab {
    /// Tab label. `{{…}}` references resolve from the workspace lexicon.
    pub label: String,
    /// Blocks rendered in the tab.
    pub children: Vec<ViewNode>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSection {
    /// Section heading. `{{…}}` references resolve from the workspace lexicon.
    pub label: String,
    /// Blocks rendered in the section.
    pub children: Vec<ViewNode>,
}

/// Host-owned per-blueprint extension layout. It is declarative data only;
/// contribution keys cannot select DOM nodes or invoke extension code.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionOutletLayout {
    /// Contribution keys (`<extension-id>:<contribution-id>`) in display order.
    #[serde(default)]
    pub order: Vec<String>,
    /// Contribution keys hidden in this outlet.
    #[serde(default)]
    pub hidden: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingRelationship {
    /// Blueprint whose entities reference this entity.
    #[schemars(extend("x-attricat-reference" = "blueprint"))]
    pub source_blueprint: String,
    /// Relationship attribute on the source blueprint.
    pub field: String,
}

/// A named view. `dropdown_option` labels relationship choices, `table`
/// configures Explorer columns, and layout roots arrange `detail` and `edit`.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewDefinition {
    /// Label shown for the entity in relationship dropdowns.
    DropdownOption {
        /// Attributes joined into the label.
        #[schemars(extend("x-attricat-reference" = "attribute"))]
        fields: Vec<String>,
        /// Text between label fields. Defaults to ` · `.
        #[serde(default = "default_display_separator")]
        separator: String,
    },
    /// Explorer table columns.
    Table {
        /// Legacy scalar-field shorthand. New definitions use `columns`.
        #[serde(default)]
        #[schemars(extend("x-attricat-reference" = "attribute"))]
        fields: Option<Vec<String>>,
        /// Table columns. Cannot be combined with `fields`.
        #[serde(default)]
        columns: Option<Vec<TableColumn>>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks stacked vertically.
    Stack {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks arranged in a responsive grid.
    Grid {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks grouped in a section.
    Section {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks split into tabs.
    Tabs {
        /// Tabs, each with a label and its own blocks.
        tabs: Vec<ViewTab>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks split into collapsible sections.
    Accordion {
        /// Collapsible sections, each with a label and its own blocks.
        sections: Vec<ViewSection>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// A versioned layout override for entity-owned extension surfaces.
    ExtensionLayout {
        /// Layout format version. Only `1` is supported.
        #[schemars(extend("const" = 1))]
        version: u32,
        /// Layouts keyed by entity outlet: `entity_preview_panel`,
        /// `entity_attribute_decoration`, or `entity_action`.
        #[schemars(extend("x-attricat-key-suggestions" = EXTENSION_LAYOUT_OUTLETS))]
        outlets: HashMap<String, ExtensionOutletLayout>,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableColumn {
    /// A local scalar field or up to three relationship hops ending in a scalar field.
    #[schemars(extend("x-attricat-reference" = "attribute_path"))]
    pub field: String,
    /// Column heading. Defaults to the attribute name. `{{…}}` references
    /// resolve from the workspace lexicon.
    #[serde(default)]
    pub label: Option<String>,
    /// Cell renderer, such as `catalog.table_image`.
    #[serde(default)]
    pub renderer: Option<ComponentReference>,
}

/// A view block. Containers hold `children`, `tabs`, or `sections`; leaves
/// render attributes or static content.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewNode {
    /// Blocks stacked vertically.
    Stack {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks arranged in a responsive grid.
    Grid {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks grouped in a section.
    Section {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks split into tabs.
    Tabs {
        /// Tabs, each with a label and its own blocks.
        tabs: Vec<ViewTab>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Blocks split into collapsible sections.
    Accordion {
        /// Collapsible sections, each with a label and its own blocks.
        sections: Vec<ViewSection>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// A static heading.
    Heading {
        /// Heading text.
        text: String,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Static text.
    Text {
        /// Text content.
        text: String,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// A horizontal divider.
    Divider {
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// A scalar attribute's value or input.
    Field {
        /// Scalar attribute to render.
        #[schemars(extend("x-attricat-reference" = "attribute"))]
        field: String,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// A relationship attribute's linked entities.
    RelationshipList {
        /// Relationship attribute to render.
        #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
        field: String,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Entities that reference this entity, opened in a paginated dialog.
    IncomingRelationshipList {
        /// Button and dialog label. `{{…}}` references resolve from the
        /// workspace lexicon.
        label: String,
        /// Source blueprint relationship fields that point at this entity.
        relationships: Vec<IncomingRelationship>,
        /// Entities requested per page.
        #[schemars(range(min = 1))]
        page_size: u32,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
}

fn default_display_separator() -> String {
    " · ".to_owned()
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlueprintKind {
    /// Creates entities.
    Entity,
    /// Provides attributes for other blueprints to include.
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncludeRef {
    /// Local name used by `from = "<alias>.<attribute>"` selections.
    #[schemars(regex(pattern = catalog_validation::CODE_PATTERN))]
    pub alias: String,
    /// Code of the included mixin blueprint.
    #[schemars(
        regex(pattern = catalog_validation::CODE_PATTERN),
        extend("x-attricat-reference" = "mixin")
    )]
    pub code: String,
    /// Exact mixin version to include.
    #[schemars(range(min = 1), extend("x-attricat-reference" = "mixin_version"))]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FilePolicy {
    pub cardinality: String,
    pub ordered: bool,
    pub allowed_mime_groups: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub max_bytes: Option<u64>,
    pub purposes: Vec<String>,
    pub image_only: bool,
}

impl FilePolicy {
    /// Whether a file of a detected MIME type is accepted by this policy, as
    /// for an ordinary upload.
    pub fn allows(&self, mime: &str, filename: &str, size: u64) -> bool {
        catalog_validation::files::FileConstraints {
            allowed_mime_groups: &self.allowed_mime_groups,
            allowed_extensions: &self.allowed_extensions,
            max_bytes: self.max_bytes,
            image_only: self.image_only,
        }
        .allows(mime, filename, size)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalAttributeDeclaration {
    pub code: String,
    pub name: Option<String>,
    pub value_type: String,
    pub value_schema: Option<serde_json::Value>,
    /// An unresolved `provider:type@range` reference. The API resolver pins it
    /// to an installed extension release before persistence.
    pub extension_type: Option<String>,
    pub extension_configuration: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    /// Every allowed target blueprint. A single target is also kept in
    /// `target_blueprint`; empty means any entity blueprint.
    pub target_blueprints: Vec<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    /// `acyclic` or `tree` for self-referencing relationships.
    pub hierarchy: Option<String>,
    pub tags: Vec<String>,
    pub context_fallback: String,
    pub context_editable: String,
    pub readonly: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttributeDeclaration {
    Local(Box<LocalAttributeDeclaration>),
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
    /// Human-readable label; clients fall back to the humanized code.
    pub name: Option<String>,
    pub value_type: String,
    pub value_schema: Option<serde_json::Value>,
    /// Host-pinned extension type metadata. It is declarative so persisted
    /// values remain readable when the provider is unavailable.
    pub extension_type: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    /// Every allowed target blueprint; empty means any entity blueprint.
    pub target_blueprints: Vec<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
    /// `acyclic` or `tree` for self-referencing relationships.
    pub hierarchy: Option<String>,
    pub tags: Vec<String>,
    pub context_fallback: String,
    pub context_editable: String,
    pub readonly: bool,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBlueprint {
    pub code: String,
    pub name: String,
    pub kind: BlueprintKind,
    pub raw_definition_hash: String,
    pub includes: Vec<IncludeRef>,
    pub views: HashMap<String, ViewDefinition>,
    pub entity_schema: Option<serde_json::Value>,
    pub rules: Vec<catalog_rules::CompiledRule>,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub attributes: Vec<EffectiveAttribute>,
}
