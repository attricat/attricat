use std::collections::HashMap;

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
    pub attributes: Vec<AttributeDeclaration>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationPolicy {
    #[serde(default)]
    pub retain_on_edit_roles: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentReference {
    pub id: String,
    /// Component manifest versions are non-negative 32-bit protocol values.
    pub version: u32,
    #[serde(default)]
    pub props: serde_json::Value,
}

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

/// Host-owned per-blueprint extension layout. It is declarative data only;
/// contribution keys cannot select DOM nodes or invoke extension code.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionOutletLayout {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub hidden: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingRelationship {
    pub source_blueprint: String,
    pub field: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewDefinition {
    DropdownOption {
        fields: Vec<String>,
        #[serde(default = "default_display_separator")]
        separator: String,
    },
    Table {
        /// Legacy scalar-field shorthand. New definitions use `columns`.
        #[serde(default)]
        fields: Option<Vec<String>>,
        #[serde(default)]
        columns: Option<Vec<TableColumn>>,
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
    /// A versioned layout override for entity-owned extension surfaces.
    ExtensionLayout {
        version: u32,
        outlets: HashMap<String, ExtensionOutletLayout>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableColumn {
    /// A local scalar field or up to three relationship hops ending in a scalar field.
    pub field: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub renderer: Option<ComponentReference>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
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
    IncomingRelationshipList {
        label: String,
        relationships: Vec<IncomingRelationship>,
        page_size: u32,
        #[serde(default)]
        component: Option<ComponentReference>,
    },
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

#[derive(Clone, Debug, PartialEq)]
pub struct LocalAttributeDeclaration {
    pub code: String,
    pub value_type: String,
    pub value_schema: Option<serde_json::Value>,
    /// An unresolved `provider:type@range` reference. The API resolver pins it
    /// to an installed extension release before persistence.
    pub extension_type: Option<String>,
    pub extension_configuration: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
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
    pub value_type: String,
    pub value_schema: Option<serde_json::Value>,
    /// Host-pinned extension type metadata. It is declarative so persisted
    /// values remain readable when the provider is unavailable.
    pub extension_type: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
    pub cardinality: Option<String>,
    pub target_cardinality: Option<String>,
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
    pub attributes: Vec<EffectiveAttribute>,
}
