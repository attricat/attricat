use std::collections::HashSet;

use crate::{
    BlueprintError, BlueprintKind, ComponentReference, EffectiveAttribute, TableColumn,
    ViewDefinition, ViewNode, component_manifest::validate_component, parser::validate_code,
};

pub(crate) fn validate_view(
    view: &str,
    definition: &ViewDefinition,
    attributes: &[EffectiveAttribute],
    blueprint_code: &str,
    blueprint_kind: &BlueprintKind,
) -> Result<(), BlueprintError> {
    match definition {
        ViewDefinition::DropdownOption { fields, .. } => {
            if fields.is_empty() {
                return Err(BlueprintError::EmptyDropdownOptionFields);
            }
            let mut unique_fields = HashSet::new();
            for field in fields {
                if field.trim().is_empty() || !unique_fields.insert(field) {
                    return Err(BlueprintError::DuplicateDropdownOptionField(field.clone()));
                }
                validate_view_field(view, field, attributes, false)?;
            }
        }
        ViewDefinition::Table {
            fields,
            columns,
            component,
        } => {
            let Some(fields) = fields else {
                if columns.is_none() {
                    return Err(BlueprintError::EmptyTableColumns);
                }
                // Table components are still part of the view contract when
                // modern `columns` are used. Validate their identity,
                // placement, and capability before resolving column paths.
                validate_component(component.as_ref(), view, "table", None)?;
                validate_table_columns(
                    columns.as_deref().unwrap_or_default(),
                    attributes,
                    component.as_ref(),
                )?;
                return Ok(());
            };
            if columns.is_some() {
                return Err(BlueprintError::TableFieldsAndColumns);
            }
            if fields.is_empty() {
                return Err(BlueprintError::EmptyTableColumns);
            }
            for field in fields {
                let attribute = validate_view_field(view, field, attributes, false)?;
                validate_component(
                    component.as_ref(),
                    view,
                    "table",
                    Some(&attribute.value_type),
                )?;
            }
        }
        ViewDefinition::Stack {
            children,
            component,
        } => {
            validate_stack_component(component.as_ref(), view, children, attributes)?;
            validate_view_nodes(view, children, attributes, blueprint_code)?;
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
            validate_view_nodes(view, children, attributes, blueprint_code)?;
        }
        ViewDefinition::Tabs { tabs, component } => {
            validate_non_data_component(component.as_ref())?;
            for tab in tabs {
                validate_view_nodes(view, &tab.children, attributes, blueprint_code)?;
            }
        }
        ViewDefinition::Accordion {
            sections,
            component,
        } => {
            validate_non_data_component(component.as_ref())?;
            for section in sections {
                validate_view_nodes(view, &section.children, attributes, blueprint_code)?;
            }
        }
        ViewDefinition::ExtensionLayout { version, outlets } => {
            if view != "extension_layout"
                || *version != 1
                || *blueprint_kind != BlueprintKind::Entity
            {
                return Err(BlueprintError::InvalidExtensionLayout);
            }
            let mut seen = HashSet::new();
            for (outlet, layout) in outlets {
                if !matches!(
                    outlet.as_str(),
                    "entity_preview_panel" | "entity_attribute_decoration" | "entity_action"
                ) || layout.order.iter().chain(&layout.hidden).any(|key| {
                    let Some((extension_id, contribution_id)) = key.split_once(':') else {
                        return true;
                    };
                    key.len() > 256
                        || extension_id.is_empty()
                        || contribution_id.is_empty()
                        || contribution_id.contains(':')
                        || !key
                            .chars()
                            .filter(|character| *character != ':')
                            .all(|character| {
                                character.is_ascii_alphanumeric() || "._-".contains(character)
                            })
                        || !seen.insert(key.as_str())
                }) {
                    return Err(BlueprintError::InvalidExtensionLayout);
                }
            }
        }
    }
    Ok(())
}

fn validate_view_nodes(
    view: &str,
    nodes: &[ViewNode],
    attributes: &[EffectiveAttribute],
    blueprint_code: &str,
) -> Result<(), BlueprintError> {
    for node in nodes {
        match node {
            ViewNode::Stack {
                children,
                component,
            } => {
                validate_stack_component(component.as_ref(), view, children, attributes)?;
                validate_view_nodes(view, children, attributes, blueprint_code)?;
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
                validate_view_nodes(view, children, attributes, blueprint_code)?;
            }
            ViewNode::Tabs { tabs, component } => {
                validate_non_data_component(component.as_ref())?;
                for tab in tabs {
                    validate_view_nodes(view, &tab.children, attributes, blueprint_code)?;
                }
            }
            ViewNode::Accordion {
                sections,
                component,
            } => {
                validate_non_data_component(component.as_ref())?;
                for section in sections {
                    validate_view_nodes(view, &section.children, attributes, blueprint_code)?;
                }
            }
            ViewNode::Heading { component, .. }
            | ViewNode::Text { component, .. }
            | ViewNode::Divider { component } => validate_non_data_component(component.as_ref())?,
            ViewNode::Field { .. }
            | ViewNode::RelationshipList { .. }
            | ViewNode::IncomingRelationshipList { .. } => {}
        }
        match node {
            ViewNode::Field { field, component } => {
                let attribute = validate_view_field(view, field, attributes, false)?;
                validate_component(
                    component.as_ref(),
                    view,
                    "field",
                    Some(&attribute.value_type),
                )?;
            }
            ViewNode::RelationshipList { field, component } => {
                let attribute = validate_view_field(view, field, attributes, true)?;
                validate_component(
                    component.as_ref(),
                    view,
                    "relationship_list",
                    Some(&attribute.value_type),
                )?;
                if component.as_ref().is_some_and(|component| {
                    component.id == "catalog.relationship_hierarchy"
                        && component
                            .props
                            .get("parent_field")
                            .and_then(serde_json::Value::as_str)
                            .is_none_or(|parent_field| parent_field.trim().is_empty())
                }) && attribute.target_blueprint.as_deref() != Some(blueprint_code)
                {
                    return Err(BlueprintError::HierarchyFieldMustTargetOwnBlueprint {
                        view: view.to_owned(),
                        field: field.clone(),
                    });
                }
            }
            ViewNode::IncomingRelationshipList {
                label,
                relationships,
                page_size,
                component,
            } => {
                if view != "detail" {
                    return Err(BlueprintError::IncomingRelationshipListOutsideDetail);
                }
                if label.trim().is_empty() {
                    return Err(BlueprintError::EmptyIncomingRelationshipLabel {
                        view: view.to_owned(),
                    });
                }
                if relationships.is_empty() {
                    return Err(BlueprintError::EmptyIncomingRelationships {
                        view: view.to_owned(),
                    });
                }
                if *page_size == 0 {
                    return Err(BlueprintError::InvalidIncomingRelationshipPageSize {
                        view: view.to_owned(),
                    });
                }
                for relationship in relationships {
                    validate_code(&relationship.source_blueprint, "source_blueprint")?;
                    validate_code(&relationship.field, "field")?;
                }
                validate_component(component.as_ref(), view, "incoming_relationship_list", None)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_table_columns(
    columns: &[TableColumn],
    attributes: &[EffectiveAttribute],
    table_component: Option<&ComponentReference>,
) -> Result<(), BlueprintError> {
    if columns.is_empty() {
        return Err(BlueprintError::EmptyTableColumns);
    }
    let mut seen = HashSet::new();
    for column in columns {
        if !seen.insert(&column.field) {
            return Err(BlueprintError::DuplicateTableColumn {
                field: column.field.clone(),
            });
        }
        if column
            .label
            .as_ref()
            .is_some_and(|label| label.trim().is_empty())
        {
            return Err(BlueprintError::InvalidTableColumnPath {
                field: column.field.clone(),
            });
        }
        if let Some(renderer) = &column.renderer {
            if !renderer.props.is_null() && !renderer.props.is_object() {
                return Err(BlueprintError::InvalidTableColumnRendererProps);
            }
            // Extension renderers are resolved while saving/publishing. The
            // compiler can still reject malformed references without knowing
            // the workspace's installed extensions.
            if renderer.id.is_empty()
                || !renderer
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
                || renderer.version == 0
            {
                return Err(BlueprintError::InvalidTableColumnPath {
                    field: column.field.clone(),
                });
            }
        }
        let parts: Vec<_> = column.field.split('.').collect();
        if parts.is_empty() || parts.len() > 4 {
            return Err(BlueprintError::InvalidTableColumnPath {
                field: column.field.clone(),
            });
        }
        for part in &parts {
            validate_code(part, "table column path segment")?;
        }
        if parts.len() == 1 {
            let attribute = validate_view_field("table", parts[0], attributes, false)?;
            // This matches legacy `fields` behavior for local scalar columns.
            validate_component(
                table_component,
                "table",
                "table",
                Some(&attribute.value_type),
            )?;
        } else {
            let relationship = parts[0];
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.code == relationship)
                .ok_or_else(|| BlueprintError::UnknownViewField {
                    view: "table".to_owned(),
                    field: relationship.to_owned(),
                })?;
            if attribute.value_type != "relationship" {
                return Err(BlueprintError::TableColumnRelationshipRequired {
                    field: column.field.clone(),
                });
            }
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
