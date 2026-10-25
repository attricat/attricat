use super::{
    AppState, RequestTiming,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    constants::{DEFAULT_PAGE_SIZE, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS},
    model::{
        Record, RecordIdentity, RecordPreviewPage, RecordPreviewResponse, RecordSearchResponse,
        RelationshipTreeFacetChildrenRequest, RelationshipTreeFacetChildrenResponse,
        ResolvedRecordPreviewResponse, SearchRecordsRequest, SearchResultVersionScope,
    },
    repository::{
        EXPORT_STATUSES, MAX_SEARCH_FILTERS, RecordSearchSort, RelationshipFacetHop,
        RepositoryError, SearchContext, SearchScope, SearchScopeError, SearchScopeRequest,
        decode_search_cursor, intersect_record_ids, valid_connection_key,
    },
};
use attricat_validation::saved_search;
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
pub(super) async fn get_record(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Record>, ApiError> {
    repository
        .get_record(record_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("record"))
}
pub(super) async fn get_preview(
    State(state): State<AppState>,
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PreviewQuery>,
) -> Result<Json<RecordPreviewResponse>, ApiError> {
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
    let (record, context) = repository
        .with_authorization_actor(principal.actor())
        .preview(record_id, depth, limit.into())
        .await?
        .ok_or_else(|| ApiError::not_found("record"))?;
    Ok(Json(RecordPreviewResponse {
        record: RecordIdentity {
            id: record.id,
            blueprint_id: record.blueprint_id,
            blueprint_version: record.blueprint_version,
            is_sample: record.is_sample,
        },
        context,
    }))
}
pub(super) async fn get_record_hierarchy(
    State(state): State<AppState>,
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HierarchyQuery>,
) -> Result<Json<crate::model::RecordHierarchyResponse>, ApiError> {
    repository
        .with_authorization_actor(principal.actor())
        .hierarchy(
            record_id,
            query.context_id,
            &query.field,
            state.max_preview_relationship_depth,
        )
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("record"))
}
pub(super) async fn get_resolved_preview(
    State(_state): State<AppState>,
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ResolvedPreviewQuery>,
) -> Result<Json<ResolvedRecordPreviewResponse>, ApiError> {
    repository
        .with_authorization_actor(principal.actor())
        .resolved_preview(record_id, query.context_id, 1)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("record"))
}
pub(super) async fn list_previews(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<ListPreviewsQuery>,
) -> Result<Json<RecordPreviewPage>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 || limit > state.max_record_page_size {
        return Err(ApiError::invalid_input(format!(
            "limit must be between 1 and {}",
            state.max_record_page_size
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
pub(super) async fn search_record_previews(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(caller, _): super::auth::AuthenticatedPrincipal,
    Extension(timing): Extension<RequestTiming>,
    ApiJson(input): ApiJson<SearchRecordsRequest>,
) -> Result<Response, ApiError> {
    let code = &input.blueprint.code;
    // Each filter and facet resolves with its own queries, so bound them all;
    // the search scope bounds the attribute and relationship filters.
    if input.relationship_tree_facets.len() > MAX_SEARCH_FILTERS {
        return Err(ApiError::invalid_input(format!(
            "relationship_tree_facets must contain at most {MAX_SEARCH_FILTERS} items"
        )));
    }
    let limit = input.page.size.unwrap_or(DEFAULT_PAGE_SIZE);
    if limit == 0 || limit > state.max_record_page_size {
        return Err(ApiError::invalid_input(format!(
            "page.size must be between 1 and {}",
            state.max_record_page_size
        )));
    }
    // The export status filter is a predicate of the page query, so it never
    // materializes the records it matches.
    if let Some(export) = &input.export_status {
        if !EXPORT_STATUSES.contains(&export.status.as_str()) {
            return Err(ApiError::invalid_input("unknown export status".to_owned()));
        }
        if export
            .connection
            .as_deref()
            .is_some_and(|connection| !valid_connection_key(connection))
        {
            return Err(ApiError::invalid_input("invalid connection key".to_owned()));
        }
    }
    let candidate_started = Instant::now();
    let SearchScope {
        current,
        selected,
        search_blueprint,
        context,
        explanations,
        mut matching,
    } = repository
        .resolve_search_scope(SearchScopeRequest {
            blueprint: &input.blueprint,
            query: input.query.as_deref(),
            filters: &input.filters,
            relationship_filters: &input.relationship_filters,
            context_code: input
                .context_code
                .as_deref()
                .unwrap_or(super::DEFAULT_CONTEXT_CODE),
            actor: caller,
        })
        .await
        .map_err(search_scope_error)?;
    for facet in input.relationship_tree_facets {
        let (target, source_path) = resolve_tree_facet_target(
            &repository,
            &search_blueprint,
            &current,
            facet.source_relationship_field.as_deref(),
            facet.hierarchy_field.as_deref(),
            "relationship_tree_facets.",
        )
        .await?;
        let facet_candidates = matching
            .as_ref()
            .map(|ids| ids.iter().copied().collect::<HashSet<_>>());
        let facet_matching = repository
            .relationship_tree_facet_matches(
                current.blueprint.id,
                &source_path,
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
            .await?;
        if let Some(facet_matching) = facet_matching {
            matching = Some(intersect_record_ids(matching, facet_matching));
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
            input.export_status.as_ref(),
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
                    .search_record_previews_sorted(
                        current.blueprint.id,
                        page_blueprint_version,
                        limit.into(),
                        input.page.cursor.as_deref(),
                        matching.as_deref(),
                        &input.system_tags,
                        input.outdated,
                        input.export_status.as_ref(),
                        current.blueprint.version,
                        sort,
                    )
                    .instrument(tracing::info_span!("sql.operation", label = "records-page"))
                    .await
            }
            None => {
                repository
                    .search_record_previews(
                        current.blueprint.id,
                        selected,
                        None,
                        limit.into(),
                        cursor,
                        matching.as_deref(),
                        &input.system_tags,
                        input.outdated,
                        input.export_status.as_ref(),
                        current.blueprint.version,
                    )
                    .instrument(tracing::info_span!("sql.operation", label = "records-page"))
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
            .count_record_previews(
                current.blueprint.id,
                None,
                None,
                &[],
                true,
                None,
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
        item.match_explanations = explanations.get(&item.id).cloned().unwrap_or_default();
    }
    let related_started = Instant::now();
    let table_paths = table_paths(&result_blueprint);
    let previews = input.include_previews || has_extension_table_renderer(&result_blueprint);
    let relationships = if previews {
        table_relationships(&result_blueprint)
    } else {
        HashMap::new()
    };
    repository
        .hydrate_table_values(&mut items, &table_paths, &relationships, &context)
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "table-values-hydrate"
        ))
        .await?;
    timing.record("related", related_started);
    if !previews {
        for item in &mut items {
            item.preview = serde_json::Value::Null;
        }
    }
    let serialization_started = Instant::now();
    let response = Json(RecordSearchResponse {
        blueprint: input.include_blueprint.then_some(result_blueprint),
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

fn search_scope_error(error: SearchScopeError) -> ApiError {
    match error {
        SearchScopeError::UnknownBlueprint => ApiError::not_found("blueprint"),
        SearchScopeError::Invalid { message, .. } => ApiError::invalid_input(message),
        SearchScopeError::Query { error, .. } => map_search_error(error),
        SearchScopeError::Repository(error) => error.into(),
    }
}

/// The blueprint whose records are a tree facet's nodes, and the hops its
/// sources reach them by: the source relationship or path's last target, or
/// `current` itself with no hops when the facet has no source relationship
/// and walks the searched blueprint's own hierarchy. `prefix` names the
/// request field in errors.
async fn resolve_tree_facet_target(
    repository: &crate::repository::AttricatRepository,
    source: &crate::model::BlueprintWithAttributes,
    current: &crate::model::BlueprintWithAttributes,
    source_field: Option<&str>,
    hierarchy_field: Option<&str>,
    prefix: &str,
) -> Result<
    (
        crate::model::BlueprintWithAttributes,
        Vec<RelationshipFacetHop>,
    ),
    ApiError,
> {
    let invalid = |message: &str| ApiError::invalid_input(format!("{prefix}{message}"));
    let (target, source_path) = match source_field {
        Some(source_field) => {
            // A dotted path follows relationships like table column paths.
            let hops: Vec<_> = source_field.split('.').collect();
            if hops.len() > saved_search::MAX_RELATIONSHIP_HOPS {
                return Err(invalid(
                    "source_relationship_field must follow at most three relationship hops",
                ));
            }
            let mut target = source.clone();
            let mut source_path = Vec::with_capacity(hops.len());
            for hop in &hops {
                let relationship = target
                    .attributes
                    .iter()
                    .find(|a| a.code == *hop)
                    .filter(|a| a.value_type == "relationship")
                    .ok_or_else(|| {
                        invalid("source_relationship_field must be a relationship attribute")
                    })?;
                let target_code = relationship
                    .target_blueprint_code
                    .clone()
                    .ok_or_else(|| invalid("source_relationship_field has no target blueprint"))?;
                if !saved_search::browse_path_hop(
                    &relationship.value_type,
                    Some(&target_code),
                    relationship.cardinality.as_deref(),
                    hops.len(),
                ) {
                    return Err(invalid(
                        "source_relationship_field path hops must have cardinality \"one\"",
                    ));
                }
                let field = relationship.code.clone();
                target = repository
                    .get_blueprint_by_code(&target_code)
                    .instrument(tracing::info_span!(
                        "sql.operation",
                        label = "blueprint-load"
                    ))
                    .await?
                    .ok_or_else(|| ApiError::not_found("target blueprint"))?;
                source_path.push(RelationshipFacetHop {
                    field,
                    target_blueprint_id: target.blueprint.id,
                });
            }
            (target, source_path)
        }
        None if hierarchy_field.is_none() => {
            return Err(invalid(
                "hierarchy_field is required without source_relationship_field",
            ));
        }
        None => (current.clone(), Vec::new()),
    };
    if let Some(hierarchy_field) = hierarchy_field {
        let hierarchy = target
            .attributes
            .iter()
            .find(|a| a.code == hierarchy_field)
            .ok_or_else(|| invalid("hierarchy_field is not an attribute"))?;
        if hierarchy.value_type != "relationship"
            || hierarchy.target_blueprint_code.as_deref() != Some(&target.blueprint.code)
        {
            return Err(invalid(
                "hierarchy_field must be a self-targeting relationship",
            ));
        }
    }
    Ok((target, source_path))
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
/// Whether a table column renders through an extension, whose cell context
/// carries the row's preview and related records.
fn has_extension_table_renderer(blueprint: &crate::model::BlueprintWithAttributes) -> bool {
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
    repository: &crate::repository::AttricatRepository,
    blueprint: &crate::model::BlueprintWithAttributes,
    sort: Option<&crate::model::SearchSort>,
    effective_source_version: Option<i64>,
    context: &SearchContext,
) -> Result<Option<RecordSearchSort>, ApiError> {
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
        return Ok(Some(RecordSearchSort {
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
        return Ok(Some(RecordSearchSort {
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
    if parts.is_empty() || parts.len() > saved_search::MAX_RELATIONSHIP_HOPS + 1 {
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
    let max_nodes = state.max_relationship_facet_nodes;
    if input.include_ids.len() as u64 > u64::from(max_nodes) {
        return Err(ApiError::invalid_input(format!(
            "include_ids must not name more than {max_nodes} nodes"
        )));
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
    let (target, source_path) = resolve_tree_facet_target(
        &repository,
        &source,
        &source,
        input.source_relationship_field.as_deref(),
        input.hierarchy_field.as_deref(),
        "",
    )
    .await?;
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let search_blueprint = published.unwrap_or_else(|| source.clone());
    // Without a search term, the facet query already scopes its source blueprint
    // and does not need a materialized set of every source record ID.
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
                &source_path,
                target.blueprint.id,
                input.hierarchy_field.as_deref(),
                input.context_id,
                input.parent_id,
                input.cursor,
                &input.selected_target_ids,
                &input.include_ids,
                state.max_relationship_facet_nodes.into(),
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "relationship-facet"
            ))
            .await?,
    ))
}

/// Most records one publication status lookup may name.
const MAX_RECORD_PUBLICATION_IDS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordPublicationsRequest {
    record_ids: Vec<Uuid>,
}

#[derive(serde::Serialize)]
pub(super) struct RecordPublicationsResponse {
    items: Vec<RecordPublications>,
}

#[derive(serde::Serialize)]
struct RecordPublications {
    record_id: Uuid,
    publications: Vec<crate::model::RecordPublicationStatus>,
}

/// Channel publication statuses of several records, such as the rows of an
/// Explorer page. Unreadable, deleted and unknown IDs are omitted alike.
pub(super) async fn record_publications(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(caller, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiJson(input): ApiJson<RecordPublicationsRequest>,
) -> Result<Json<RecordPublicationsResponse>, ApiError> {
    let mut record_ids = input.record_ids;
    record_ids.sort_unstable();
    record_ids.dedup();
    if record_ids.is_empty() || record_ids.len() > MAX_RECORD_PUBLICATION_IDS {
        return Err(ApiError::invalid_input(format!(
            "record_ids must contain between 1 and {MAX_RECORD_PUBLICATION_IDS} distinct IDs"
        )));
    }
    let readable = repository
        .authorized_record_ids(caller, workspace, "records.read", &record_ids)
        .await?;
    record_ids.retain(|id| readable.contains(id));
    let mut statuses = repository
        .publication_statuses_by_record(&record_ids)
        .await?;
    Ok(Json(RecordPublicationsResponse {
        items: record_ids
            .into_iter()
            .filter_map(|record_id| {
                statuses
                    .remove(&record_id)
                    .map(|publications| RecordPublications {
                        record_id,
                        publications,
                    })
            })
            .collect(),
    }))
}

/// Most records one label lookup may name.
const MAX_RECORD_LABEL_IDS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordLabelsRequest {
    record_ids: Vec<Uuid>,
}

#[derive(serde::Serialize)]
pub(super) struct RecordLabelsResponse {
    items: Vec<crate::model::RecordLabel>,
}

/// Names records that other views only know by ID. Unreadable, deleted and
/// unknown IDs are omitted alike, so the response does not reveal which exist.
pub(super) async fn record_labels(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(caller, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiJson(input): ApiJson<RecordLabelsRequest>,
) -> Result<Json<RecordLabelsResponse>, ApiError> {
    let mut record_ids = input.record_ids;
    record_ids.sort_unstable();
    record_ids.dedup();
    if record_ids.is_empty() || record_ids.len() > MAX_RECORD_LABEL_IDS {
        return Err(ApiError::invalid_input(format!(
            "record_ids must contain between 1 and {MAX_RECORD_LABEL_IDS} distinct IDs"
        )));
    }
    let readable = repository
        .authorized_record_ids(caller, workspace, "records.read", &record_ids)
        .await?;
    record_ids.retain(|id| readable.contains(id));
    Ok(Json(RecordLabelsResponse {
        items: repository.record_labels(&record_ids).await?,
    }))
}
