use super::{
    AppState, RequestTiming,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    constants::{DEFAULT_PAGE_SIZE, DEFAULT_PREVIEW_RELATIONSHIP_ITEMS},
    model::{
        Record, RecordIdentity, RecordPreviewPage, RecordPreviewResponse,
        RelationshipTreeFacetChildrenRequest, RelationshipTreeFacetChildrenResponse,
        ResolvedRecordPreviewResponse, SearchRecordsRequest,
    },
    repository::{
        EXPORT_STATUSES, MAX_SEARCH_FILTERS, RelationshipFacetHop, RepositoryError,
        SearchPageRequest, SearchPhase, SearchScopeError, SearchScopeRequest, intersect_record_ids,
        valid_connection_key,
    },
};
use attricat_domain::permissions::Permission;
use attricat_domain::value_type::ValueType;
use attricat_validation::saved_search;
use axum::{
    Json,
    extract::{Extension, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::{collections::HashSet, time::Instant};
use tracing::Instrument;
use uuid::Uuid;

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
    let mut scope = repository
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
            &scope.search_blueprint,
            &scope.current,
            facet.source_relationship_field.as_deref(),
            facet.hierarchy_field.as_deref(),
            "relationship_tree_facets.",
        )
        .await?;
        let facet_candidates = scope
            .matching
            .as_ref()
            .map(|ids| ids.iter().copied().collect::<HashSet<_>>());
        let facet_matching = repository
            .relationship_tree_facet_matches(
                scope.current.blueprint.id,
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
            scope.matching = Some(intersect_record_ids(scope.matching.take(), facet_matching));
        }
    }
    let mut response = repository
        .search_page(
            &scope,
            SearchPageRequest {
                code,
                system_tags: &input.system_tags,
                outdated: input.outdated,
                export_status: input.export_status.as_ref(),
                sort: input.sort.as_ref(),
                cursor: input.page.cursor.as_deref(),
                limit: limit.into(),
                include_total: input.include_total,
                include_previews: input.include_previews,
            },
            &|phase| match phase {
                SearchPhase::Candidates => timing.record("candidate", candidate_started),
                SearchPhase::Page(started) => timing.record("page", started),
                SearchPhase::Related(started) => timing.record("related", started),
            },
        )
        .await
        .map_err(search_scope_error)?;
    if !input.include_blueprint {
        response.blueprint = None;
    }
    let serialization_started = Instant::now();
    let response = Json(response).into_response();
    timing.record("serialize", serialization_started);
    Ok(response)
}

fn search_scope_error(error: SearchScopeError) -> ApiError {
    match error {
        SearchScopeError::UnknownBlueprint => ApiError::not_found("blueprint"),
        SearchScopeError::Invalid { message, .. } => ApiError::invalid_input(message),
        SearchScopeError::Query { error, .. } => map_search_error(error),
        SearchScopeError::RelationshipSortRequiresSingleVersion => {
            ApiError::relationship_sort_requires_single_version()
        }
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
                    .filter(|a| a.value_type == ValueType::Relationship)
                    .ok_or_else(|| {
                        invalid("source_relationship_field must be a relationship attribute")
                    })?;
                let target_code = relationship
                    .target_blueprint_code
                    .clone()
                    .ok_or_else(|| invalid("source_relationship_field has no target blueprint"))?;
                if !saved_search::browse_path_hop(
                    relationship.value_type,
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
        if hierarchy.value_type != ValueType::Relationship
            || hierarchy.target_blueprint_code.as_deref() != Some(&target.blueprint.code)
        {
            return Err(invalid(
                "hierarchy_field must be a self-targeting relationship",
            ));
        }
    }
    Ok((target, source_path))
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
        .authorized_record_ids(caller, workspace, Permission::RecordsRead, &record_ids)
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
        .authorized_record_ids(caller, workspace, Permission::RecordsRead, &record_ids)
        .await?;
    record_ids.retain(|id| readable.contains(id));
    Ok(Json(RecordLabelsResponse {
        items: repository.record_labels(&record_ids).await?,
    }))
}
