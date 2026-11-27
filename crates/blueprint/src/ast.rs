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
    /// The source still declares the removed `rules` list, which a
    /// stored revision may; saving such a definition is refused.
    pub embedded_rules: bool,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub movements: Vec<crate::MovementDefinition>,
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
    pub input: serde_json::Value,
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

/// The most extension contributions one view may list under `extensions`.
pub const MAX_VIEW_EXTENSIONS: usize = 32;
/// The view whose `extensions` place record actions, tabs and form assists.
pub const DETAIL_VIEW: &str = "detail";
/// The view whose `extensions` place Explorer actions.
pub const TABLE_VIEW: &str = "table";
/// Views whose `extensions` list places contributions: `detail` (the record
/// page and form) and `table` (the Explorer).
pub const EXTENSION_VIEWS: &[&str] = &[DETAIL_VIEW, TABLE_VIEW];
/// View names with platform meaning, offered to editors as suggestions.
pub const KNOWN_VIEW_NAMES: &[&str] = &[
    "dropdown_option",
    "page_title",
    "detail",
    "table",
    "explorer",
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

/// An extension's attribute editor that a detail field uses.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EditorReference {
    /// The enabled extension that declares the editor.
    pub extension_id: String,
    /// The editor contribution's ID in that extension.
    #[schemars(extend("x-attricat-reference" = "attribute_editor"))]
    pub id: String,
    /// The editor contribution's version.
    pub version: u32,
    /// Props the editor declares.
    #[serde(default)]
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub props: serde_json::Value,
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
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
    /// Explorer defaults for this blueprint's results.
    Explorer {
        /// Relationship field the browse rail lists until a person picks
        /// another, or `false` to keep the rail closed. The field must have
        /// a single target blueprint. A dotted path such as `model.brand`
        /// follows up to three such relationships, each with
        /// `cardinality: one`, and lists the last one's targets.
        #[serde(default)]
        browse_by: Option<BrowseBy>,
    },
    /// Blocks stacked vertically. A `detail` stack without children keeps
    /// the default record layout and only lists `extensions`.
    Stack {
        /// Nested blocks.
        #[serde(default)]
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
    /// Blocks arranged in a responsive grid.
    Grid {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
    /// Blocks grouped in a section.
    Section {
        /// Nested blocks.
        children: Vec<ViewNode>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
    /// Blocks split into tabs.
    Tabs {
        /// Tabs, each with a label and its own blocks.
        tabs: Vec<ViewTab>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
    /// Blocks split into collapsible sections.
    Accordion {
        /// Collapsible sections, each with a label and its own blocks.
        sections: Vec<ViewSection>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
        /// Extension contributions placed in this view, as
        /// `<extension-id>/<contribution-id>`, in display order. Only listed
        /// contributions that target the view appear on its records.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[schemars(length(max = MAX_VIEW_EXTENSIONS))]
        #[schemars(extend("x-attricat-reference" = "view_extension"))]
        extensions: Vec<String>,
    },
}

impl ViewDefinition {
    /// The contributions the view lists under `extensions`.
    pub fn extensions(&self) -> &[String] {
        match self {
            Self::Table { extensions, .. }
            | Self::Stack { extensions, .. }
            | Self::Grid { extensions, .. }
            | Self::Section { extensions, .. }
            | Self::Tabs { extensions, .. }
            | Self::Accordion { extensions, .. } => extensions,
            Self::DropdownOption { .. } | Self::PageTitle { .. } | Self::Explorer { .. } => &[],
        }
    }
}

/// Splits a view `extensions` entry into its extension and contribution IDs.
pub fn view_extension_key(entry: &str) -> Option<(&str, &str)> {
    let (extension_id, contribution_id) = entry.split_once('/')?;
    let valid = |id: &str| {
        (1..=MAX_VIEW_EXTENSION_ID_BYTES).contains(&id.len())
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    };
    (valid(extension_id) && valid(contribution_id)).then_some((extension_id, contribution_id))
}

/// The longest extension or contribution ID a view `extensions` entry names:
/// the manifest's identifier limit, which this crate cannot depend on; a
/// repository test keeps the two equal.
pub const MAX_VIEW_EXTENSION_ID_BYTES: usize = 128;

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
        /// An extension's attribute editor that edits this field in
        /// `views.detail`, by `id`, `version` and `props`. The default
        /// control is used whenever the editor is unavailable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<EditorReference>,
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
        /// `links` opens the records in the Explorer; `table` lists them on
        /// the page with the source blueprint's table columns, for one
        /// relationship.
        #[serde(default, skip_serializing_if = "IncomingDisplay::is_links")]
        display: IncomingDisplay,
        /// With `display: table`, people who can write records add, open,
        /// edit and delete them from the table, such as an order's lines.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        editable: bool,
        /// With `display: table`, the source blueprint's attributes the
        /// table shows, in order. Every attribute but the relationship back
        /// to this record when empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        columns: Vec<String>,
        /// Registered component that renders this block.
        #[serde(default)]
        component: Option<ComponentReference>,
    },
}

/// How an `incoming_relationship_list` shows the records linking to a record.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncomingDisplay {
    /// A link per relationship that opens the records in the Explorer.
    #[default]
    Links,
    /// A table of the records on the record's page.
    Table,
}

impl IncomingDisplay {
    fn is_links(&self) -> bool {
        *self == Self::Links
    }
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
    /// Local name used by `from: <alias>.<attribute>` selections.
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
    /// A relationship path, such as `item.sale_price_net`, whose value is
    /// copied into this empty attribute when the path's first relationship
    /// gains a target. A snapshot: later source changes do not reach it.
    pub default_from: Option<String>,
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
    pub balance: Option<crate::BalancePolicy>,
    pub target_scope: Option<crate::TargetScope>,
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
    /// Set only on the record blueprint that declares it, like `generated`:
    /// the path starts at that blueprint's relationships.
    pub default_from: Option<String>,
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
    /// Set only on the record blueprint that declares it, like `generated`:
    /// only that blueprint's movements write it.
    pub balance: Option<crate::BalancePolicy>,
    /// Filters the record picker; carried over by `from` selections.
    pub target_scope: Option<crate::TargetScope>,
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
    /// The source still declares the removed `rules` list, which a
    /// stored revision may; saving such a definition is refused.
    pub embedded_rules: bool,
    pub unique_keys: Vec<UniqueKeyDefinition>,
    pub movements: Vec<crate::MovementDefinition>,
    pub attributes: Vec<EffectiveAttribute>,
}

impl attricat_validation::relationship_path::PathAttribute for EffectiveAttribute {
    fn path_code(&self) -> &str {
        &self.code
    }

    fn path_value_type(&self) -> attricat_domain::value_type::ValueType {
        self.value_type
    }

    fn path_target_codes(&self) -> Vec<String> {
        attricat_domain::model::declared_target_codes(
            self.target_blueprint.as_ref(),
            &self.target_blueprints,
        )
    }

    fn path_cardinality(&self) -> Option<attricat_domain::attribute_options::Cardinality> {
        self.cardinality
    }
}
