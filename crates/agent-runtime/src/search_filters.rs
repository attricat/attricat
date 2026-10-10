//! Resolve agent filter inputs using the same attribute and relationship semantics as Explorer.
use std::collections::HashSet;
use uuid::Uuid;

use crate::agent_tools::ToolError;
use crate::{
    model::{BlueprintWithAttributes, RelationshipFilter, SearchFilter},
    repository::{
        AttricatRepository, RecordRelationshipFilter, RecordSearchFilter, RepositoryError,
    },
};

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(message.into())
}

/// What a blueprint offers a search path, appended to field errors so the
/// model can correct a filter, query or sort without reading the blueprint.
pub(crate) fn search_fields_hint(blueprint: &BlueprintWithAttributes) -> String {
    let (relationships, attributes): (Vec<_>, Vec<_>) = blueprint
        .attributes
        .iter()
        .partition(|attribute| attribute.value_type == "relationship");
    let relationships = relationships
        .iter()
        .map(|attribute| {
            let targets = if attribute.target_blueprint_codes.is_empty() {
                "any blueprint".to_owned()
            } else {
                attribute.target_blueprint_codes.join(" | ")
            };
            format!("{} -> {targets}", attribute.code)
        })
        .collect::<Vec<_>>();
    let attributes = attributes
        .iter()
        .map(|attribute| format!("{} ({})", attribute.code, attribute.value_type))
        .collect::<Vec<_>>();
    let list = |items: Vec<String>| {
        if items.is_empty() {
            "none".to_owned()
        } else {
            items.join(", ")
        }
    };
    format!(
        "{} has relationship fields: {}; attributes: {}",
        blueprint.blueprint.code,
        list(relationships),
        list(attributes)
    )
}

pub(crate) fn intersect_ids(current: Option<Vec<Uuid>>, next: Vec<Uuid>) -> Vec<Uuid> {
    match current {
        None => next,
        Some(current) => {
            let allowed: HashSet<_> = next.into_iter().collect();
            current
                .into_iter()
                .filter(|id| allowed.contains(id))
                .collect()
        }
    }
}

/// `actor` is the person who started the conversation; `@me` filters on
/// assignment attributes match them and their teams.
pub(crate) async fn resolve_agent_filter(
    repository: &AttricatRepository,
    blueprint: &BlueprintWithAttributes,
    filter: &SearchFilter,
    actor: Uuid,
) -> Result<RecordSearchFilter, ToolError> {
    let parts: Vec<_> = filter.field.split('.').collect();
    if parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
        return Err(invalid(
            "filters.field may contain at most three relationship hops and a scalar or file leaf",
        ));
    }
    let mut current = blueprint.clone();
    let mut relationship_path = Vec::new();
    for relationship_name in &parts[..parts.len() - 1] {
        let relationship = current
            .attributes
            .iter()
            .find(|attribute| {
                attribute.code == *relationship_name && attribute.value_type == "relationship"
            })
            .ok_or_else(|| {
                invalid(format!(
                    "filters.field segment '{relationship_name}' is not a relationship. {}",
                    search_fields_hint(&current)
                ))
            })?;
        relationship_path.push(relationship.code.clone());
        let target = relationship
            .target_blueprint_code
            .as_deref()
            .ok_or_else(|| {
                invalid(format!(
                    "filters.field relationship '{relationship_name}' has no target blueprint"
                ))
            })?;
        current = repository
            .get_blueprint_by_code(target)
            .await?
            .ok_or(RepositoryError::NotFound("target blueprint"))?;
    }
    let leaf = parts[parts.len() - 1];
    let (attribute_code, value_type, reusable, value_schema) =
        match current.attributes.iter().find(|a| a.code == leaf) {
            Some(attribute) => (
                attribute.code.clone(),
                attribute.value_type.clone(),
                false,
                attribute.value_schema.clone(),
            ),
            None if relationship_path.is_empty() => {
                let attribute = repository
                    .attached_reusable_attribute_for_blueprint(
                        blueprint.blueprint.id,
                        blueprint.blueprint.version,
                        leaf,
                    )
                    .await?
                    .filter(|attribute| attribute.searchable)
                    .ok_or_else(|| {
                        invalid(format!(
                            "filters.field leaf '{leaf}' is not a searchable attribute. {}",
                            search_fields_hint(&current)
                        ))
                    })?;
                (
                    format!("{}:{}", attribute.namespace, attribute.code),
                    attribute.value_type,
                    true,
                    attribute.value_schema,
                )
            }
            None => {
                return Err(invalid(format!(
                    "filters.field leaf '{leaf}' is not an attribute. {}",
                    search_fields_hint(&current)
                )));
            }
        };
    if let Some(value) = repository
        .current_user_filter_value(
            value_schema.as_ref(),
            &filter.operator,
            &filter.value,
            actor,
        )
        .await?
    {
        return Ok(RecordSearchFilter {
            field: filter.field.clone(),
            relationship_path,
            leaf_field: attribute_code,
            reusable,
            operator: crate::repository::SEARCH_FILTER_EQ_ANY.to_owned(),
            value_type,
            value,
        });
    }
    let presence = filter.operator == attricat_validation::saved_search::FILTER_OPERATOR_IS_SET;
    let valid_operator = match value_type.as_str() {
        "string" => {
            presence || matches!(filter.operator.as_str(), "eq" | "contains" | "starts_with")
        }
        "number" | "integer" | "date" | "datetime" | "time" => {
            presence || matches!(filter.operator.as_str(), "eq" | "gt" | "gte" | "lt" | "lte")
        }
        "boolean" => presence || filter.operator == "eq",
        // A file value has no comparable scalar, only attached files.
        "file" => presence,
        _ => false,
    };
    if !valid_operator {
        return Err(invalid(format!(
            "operator '{}' is not supported for {} attribute '{}'",
            filter.operator, value_type, filter.field
        )));
    }
    let value = if presence {
        filter.value.as_bool().map(|value| value.to_string())
    } else {
        match value_type.as_str() {
            "string" => filter.value.as_str().map(str::to_owned),
            "number" => filter.value.as_number().map(ToString::to_string),
            "integer" => filter.value.as_i64().map(|value| value.to_string()),
            "boolean" => filter.value.as_bool().map(|value| value.to_string()),
            "date" => filter.value.as_str().and_then(|value| {
                chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .ok()
                    .map(|_| value.to_owned())
            }),
            "datetime" => filter.value.as_str().and_then(|value| {
                chrono::DateTime::parse_from_rfc3339(value)
                    .ok()
                    .map(|_| value.to_owned())
            }),
            "time" => filter.value.as_str().and_then(|value| {
                ["%H:%M", "%H:%M:%S", "%H:%M:%S%.f"]
                    .iter()
                    .any(|format| chrono::NaiveTime::parse_from_str(value, format).is_ok())
                    .then(|| value.to_owned())
            }),
            _ => None,
        }
    }
    .ok_or_else(|| {
        invalid(format!(
            "filters.value is invalid for {} attribute '{}'",
            value_type, filter.field
        ))
    })?;
    Ok(RecordSearchFilter {
        field: filter.field.clone(),
        relationship_path,
        leaf_field: attribute_code,
        reusable,
        operator: filter.operator.clone(),
        value_type,
        value,
    })
}

pub(crate) async fn resolve_agent_relationship_filter(
    repository: &AttricatRepository,
    blueprint: &BlueprintWithAttributes,
    filter: &RelationshipFilter,
) -> Result<RecordRelationshipFilter, ToolError> {
    if filter.selected_target_ids.is_empty() || filter.selected_target_ids.len() > 100 {
        return Err(invalid(
            "relationship_filters.selected_target_ids must contain 1–100 IDs",
        ));
    }
    let path: Vec<_> = filter.field.split('.').collect();
    if path.len() > 3 || path.iter().any(|segment| segment.is_empty()) {
        return Err(invalid(
            "relationship_filters.field must contain one to three relationship hops",
        ));
    }
    let mut current = blueprint.clone();
    let mut relationship_path = Vec::with_capacity(path.len());
    for name in path {
        let relationship = current
            .attributes
            .iter()
            .find(|a| a.code == name && a.value_type == "relationship")
            .ok_or_else(|| {
                invalid(format!(
                    "relationship_filters.field segment '{name}' is not a relationship. {}",
                    search_fields_hint(&current)
                ))
            })?;
        let target = relationship
            .target_blueprint_code
            .as_deref()
            .ok_or_else(|| {
                invalid(format!(
                    "relationship_filters.field relationship '{name}' has no target blueprint"
                ))
            })?;
        relationship_path.push(relationship.code.clone());
        current = repository
            .get_blueprint_by_code(target)
            .await?
            .ok_or(RepositoryError::NotFound("target blueprint"))?;
    }
    Ok(RecordRelationshipFilter {
        field: filter.field.clone(),
        relationship_path,
        selected_target_ids: filter.selected_target_ids.clone(),
    })
}
