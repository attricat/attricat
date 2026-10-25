//! Resolves a record search request into the blueprint, context and matching
//! records it covers. The records API and the agent's search tools share it so
//! filters, relationship paths and `@me` mean the same thing in both.
use attricat_domain::value_type::ValueType;
use std::collections::{HashMap, HashSet};

use attricat_validation::saved_search::{FILTER_OPERATOR_IS_SET, MAX_RELATIONSHIP_HOPS};
use thiserror::Error;
use tracing::Instrument;
use uuid::Uuid;

use super::{
    AttricatRepository, RecordRelationshipFilter, RecordSearchFilter, RecordSearchSort,
    RepositoryError, SEARCH_FILTER_EQ_ANY, SearchContext,
};
use crate::model::{
    BlueprintWithAttributes, MatchExplanation, RelationshipFilter, SearchBlueprint, SearchFilter,
};

/// Attribute filters, and separately relationship filters, a search may apply.
pub const MAX_SEARCH_FILTERS: usize = 20;

/// Why a search request could not be resolved into a scope.
#[derive(Debug, Error)]
pub enum SearchScopeError {
    /// No blueprint has the requested code.
    #[error("blueprint was not found")]
    UnknownBlueprint,
    /// The request is malformed. `blueprint` is the blueprint a field path
    /// failed to resolve against, so callers can say which fields it offers.
    #[error("{message}")]
    Invalid {
        message: String,
        blueprint: Option<Box<BlueprintWithAttributes>>,
    },
    /// A related-value sort over every version matched records of several
    /// versions, whose relationship paths may differ.
    #[error("related-value sorting requires results from one blueprint version")]
    RelationshipSortRequiresSingleVersion,
    /// The text query failed against the searched blueprint.
    #[error("{error}")]
    Query {
        error: RepositoryError,
        blueprint: Box<BlueprintWithAttributes>,
    },
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn invalid(message: impl Into<String>) -> SearchScopeError {
    SearchScopeError::Invalid {
        message: message.into(),
        blueprint: None,
    }
}

fn invalid_in(message: impl Into<String>, blueprint: &BlueprintWithAttributes) -> SearchScopeError {
    SearchScopeError::Invalid {
        message: message.into(),
        blueprint: Some(Box::new(blueprint.clone())),
    }
}

pub struct SearchScopeRequest<'a> {
    pub blueprint: &'a SearchBlueprint,
    pub query: Option<&'a str>,
    pub filters: &'a [SearchFilter],
    pub relationship_filters: &'a [RelationshipFilter],
    pub context_code: &'a str,
    /// The person `@me` filters on assignment attributes match, with their teams.
    pub actor: Uuid,
}

/// A search's blueprint, context and matching records.
pub struct SearchScope {
    /// The latest revision of the searched blueprint.
    pub current: BlueprintWithAttributes,
    /// The requested version, or `None` to search every version.
    pub selected: Option<i64>,
    /// The revision filters and the query resolve against.
    pub search_blueprint: BlueprintWithAttributes,
    pub context: SearchContext,
    pub explanations: HashMap<Uuid, Vec<MatchExplanation>>,
    /// `None` when neither a query nor a filter narrows the blueprint.
    pub matching: Option<Vec<Uuid>>,
}

/// The IDs in both lists, keeping `current`'s order; `None` means unrestricted.
pub fn intersect_record_ids(current: Option<Vec<Uuid>>, next: Vec<Uuid>) -> Vec<Uuid> {
    match current {
        None => next,
        Some(current) => {
            let next: HashSet<_> = next.into_iter().collect();
            current.into_iter().filter(|id| next.contains(id)).collect()
        }
    }
}

impl AttricatRepository {
    pub async fn resolve_search_scope(
        &self,
        request: SearchScopeRequest<'_>,
    ) -> Result<SearchScope, SearchScopeError> {
        let code = &request.blueprint.code;
        if code.is_empty() {
            return Err(invalid("blueprint.code must not be empty"));
        }
        for (field, len) in [
            ("filters", request.filters.len()),
            ("relationship_filters", request.relationship_filters.len()),
        ] {
            if len > MAX_SEARCH_FILTERS {
                return Err(invalid(format!(
                    "{field} must contain at most {MAX_SEARCH_FILTERS} items"
                )));
            }
        }
        let current = self
            .get_blueprint_by_code(code)
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "blueprint-load"
            ))
            .await?
            .ok_or(SearchScopeError::UnknownBlueprint)?;
        let (selected, search_blueprint) = match request.blueprint.version {
            // The latest published revision is the requested one.
            Some(version) if version == current.blueprint.version => {
                (Some(version), current.clone())
            }
            Some(version) => {
                let published = self
                    .get_published_blueprint_by_code_and_version(code, version)
                    .instrument(tracing::info_span!(
                        "sql.operation",
                        label = "blueprint-load"
                    ))
                    .await?
                    .ok_or(RepositoryError::NotFound("blueprint"))?;
                (Some(published.blueprint.version), published)
            }
            None => (None, current.clone()),
        };
        let context = self
            .search_context(request.context_code)
            .await?
            .ok_or_else(|| invalid("context_code is not a context"))?;
        let mut filters = Vec::with_capacity(request.filters.len());
        for filter in request.filters {
            filters.push(
                self.resolve_search_filter(&search_blueprint, filter, request.actor)
                    .await?,
            );
        }
        let mut relationship_filters = Vec::with_capacity(request.relationship_filters.len());
        for filter in request.relationship_filters {
            relationship_filters.push(
                self.resolve_relationship_filter(&search_blueprint, filter)
                    .await?,
            );
        }
        // Without a search term, the page query already scopes the blueprint
        // and version, so no record IDs are materialized.
        let query = request
            .query
            .map(str::trim)
            .filter(|query| !query.is_empty());
        let resolved = match query {
            Some(query) => Some(
                self.resolve_search(&search_blueprint, selected, Some(query))
                    .instrument(tracing::info_span!(
                        "sql.operation",
                        label = "search-resolve"
                    ))
                    .await
                    .map_err(|error| SearchScopeError::Query {
                        error,
                        blueprint: Box::new(search_blueprint.clone()),
                    })?,
            ),
            None => None,
        };
        let mut matching: Option<Vec<Uuid>> = resolved
            .as_ref()
            .map(|resolved| resolved.ids.iter().copied().collect());
        if !filters.is_empty() {
            let filtered = self
                .filter_record_ids(current.blueprint.id, selected, &filters, &context)
                .instrument(tracing::info_span!(
                    "sql.operation",
                    label = "attribute-filter"
                ))
                .await?;
            matching = Some(intersect_record_ids(matching, filtered));
        }
        if !relationship_filters.is_empty() {
            let filtered = self
                .filter_relationship_record_ids(
                    current.blueprint.id,
                    selected,
                    &relationship_filters,
                    &context,
                )
                .instrument(tracing::info_span!(
                    "sql.operation",
                    label = "relationship-filter"
                ))
                .await?;
            matching = Some(intersect_record_ids(matching, filtered));
        }
        Ok(SearchScope {
            current,
            selected,
            search_blueprint,
            context,
            explanations: resolved
                .map(|resolved| resolved.explanations)
                .unwrap_or_default(),
            matching,
        })
    }

    /// Resolves an attribute filter's field path, operator and value against
    /// `blueprint`. `actor` is who `@me` matches, with their teams.
    pub async fn resolve_search_filter(
        &self,
        blueprint: &BlueprintWithAttributes,
        filter: &SearchFilter,
        actor: Uuid,
    ) -> Result<RecordSearchFilter, SearchScopeError> {
        let parts: Vec<_> = filter.field.split('.').collect();
        if parts.len() > MAX_RELATIONSHIP_HOPS + 1 || parts.iter().any(|part| part.is_empty()) {
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
                    attribute.code == *relationship_name
                        && attribute.value_type == ValueType::Relationship
                })
                .ok_or_else(|| {
                    invalid_in(
                        format!(
                            "filters.field segment '{relationship_name}' is not a relationship"
                        ),
                        &current,
                    )
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
            current = self
                .get_blueprint_by_code(target)
                .await?
                .ok_or(RepositoryError::NotFound("target blueprint"))?;
        }
        let leaf = parts[parts.len() - 1];
        let (attribute_code, value_type, reusable, value_schema) = match current
            .attributes
            .iter()
            .find(|attribute| attribute.code == leaf)
        {
            Some(attribute) => (
                attribute.code.clone(),
                attribute.value_type,
                false,
                attribute.value_schema.clone(),
            ),
            None if relationship_path.is_empty() => {
                let attribute = self
                    .attached_reusable_attribute_for_blueprint(
                        blueprint.blueprint.id,
                        blueprint.blueprint.version,
                        leaf,
                    )
                    .await?
                    .filter(|attribute| attribute.searchable)
                    .ok_or_else(|| {
                        invalid_in(
                            format!("filters.field leaf '{leaf}' is not a searchable attribute"),
                            &current,
                        )
                    })?;
                (
                    format!("{}:{}", attribute.namespace, attribute.code),
                    attribute.value_type,
                    true,
                    attribute.value_schema,
                )
            }
            None => {
                return Err(invalid_in(
                    format!("filters.field leaf '{leaf}' is not an attribute"),
                    &current,
                ));
            }
        };
        // "Assigned to me" matches the actor and each of the actor's teams.
        if let Some(value) = self
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
                operator: SEARCH_FILTER_EQ_ANY.to_owned(),
                value_type,
                value,
            });
        }
        let presence = filter.operator == FILTER_OPERATOR_IS_SET;
        let valid_operator = match value_type {
            ValueType::String => {
                presence || matches!(filter.operator.as_str(), "eq" | "contains" | "starts_with")
            }
            ValueType::Number
            | ValueType::Integer
            | ValueType::Date
            | ValueType::Datetime
            | ValueType::Time => {
                presence || matches!(filter.operator.as_str(), "eq" | "gt" | "gte" | "lt" | "lte")
            }
            ValueType::Boolean => presence || filter.operator == "eq",
            // A file value has no comparable scalar, only attached files.
            ValueType::File => presence,
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
            match value_type {
                ValueType::String => filter.value.as_str().map(str::to_owned),
                ValueType::Number => filter.value.as_number().map(ToString::to_string),
                ValueType::Integer => filter.value.as_i64().map(|value| value.to_string()),
                ValueType::Boolean => filter.value.as_bool().map(|value| value.to_string()),
                ValueType::Date => filter.value.as_str().and_then(|value| {
                    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .ok()
                        .map(|_| value.to_owned())
                }),
                ValueType::Datetime => filter.value.as_str().and_then(|value| {
                    chrono::DateTime::parse_from_rfc3339(value)
                        .ok()
                        .map(|_| value.to_owned())
                }),
                ValueType::Time => filter.value.as_str().and_then(|value| {
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

    /// Resolves a relationship filter's one-to-three hop path against
    /// `blueprint`.
    pub async fn resolve_relationship_filter(
        &self,
        blueprint: &BlueprintWithAttributes,
        filter: &RelationshipFilter,
    ) -> Result<RecordRelationshipFilter, SearchScopeError> {
        if filter.selected_target_ids.is_empty() {
            return Err(invalid(
                "relationship_filters.selected_target_ids must not be empty",
            ));
        }
        let path: Vec<_> = filter.field.split('.').collect();
        if path.len() > MAX_RELATIONSHIP_HOPS || path.iter().any(|segment| segment.is_empty()) {
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
                .find(|attribute| {
                    attribute.code == name && attribute.value_type == ValueType::Relationship
                })
                .ok_or_else(|| {
                    invalid_in(
                        format!(
                            "relationship_filters.field segment '{name}' is not a relationship"
                        ),
                        &current,
                    )
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
            current = self
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

    /// Resolves a table sort against `blueprint`: a configured table column
    /// (through single-valued relationship hops), `blueprint_version`, or
    /// `publication_status` in an enabled channel.
    pub async fn resolve_search_sort(
        &self,
        blueprint: &crate::model::BlueprintWithAttributes,
        sort: Option<&crate::model::SearchSort>,
        effective_source_version: Option<i64>,
        context: &SearchContext,
    ) -> Result<Option<RecordSearchSort>, SearchScopeError> {
        let Some(sort) = sort else { return Ok(None) };
        let descending = match sort.direction.as_str() {
            "asc" => false,
            "desc" => true,
            _ => {
                return Err(invalid("sort.direction must be asc or desc".to_owned()));
            }
        };
        if sort.field == "publication_status" {
            let code = sort.context_code.as_deref().ok_or_else(|| {
                invalid("sort.context_code is required for publication_status".to_owned())
            })?;
            let channel = self
                .list_publication_channels()
                .await?
                .into_iter()
                .find(|channel| channel.context_code == code && channel.enabled)
                .ok_or_else(|| {
                    invalid("sort.context_code must be an enabled publication channel".to_owned())
                })?;
            return Ok(Some(RecordSearchSort {
                field: sort.field.clone(),
                relationship_path: Vec::new(),
                leaf_field: sort.field.clone(),
                leaf_blueprint_id: blueprint.blueprint.id,
                value_type: ValueType::Integer,
                descending,
                effective_source_version,
                publication_context_id: Some(channel.context_id),
                context: context.clone(),
            }));
        }
        if sort.context_code.is_some() {
            return Err(invalid(
                "sort.context_code is only valid for publication_status".to_owned(),
            ));
        }
        if sort.field == "blueprint_version" {
            return Ok(Some(RecordSearchSort {
                field: sort.field.clone(),
                relationship_path: Vec::new(),
                leaf_field: sort.field.clone(),
                leaf_blueprint_id: blueprint.blueprint.id,
                value_type: ValueType::Integer,
                descending,
                effective_source_version,
                publication_context_id: None,
                context: context.clone(),
            }));
        }
        let columns = blueprint
            .blueprint
            .views
            .get("table")
            .and_then(|table| table.get("columns"))
            .and_then(serde_json::Value::as_array)
            .map(|columns| {
                columns
                    .iter()
                    .filter_map(|column| column.get("field").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !columns.contains(&sort.field.as_str()) {
            return Err(invalid(format!(
                "sort.field must be a configured table column, blueprint_version or publication_status; {} table columns: {}",
                blueprint.blueprint.code,
                if columns.is_empty() {
                    "none".to_owned()
                } else {
                    columns.join(", ")
                }
            )));
        }
        let parts: Vec<_> = sort.field.split('.').collect();
        if parts.is_empty() || parts.len() > MAX_RELATIONSHIP_HOPS + 1 {
            return Err(invalid(
                "sort.field may contain at most three relationship hops and a scalar leaf"
                    .to_owned(),
            ));
        }
        let mut current = blueprint.clone();
        let mut relationship_path = Vec::new();
        for relationship_name in &parts[..parts.len() - 1] {
            let relationship = current
                .attributes
                .iter()
                .find(|attribute| {
                    attribute.code == *relationship_name
                        && attribute.value_type == ValueType::Relationship
                })
                .ok_or_else(|| invalid("sort.field relationship is invalid".to_owned()))?;
            if relationship.cardinality.as_deref() != Some("one") {
                return Err(invalid(
                    "sort.field relationship path must be single-valued".to_owned(),
                ));
            }
            relationship_path.push(relationship.code.clone());
            current = self
                .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                    || invalid("sort.field relationship has no target blueprint".to_owned()),
                )?)
                .await?
                .ok_or(RepositoryError::NotFound("target blueprint"))?;
        }
        let leaf_field = parts[parts.len() - 1];
        let value_type = current
            .attributes
            .iter()
            .find(|attribute| attribute.code == leaf_field)
            .map(|attribute| attribute.value_type)
            .ok_or_else(|| {
                invalid("sort.field must resolve to a scalar table column".to_owned())
            })?;
        if !value_type.is_scalar() {
            return Err(invalid(
                "sort.field must resolve to a scalar table column".to_owned(),
            ));
        }
        Ok(Some(RecordSearchSort {
            field: sort.field.clone(),
            relationship_path,
            leaf_field: leaf_field.to_owned(),
            leaf_blueprint_id: current.blueprint.id,
            value_type,
            descending,
            effective_source_version,
            publication_context_id: None,
            context: context.clone(),
        }))
    }
}

/// The scalar table columns a page hydrates, by attribute code.
pub fn table_value_paths(blueprint: &BlueprintWithAttributes) -> HashMap<String, ValueType> {
    blueprint
        .table_path_attributes
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type))
        .collect()
}

/// The direct relationships table columns traverse, with their target
/// blueprint codes.
pub fn table_relationship_targets(blueprint: &BlueprintWithAttributes) -> HashMap<String, String> {
    let Some(columns) = blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("columns"))
        .and_then(serde_json::Value::as_array)
    else {
        return HashMap::new();
    };
    columns
        .iter()
        .filter_map(|column| column.get("field").and_then(serde_json::Value::as_str))
        .filter_map(|field| field.split_once('.').map(|(relationship, _)| relationship))
        .filter_map(|relationship| {
            blueprint
                .attributes
                .iter()
                .find(|attribute| attribute.code == relationship)
                .filter(|attribute| attribute.value_type == ValueType::Relationship)
                .and_then(|attribute| {
                    attribute
                        .target_blueprint_code
                        .as_ref()
                        .map(|target| (relationship.to_owned(), target.clone()))
                })
        })
        .collect()
}

/// Whether a table column renders through an extension, whose cell context
/// carries the row's preview and related records.
pub(super) fn has_extension_table_renderer(blueprint: &BlueprintWithAttributes) -> bool {
    blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("columns"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|columns| {
            columns.iter().any(|column| {
                column
                    .get("renderer")
                    .and_then(|renderer| renderer.get("id"))
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| !id.starts_with("attricat."))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_keeps_the_current_order_and_treats_none_as_unrestricted() {
        let [first, second, third] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        assert_eq!(
            intersect_record_ids(None, vec![second, first]),
            [second, first]
        );
        assert_eq!(
            intersect_record_ids(Some(vec![first, second, third]), vec![third, first]),
            [first, third]
        );
        assert!(intersect_record_ids(Some(vec![first]), vec![second]).is_empty());
    }
}
