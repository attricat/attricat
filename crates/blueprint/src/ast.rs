use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct BlueprintDefinition {
    pub format_version: u32,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: BlueprintKind,
    pub includes: Vec<IncludeRef>,
    pub views: HashMap<String, ViewDefinition>,
    pub record_schema: Option<serde_json::Value>,
    pub publication: PublicationPolicy,
    pub connector_jobs: Vec<ConnectorJobDefinition>,
    pub rules: Vec<attricat_rules::CompiledRule>,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub attributes: Vec<AttributeDeclaration>,
}

/// Unique-key scopes: `workspace` compares default-context values across the
/// blueprint family; `context` compares resolved values within each context.
pub const UNIQUE_KEY_SCOPES: &[&str] = &["workspace", "context"];
/// Attributes a unique key may combine.
pub const MAX_UNIQUE_KEY_ATTRIBUTES: usize = 8;

/// A business key: no two records of the blueprint family may share the
/// normalized values of these attributes.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniqueKeyDefinition {
    /// Stable key identifier, unique within the blueprint.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
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
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
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
    /// Context code that receives imported values, required for imports; for
    /// an export, the publication channel it exports.
    #[schemars(extend("x-attricat-reference" = "context"))]
    pub context: Option<String>,
    /// Uploaded file supplied to an import.
    pub input_file_id: Option<String>,
    /// Run interval in seconds, from 60 to 2592000 (30 days).
    #[schemars(range(min = MIN_CONNECTOR_INTERVAL_SECONDS, max = MAX_CONNECTOR_INTERVAL_SECONDS))]
    pub interval_seconds: Option<i32>,
    /// Whether the job is scheduled. Defaults to `true`.
    #[serde(default = "default_connector_enabled")]
    pub enabled: bool,
    /// The extension connection (destination account) an export pushes to.
    /// Without it the extension's default destination is used.
    #[schemars(regex(pattern = attricat_validation::CONNECTION_KEY_PATTERN))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    /// Also run an export when records of this blueprint change, once they
    /// have been quiet for `debounce_seconds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<ConnectorJobTrigger>,
}

/// A change trigger of an export connector job.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorJobTrigger {
    /// Record event types that start a run, such as `record.updated.v1`.
    pub events: Vec<String>,
    /// Quiet period before a triggered run, from 1 to 3600 seconds.
    #[schemars(range(min = 1, max = 3600))]
    pub debounce_seconds: i32,
}

/// Record-owned outlets whose placement a blueprint `extension_layout` may override.
pub const EXTENSION_LAYOUT_OUTLETS: &[&str] = &[
    "record_preview_panel",
    "record_attribute_decoration",
    "record_action",
];
/// View names with platform meaning, offered to editors as suggestions.
pub const KNOWN_VIEW_NAMES: &[&str] = &[
    "dropdown_option",
    "page_title",
    "detail",
    "table",
    "explorer",
    "extension_layout",
];

pub const CONNECTOR_JOB_DIRECTIONS: &[&str] = &["import", "export"];
/// Record events that may trigger an export connector job: every event
/// that changes a record's content revision.
pub const CONNECTOR_TRIGGER_EVENTS: &[&str] = attricat_events::RECORD_CONTENT_EVENT_TYPES_V1;
/// Shortest connector job run interval, in seconds.
pub(crate) const MIN_CONNECTOR_INTERVAL_SECONDS: i32 = 60;
/// Longest connector job run interval, in seconds (30 days).
pub(crate) const MAX_CONNECTOR_INTERVAL_SECONDS: i32 = 30 * 24 * 60 * 60;

fn default_connector_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationPolicy {
    /// Workspace roles whose edits keep records published on their channels.
    #[serde(default)]
    #[schemars(extend("x-attricat-reference" = "role"))]
    pub retain_on_edit_roles: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentReference {
    /// Registered component ID, such as `attricat.field_edit`.
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
    /// Blueprint whose records reference this record.
    #[schemars(extend("x-attricat-reference" = "blueprint"))]
    pub source_blueprint: String,
    /// Relationship attribute on the source blueprint.
    pub field: String,
}

/// A named view. `dropdown_option` labels relationship choices, `page_title`
/// names the record page, `table` configures Explorer columns, `explorer`
/// sets Explorer defaults, and layout roots arrange `detail` and `edit`.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewDefinition {
    /// Label shown for the record in relationship dropdowns.
    DropdownOption {
        /// Attributes joined into the label.
        #[schemars(extend("x-attricat-reference" = "attribute"))]
        fields: Vec<String>,
        /// Text between label fields. Defaults to ` · `.
        #[serde(default = "default_display_separator")]
        separator: String,
    },
    /// Title of the record page and its browser tab.
    PageTitle {
        /// Attributes joined into the title.
        #[schemars(extend("x-attricat-reference" = "attribute"))]
        fields: Vec<String>,
        /// Text between title fields. Defaults to ` · `.
        #[serde(default = "default_display_separator")]
        separator: String,
    },
    /// Explorer table columns.
    Table {
        /// Table columns.
        #[serde(default)]
        columns: Option<Vec<TableColumn>>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Explorer defaults for this blueprint's results.
    Explorer {
        /// Relationship field the browse rail lists until a person picks
        /// another, or `false` to keep the rail closed. The field must have
        /// a single target blueprint. A dotted path such as `model.brand`
        /// follows up to three such relationships, each with
        /// `cardinality = "one"`, and lists the last one's targets.
        #[serde(default)]
        browse_by: Option<BrowseBy>,
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
    /// A versioned layout override for record-owned extension surfaces.
    ExtensionLayout {
        /// Layout format version. Only `1` is supported.
        #[schemars(extend("const" = 1))]
        version: u32,
        /// Layouts keyed by record outlet: `record_preview_panel`,
        /// `record_attribute_decoration`, or `record_action`.
        #[schemars(extend("x-attricat-key-suggestions" = EXTENSION_LAYOUT_OUTLETS))]
        outlets: HashMap<String, ExtensionOutletLayout>,
    },
}

/// The Explorer browse rail's default: a relationship field or path, or `false`.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
#[schemars(extend("x-attricat-reference" = "relationship_path"))]
pub enum BrowseBy {
    /// Relationship attribute code, or a dotted path of up to three
    /// relationship hops.
    Field(String),
    /// `false` keeps the rail closed until a person opens it.
    #[schemars(extend("const" = false))]
    Hidden(bool),
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
    /// Cell renderer, such as `attricat.table_image`.
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
    /// A relationship attribute's linked records.
    RelationshipList {
        /// Relationship attribute to render.
        #[schemars(extend("x-attricat-reference" = "relationship_attribute"))]
        field: String,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
    /// Records that reference this record, opened in a paginated dialog.
    IncomingRelationshipList {
        /// Button and dialog label. `{{…}}` references resolve from the
        /// workspace lexicon.
        label: String,
        /// Source blueprint relationship fields that point at this record.
        relationships: Vec<IncomingRelationship>,
        /// Records requested per page.
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
    /// Creates records.
    Record,
    /// Provides attributes for other blueprints to include.
    Mixin,
}

impl BlueprintKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Mixin => "mixin",
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncludeRef {
    /// Local name used by `from = "<alias>.<attribute>"` selections.
    #[schemars(regex(pattern = attricat_validation::CODE_PATTERN))]
    pub alias: String,
    /// Code of the included mixin blueprint.
    #[schemars(
        regex(pattern = attricat_validation::CODE_PATTERN),
        extend("x-attricat-reference" = "mixin")
    )]
    pub code: String,
    /// Exact mixin version to include.
    #[schemars(range(min = 1), extend("x-attricat-reference" = "mixin_version"))]
    pub version: i64,
}

/// Stored as JSON on the attribute row and read back by the repository.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilePolicy {
    pub cardinality: attricat_domain::attribute_options::Cardinality,
    #[serde(default)]
    pub ordered: bool,
    pub allowed_mime_groups: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub max_bytes: Option<u64>,
    #[serde(default)]
    pub purposes: Vec<String>,
    pub image_only: bool,
}

impl FilePolicy {
    /// Whether a file of a detected MIME type is accepted by this policy, as
    /// for an ordinary upload.
    pub fn allows(&self, mime: &str, filename: &str, size: u64) -> bool {
        attricat_validation::files::FileConstraints {
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
    pub description: Option<String>,
    pub value_type: attricat_domain::value_type::ValueType,
    pub value_schema: Option<serde_json::Value>,
    /// An unresolved `provider:type@range` reference. The API resolver pins it
    /// to an installed extension release before persistence.
    pub extension_type: Option<String>,
    pub extension_configuration: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    /// Every allowed target blueprint. A single target is also kept in
    /// `target_blueprint`; empty means any record blueprint.
    pub target_blueprints: Vec<String>,
    pub cardinality: Option<attricat_domain::attribute_options::Cardinality>,
    pub target_cardinality: Option<attricat_domain::attribute_options::Cardinality>,
    /// `acyclic` or `tree` for self-referencing relationships.
    pub hierarchy: Option<String>,
    pub tags: Vec<String>,
    pub context_fallback: attricat_domain::attribute_options::ContextFallback,
    pub context_editable: attricat_domain::attribute_options::ContextEditable,
    pub readonly: bool,
    pub generated: Option<crate::GeneratedValue>,
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
    pub value_type: attricat_domain::value_type::ValueType,
    pub value_schema: Option<serde_json::Value>,
    /// Host-pinned extension type metadata. It is declarative so persisted
    /// values remain readable when the provider is unavailable.
    pub extension_type: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    /// Every allowed target blueprint; empty means any record blueprint.
    pub target_blueprints: Vec<String>,
    pub cardinality: Option<attricat_domain::attribute_options::Cardinality>,
    pub target_cardinality: Option<attricat_domain::attribute_options::Cardinality>,
    /// `acyclic` or `tree` for self-referencing relationships.
    pub hierarchy: Option<String>,
    pub tags: Vec<String>,
    pub context_fallback: attricat_domain::attribute_options::ContextFallback,
    pub context_editable: attricat_domain::attribute_options::ContextEditable,
    pub readonly: bool,
    /// Set only on the record blueprint that declares it: a `from` selection
    /// never generates.
    pub generated: Option<crate::GeneratedValue>,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBlueprint {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: BlueprintKind,
    pub raw_definition_hash: String,
    pub includes: Vec<IncludeRef>,
    pub views: HashMap<String, ViewDefinition>,
    pub record_schema: Option<serde_json::Value>,
    pub rules: Vec<attricat_rules::CompiledRule>,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub attributes: Vec<EffectiveAttribute>,
}
