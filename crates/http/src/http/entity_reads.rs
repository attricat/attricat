use super::{
    AppState, RequestTiming,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    constants::{DEFAULT_PAGE_SIZE, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS},
    model::{
        Entity, EntityIdentity, EntityPreviewPage, EntityPreviewResponse, EntitySearchResponse,
        RelationshipFilter, RelationshipTreeFacetChildrenRequest,
        RelationshipTreeFacetChildrenResponse, ResolvedEntityPreviewResponse,
        SearchEntitiesRequest, SearchFilter, SearchResultVersionScope,
    },
    repository::{
        EntityRelationshipFilter, EntitySearchFilter, EntitySearchSort, RepositoryError,
        SearchContext, decode_search_cursor,
    },
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

fn map_search_error(error: RepositoryError) -> ApiError {
    match error {
        // Search budgets keep their own codes. Infrastructure failures are
        // server errors; their text (SQL, pool state) must never be echoed to
        // the client as a query problem.
        error @ (RepositoryError::RelationshipSearchBudgetExceeded { .. }
        | RepositoryError::RelationshipSearchTimedOut
        | RepositoryError::Database(_)
        | RepositoryError::Task(_)) => error.into(),
        error => ApiError::invalid_search_query(error.to_string()),
    }
}
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
    principal: super::auth::AuthenticatedPrincipal,
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
    let (entity, context) = repository
        .with_authorization_actor(principal.actor())
        .preview(entity_id, depth, limit.into())
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    Ok(Json(EntityPreviewResponse {
        entity: EntityIdentity {
            id: entity.id,
            blueprint_id: entity.blueprint_id,
            blueprint_version: entity.blueprint_version,
            is_sample: entity.is_sample,
        },
        context,
    }))
}
pub(super) async fn get_entity_hierarchy(
    State(state): State<AppState>,
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HierarchyQuery>,
) -> Result<Json<crate::model::EntityHierarchyResponse>, ApiError> {
    repository
        .with_authorization_actor(principal.actor())
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
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ResolvedPreviewQuery>,
) -> Result<Json<ResolvedEntityPreviewResponse>, ApiError> {
    repository
        .with_authorization_actor(principal.actor())
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
    super::auth::AuthenticatedPrincipal(caller, _): super::auth::AuthenticatedPrincipal,
    Extension(timing): Extension<RequestTiming>,
    ApiJson(input): ApiJson<SearchEntitiesRequest>,
) -> Result<Response, ApiError> {
    let code = &input.blueprint.code;
    if code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    // Each filter and facet resolves with its own queries, so bound them all.
    for (field, len) in [
        ("filters", input.filters.len()),
        ("relationship_filters", input.relationship_filters.len()),
        (
            "relationship_tree_facets",
            input.relationship_tree_facets.len(),
        ),
    ] {
        if len > MAX_SEARCH_FILTERS {
            return Err(ApiError::invalid_input(format!(
                "{field} must contain at most {MAX_SEARCH_FILTERS} items"
            )));
        }
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
    let (selected, search_blueprint) = match input.blueprint.version {
        // The latest published revision is the requested one.
        Some(version) if version == current.blueprint.version => (Some(version), current.clone()),
        Some(version) => {
            let published = repository
                .get_published_blueprint_by_code_and_version(code, version)
                .instrument(tracing::info_span!(
                    "sql.operation",
                    label = "blueprint-load"
                ))
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint"))?;
            (Some(published.blueprint.version), published)
        }
        None => (None, current.clone()),
    };
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let context = repository
        .search_context(input.context_code.as_deref().unwrap_or("default"))
        .await?
        .ok_or_else(|| ApiError::invalid_input("context_code is not a context".to_owned()))?;
    let mut filters = Vec::with_capacity(input.filters.len());
    for filter in &input.filters {
        filters.push(resolve_search_filter(&repository, &search_blueprint, filter, caller).await?);
    }
    let mut relationship_filters = Vec::with_capacity(input.relationship_filters.len());
    for filter in &input.relationship_filters {
        relationship_filters
            .push(resolve_relationship_filter(&repository, &search_blueprint, filter).await?);
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
                .map_err(map_search_error)?,
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
            .filter_entity_ids(current.blueprint.id, selected, &filters, &context)
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "attribute-filter"
            ))
            .await?;
        matching = Some(intersect_entity_ids(matching, filtered));
    }
    if !relationship_filters.is_empty() {
        let filtered = repository
            .filter_relationship_entity_ids(
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
    // Totals are intentionally limited and only calculated for a first page.
    // Cursor pages retain the first response's total in the client cache.
    let include_total = input.include_total && input.page.cursor.is_none();
    let (matching_versions, total_count) = repository
        .search_result_versions_and_count(
            current.blueprint.id,
            selected,
            matching.as_deref(),
            &input.system_tags,
            input.outdated,
            current.blueprint.version,
            include_total.then_some(SEARCH_TOTAL_COUNT_CAP + 1),
        )
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "result-version-scope"
        ))
        .await?;
    let result_version_scope = match matching_versions.as_slice() {
        [] => SearchResultVersionScope::Empty,
        [version] => SearchResultVersionScope::Single { version: *version },
        _ => SearchResultVersionScope::Multiple,
    };
    let effective_source_version = match matching_versions.as_slice() {
        [version] => Some(*version),
        _ => selected,
    };
    let sort_uses_relationship = input
        .sort
        .as_ref()
        .is_some_and(|sort| sort.field.contains('.'));
    if selected.is_none() && sort_uses_relationship && matching_versions.len() != 1 {
        return Err(ApiError::relationship_sort_requires_single_version());
    }
    let result_blueprint = match effective_source_version {
        Some(version)
            if version != current.blueprint.version
                && version != search_blueprint.blueprint.version =>
        {
            repository
                .get_published_blueprint_by_code_and_version(code, version)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint"))?
        }
        _ => search_blueprint.clone(),
    };
    let sort = resolve_table_sort(
        &repository,
        &result_blueprint,
        input.sort.as_ref(),
        sort_uses_relationship
            .then_some(effective_source_version)
            .flatten(),
        &context,
    )
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
    timing.record("candidate", candidate_started);
    let page_blueprint_version = if sort_uses_relationship {
        effective_source_version
    } else {
        selected
    };
    let page_started = Instant::now();
    let page = async {
        match sort.as_ref() {
            Some(sort) => {
                repository
                    .search_entity_previews_sorted(
                        current.blueprint.id,
                        page_blueprint_version,
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
    // The total was counted with the result versions.
    let total = async { Ok::<_, crate::repository::RepositoryError>(total_count) };
    let hidden_outdated = async {
        if selected != Some(current.blueprint.version) || input.page.cursor.is_some() {
            return Ok(None);
        }
        repository
            .count_entity_previews(
                current.blueprint.id,
                None,
                None,
                &[],
                true,
                current.blueprint.version,
                SEARCH_TOTAL_COUNT_CAP + 1,
            )
            .await
            .map(Some)
    };
    let ((mut items, next_cursor), total_count, hidden_outdated_count) =
        tokio::try_join!(page, total, hidden_outdated)?;
    timing.record("page", page_started);
    let total_count_capped = total_count.is_some_and(|count| count > SEARCH_TOTAL_COUNT_CAP);
    let total_count = total_count.map(|count| count.min(SEARCH_TOTAL_COUNT_CAP));
    let hidden_outdated_count_capped =
        hidden_outdated_count.is_some_and(|count| count > SEARCH_TOTAL_COUNT_CAP);
    let hidden_outdated_count =
        hidden_outdated_count.map(|count| count.min(SEARCH_TOTAL_COUNT_CAP));
    for item in &mut items {
        item.schema_outdated = item.blueprint_version != current.blueprint.version;
        item.match_explanations = resolved
            .as_ref()
            .and_then(|resolved| resolved.explanations.get(&item.id))
            .cloned()
            .unwrap_or_default();
    }
    let related_started = Instant::now();
    let table_paths = table_paths(&result_blueprint);
    repository
        .hydrate_table_path_values(&mut items, &table_paths, &context)
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "table-path-hydrate"
        ))
        .await?;
    repository
        .hydrate_related_table_previews(
            &mut items,
            &table_relationships(&result_blueprint),
            &context,
        )
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "related-hydrate"
        ))
        .await?;
    timing.record("related", related_started);
    let serialization_started = Instant::now();
    let response = Json(EntitySearchResponse {
        blueprint: result_blueprint,
        items,
        next_cursor,
        total_count,
        total_count_capped,
        result_version_scope,
        hidden_outdated_count,
        hidden_outdated_count_capped,
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
    caller: Uuid,
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
    let (attribute_code, value_type, reusable, value_schema) = match current
        .attributes
        .iter()
        .find(|attribute| attribute.code == leaf_field)
    {
        Some(attribute) => (
            attribute.code.clone(),
            attribute.value_type.as_str().to_owned(),
            false,
            attribute.value_schema.clone(),
        ),
        None if relationship_path.is_empty() => {
            let reusable = repository
                .attached_reusable_attribute_for_blueprint(
                    blueprint.blueprint.id,
                    blueprint.blueprint.version,
                    leaf_field,
                )
                .await?
                .filter(|attribute| attribute.searchable)
                .ok_or_else(|| {
                    ApiError::invalid_input(format!(
                        "filters.field leaf '{}' is not a searchable attribute",
                        leaf_field
                    ))
                })?;
            (
                format!("{}:{}", reusable.namespace, reusable.code),
                reusable.value_type,
                true,
                reusable.value_schema,
            )
        }
        None => {
            return Err(ApiError::invalid_input(format!(
                "filters.field leaf '{}' is not an attribute",
                leaf_field
            )));
        }
    };
    let value_type = value_type.as_str();
    // "Assigned to me" matches the caller and each of the caller's teams.
    if let Some(value) = repository
        .current_user_filter_value(
            value_schema.as_ref(),
            &filter.operator,
            &filter.value,
            caller,
        )
        .await?
    {
        return Ok(EntitySearchFilter {
            field: filter.field.clone(),
            relationship_path,
            leaf_field: attribute_code,
            reusable,
            operator: crate::repository::SEARCH_FILTER_EQ_ANY.to_owned(),
            value_type: value_type.to_owned(),
            value,
        });
    }
    let presence = filter.operator == catalog_validation::saved_search::FILTER_OPERATOR_IS_SET;
    let valid_operator = match value_type {
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
        return Err(ApiError::invalid_input(format!(
            "operator '{}' is not supported for {} attribute '{}'",
            filter.operator, value_type, filter.field
        )));
    }
    let value = if presence {
        filter.value.as_bool().map(|value| value.to_string())
    } else {
        match value_type {
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
        ApiError::invalid_input(format!(
            "filters.value is invalid for {} attribute '{}'",
            value_type, filter.field
        ))
    })?;
    Ok(EntitySearchFilter {
        field: filter.field.clone(),
        relationship_path,
        leaf_field: attribute_code,
        reusable,
        operator: filter.operator.clone(),
        value_type: value_type.to_owned(),
        value,
    })
}

async fn resolve_relationship_filter(
    repository: &crate::repository::CatalogRepository,
    blueprint: &crate::model::BlueprintWithAttributes,
    filter: &RelationshipFilter,
) -> Result<EntityRelationshipFilter, ApiError> {
    if filter.selected_target_ids.is_empty() {
        return Err(ApiError::invalid_input(
            "relationship_filters.selected_target_ids must not be empty".to_owned(),
        ));
    }
    let path: Vec<_> = filter.field.split('.').collect();
    if path.is_empty() || path.len() > 3 || path.iter().any(|segment| segment.is_empty()) {
        return Err(ApiError::invalid_input(
            "relationship_filters.field must contain one to three relationship hops".to_owned(),
        ));
    }
    let mut current = blueprint.clone();
    let mut relationship_path = Vec::with_capacity(path.len());
    for relationship_name in path {
        let relationship = current
            .attributes
            .iter()
            .find(|attribute| {
                attribute.code == relationship_name && attribute.value_type == "relationship"
            })
            .ok_or_else(|| {
                ApiError::invalid_input(format!(
                    "relationship_filters.field segment '{}' is not a relationship",
                    relationship_name
                ))
            })?;
        let target = relationship
            .target_blueprint_code
            .as_deref()
            .ok_or_else(|| {
                ApiError::invalid_input(format!(
                    "relationship_filters.field relationship '{}' has no target blueprint",
                    relationship_name
                ))
            })?;
        relationship_path.push(relationship.code.clone());
        current = repository
            .get_blueprint_by_code(target)
            .await?
            .ok_or_else(|| ApiError::not_found("target blueprint"))?;
    }
    Ok(EntityRelationshipFilter {
        field: filter.field.clone(),
        relationship_path,
        selected_target_ids: filter.selected_target_ids.clone(),
    })
}

fn table_paths(blueprint: &crate::model::BlueprintWithAttributes) -> HashMap<String, String> {
    let mut paths: HashMap<_, _> = blueprint
        .table_path_attributes
        .iter()
        .map(|attribute| (attribute.code.clone(), attribute.value_type.clone()))
        .collect();
    if let Some(fields) = blueprint
        .blueprint
        .views
        .get("table")
        .and_then(|table| table.get("fields"))
        .and_then(serde_json::Value::as_array)
    {
        for field in fields.iter().filter_map(serde_json::Value::as_str) {
            if let Some(attribute) = blueprint
                .attributes
                .iter()
                .find(|attribute| attribute.code == field && attribute.value_type != "relationship")
            {
                paths.insert(attribute.code.clone(), attribute.value_type.clone());
            }
        }
    }
    paths
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
    effective_source_version: Option<i64>,
    context: &SearchContext,
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
    if sort.field == "publication_status" {
        let code = sort.context_code.as_deref().ok_or_else(|| {
            ApiError::invalid_input(
                "sort.context_code is required for publication_status".to_owned(),
            )
        })?;
        let channel = repository
            .list_publication_channels()
            .await?
            .into_iter()
            .find(|channel| channel.context_code == code && channel.enabled)
            .ok_or_else(|| {
                ApiError::invalid_input(
                    "sort.context_code must be an enabled publication channel".to_owned(),
                )
            })?;
        return Ok(Some(EntitySearchSort {
            field: sort.field.clone(),
            relationship_path: Vec::new(),
            leaf_field: sort.field.clone(),
            leaf_blueprint_id: blueprint.blueprint.id,
            value_type: "integer".to_owned(),
            descending,
            effective_source_version,
            publication_context_id: Some(channel.context_id),
            context: context.clone(),
        }));
    }
    if sort.context_code.is_some() {
        return Err(ApiError::invalid_input(
            "sort.context_code is only valid for publication_status".to_owned(),
        ));
    }
    if sort.field == "blueprint_version" {
        return Ok(Some(EntitySearchSort {
            field: sort.field.clone(),
            relationship_path: Vec::new(),
            leaf_field: sort.field.clone(),
            leaf_blueprint_id: blueprint.blueprint.id,
            value_type: "integer".to_owned(),
            descending,
            effective_source_version,
            publication_context_id: None,
            context: context.clone(),
        }));
    }
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
        leaf_blueprint_id: current.blueprint.id,
        value_type,
        descending,
        effective_source_version,
        publication_context_id: None,
        context: context.clone(),
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
    let published = match input.blueprint.version {
        Some(v) if v == source.blueprint.version => Some(source.clone()),
        Some(v) => Some(
            repository
                .get_published_blueprint_by_code_and_version(&input.blueprint.code, v)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint"))?,
        ),
        None => None,
    };
    let version = published.as_ref().map(|b| b.blueprint.version);
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
    let search_blueprint = published.unwrap_or_else(|| source.clone());
    // Without a search term, the facet query already scopes its source blueprint
    // and does not need a materialized set of every source entity ID.
    let resolved = match query {
        Some(query) => Some(
            repository
                .resolve_search(&search_blueprint, version, Some(query))
                .await
                .map_err(map_search_error)?,
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

/// Most entities one label lookup may name.
const MAX_ENTITY_LABEL_IDS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EntityLabelsRequest {
    entity_ids: Vec<Uuid>,
}

#[derive(serde::Serialize)]
pub(super) struct EntityLabelsResponse {
    items: Vec<crate::model::EntityLabel>,
}

/// Names entities that other views only know by ID. Unreadable, deleted and
/// unknown IDs are omitted alike, so the response does not reveal which exist.
pub(super) async fn entity_labels(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(caller, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiJson(input): ApiJson<EntityLabelsRequest>,
) -> Result<Json<EntityLabelsResponse>, ApiError> {
    let mut entity_ids = input.entity_ids;
    entity_ids.sort_unstable();
    entity_ids.dedup();
    if entity_ids.is_empty() || entity_ids.len() > MAX_ENTITY_LABEL_IDS {
        return Err(ApiError::invalid_input(format!(
            "entity_ids must contain between 1 and {MAX_ENTITY_LABEL_IDS} distinct IDs"
        )));
    }
    let readable = repository
        .authorized_entity_ids(caller, workspace, "entities.read", &entity_ids)
        .await?;
    entity_ids.retain(|id| readable.contains(id));
    Ok(Json(EntityLabelsResponse {
        items: repository.entity_labels(&entity_ids).await?,
    }))
}
