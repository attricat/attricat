use super::{
    AppState, RequestTiming,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    constants::{DEFAULT_PAGE_SIZE, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS},
    model::{
        Entity, EntityIdentity, EntityPreviewPage, EntityPreviewResponse, EntitySearchResponse,
        RelationshipTreeFacetChildrenRequest, RelationshipTreeFacetChildrenResponse,
        ResolvedEntityPreviewResponse, SearchEntitiesRequest, SearchFilter,
    },
    repository::{EntitySearchFilter, EntitySearchSort, decode_search_cursor},
};
use axum::{
    Json,
    extract::{Extension, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use tracing::Instrument;
use uuid::Uuid;

const SEARCH_TOTAL_COUNT_CAP: i64 = 500;
const MAX_SEARCH_FILTERS: usize = 20;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewQuery {
    relationship_depth: Option<u8>,
    relationship_limit: Option<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolvedPreviewQuery {
    context_id: Uuid,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HierarchyQuery {
    context_id: Uuid,
    field: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListPreviewsQuery {
    blueprint: String,
    related_from: Uuid,
    relationship: String,
    limit: Option<u32>,
    cursor: Option<Uuid>,
}
pub(super) async fn get_entity(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Entity>, ApiError> {
    repository
        .get_entity(entity_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}
pub(super) async fn get_preview(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PreviewQuery>,
) -> Result<Json<EntityPreviewResponse>, ApiError> {
    let depth = query.relationship_depth.unwrap_or(1);
    let limit = query
        .relationship_limit
        .unwrap_or(DEFAULT_PREVIEW_RELATIONSHIP_ITEMS);
    if depth > state.max_preview_relationship_depth {
        return Err(ApiError::invalid_input(format!(
            "relationship_depth must not exceed {}",
            state.max_preview_relationship_depth
        )));
    }
    if limit == 0 || limit > state.max_preview_relationship_items {
        return Err(ApiError::invalid_input(format!(
            "relationship_limit must be between 1 and {}",
            state.max_preview_relationship_items
        )));
    }
    let context = repository
        .preview(entity_id, depth, limit.into())
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    let entity = repository
        .get_entity(entity_id)
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    Ok(Json(EntityPreviewResponse {
        entity: EntityIdentity {
            id: entity.id,
            blueprint_id: entity.blueprint_id,
            blueprint_version: entity.blueprint_version,
        },
        context,
    }))
}
pub(super) async fn get_entity_hierarchy(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HierarchyQuery>,
) -> Result<Json<crate::model::EntityHierarchyResponse>, ApiError> {
    repository
        .hierarchy(
            entity_id,
            query.context_id,
            &query.field,
            state.max_preview_relationship_depth,
        )
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}
pub(super) async fn get_resolved_preview(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ResolvedPreviewQuery>,
) -> Result<Json<ResolvedEntityPreviewResponse>, ApiError> {
    repository
        .resolved_preview(entity_id, query.context_id, 1)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}
pub(super) async fn list_previews(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<ListPreviewsQuery>,
) -> Result<Json<EntityPreviewPage>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 || limit > state.max_entity_page_size {
        return Err(ApiError::invalid_input(format!(
            "limit must be between 1 and {}",
            state.max_entity_page_size
        )));
    }
    Ok(Json(
        repository
            .list_previews(
                &query.blueprint,
                query.related_from,
                &query.relationship,
                limit.into(),
                query.cursor,
            )
            .await?,
    ))
}
pub(super) async fn search_entity_previews(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    Extension(timing): Extension<RequestTiming>,
    ApiJson(input): ApiJson<SearchEntitiesRequest>,
) -> Result<Response, ApiError> {
    let code = &input.blueprint.code;
    if code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    if input.filters.len() > MAX_SEARCH_FILTERS {
        return Err(ApiError::invalid_input(format!(
            "filters must contain at most {MAX_SEARCH_FILTERS} items"
        )));
    }
    let limit = input.page.size.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 || limit > state.max_entity_page_size {
        return Err(ApiError::invalid_input(format!(
            "page.size must be between 1 and {}",
            state.max_entity_page_size
        )));
    }
    let current = repository
        .get_blueprint_by_code(code)
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "blueprint-load"
        ))
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint"))?;
    let selected = match input.blueprint.version {
        Some(version) => Some(
            repository
                .get_blueprint_by_code_and_version(code, version)
                .instrument(tracing::info_span!(
                    "sql.operation",
                    label = "blueprint-load"
                ))
                .await?
                .map(|b| b.blueprint.version)
                .ok_or_else(|| ApiError::not_found("blueprint"))?,
        ),
        None => None,
    };
    let search_blueprint = match selected {
        Some(version) => repository
            .get_blueprint_by_code_and_version(code, version)
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "blueprint-load"
            ))
            .await?
            .expect("selected version was checked above"),
        None => current.clone(),
    };
    let sort = resolve_table_sort(&repository, &search_blueprint, input.sort.as_ref())
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "table-sort-resolve"
        ))
        .await?;
    let cursor = match (sort.is_some(), input.page.cursor.as_deref()) {
        (true, _) => None,
        (false, Some(cursor)) => Some(
            decode_search_cursor(cursor)
                .ok_or_else(|| ApiError::invalid_input("page.cursor is invalid".to_owned()))?,
        ),
        (false, None) => None,
    };
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let mut filters = Vec::with_capacity(input.filters.len());
    for filter in &input.filters {
        filters.push(resolve_search_filter(&repository, &search_blueprint, filter).await?);
    }
    let candidate_started = Instant::now();
    // An unfiltered current-version search is already constrained by the page query.
    // Avoid materializing every entity ID only to pass it back as `id = ANY(...)`.
    let must_resolve =
        query.is_some() || (selected.is_some() && !input.relationship_tree_facets.is_empty());
    let resolved = if must_resolve {
        Some(
            repository
                .resolve_search(&search_blueprint, selected, query)
                .instrument(tracing::info_span!(
                    "sql.operation",
                    label = "search-resolve"
                ))
                .await
                .map_err(|error| ApiError::invalid_search_query(error.to_string()))?,
        )
    } else {
        None
    };
    let ids = resolved.as_ref().map(|resolved| &resolved.ids);
    let mut matching: Option<Vec<Uuid>> = query.is_some().then(|| {
        ids.expect("search queries are resolved")
            .iter()
            .copied()
            .collect()
    });
    if !filters.is_empty() {
        let filtered = repository
            .filter_entity_ids(current.blueprint.id, selected, &filters)
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "attribute-filter"
            ))
            .await?;
        matching = Some(intersect_entity_ids(matching, filtered));
    }
    for facet in input.relationship_tree_facets {
        let source = &search_blueprint
            .attributes
            .iter()
            .find(|a| a.code == facet.source_relationship_field)
            .ok_or_else(|| {
                ApiError::invalid_input(
                    "relationship_tree_facets.source_relationship_field is not an attribute"
                        .to_owned(),
                )
            })?;
        if source.value_type != "relationship" {
            return Err(ApiError::invalid_input(
                "relationship_tree_facets.source_relationship_field must be a relationship"
                    .to_owned(),
            ));
        }
        let target_code = source.target_blueprint_code.as_deref().ok_or_else(|| {
            ApiError::invalid_input(
                "relationship_tree_facets.source_relationship_field has no target blueprint"
                    .to_owned(),
            )
        })?;
        let target = repository
            .get_blueprint_by_code(target_code)
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "blueprint-load"
            ))
            .await?
            .ok_or_else(|| ApiError::not_found("target blueprint"))?;
        if let Some(hierarchy_field) = &facet.hierarchy_field {
            let hierarchy = target
                .attributes
                .iter()
                .find(|a| a.code == *hierarchy_field)
                .ok_or_else(|| {
                    ApiError::invalid_input(
                        "relationship_tree_facets.hierarchy_field is not an attribute".to_owned(),
                    )
                })?;
            if hierarchy.value_type != "relationship"
                || hierarchy.target_blueprint_code.as_deref() != Some(target_code)
            {
                return Err(ApiError::invalid_input(
                    "relationship_tree_facets.hierarchy_field must be a self-targeting relationship"
                        .to_owned(),
                ));
            }
        }
        let facet_candidates = matching
            .as_ref()
            .map(|ids| ids.iter().copied().collect::<HashSet<_>>());
        let facet_matching = repository
            .relationship_tree_facet(
                current.blueprint.id,
                &facet.source_relationship_field,
                target.blueprint.id,
                facet.hierarchy_field.as_deref(),
                facet.context_id,
                &facet.selected_target_ids,
                facet_candidates.as_ref(),
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "relationship-facet"
            ))
            .await?
            .1;
        if let Some(facet_matching) = facet_matching {
            matching = Some(intersect_entity_ids(matching, facet_matching));
        }
    }
    timing.record("candidate", candidate_started);
    let page_started = Instant::now();
    let page = async {
        match sort.as_ref() {
            Some(sort) => {
                repository
                    .search_entity_previews_sorted(
                        current.blueprint.id,
                        selected,
                        limit.into(),
                        input.page.cursor.as_deref(),
                        matching.as_deref(),
                        &input.system_tags,
                        input.outdated,
                        current.blueprint.version,
                        sort,
                    )
                    .instrument(tracing::info_span!(
                        "sql.operation",
                        label = "entities-page"
                    ))
                    .await
            }
            None => {
                repository
                    .search_entity_previews(
                        current.blueprint.id,
                        selected,
                        None,
                        limit.into(),
                        cursor,
                        matching.as_deref(),
                        &input.system_tags,
                        input.outdated,
                        current.blueprint.version,
                    )
                    .instrument(tracing::info_span!(
                        "sql.operation",
                        label = "entities-page"
                    ))
                    .await
            }
        }
    };
    // Totals are intentionally limited and only calculated for a first page.
    // Cursor pages retain the first response's total in the client cache.
    let include_total = input.include_total && input.page.cursor.is_none();
    let total = async {
        if !include_total {
            return Ok(None);
        }
        let count = repository
            .count_entity_previews(
                current.blueprint.id,
                selected,
                matching.as_deref(),
                &input.system_tags,
                input.outdated,
                current.blueprint.version,
                SEARCH_TOTAL_COUNT_CAP + 1,
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "entities-count"
            ))
            .await?;
        Ok(Some(count))
    };
    let ((mut items, next_cursor), total_count) = tokio::try_join!(page, total)?;
    timing.record("page", page_started);
    let total_count_capped = total_count.is_some_and(|count| count > SEARCH_TOTAL_COUNT_CAP);
    let total_count = total_count.map(|count| count.min(SEARCH_TOTAL_COUNT_CAP));
    for item in &mut items {
        item.schema_outdated = item.blueprint_version != current.blueprint.version;
        item.match_explanations = resolved
            .as_ref()
            .and_then(|resolved| resolved.explanations.get(&item.id))
            .cloned()
            .unwrap_or_default();
    }
    let related_started = Instant::now();
    let table_paths = table_paths(&current);
    repository
        .hydrate_table_path_values(&mut items, &table_paths)
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "table-path-hydrate"
        ))
        .await?;
    repository
        .hydrate_related_table_previews(&mut items, &table_relationships(&current))
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "related-hydrate"
        ))
        .await?;
    timing.record("related", related_started);
    let serialization_started = Instant::now();
    let response = Json(EntitySearchResponse {
        blueprint: current,
        items,
        next_cursor,
        total_count,
        total_count_capped,
    })
    .into_response();
    timing.record("serialize", serialization_started);
    Ok(response)
}

fn intersect_entity_ids(current: Option<Vec<Uuid>>, next: Vec<Uuid>) -> Vec<Uuid> {
    match current {
        None => next,
        Some(current) => {
            let next: HashSet<_> = next.into_iter().collect();
            current.into_iter().filter(|id| next.contains(id)).collect()
        }
    }
}

async fn resolve_search_filter(
    repository: &crate::repository::CatalogRepository,
    blueprint: &crate::model::BlueprintWithAttributes,
    filter: &SearchFilter,
) -> Result<EntitySearchFilter, ApiError> {
    let parts: Vec<_> = filter.field.split('.').collect();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
        return Err(ApiError::invalid_input(
            "filters.field may contain at most three relationship hops and a scalar leaf"
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
                attribute.code == *relationship_name && attribute.value_type == "relationship"
            })
            .ok_or_else(|| {
                ApiError::invalid_input(format!(
                    "filters.field segment '{}' is not a relationship",
                    relationship_name
                ))
            })?;
        relationship_path.push(relationship.code.clone());
        let target = relationship
            .target_blueprint_code
            .as_deref()
            .ok_or_else(|| {
                ApiError::invalid_input(format!(
                    "filters.field relationship '{}' has no target blueprint",
                    relationship_name
                ))
            })?;
        current = repository
            .get_blueprint_by_code(target)
            .await?
            .ok_or_else(|| ApiError::not_found("target blueprint"))?;
    }
    let leaf_field = parts[parts.len() - 1];
    let attribute = current
        .attributes
        .iter()
        .find(|attribute| attribute.code == leaf_field)
        .ok_or_else(|| {
            ApiError::invalid_input(format!(
                "filters.field leaf '{}' is not an attribute",
                leaf_field
            ))
        })?;
    let value_type = attribute.value_type.as_str();
    let valid_operator = match value_type {
        "string" => matches!(filter.operator.as_str(), "eq" | "contains" | "starts_with"),
        "number" | "integer" | "date" | "datetime" | "time" => {
            matches!(filter.operator.as_str(), "eq" | "gt" | "gte" | "lt" | "lte")
        }
        "boolean" => filter.operator == "eq",
        _ => false,
    };
    if !valid_operator {
        return Err(ApiError::invalid_input(format!(
            "operator '{}' is not supported for {} attribute '{}'",
            filter.operator, value_type, filter.field
        )));
    }
    let value = match value_type {
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
    .ok_or_else(|| {
        ApiError::invalid_input(format!(
            "filters.value is invalid for {} attribute '{}'",
            value_type, filter.field
        ))
    })?;
    Ok(EntitySearchFilter {
        field: filter.field.clone(),
        relationship_path,
        leaf_field: attribute.code.clone(),
        operator: filter.operator.clone(),
        value_type: value_type.to_owned(),
        value,
    })
}

fn table_paths(blueprint: &crate::model::BlueprintWithAttributes) -> HashMap<String, String> {
    blueprint
        .table_path_attributes
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type.clone()))
        .collect()
}

/// Only direct relationship hops used by legacy rich table columns need hydration.
fn table_relationships(
    blueprint: &crate::model::BlueprintWithAttributes,
) -> HashMap<String, String> {
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
                .filter(|attribute| attribute.value_type == "relationship")
                .and_then(|attribute| {
                    attribute
                        .target_blueprint_code
                        .as_ref()
                        .map(|target| (relationship.to_owned(), target.clone()))
                })
        })
        .collect()
}

async fn resolve_table_sort(
    repository: &crate::repository::CatalogRepository,
    blueprint: &crate::model::BlueprintWithAttributes,
    sort: Option<&crate::model::SearchSort>,
) -> Result<Option<EntitySearchSort>, ApiError> {
    let Some(sort) = sort else { return Ok(None) };
    let descending = match sort.direction.as_str() {
        "asc" => false,
        "desc" => true,
        _ => {
            return Err(ApiError::invalid_input(
                "sort.direction must be asc or desc".to_owned(),
            ));
        }
    };
    let configured = blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("columns"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|columns| {
            columns.iter().any(|column| {
                column.get("field").and_then(serde_json::Value::as_str) == Some(&sort.field)
            })
        });
    if !configured {
        return Err(ApiError::invalid_input(
            "sort.field must be a configured table column".to_owned(),
        ));
    }
    let parts: Vec<_> = sort.field.split('.').collect();
    if parts.is_empty() || parts.len() > 4 {
        return Err(ApiError::invalid_input(
            "sort.field may contain at most three relationship hops and a scalar leaf".to_owned(),
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
                ApiError::invalid_input("sort.field relationship is invalid".to_owned())
            })?;
        if relationship.cardinality.as_deref() != Some("one") {
            return Err(ApiError::invalid_input(
                "sort.field relationship path must be single-valued".to_owned(),
            ));
        }
        relationship_path.push(relationship.code.clone());
        current = repository
            .get_blueprint_by_code(relationship.target_blueprint_code.as_deref().ok_or_else(
                || {
                    ApiError::invalid_input(
                        "sort.field relationship has no target blueprint".to_owned(),
                    )
                },
            )?)
            .await?
            .ok_or_else(|| ApiError::not_found("target blueprint"))?;
    }
    let leaf_field = parts[parts.len() - 1];
    let value_type = current
        .attributes
        .iter()
        .find(|attribute| attribute.code == leaf_field)
        .map(|attribute| attribute.value_type.clone())
        .ok_or_else(|| {
            ApiError::invalid_input("sort.field must resolve to a scalar table column".to_owned())
        })?;
    if !matches!(
        value_type.as_str(),
        "string" | "number" | "integer" | "boolean" | "date" | "datetime" | "time"
    ) {
        return Err(ApiError::invalid_input(
            "sort.field must resolve to a scalar table column".to_owned(),
        ));
    }
    Ok(Some(EntitySearchSort {
        field: sort.field.clone(),
        relationship_path,
        leaf_field: leaf_field.to_owned(),
        value_type,
        descending,
    }))
}

pub(super) async fn relationship_tree_facet_children(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<RelationshipTreeFacetChildrenRequest>,
) -> Result<Json<RelationshipTreeFacetChildrenResponse>, ApiError> {
    if input.blueprint.code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    let source = repository
        .get_blueprint_by_code(&input.blueprint.code)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint"))?;
    let version = match input.blueprint.version {
        Some(v) => Some(
            repository
                .get_blueprint_by_code_and_version(&input.blueprint.code, v)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint"))?
                .blueprint
                .version,
        ),
        None => None,
    };
    let relationship = source
        .attributes
        .iter()
        .find(|a| a.code == input.source_relationship_field)
        .filter(|a| a.value_type == "relationship")
        .ok_or_else(|| {
            ApiError::invalid_input(
                "source_relationship_field must be a relationship attribute".to_owned(),
            )
        })?;
    let target_code = relationship
        .target_blueprint_code
        .as_deref()
        .ok_or_else(|| {
            ApiError::invalid_input("source_relationship_field has no target blueprint".to_owned())
        })?;
    let target = repository
        .get_blueprint_by_code(target_code)
        .await?
        .ok_or_else(|| ApiError::not_found("target blueprint"))?;
    if let Some(hierarchy_field) = &input.hierarchy_field {
        let hierarchy = target
            .attributes
            .iter()
            .find(|a| a.code == *hierarchy_field)
            .ok_or_else(|| {
                ApiError::invalid_input("hierarchy_field is not an attribute".to_owned())
            })?;
        if hierarchy.value_type != "relationship"
            || hierarchy.target_blueprint_code.as_deref() != Some(target_code)
        {
            return Err(ApiError::invalid_input(
                "hierarchy_field must be a self-targeting relationship".to_owned(),
            ));
        }
    }
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let search_blueprint = match version {
        Some(version) => repository
            .get_blueprint_by_code_and_version(&input.blueprint.code, version)
            .await?
            .expect("selected version was checked above"),
        None => source.clone(),
    };
    // Without a search term, the facet query already scopes its source blueprint
    // and does not need a materialized set of every source entity ID.
    let resolved = match query {
        Some(query) => Some(
            repository
                .resolve_search(&search_blueprint, version, Some(query))
                .await
                .map_err(|error| ApiError::invalid_search_query(error.to_string()))?,
        ),
        None => None,
    };
    Ok(Json(
        repository
            .relationship_tree_facet_children(
                source.blueprint.id,
                version,
                resolved.as_ref().map(|resolved| &resolved.ids),
                &input.source_relationship_field,
                target.blueprint.id,
                input.hierarchy_field.as_deref(),
                input.context_id,
                input.parent_id,
                input.cursor,
                &input.selected_target_ids,
                state.max_relationship_facet_nodes.into(),
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "relationship-facet"
            ))
            .await?,
    ))
}
