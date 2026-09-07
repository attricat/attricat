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
pub enum AttributeDeclaration {
    Local {
        code: String,
        value_type: String,
        value_schema: Option<serde_json::Value>,
        default_value: Option<serde_json::Value>,
        file_policy: Option<FilePolicy>,
        target_blueprint: Option<String>,
        tags: Vec<String>,
        context_fallback: String,
        context_editable: String,
        readonly: bool,
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
    pub value_schema: Option<serde_json::Value>,
    pub default_value: Option<serde_json::Value>,
    pub file_policy: Option<FilePolicy>,
    pub target_blueprint: Option<String>,
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
