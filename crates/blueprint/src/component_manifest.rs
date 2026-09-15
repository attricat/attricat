use std::sync::LazyLock;

use serde::Deserialize;

use crate::{BlueprintError, ComponentReference};

#[derive(Deserialize)]
struct ComponentManifest {
    components: Vec<ComponentManifestEntry>,
}

#[derive(Deserialize)]
struct ComponentManifestEntry {
    id: String,
    version: u32,
    capabilities: Vec<String>,
    placements: Vec<String>,
    value_types: Vec<String>,
    allowed_props: Vec<String>,
}

static COMPONENT_MANIFEST: LazyLock<ComponentManifest> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../contracts/view-components.json"))
        .expect("component manifest must be valid JSON")
});

pub fn validate_table_renderer(
    renderer: &ComponentReference,
    value_type: &str,
) -> Result<(), BlueprintError> {
    validate_component(Some(renderer), "table", "table", Some(value_type))
}

pub(crate) fn validate_component(
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
    if component.version == 0 {
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
    if let Some(value_type) = value_type
        && !manifest_component
            .value_types
            .iter()
            .any(|item| item == value_type)
    {
        return Err(BlueprintError::InvalidComponentValueType {
            id: component.id.clone(),
            value_type: value_type.to_owned(),
        });
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
