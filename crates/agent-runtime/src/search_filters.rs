//! Agent-facing errors and bounds around the repository's search scope, which
//! resolves filters with the same semantics as Explorer.
use attricat_validation::saved_search::MAX_SELECTED_IDS;

use crate::agent_tools::ToolError;
use crate::{
    model::{BlueprintWithAttributes, RelationshipFilter},
    repository::{AttricatRepository, RecordRelationshipFilter, SearchScopeError},
};

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(message.into())
}

/// What a blueprint offers a search path, appended to field errors so the
/// model can correct a filter, query or sort without reading the blueprint.
fn search_fields_hint(blueprint: &BlueprintWithAttributes) -> String {
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

impl From<SearchScopeError> for ToolError {
    fn from(error: SearchScopeError) -> Self {
        match error {
            SearchScopeError::Invalid {
                message,
                blueprint: Some(blueprint),
            } => invalid(format!("{message}. {}", search_fields_hint(&blueprint))),
            SearchScopeError::Invalid { message, .. } => invalid(message),
            SearchScopeError::Query { error, blueprint } => {
                invalid(format!("{error}. {}", search_fields_hint(&blueprint)))
            }
            SearchScopeError::UnknownBlueprint => invalid("blueprint does not exist"),
            SearchScopeError::Repository(error) => ToolError::Repository(error),
        }
    }
}

/// Resolves a relationship filter, also bounding its selected IDs as the
/// saved searches an agent writes are.
pub(crate) async fn resolve_agent_relationship_filter(
    repository: &AttricatRepository,
    blueprint: &BlueprintWithAttributes,
    filter: &RelationshipFilter,
) -> Result<RecordRelationshipFilter, ToolError> {
    check_selected_ids(filter)?;
    Ok(repository
        .resolve_relationship_filter(blueprint, filter)
        .await?)
}

pub(crate) fn check_selected_ids(filter: &RelationshipFilter) -> Result<(), ToolError> {
    if filter.selected_target_ids.is_empty() || filter.selected_target_ids.len() > MAX_SELECTED_IDS
    {
        return Err(invalid(format!(
            "relationship_filters.selected_target_ids must contain 1–{MAX_SELECTED_IDS} IDs"
        )));
    }
    Ok(())
}
