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
        ResolvedEntityPreviewResponse, SearchEntitiesRequest,
    },
    repository::{EntitySearchSort, decode_search_cursor},
};
use axum::{
    Json,
    extract::{Extension, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::{collections::HashMap, time::Instant};
use tracing::Instrument;
use uuid::Uuid;
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
    if !input.filters.is_empty() {
        return Err(ApiError::invalid_input(
            "field filters are not supported by v1 search yet".to_owned(),
        ));
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
    let sort = resolve_table_sort(&repository, &current, input.sort.as_ref())
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
        let facet_matching = repository
            .relationship_tree_facet(
                current.blueprint.id,
                &facet.source_relationship_field,
                target.blueprint.id,
                facet.hierarchy_field.as_deref(),
                facet.context_id,
                &facet.selected_target_ids,
                ids,
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "relationship-facet"
            ))
            .await?
            .1;
        if let Some(facet_matching) = facet_matching {
            matching = Some(match matching {
                Some(current_matching) => current_matching
                    .into_iter()
                    .filter(|id| facet_matching.contains(id))
                    .collect(),
                None => facet_matching,
            });
        }
    }
    timing.record("candidate", candidate_started);
    let page_started = Instant::now();
    let (mut items, next_cursor) = match sort.as_ref() {
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
                .await?
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
                .await?
        }
    };
    timing.record("page", page_started);
    for item in &mut items {
        item.schema_outdated = item.blueprint_version != current.blueprint.version;
        item.match_explanations = resolved
            .as_ref()
            .and_then(|resolved| resolved.explanations.get(&item.id))
            .cloned()
            .unwrap_or_default();
    }
    let related_started = Instant::now();
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
    })
    .into_response();
    timing.record("serialize", serialization_started);
    Ok(response)
}

/// Only relationship hops used by rich table columns need page-level hydration.
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
    let (relationship, attribute_code) = match sort.field.split_once('.') {
        Some((relationship, field)) => (Some(relationship.to_owned()), field),
        None => (None, sort.field.as_str()),
    };
    let value_type = match &relationship {
        None => blueprint
            .attributes
            .iter()
            .find(|attribute| attribute.code == attribute_code)
            .map(|attribute| attribute.value_type.clone()),
        Some(relationship) => {
            let source = blueprint
                .attributes
                .iter()
                .find(|attribute| {
                    attribute.code == *relationship && attribute.value_type == "relationship"
                })
                .ok_or_else(|| {
                    ApiError::invalid_input("sort.field relationship is invalid".to_owned())
                })?;
            let target_code = source.target_blueprint_code.as_deref().ok_or_else(|| {
                ApiError::invalid_input(
                    "sort.field relationship has no target blueprint".to_owned(),
                )
            })?;
            let target = repository
                .get_blueprint_by_code(target_code)
                .await?
                .ok_or_else(|| ApiError::not_found("target blueprint"))?;
            target
                .attributes
                .iter()
                .find(|attribute| attribute.code == attribute_code)
                .map(|attribute| attribute.value_type.clone())
        }
    }
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
        relationship,
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
                state.max_relationship_facet_nodes.into(),
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "relationship-facet"
            ))
            .await?,
    ))
}
