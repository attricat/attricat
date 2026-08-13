use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint,
        CreateEntity, CreateEntityFormRequest, Entity, EntityFormResponse, EntityIdentity,
        EntityPreviewPage, EntityPreviewResponse, EntitySearchResponse, RelationshipMutation,
        SearchEntitiesRequest, UpdateEntityFormRequest,
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
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/blueprints",
            get(list_entity_blueprints).post(create_blueprint),
        )
        .route(
            "/blueprints/{blueprint_id}/versions",
            post(create_blueprint_revision),
        )
        .route("/blueprints/{blueprint_id}", get(get_blueprint))
        .route(
            "/blueprints/{blueprint_id}/versions/{version}",
            get(get_blueprint_revision),
        )
        .route("/blueprints/by-code/{code}", get(get_blueprint_by_code))
        .route(
            "/blueprints/by-code/{code}/versions/{version}",
            get(get_blueprint_by_code_and_version),
        )
        .route("/contexts", post(create_context))
        .route("/contexts/{code}", get(get_context))
        // The v1 routes are form-oriented composites. The older entity routes
        // remain lower-level primitives for clients that manage values and
        // relationships independently.
        .route("/v1/entities/search", post(search_entity_previews))
        .route("/v1/entities", post(create_entity_form))
        .route(
            "/v1/entities/{entity_id}",
            get(get_entity_form).put(update_entity_form),
        )
        .route("/entities", get(list_previews).post(create_entity))
        .route("/entities/{entity_id}", get(get_entity))
        .route("/entities/{entity_id}/preview", get(get_preview))
        .route("/entities/{entity_id}/values", post(append_values))
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

async fn create_blueprint(
    State(state): State<AppState>,
    Json(input): Json<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_blueprint(input).await?),
    ))
}

async fn list_entity_blueprints(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::model::Blueprint>>, ApiError> {
    Ok(Json(state.repository.list_entity_blueprints().await?))
}

async fn create_blueprint_revision(
    State(state): State<AppState>,
    Path(blueprint_id): Path<Uuid>,
    Json(input): Json<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .repository
                .create_blueprint_revision(blueprint_id, input)
                .await?,
        ),
    ))
}

async fn get_blueprint(
    State(state): State<AppState>,
    Path(blueprint_id): Path<Uuid>,
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
    Path((blueprint_id, version)): Path<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_revision(blueprint_id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}

async fn get_blueprint_by_code(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_by_code(&code)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn get_blueprint_by_code_and_version(
    State(state): State<AppState>,
    Path((code, version)): Path<(String, i64)>,
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
    Json(input): Json<CreateAttributeContext>,
) -> Result<(StatusCode, Json<crate::model::AttributeContext>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_context(input).await?),
    ))
}

async fn get_context(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<crate::model::AttributeContext>, ApiError> {
    state
        .repository
        .get_context_by_code(&code)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("context"))
}

async fn create_entity(
    State(state): State<AppState>,
    Json(input): Json<CreateEntity>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_entity(input).await?),
    ))
}

async fn get_entity(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
) -> Result<Json<Entity>, ApiError> {
    state
        .repository
        .get_entity(entity_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}

async fn get_preview(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Query(query): Query<PreviewQuery>,
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
        .expect("preview only returns an existing entity");
    Ok(Json(EntityPreviewResponse {
        entity: EntityIdentity {
            id: entity.id,
            blueprint_id: entity.blueprint_id,
            blueprint_version: entity.blueprint_version,
        },
        context,
    }))
}

async fn list_previews(
    State(state): State<AppState>,
    Query(query): Query<ListPreviewsQuery>,
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
    Json(input): Json<SearchEntitiesRequest>,
) -> Result<Json<EntitySearchResponse>, ApiError> {
    let blueprint_code = input.blueprint.code.trim();
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
    // Resolve once before querying so every returned entity shares the schema
    // sent in the response. Falling back to the current version is deliberate
    // for discovery; callers can pin a version for repeatable pagination.
    let blueprint = match input.blueprint.version {
        Some(version) => {
            state
                .repository
                .get_blueprint_by_code_and_version(blueprint_code, version)
                .await?
        }
        None => {
            state
                .repository
                .get_blueprint_by_code(blueprint_code)
                .await?
        }
    }
    .ok_or_else(|| ApiError::not_found("blueprint"))?;
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
    let (items, next_cursor) = state
        .repository
        .search_entity_previews(
            blueprint.blueprint.id,
            blueprint.blueprint.version,
            query,
            limit.into(),
            cursor,
        )
        .await?;
    Ok(Json(EntitySearchResponse {
        blueprint,
        items,
        next_cursor,
    }))
}

async fn create_entity_form(
    State(state): State<AppState>,
    Json(input): Json<CreateEntityFormRequest>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    let blueprint = resolve_search_blueprint(&state, &input.blueprint).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .repository
                .create_entity_with_values(
                    blueprint.blueprint.id,
                    blueprint.blueprint.version,
                    input.values,
                )
                .await?,
        ),
    ))
}

async fn get_entity_form(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
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
    Path(entity_id): Path<Uuid>,
    Json(input): Json<UpdateEntityFormRequest>,
) -> Result<Json<Entity>, ApiError> {
    Ok(Json(
        state
            .repository
            .update_entity_with_values(entity_id, input.values, input.relationships)
            .await?,
    ))
}

async fn resolve_search_blueprint(
    state: &AppState,
    blueprint: &crate::model::SearchBlueprint,
) -> Result<BlueprintWithAttributes, ApiError> {
    let code = blueprint.code.trim();
    if code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    match blueprint.version {
        Some(version) => {
            state
                .repository
                .get_blueprint_by_code_and_version(code, version)
                .await?
        }
        None => state.repository.get_blueprint_by_code(code).await?,
    }
    .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn append_values(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Json(input): Json<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.append_values(entity_id, input).await?),
    ))
}

async fn get_current_values(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
) -> Result<Json<Vec<crate::model::AttributeValue>>, ApiError> {
    if state.repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }

    Ok(Json(state.repository.current_values(entity_id).await?))
}

async fn replace_relationships(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Json(input): Json<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .repository
                .replace_relationships(entity_id, input)
                .await?,
        ),
    ))
}

async fn remove_relationships(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Json(input): Json<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .repository
                .remove_relationships(entity_id, input)
                .await?,
        ),
    ))
}

#[derive(Deserialize)]
struct PreviewQuery {
    relationship_depth: Option<u8>,
    relationship_limit: Option<u32>,
}

#[derive(Deserialize)]
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
            RepositoryError::InvalidStoredAttributeValue => {
                Self::internal("stored attribute value is invalid")
            }
            RepositoryError::RelationshipTargetTypeMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "relationship_target_type_mismatch",
                message: error.to_string(),
            },
            RepositoryError::InvalidProjections
            | RepositoryError::InvalidPreview
            | RepositoryError::ReservedContextCode
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
