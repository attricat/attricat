use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    catalog_read_service::CatalogReadService,
    catalog_service::CatalogMutationService,
    constants::DEFAULT_PAGE_SIZE,
    model::{
        AppendAttributeValues, AttributeValue, AttributeValueHistory, CreateEntityFormRequest,
        Entity, EntityAuditChange, EntityFormResponse, IncomingRelationshipsPage,
        IncomingRelationshipsRequest, MigrateEntityRequest, PublicationContextRequest,
        RelationshipMutation, UpdateEntityFormRequest,
    },
    repository::decode_search_cursor,
};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agents::{MAX_CONVERSATION_MESSAGE_BYTES, MAX_TOOL_CALL_ARGUMENT_BYTES};
use catalog_workers::{
    agent_provider::{ChatMessage, OpenAiCompatibleClient},
    agent_tools::{ToolDefinition, ToolFunction},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SmartFillEntityFormRequest {
    entity_id: Uuid,
    context_id: Option<Uuid>,
    is_default_context: bool,
    content: String,
}

#[derive(Serialize)]
pub(super) struct SmartFillEntityFormResponse {
    fields: std::collections::BTreeMap<String, String>,
}

const SMART_FILL_SYSTEM_PROMPT: &str = "You fill the editable scalar fields of one catalogue entity from pasted text. Use only the supplied field codes, preserve values not supported by the text, and never invent facts. Return exactly one propose_entity_form_values tool call. Values must be strings suitable for the browser form; do not propose relationship or file fields.";

fn smart_fill_definition() -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: ToolFunction {
            name: "propose_entity_form_values",
            description: "Propose editable entity form field values. This does not save catalog data.",
            parameters: json!({
                "type": "object",
                "required": ["fields"],
                "properties": {
                    "fields": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "additionalProperties": false
            }),
        },
    }
}

pub(super) async fn smart_fill_entity_form(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace_id): super::auth::ActiveWorkspace,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<SmartFillEntityFormRequest>,
) -> Result<Json<SmartFillEntityFormResponse>, ApiError> {
    if input.content.trim().is_empty() || input.content.len() > MAX_CONVERSATION_MESSAGE_BYTES {
        return Err(ApiError::invalid_input(
            "content must be between 1 and 32768 bytes".to_owned(),
        ));
    }
    if !repository
        .is_authorized(
            user,
            workspace_id,
            "entities.read",
            Some(input.entity_id),
            None,
        )
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let config = state
        .agent_provider
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("agents are not configured"))?;
    let (entity, values) = CatalogReadService::new(&repository)
        .entity_with_values(input.entity_id)
        .await?;
    let blueprint = repository
        .get_blueprint_revision(entity.blueprint_id, entity.blueprint_version)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint version"))?;
    let reusable_attributes = repository
        .entity_reusable_attributes(input.entity_id)
        .await?;
    let reusable_values = repository.reusable_form_values(input.entity_id).await?;
    let mut editable = Vec::new();
    for attribute in &blueprint.attributes {
        if !attribute.readonly
            && attribute.value_type != "relationship"
            && attribute.value_type != "file"
            && (input.is_default_context || attribute.context_editable != "default")
        {
            editable.push(json!({
                "code": attribute.code,
                "name": attribute.code,
                "value_type": attribute.value_type,
            }));
        }
    }
    for attribute in &reusable_attributes {
        if !attribute.readonly
            && attribute.value_type != "relationship"
            && attribute.value_type != "file"
            && (input.is_default_context || attribute.context_editable != "default")
        {
            editable.push(json!({
                "code": attribute.code,
                "name": attribute.name,
                "value_type": attribute.value_type,
            }));
        }
    }
    let current_values = values
        .iter()
        .chain(reusable_values.iter())
        .filter(|value| matches!(value, crate::model::FormAttributeValue::Scalar { context_id, .. } if *context_id == input.context_id))
        .collect::<Vec<_>>();
    let prompt = json!({
        "entity_id": input.entity_id,
        "context_id": input.context_id,
        "editable_fields": editable,
        "current_values": current_values,
        "pasted_text": input.content,
    });
    let provider = OpenAiCompatibleClient::new(config)
        .map_err(|_| ApiError::service_unavailable("agent provider is unavailable"))?;
    let completion = provider
        .complete(
            vec![
                ChatMessage {
                    role: "system".to_owned(),
                    content: Value::String(SMART_FILL_SYSTEM_PROMPT.to_owned()),
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: "user".to_owned(),
                    content: prompt,
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
            vec![smart_fill_definition()],
        )
        .await
        .map_err(|_| ApiError::service_unavailable("agent provider is unavailable"))?;
    let call = completion
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.tool_calls.into_iter().next())
        .filter(|call| call.function.name == "propose_entity_form_values")
        .ok_or_else(|| ApiError::invalid_input("agent did not return form values".to_owned()))?;
    if call.function.arguments.len() > MAX_TOOL_CALL_ARGUMENT_BYTES {
        return Err(ApiError::invalid_input(
            "agent returned oversized form values".to_owned(),
        ));
    }
    let fields = serde_json::from_str::<Value>(&call.function.arguments)
        .ok()
        .and_then(|value| value.get("fields").cloned())
        .and_then(|value| {
            serde_json::from_value::<std::collections::BTreeMap<String, String>>(value).ok()
        })
        .ok_or_else(|| ApiError::invalid_input("agent returned invalid form values".to_owned()))?;
    let allowed = editable
        .iter()
        .filter_map(|field| field.get("code").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    Ok(Json(SmartFillEntityFormResponse {
        fields: fields
            .into_iter()
            .filter(|(code, _)| allowed.contains(code.as_str()))
            .collect(),
    }))
}
pub(super) async fn delete_entity(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    CatalogMutationService::new(&repository)
        .delete_entity(entity_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_entity_form(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateEntityFormRequest>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    let entity = CatalogMutationService::new(&repository)
        .create_entity(input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(entity)))
}
pub(super) async fn get_entity_form(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<EntityFormResponse>, ApiError> {
    let (entity, values) = CatalogReadService::new(&repository)
        .entity_with_values(entity_id)
        .await?;
    let blueprint = repository
        .get_blueprint_revision(entity.blueprint_id, entity.blueprint_version)
        .await?
        .ok_or_else(|| ApiError::not_found("blueprint version"))?;
    let reusable_attributes = repository.entity_reusable_attributes(entity_id).await?;
    let reusable_values = repository.reusable_form_values(entity_id).await?;
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
        reusable_attributes,
        reusable_values,
    }))
}
pub(super) async fn update_entity_form(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateEntityFormRequest>,
) -> Result<Json<Entity>, ApiError> {
    let entity = CatalogMutationService::new(&repository)
        .update_entity(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(entity))
}
pub(super) async fn list_incoming_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<IncomingRelationshipsRequest>,
) -> Result<Json<IncomingRelationshipsPage>, ApiError> {
    if input.relationships.is_empty() {
        return Err(ApiError::invalid_input(
            "relationships must not be empty".to_owned(),
        ));
    }
    let requested = input.page.size.unwrap_or(DEFAULT_PAGE_SIZE);
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
        repository
            .incoming_relationships(entity_id, input.relationships, limit.into(), cursor)
            .await?,
    ))
}
pub(super) async fn list_entity_publications(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::EntityPublicationStatus>>, ApiError> {
    Ok(Json(repository.publication_statuses(entity_id).await?))
}
pub(super) async fn publish_entity(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PublicationContextRequest>,
) -> Result<Json<crate::model::EntityPublicationStatus>, ApiError> {
    let status = CatalogMutationService::new(&repository)
        .publish_entity(entity_id, input.context_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(status))
}
pub(super) async fn publish_entity_all_channels(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::EntityPublicationStatus>>, ApiError> {
    let status = CatalogMutationService::new(&repository)
        .publish_entity_all_channels(entity_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(status))
}
pub(super) async fn unpublish_entity(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PublicationContextRequest>,
) -> Result<StatusCode, ApiError> {
    CatalogMutationService::new(&repository)
        .unpublish_entity(entity_id, input.context_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn preview_entity_migration(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<crate::model::EntityMigrationPreview>, ApiError> {
    Ok(Json(repository.preview_entity_migration(entity_id).await?))
}
pub(super) async fn migrate_entity_to_latest(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<MigrateEntityRequest>,
) -> Result<Json<Entity>, ApiError> {
    let entity = CatalogMutationService::new(&repository)
        .migrate_entity(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(entity))
}
pub(super) async fn append_values(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = CatalogMutationService::new(&repository)
        .append_values(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn get_current_values(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<AttributeValue>>, ApiError> {
    if repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(repository.current_values(entity_id).await?))
}
pub(super) async fn get_entity_changes(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<EntityAuditChange>>, ApiError> {
    if repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(repository.entity_audit_changes(entity_id).await?))
}
pub(super) async fn get_value_history(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<Json<Vec<AttributeValueHistory>>, ApiError> {
    if repository.get_entity(entity_id).await?.is_none() {
        return Err(ApiError::not_found("entity"));
    }
    Ok(Json(repository.value_history(entity_id).await?))
}
pub(super) async fn restore_value(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((entity_id, history_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<AttributeValue>), ApiError> {
    let value = CatalogMutationService::new(&repository)
        .restore_value(entity_id, history_id)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(value)))
}
pub(super) async fn replace_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = CatalogMutationService::new(&repository)
        .replace_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn remove_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = CatalogMutationService::new(&repository)
        .remove_relationships(entity_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(values)))
}
