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
use base64::{Engine, engine::general_purpose::STANDARD};
use catalog_validation::validate_json_schema;
use chrono::{DateTime, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agents::{MAX_CONVERSATION_MESSAGE_BYTES, MAX_TOOL_CALL_ARGUMENT_BYTES};
use catalog_agent_runtime::{
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
    #[serde(default)]
    conversation_id: Option<Uuid>,
    #[serde(default)]
    draft_values: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    attachment_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub(super) struct SmartFillEntityFormResponse {
    fields: std::collections::BTreeMap<String, String>,
    explanation: String,
}

#[derive(Serialize)]
struct SmartFillField {
    code: String,
    name: String,
    value_type: String,
    value_schema: Option<Value>,
}

const SMART_FILL_SYSTEM_PROMPT: &str = "You are an entity editing assistant. Answer the user's editing request using the selected entity context, current unsaved form draft and conversation history. Propose only fields that should change, using supplied editable scalar field codes. Never invent facts. Return exactly one propose_entity_form_values tool call with a short explanation of changes and uncertainties. The values are draft-only and require explicit user application and a separate save; never claim they were saved. Do not propose relationship or file fields.";

fn smart_fill_definition() -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: ToolFunction {
            name: "propose_entity_form_values",
            description: "Propose editable entity form field values. This does not save catalog data.",
            parameters: json!({
                "type": "object",
                "required": ["fields", "explanation"],
                "properties": {
                    "fields": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    },
                    "explanation": {"type": "string"}
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
    if (input.content.trim().is_empty() && input.attachment_ids.is_empty())
        || input.content.len() > MAX_CONVERSATION_MESSAGE_BYTES
        || input.draft_values.len() > 256
        || input
            .draft_values
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>()
            > MAX_CONVERSATION_MESSAGE_BYTES
    {
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
    let is_default_context = match input.context_id {
        Some(id) => {
            repository
                .get_context_by_id(id)
                .await?
                .ok_or_else(|| ApiError::invalid_input("unknown context".to_owned()))?
                .code
                == "default"
        }
        None => true,
    };
    if input.is_default_context != is_default_context {
        return Err(ApiError::invalid_input(
            "is_default_context does not match context_id".to_owned(),
        ));
    }
    let mut editable = Vec::new();
    for attribute in &blueprint.attributes {
        if !attribute.readonly
            && attribute.value_type != "relationship"
            && attribute.value_type != "file"
            && (is_default_context || attribute.context_editable != "default")
        {
            editable.push(SmartFillField {
                code: attribute.code.clone(),
                name: attribute.code.clone(),
                value_type: attribute.value_type.clone(),
                value_schema: attribute.value_schema.clone(),
            });
        }
    }
    for attribute in &reusable_attributes {
        if !attribute.readonly
            && attribute.value_type != "relationship"
            && attribute.value_type != "file"
            && (is_default_context || attribute.context_editable != "default")
        {
            editable.push(SmartFillField {
                code: attribute.code.clone(),
                name: attribute.name.clone(),
                value_type: attribute.value_type.clone(),
                value_schema: attribute.value_schema.clone(),
            });
        }
    }
    let allowed = editable
        .iter()
        .map(|field| field.code.as_str())
        .collect::<std::collections::HashSet<_>>();
    let current_values = editable_scalar_values(
        values.iter().chain(reusable_values.iter()),
        input.context_id,
        &allowed,
    );
    if input.attachment_ids.len() > 16
        || input
            .attachment_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != input.attachment_ids.len()
    {
        return Err(ApiError::invalid_input("invalid attachments".into()));
    }
    if !input.attachment_ids.is_empty() && input.conversation_id.is_none() {
        return Err(ApiError::invalid_input(
            "attachments require a conversation".into(),
        ));
    }
    if let Some(id) = input.conversation_id {
        repository
            .verify_conversation_uploads(id, user, &input.attachment_ids)
            .await?;
    }
    let history = if let Some(id) = input.conversation_id {
        let conversation = repository.get_conversation(id).await?;
        if conversation.entity_id != Some(input.entity_id)
            || conversation.context_id != input.context_id
        {
            return Err(ApiError::invalid_input(
                "conversation belongs to another entity or context".into(),
            ));
        }
        let recent = repository.conversation_messages(id).await?;
        let mut remaining_attachment_bytes = 65536usize;
        let mut history = Vec::new();
        for message in recent
            .into_iter()
            .rev()
            .take(16)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let mut prior_files = Vec::new();
            for attachment in &message.attachments {
                let Ok(file) = repository.file_object(attachment.id, None).await else {
                    continue;
                };
                if (file.mime_type.starts_with("text/") || file.mime_type == "application/json")
                    && file.byte_size >= 0
                    && (file.byte_size as usize) <= remaining_attachment_bytes
                {
                    if let Ok(object) = state.object_store.get(&file.object_key).await
                        && object.bytes.len() <= remaining_attachment_bytes
                        && let Ok(text) = std::str::from_utf8(&object.bytes)
                    {
                        remaining_attachment_bytes -= object.bytes.len();
                        prior_files.push(json!({"filename":file.display_filename,"text":text}));
                    }
                } else {
                    prior_files.push(json!({"filename":file.display_filename,"note":"Reattach this file if its contents are needed."}));
                }
            }
            history.push(
                json!({"role":message.role,"content":message.content,"attachments":prior_files}),
            );
        }
        history
    } else {
        Vec::new()
    };
    let prompt = json!({
        "entity_id": input.entity_id,
        "context_id": input.context_id,
        "editable_fields": editable,
        "current_values": current_values,
        "unsaved_draft_values": input.draft_values.iter().filter(|(code, _)| allowed.contains(code.as_str())).collect::<std::collections::BTreeMap<_, _>>(),
        "conversation_history": history,
        "user_request": input.content,
    });
    let mut parts = vec![json!({"type":"text","text":prompt.to_string()})];
    for file_id in &input.attachment_ids {
        let file = repository.file_object(*file_id, None).await?;
        if file.mime_type.starts_with("image/") {
            let image = repository
                .file_object(*file_id, Some("display"))
                .await
                .unwrap_or(file);
            if image.byte_size > 1024 * 1024 {
                return Err(ApiError::invalid_input("image is too large".into()));
            }
            let object = state
                .object_store
                .get(&image.object_key)
                .await
                .map_err(|_| ApiError::service_unavailable("attachment unavailable"))?;
            if object.bytes.len() > 1024 * 1024 {
                return Err(ApiError::invalid_input("image is too large".into()));
            }
            parts.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}", image.mime_type, STANDARD.encode(object.bytes))}}));
        } else if file.mime_type.starts_with("text/") || file.mime_type == "application/json" {
            if file.byte_size > 65536 {
                return Err(ApiError::invalid_input(
                    "text attachment is too large".into(),
                ));
            }
            let object = state
                .object_store
                .get(&file.object_key)
                .await
                .map_err(|_| ApiError::service_unavailable("attachment unavailable"))?;
            if object.bytes.len() > 65536 {
                return Err(ApiError::invalid_input(
                    "text attachment is too large".into(),
                ));
            }
            let text = std::str::from_utf8(&object.bytes)
                .map_err(|_| ApiError::invalid_input("attachment is not UTF-8".into()))?;
            parts.push(json!({"type":"text","text":format!("Attachment {}:\n{text}",file.display_filename)}));
        } else {
            return Err(ApiError::invalid_input(
                "only images and text attachments are supported for draft proposals".into(),
            ));
        }
    }
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
                    content: Value::Array(parts),
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
    let arguments = serde_json::from_str::<Value>(&call.function.arguments)
        .map_err(|_| ApiError::invalid_input("agent returned invalid form values".to_owned()))?;
    let explanation = arguments
        .get("explanation")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(2048)
        .collect::<String>();
    let fields = Some(arguments)
        .and_then(|value| value.get("fields").cloned())
        .and_then(|value| {
            serde_json::from_value::<std::collections::BTreeMap<String, String>>(value).ok()
        })
        .ok_or_else(|| ApiError::invalid_input("agent returned invalid form values".to_owned()))?;
    let fields = fields
        .into_iter()
        .filter(|(code, value)| {
            editable
                .iter()
                .any(|field| field.code == *code && smart_fill_value_is_valid(field, value))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if let Some(id) = input.conversation_id {
        repository
            .append_conversation_message_with_attachments(
                id,
                None,
                "user",
                Value::String(input.content),
                &input.attachment_ids,
            )
            .await?;
        let base_values = fields
            .keys()
            .map(|code| (code.clone(), input.draft_values.get(code).cloned()))
            .collect::<std::collections::BTreeMap<_, _>>();
        repository.append_conversation_message(id, None, "assistant", json!({"draft_proposal":{"fields":fields,"explanation":explanation,"base_values":base_values}})).await?;
        let title_repository = repository.clone();
        tokio::spawn(async move {
            catalog_agent_runtime::conversation_title::maybe_generate_title(
                &title_repository,
                &provider,
                id,
            )
            .await;
        });
    }
    Ok(Json(SmartFillEntityFormResponse {
        fields,
        explanation,
    }))
}
/// Parse a suggestion exactly as the browser will parse a scalar form field,
/// then apply the attribute's JSON schema before offering it to the user.
fn smart_fill_value_is_valid(field: &SmartFillField, proposed: &str) -> bool {
    let value = proposed.trim();
    if value.is_empty() {
        return false;
    }
    let native = match field.value_type.as_str() {
        "string" => Some(Value::String(value.to_owned())),
        "number" => value
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number),
        "integer" => value
            .parse::<i64>()
            .ok()
            .filter(|value| (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(value))
            .map(|value| json!(value)),
        "boolean" => value.parse::<bool>().ok().map(|value| json!(value)),
        "date" => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|_| value.len() == 10)
            .map(|_| json!(value)),
        "datetime" => DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|_| json!(value)),
        "time" => value.split_once(' ').and_then(|(time, zone)| {
            NaiveTime::parse_from_str(time, "%H:%M:%S%.f").ok()?;
            zone.parse::<chrono_tz::Tz>().ok()?;
            Some(json!({"time": time, "time_zone": zone}))
        }),
        "json" => serde_json::from_str(value).ok(),
        _ => None,
    };
    let Some(native) = native else { return false };
    field.value_schema.as_ref().is_none_or(|schema| {
        validate_json_schema(schema, &native).is_ok_and(|errors| errors.is_empty())
    })
}

fn editable_scalar_values<'a>(
    values: impl Iterator<Item = &'a crate::model::FormAttributeValue>,
    context_id: Option<Uuid>,
    allowed: &std::collections::HashSet<&str>,
) -> Vec<&'a crate::model::FormAttributeValue> {
    values
        .filter(|value| matches!(value, crate::model::FormAttributeValue::Scalar { attribute_code, context_id: value_context, .. }
            if *value_context == context_id && allowed.contains(attribute_code.as_str())))
        .collect()
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
pub(super) async fn duplicate_entity(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(entity_id): ApiPath<Uuid>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    let entity = CatalogMutationService::new(&repository)
        .duplicate_entity(entity_id)
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

#[cfg(test)]
mod smart_fill_tests {
    use super::*;

    #[test]
    fn only_editable_scalar_values_in_the_requested_context_reach_the_provider() {
        let context = Uuid::new_v4();
        let values = [
            crate::model::FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(context),
                value: json!("ok"),
            },
            crate::model::FormAttributeValue::Scalar {
                attribute_code: "private".into(),
                context_id: Some(context),
                value: json!("secret"),
            },
            crate::model::FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(Uuid::new_v4()),
                value: json!("other"),
            },
        ];
        let allowed = std::collections::HashSet::from(["name"]);
        let filtered = editable_scalar_values(values.iter(), Some(context), &allowed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(serde_json::to_value(filtered[0]).unwrap()["value"], "ok");
    }

    #[test]
    fn suggestions_must_match_the_browser_scalar_type_and_attribute_schema() {
        let field = SmartFillField {
            code: "state".into(),
            name: "State".into(),
            value_type: "string".into(),
            value_schema: Some(json!({"type": "string", "enum": ["active", "inactive"]})),
        };
        assert!(smart_fill_value_is_valid(&field, "active"));
        assert!(!smart_fill_value_is_valid(&field, "discontinued"));
        let integer = SmartFillField {
            value_type: "integer".into(),
            value_schema: None,
            ..field
        };
        assert!(smart_fill_value_is_valid(&integer, "42"));
        assert!(!smart_fill_value_is_valid(&integer, "1.5"));
        assert!(!smart_fill_value_is_valid(&integer, "9223372036854775808"));
        assert!(!smart_fill_value_is_valid(&integer, "-9223372036854775808"));
        let date = SmartFillField {
            value_type: "date".into(),
            value_schema: None,
            ..integer
        };
        assert!(smart_fill_value_is_valid(&date, "2026-09-23"));
        assert!(!smart_fill_value_is_valid(&date, "2026-9-23"));
    }
}
