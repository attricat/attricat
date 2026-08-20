use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    model::{
        AppendAttributeValues, AttributeValue, AttributeValueHistory, BlueprintWithAttributes,
        CreateEntityFormRequest, Entity, EntityFormResponse, IncomingRelationshipsPage,
        IncomingRelationshipsRequest, MigrateEntityRequest, RelationshipMutation, SearchBlueprint,
        UpdateEntityFormRequest,
    },
    repository::decode_search_cursor,
};
use axum::{Json, extract::State, http::StatusCode};
use uuid::Uuid;
pub(super) async fn delete_entity(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.repository.delete_entity(entity_id).await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_entity_form(
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
pub(super) async fn get_entity_form(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<EntityFormResponse>, ApiError> {
    let entity = state
        .repository
        .get_entity(entity_id)
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
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
pub(super) async fn update_entity_form(
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
pub(super) async fn list_incoming_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<IncomingRelationshipsRequest>,
) -> Result<Json<IncomingRelationshipsPage>, ApiError> {
    if input.relationships.is_empty() {
        return Err(ApiError::invalid_input(
            "relationships must not be empty".to_owned(),
        ));
    }
    let requested = input.page.size.unwrap_or(20);
    if requested == 0 {
        return Err(ApiError::invalid_input(
            "page.size must be greater than zero".to_owned(),
        ));
    }
    let limit = requested.min(state.max_incoming_relationship_page_size);
    let cursor = match input.page.cursor.as_deref() {
        Some(v) => Some(
            decode_search_cursor(v)
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
pub(super) async fn preview_entity_migration(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<crate::model::EntityMigrationPreview>, ApiError> {
    Ok(Json(
        state.repository.preview_entity_migration(entity_id).await?,
    ))
}
pub(super) async fn migrate_entity_to_latest(
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
    blueprint: &SearchBlueprint,
) -> Result<BlueprintWithAttributes, ApiError> {
    if blueprint.code.is_empty() {
        return Err(ApiError::invalid_input(
            "blueprint.code must not be empty".to_owned(),
        ));
    }
    match blueprint.version {
        Some(version) => {
            state
                .repository
                .get_published_blueprint_by_code_and_version(&blueprint.code, version)
                .await?
        }
        None => {
            state
                .repository
                .get_blueprint_by_code(&blueprint.code)
                .await?
        }
    }
    .ok_or_else(|| ApiError::not_found("blueprint"))
}
pub(super) async fn append_values(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = state.repository.append_values(entity_id, input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn get_current_values(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<AttributeValue>>, ApiError> {
    if state.repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(state.repository.current_values(entity_id).await?))
}
pub(super) async fn get_value_history(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<AttributeValueHistory>>, ApiError> {
    if state.repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(state.repository.value_history(entity_id).await?))
}
pub(super) async fn restore_value(
    State(state): State<AppState>,
    ApiPath((entity_id, history_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<AttributeValue>), ApiError> {
    let value = state
        .repository
        .restore_value(entity_id, history_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(value)))
}
pub(super) async fn replace_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = state
        .repository
        .replace_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn remove_relationships(
    State(state): State<AppState>,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = state
        .repository
        .remove_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
