use super::{
    AppState,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    model::{
        Entity, EntityIdentity, EntityPreviewPage, EntityPreviewResponse, EntitySearchResponse,
        RelationshipTreeFacetChildrenRequest, RelationshipTreeFacetChildrenResponse,
        ResolvedEntityPreviewResponse, SearchEntitiesRequest,
    },
    repository::decode_search_cursor,
};
use axum::{Json, extract::State};
use serde::Deserialize;
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
    let limit = query.relationship_limit.unwrap_or(10);
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
    let limit = query.limit.unwrap_or(20);
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
    ApiJson(input): ApiJson<SearchEntitiesRequest>,
) -> Result<Json<EntitySearchResponse>, ApiError> {
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
    let limit = input.page.size.unwrap_or(20);
    if limit == 0 || limit > state.max_entity_page_size {
        return Err(ApiError::invalid_input(format!(
            "page.size must be between 1 and {}",
            state.max_entity_page_size
        )));
    }
    let current = repository
        .get_blueprint_by_code(code)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint"))?;
    let selected = match input.blueprint.version {
        Some(version) => Some(
            repository
                .get_blueprint_by_code_and_version(code, version)
                .await?
                .map(|b| b.blueprint.version)
                .ok_or_else(|| ApiError::not_found("blueprint"))?,
        ),
        None => None,
    };
    let cursor = match input.page.cursor.as_deref() {
        Some(cursor) => Some(
            decode_search_cursor(cursor)
                .ok_or_else(|| ApiError::invalid_input("page.cursor is invalid".to_owned()))?,
        ),
        None => None,
    };
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let matching = match input.relationship_tree_facet {
        Some(facet) => {
            let source = current
                .attributes
                .iter()
                .find(|a| a.code == facet.source_relationship_field)
                .ok_or_else(|| {
                    ApiError::invalid_input(
                        "relationship_tree_facet.source_relationship_field is not an attribute"
                            .to_owned(),
                    )
                })?;
            if source.value_type != "relationship" {
                return Err(ApiError::invalid_input(
                    "relationship_tree_facet.source_relationship_field must be a relationship"
                        .to_owned(),
                ));
            }
            let target_code = source.target_blueprint_code.as_deref().ok_or_else(|| {
                ApiError::invalid_input(
                    "relationship_tree_facet.source_relationship_field has no target blueprint"
                        .to_owned(),
                )
            })?;
            let target = repository
                .get_blueprint_by_code(target_code)
                .await?
                .ok_or_else(|| ApiError::not_found("target blueprint"))?;
            let hierarchy = target
                .attributes
                .iter()
                .find(|a| a.code == facet.hierarchy_field)
                .ok_or_else(|| {
                    ApiError::invalid_input(
                        "relationship_tree_facet.hierarchy_field is not an attribute".to_owned(),
                    )
                })?;
            if hierarchy.value_type != "relationship"
                || hierarchy.target_blueprint_code.as_deref() != Some(target_code)
            {
                return Err(ApiError::invalid_input(
                    "relationship_tree_facet.hierarchy_field must be a self-targeting relationship"
                        .to_owned(),
                ));
            }
            let ids = repository
                .search_matching_entity_ids(current.blueprint.id, selected, query)
                .await?;
            if facet.selected_target_ids.is_empty() {
                None
            } else {
                repository
                    .relationship_tree_facet(
                        current.blueprint.id,
                        &facet.source_relationship_field,
                        target.blueprint.id,
                        &facet.hierarchy_field,
                        facet.context_id,
                        &facet.selected_target_ids,
                        &ids,
                    )
                    .await?
                    .1
            }
        }
        None => None,
    };
    let (mut items, next_cursor) = repository
        .search_entity_previews(
            current.blueprint.id,
            selected,
            query,
            limit.into(),
            cursor,
            matching.as_deref(),
        )
        .await?;
    for item in &mut items {
        item.schema_outdated = item.blueprint_version != current.blueprint.version
    }
    Ok(Json(EntitySearchResponse {
        blueprint: current,
        items,
        next_cursor,
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
    let hierarchy = target
        .attributes
        .iter()
        .find(|a| a.code == input.hierarchy_field)
        .ok_or_else(|| ApiError::invalid_input("hierarchy_field is not an attribute".to_owned()))?;
    if hierarchy.value_type != "relationship"
        || hierarchy.target_blueprint_code.as_deref() != Some(target_code)
    {
        return Err(ApiError::invalid_input(
            "hierarchy_field must be a self-targeting relationship".to_owned(),
        ));
    }
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    Ok(Json(
        repository
            .relationship_tree_facet_children(
                source.blueprint.id,
                version,
                query,
                &input.source_relationship_field,
                target.blueprint.id,
                &input.hierarchy_field,
                input.context_id,
                input.parent_id,
                input.cursor,
                state.max_relationship_facet_nodes.into(),
            )
            .await?,
    ))
}
