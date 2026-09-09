use std::collections::HashSet;

use crate::{
    BlueprintError, ComponentReference, EffectiveAttribute, TableColumn, ViewDefinition, ViewNode,
    component_manifest::validate_component, parser::validate_code,
};

pub(crate) fn validate_view(
    view: &str,
    definition: &ViewDefinition,
    attributes: &[EffectiveAttribute],
    blueprint_code: &str,
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
                validate_table_columns(columns.as_deref().unwrap_or_default(), attributes)?;
                return Ok(());
            };
            if columns.is_some() {
                return Err(BlueprintError::TableFieldsAndColumns);
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
                            .is_none()
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
                || renderer.version <= 0
            {
                return Err(BlueprintError::InvalidTableColumnPath {
                    field: column.field.clone(),
                });
            }
        }
        let parts: Vec<_> = column.field.split('.').collect();
        match parts.as_slice() {
            [field] => {
                validate_code(field, "table column field")?;
                validate_view_field("table", field, attributes, false)?;
            }
            [relationship, target_field] => {
                validate_code(relationship, "table column relationship")?;
                validate_code(target_field, "table column target field")?;
                let attribute = attributes
                    .iter()
                    .find(|attribute| attribute.code == *relationship)
                    .ok_or_else(|| BlueprintError::UnknownViewField {
                        view: "table".to_owned(),
                        field: (*relationship).to_owned(),
                    })?;
                if attribute.value_type != "relationship" {
                    return Err(BlueprintError::TableColumnRelationshipRequired {
                        field: column.field.clone(),
                    });
                }
            }
            _ => {
                return Err(BlueprintError::InvalidTableColumnPath {
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
