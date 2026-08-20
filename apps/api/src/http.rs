use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{FromRequest, FromRequestParts, Path, Query, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint,
        CreateEntityFormRequest, Entity, EntityFormResponse, EntityIdentity, EntityPreviewPage,
        EntityPreviewResponse, EntitySearchResponse, IncomingRelationshipsPage,
        IncomingRelationshipsRequest, MigrateEntityRequest, RelationshipMutation,
        ResolvedEntityPreviewResponse, SearchEntitiesRequest, UpdateAttributeContext,
        UpdateEntityFormRequest,
    },
    repository::{CatalogRepository, RepositoryError, decode_search_cursor},
};

#[derive(Clone)]
pub struct AppState {
    pub repository: CatalogRepository,
    // Preview expansion is request-controlled, so these limits keep cyclic or
    // high-cardinality relationship graphs from turning one read into an
    // unbounded amount of database work.
    pub max_preview_relationship_depth: u8,
    pub max_preview_relationship_items: u32,
    pub max_entity_page_size: u32,
    pub max_incoming_relationship_page_size: u32,
    pub data_health_cache_ttl_seconds: u64,
    pub data_health_cache: DataHealthCache,
}

pub type DataHealthCache = Arc<Mutex<HashMap<String, (Instant, Value)>>>;

struct ApiJson<T>(T);
struct ApiPath<T>(T);
struct ApiQuery<T>(T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(ApiError::from_json_rejection)
    }
}

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(ApiError::from_path_rejection)
    }
}

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Self(value))
            .map_err(ApiError::from_query_rejection)
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/data-health/summary", get(data_health_summary))
        .route("/data-health/blueprints", get(data_health_blueprints))
        .route("/data-health/freshness", get(data_health_freshness))
        .route("/data-health/completeness", get(data_health_completeness))
        .route("/data-health/contexts", get(data_health_contexts))
        .route("/data-health/relationships", get(data_health_relationships))
        .route("/data-health/storage", get(data_health_storage))
        .route("/data-health/refresh", post(refresh_data_health))
        .route(
            "/blueprints",
            get(list_entity_blueprints).post(create_blueprint),
        )
        .route("/blueprints/catalogue", get(list_blueprints))
        .route(
            "/blueprints/{blueprint_id}/versions",
            get(list_blueprint_revisions).post(create_blueprint_revision),
        )
        .route("/blueprints/{blueprint_id}", get(get_blueprint))
        .route(
            "/blueprints/{blueprint_id}/versions/{version}",
            get(get_blueprint_revision),
        )
        .route(
            "/blueprints/{blueprint_id}/versions/{version}/publish",
            post(publish_blueprint_revision),
        )
        .route("/blueprints/by-code/{code}", get(get_blueprint_by_code))
        .route(
            "/blueprints/by-code/{code}/versions/{version}",
            get(get_blueprint_by_code_and_version),
        )
        .route("/contexts", get(list_contexts).post(create_context))
        .route("/contexts/{code}", get(get_context))
        .route(
            "/contexts/id/{id}",
            put(update_context).delete(delete_context),
        )
        // The v1 routes are the entity creation and editing surface. The older
        // entity routes remain read and value/relationship primitives.
        .route("/v1/entities/search", post(search_entity_previews))
        .route("/v1/entities", post(create_entity_form))
        .route(
            "/v1/entities/{entity_id}",
            get(get_entity_form).put(update_entity_form),
        )
        .route(
            "/v1/entities/{entity_id}/incoming-relationships",
            post(list_incoming_relationships),
        )
        .route(
            "/v1/entities/{entity_id}/blueprint-migration/preview",
            post(preview_entity_migration),
        )
        .route(
            "/v1/entities/{entity_id}/blueprint-migration",
            post(migrate_entity_to_latest),
        )
        .route("/entities", get(list_previews))
        .route(
            "/entities/{entity_id}",
            get(get_entity).delete(delete_entity),
        )
        .route("/entities/{entity_id}/preview", get(get_preview))
        .route(
            "/entities/{entity_id}/resolved-preview",
            get(get_resolved_preview),
        )
        .route("/entities/{entity_id}/hierarchy", get(get_entity_hierarchy))
        .route("/entities/{entity_id}/values", post(append_values))
        .route(
            "/entities/{entity_id}/values/history",
            get(get_value_history),
        )
        .route(
            "/entities/{entity_id}/values/history/{history_id}/restore",
            post(restore_value),
        )
        .route(
            "/entities/{entity_id}/relationships/replace",
            post(replace_relationships),
        )
        .route(
            "/entities/{entity_id}/relationships/remove",
            post(remove_relationships),
        )
        .route(
            "/entities/{entity_id}/values/current",
            get(get_current_values),
        )
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataHealthQuery {
    stale_after_days: Option<u16>,
}

fn stale_after_days(query: DataHealthQuery) -> Result<u16, ApiError> {
    let days = query.stale_after_days.unwrap_or(90);
    if !(1..=3650).contains(&days) {
        return Err(ApiError::invalid_input(
            "stale_after_days must be between 1 and 3650".to_owned(),
        ));
    }
    Ok(days)
}

async fn cached_data_health<T>(
    state: &AppState,
    key: String,
    load: impl std::future::Future<Output = Result<T, RepositoryError>>,
) -> Result<Json<Value>, ApiError>
where
    T: Serialize,
{
    if state.data_health_cache_ttl_seconds > 0 {
        if let Some((created_at, value)) = state.data_health_cache.lock().await.get(&key).cloned() {
            if created_at.elapsed() < Duration::from_secs(state.data_health_cache_ttl_seconds) {
                return Ok(Json(value));
            }
        }
    }
    let value = serde_json::to_value(load.await?).expect("data health response serializes");
    if state.data_health_cache_ttl_seconds > 0 {
        state
            .data_health_cache
            .lock()
            .await
            .insert(key, (Instant::now(), value.clone()));
    }
    Ok(Json(value))
}

async fn invalidate_data_health(state: &AppState) {
    state.data_health_cache.lock().await.clear();
}

async fn data_health_summary(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Json<Value>, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("summary:{days}"),
        state.repository.data_health_summary(days.into()),
    )
    .await
}

async fn data_health_blueprints(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<DataHealthQuery>,
) -> Result<Json<Value>, ApiError> {
    let days = stale_after_days(query)?;
    cached_data_health(
        &state,
        format!("blueprints:{days}"),
        state.repository.data_health_blueprints(days.into()),
    )
    .await
}

async fn data_health_freshness(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "freshness".to_owned(),
        state.repository.data_health_freshness(),
    )
    .await
}

async fn data_health_completeness(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "completeness".to_owned(),
        state.repository.data_health_completeness(),
    )
    .await
}

async fn data_health_contexts(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "contexts".to_owned(),
        state.repository.data_health_contexts(),
    )
    .await
}

async fn data_health_relationships(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "relationships".to_owned(),
        state.repository.data_health_relationships(),
    )
    .await
}

async fn data_health_storage(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    cached_data_health(
        &state,
        "storage".to_owned(),
        state.repository.data_health_storage(),
    )
    .await
}

async fn refresh_data_health(State(state): State<AppState>) -> StatusCode {
    invalidate_data_health(&state).await;
    StatusCode::NO_CONTENT
}

async fn create_blueprint(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = state.repository.create_blueprint(input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}

async fn list_entity_blueprints(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<Vec<crate::model::Blueprint>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_entity_blueprints(query.include_drafts)
            .await?,
    ))
}

async fn list_blueprints(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::model::Blueprint>>, ApiError> {
    Ok(Json(state.repository.list_blueprints().await?))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlueprintQuery {
    #[serde(default)]
    include_drafts: bool,
}

async fn create_blueprint_revision(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = state
        .repository
        .create_blueprint_revision(blueprint_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}

async fn list_blueprint_revisions(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::Blueprint>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_blueprint_revisions(blueprint_id)
            .await?,
    ))
}

async fn get_blueprint(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_current_blueprint(blueprint_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn get_blueprint_revision(
    State(state): State<AppState>,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_revision(blueprint_id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}

async fn publish_blueprint_revision(
    State(state): State<AppState>,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = state
        .repository
        .publish_blueprint_revision(blueprint_id, version)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(blueprint))
}

async fn get_blueprint_by_code(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = if query.include_drafts {
        state
            .repository
            .get_blueprint_by_code_including_drafts(&code)
            .await?
    } else {
        state.repository.get_blueprint_by_code(&code).await?
    };
    blueprint
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn get_blueprint_by_code_and_version(
    State(state): State<AppState>,
    ApiPath((code, version)): ApiPath<(String, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_by_code_and_version(&code, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}

async fn create_context(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<CreateAttributeContext>,
) -> Result<(StatusCode, Json<crate::model::AttributeContext>), ApiError> {
    let context = state.repository.create_context(input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(context)))
}

async fn list_contexts(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::model::AttributeContext>>, ApiError> {
    Ok(Json(state.repository.list_contexts().await?))
}

async fn get_context(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
) -> Result<Json<crate::model::AttributeContext>, ApiError> {
    state
        .repository
        .get_context_by_code(&code)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("context"))
}

async fn update_context(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateAttributeContext>,
) -> Result<Json<crate::model::AttributeContext>, ApiError> {
    let context = state.repository.update_context(id, input).await?;
    invalidate_data_health(&state).await;
    Ok(Json(context))
}

async fn delete_context(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.repository.delete_context(id).await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_entity(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Entity>, ApiError> {
    state
        .repository
        .get_entity(entity_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}

async fn delete_entity(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.repository.delete_entity(entity_id).await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_preview(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<PreviewQuery>,
) -> Result<Json<EntityPreviewResponse>, ApiError> {
    let relationship_depth = query.relationship_depth.unwrap_or(1);
    let relationship_limit = query.relationship_limit.unwrap_or(10);
    if relationship_depth > state.max_preview_relationship_depth {
        return Err(ApiError::invalid_input(format!(
            "relationship_depth must not exceed {}",
            state.max_preview_relationship_depth
        )));
    }
    if relationship_limit == 0 || relationship_limit > state.max_preview_relationship_items {
        return Err(ApiError::invalid_input(format!(
            "relationship_limit must be between 1 and {}",
            state.max_preview_relationship_items
        )));
    }
    let context = state
        .repository
        .preview(entity_id, relationship_depth, relationship_limit.into())
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    let entity = state
        .repository
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedPreviewQuery {
    context_id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HierarchyQuery {
    context_id: Uuid,
    field: String,
}

async fn get_entity_hierarchy(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HierarchyQuery>,
) -> Result<Json<crate::model::EntityHierarchyResponse>, ApiError> {
    state
        .repository
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

async fn get_resolved_preview(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<ResolvedPreviewQuery>,
) -> Result<Json<ResolvedEntityPreviewResponse>, ApiError> {
    state
        .repository
        .resolved_preview(entity_id, query.context_id, 1)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}

async fn list_previews(
    State(state): State<AppState>,
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
        state
            .repository
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

async fn search_entity_previews(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<SearchEntitiesRequest>,
) -> Result<Json<EntitySearchResponse>, ApiError> {
    let blueprint_code = &input.blueprint.code;
    if blueprint_code.is_empty() {
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
    // Always resolve the current revision for the response and compatibility
    // status. A requested version limits results but does not change the schema
    // that discovery presents.
    let current_blueprint = state
        .repository
        .get_blueprint_by_code(blueprint_code)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint"))?;
    let selected_version = match input.blueprint.version {
        Some(version) => Some(
            state
                .repository
                .get_blueprint_by_code_and_version(blueprint_code, version)
                .await?
                .map(|blueprint| blueprint.blueprint.version)
                .ok_or_else(|| ApiError::not_found("blueprint"))?,
        ),
        None => None,
    };
    // Cursors encode the database ordering tuple rather than an offset, which
    // avoids duplicate or skipped rows as earlier pages are inserted into.
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
        .filter(|value| !value.is_empty());
    let (relationship_tree_facet, matching_entity_ids) = match input.relationship_tree_facet {
        Some(facet) => {
            let source = current_blueprint
                .attributes
                .iter()
                .find(|attribute| attribute.code == facet.source_relationship_field)
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
            let target = state
                .repository
                .get_blueprint_by_code(target_code)
                .await?
                .ok_or_else(|| ApiError::not_found("target blueprint"))?;
            let hierarchy = target
                .attributes
                .iter()
                .find(|attribute| attribute.code == facet.hierarchy_field)
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
            let matching_entity_ids = state
                .repository
                .search_matching_entity_ids(current_blueprint.blueprint.id, selected_version, query)
                .await?;
            let (tree, matching) = state
                .repository
                .relationship_tree_facet(
                    current_blueprint.blueprint.id,
                    &facet.source_relationship_field,
                    target.blueprint.id,
                    &facet.hierarchy_field,
                    facet.context_id,
                    &facet.selected_target_ids,
                    &matching_entity_ids,
                )
                .await?;
            (Some(tree), matching)
        }
        None => (None, None),
    };
    let (mut items, next_cursor) = state
        .repository
        .search_entity_previews(
            current_blueprint.blueprint.id,
            selected_version,
            query,
            limit.into(),
            cursor,
            matching_entity_ids.as_deref(),
        )
        .await?;
    for item in &mut items {
        item.schema_outdated = item.blueprint_version != current_blueprint.blueprint.version;
    }
    Ok(Json(EntitySearchResponse {
        blueprint: current_blueprint,
        items,
        next_cursor,
        relationship_tree_facet,
    }))
}

async fn create_entity_form(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<CreateEntityFormRequest>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    let blueprint = resolve_search_blueprint(&state, &input.blueprint).await?;
    let entity = state
        .repository
        .create_entity_with_values(
            blueprint.blueprint.id,
            blueprint.blueprint.version,
            input.values,
        )
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(entity)))
}

async fn get_entity_form(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<EntityFormResponse>, ApiError> {
    let entity = state
        .repository
        .get_entity(entity_id)
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    // An entity stays bound to its creation revision. Loading the latest
    // blueprint here could make old values invalid or hide fields in its form.
    let blueprint = state
        .repository
        .get_blueprint_revision(entity.blueprint_id, entity.blueprint_version)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint version"))?;
    let values = state.repository.form_values(entity_id).await?;
    Ok(Json(EntityFormResponse {
        context: entity
            .projections
            .get("preview")
            .cloned()
            .ok_or(ApiError::internal(
                "entity is missing its preview projection",
            ))?,
        entity,
        blueprint,
        values,
    }))
}

async fn update_entity_form(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateEntityFormRequest>,
) -> Result<Json<Entity>, ApiError> {
    let entity = state
        .repository
        .update_entity_with_values(
            entity_id,
            input.values,
            input.relationships,
            input.remove_values,
        )
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(entity))
}

async fn list_incoming_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<IncomingRelationshipsRequest>,
) -> Result<Json<IncomingRelationshipsPage>, ApiError> {
    if input.relationships.is_empty() {
        return Err(ApiError::invalid_input(
            "relationships must not be empty".to_owned(),
        ));
    }
    let requested_limit = input.page.size.unwrap_or(20);
    if requested_limit == 0 {
        return Err(ApiError::invalid_input(
            "page.size must be greater than zero".to_owned(),
        ));
    }
    // Blueprint page sizes are presentation preferences; this setting is the
    // server-side bound on database work for any one page.
    let limit = requested_limit.min(state.max_incoming_relationship_page_size);
    let cursor = match input.page.cursor.as_deref() {
        Some(cursor) => Some(
            decode_search_cursor(cursor)
                .ok_or_else(|| ApiError::invalid_input("page.cursor is invalid".to_owned()))?,
        ),
        None => None,
    };
    Ok(Json(
        state
            .repository
            .incoming_relationships(entity_id, input.relationships, limit.into(), cursor)
            .await?,
    ))
}

async fn preview_entity_migration(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<crate::model::EntityMigrationPreview>, ApiError> {
    let preview = state.repository.preview_entity_migration(entity_id).await?;
    Ok(Json(preview))
}

async fn migrate_entity_to_latest(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<MigrateEntityRequest>,
) -> Result<Json<Entity>, ApiError> {
    let entity = state
        .repository
        .migrate_entity_to_latest(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(entity))
}

async fn resolve_search_blueprint(
    state: &AppState,
    blueprint: &crate::model::SearchBlueprint,
) -> Result<BlueprintWithAttributes, ApiError> {
    let code = &blueprint.code;
    if code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    match blueprint.version {
        Some(version) => {
            state
                .repository
                .get_published_blueprint_by_code_and_version(code, version)
                .await?
        }
        None => state.repository.get_blueprint_by_code(code).await?,
    }
    .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn append_values(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    let values = state.repository.append_values(entity_id, input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}

async fn get_current_values(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::AttributeValue>>, ApiError> {
    if state.repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }

    Ok(Json(state.repository.current_values(entity_id).await?))
}

async fn get_value_history(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::AttributeValueHistory>>, ApiError> {
    if state.repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(state.repository.value_history(entity_id).await?))
}

async fn restore_value(
    State(state): State<AppState>,
    ApiPath((entity_id, history_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<crate::model::AttributeValue>), ApiError> {
    let value = state
        .repository
        .restore_value(entity_id, history_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(value)))
}

async fn replace_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    let values = state
        .repository
        .replace_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}

async fn remove_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    let values = state
        .repository
        .remove_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewQuery {
    relationship_depth: Option<u8>,
    relationship_limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListPreviewsQuery {
    blueprint: String,
    related_from: Uuid,
    relationship: String,
    limit: Option<u32>,
    cursor: Option<Uuid>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn not_found(resource: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{resource} was not found"),
        }
    }

    fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.to_owned(),
        }
    }

    fn invalid_input(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_input",
            message,
        }
    }

    fn bad_request(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.to_owned(),
        }
    }

    fn from_json_rejection(rejection: axum::extract::rejection::JsonRejection) -> Self {
        use axum::extract::rejection::JsonRejection;

        match rejection {
            JsonRejection::JsonDataError(_) => Self::invalid_input(
                "request body does not match the expected JSON shape".to_owned(),
            ),
            JsonRejection::JsonSyntaxError(_)
            | JsonRejection::MissingJsonContentType(_)
            | JsonRejection::BytesRejection(_) => Self::bad_request("request body is malformed"),
            _ => Self::bad_request("request body is malformed"),
        }
    }

    fn from_path_rejection(_: axum::extract::rejection::PathRejection) -> Self {
        Self::bad_request("path parameters are invalid")
    }

    fn from_query_rejection(_: axum::extract::rejection::QueryRejection) -> Self {
        Self::bad_request("query parameters are invalid")
    }
}

impl From<RepositoryError> for ApiError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound(resource) => Self::not_found(resource),
            RepositoryError::AttributeNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_not_applicable",
                message: error.to_string(),
            },
            RepositoryError::AttributeKindMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_kind_mismatch",
                message: error.to_string(),
            },
            RepositoryError::AttributeValueTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_type_mismatch",
                message: error.to_string(),
            },
            RepositoryError::AttributeValueSchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_value_schema_mismatch",
                message: error.to_string(),
            },
            RepositoryError::EntitySchemaMismatch { .. } => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "entity_schema_mismatch",
                message: error.to_string(),
            },
            RepositoryError::EntityBlueprintCurrent => Self {
                status: StatusCode::CONFLICT,
                code: "entity_blueprint_current",
                message: error.to_string(),
            },
            RepositoryError::MigrationTargetChanged => Self {
                status: StatusCode::CONFLICT,
                code: "migration_target_changed",
                message: error.to_string(),
            },
            RepositoryError::MigrationNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_not_applicable",
                message: error.to_string(),
            },
            RepositoryError::MigrationNeedsResolution(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "migration_needs_resolution",
                message: error.to_string(),
            },
            RepositoryError::InvalidStoredAttributeValue => {
                Self::internal("stored attribute value is invalid")
            }
            RepositoryError::RelationshipTargetTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "relationship_target_type_mismatch",
                message: error.to_string(),
            },
            RepositoryError::InvalidPreview
            | RepositoryError::InvalidHierarchyRelationship
            | RepositoryError::ReservedContextCode
            | RepositoryError::InvalidCode
            | RepositoryError::InvalidContextData
            | RepositoryError::InvalidContext
            | RepositoryError::DefaultContextProtected
            | RepositoryError::ContextCycle
            | RepositoryError::ContextInUse
            | RepositoryError::DefaultContextOnly
            | RepositoryError::InvalidAttributeSelector => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_input",
                message: error.to_string(),
            },
            RepositoryError::InvalidBlueprintDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_blueprint_definition",
                message: error.to_string(),
            },
            RepositoryError::BlueprintCodeTaken => Self {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: error.to_string(),
            },
            RepositoryError::BlueprintNotPublished => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "blueprint_not_published",
                message: error.to_string(),
            },
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                Self {
                    status: StatusCode::CONFLICT,
                    code: "conflict",
                    message: "a record with the same unique value already exists".to_owned(),
                }
            }
            RepositoryError::Database(_) => Self::internal("database operation failed"),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: &self.message,
                },
            })),
        )
            .into_response()
    }
}
