//! Draft-only record form proposals ("smart fill"): the agent provider
//! suggests values for a record's editable scalar fields from the user's
//! request, the unsaved draft, attachments and conversation history.
use std::collections::{BTreeMap, HashSet};

use attricat_domain::value_type::ValueType;
use attricat_validation::validate_json_schema;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    agent_provider::{ChatMessage, OpenAiCompatibleClient},
    agent_tools::{ToolDefinition, ToolFunction},
    agents::{
        MAX_CONVERSATION_ATTACHMENTS, MAX_CONVERSATION_MESSAGE_BYTES, MAX_TOOL_CALL_ARGUMENT_BYTES,
    },
    model::FormAttributeValue,
    provider_config::AgentProviderConfig,
    repository::{AttricatRepository, RepositoryError},
};
use attricat_repository::attricat_read_service::AttricatReadService;
use attricat_storage::ObjectStore;

use attricat_domain::constants::DEFAULT_CONTEXT_CODE;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartFillRequest {
    pub record_id: Uuid,
    pub context_id: Option<Uuid>,
    pub is_default_context: bool,
    pub content: String,
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    #[serde(default)]
    pub draft_values: BTreeMap<String, String>,
    #[serde(default)]
    pub attachment_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct SmartFillProposal {
    pub fields: BTreeMap<String, String>,
    pub explanation: String,
}

#[derive(Debug, Error)]
pub enum SmartFillError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0} was not found")]
    NotFound(&'static str),
    /// The provider or an attachment's storage is unavailable.
    #[error("{0}")]
    Unavailable(&'static str),
    /// The provider answered with something other than valid form values.
    #[error("{0}")]
    BadGateway(&'static str),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Serialize)]
struct SmartFillField {
    code: String,
    name: String,
    value_type: ValueType,
    value_schema: Option<Value>,
}

const MAX_SMART_FILL_DRAFT_VALUES: usize = 256;
/// Most recent conversation messages replayed to the provider as history.
const SMART_FILL_HISTORY_MESSAGES: usize = 16;
/// Combined size of the text attachments inlined from conversation history.
const SMART_FILL_HISTORY_ATTACHMENT_BYTES: usize = 64 * 1024;
const MAX_SMART_FILL_TEXT_ATTACHMENT_BYTES: usize = 64 * 1024;
const MAX_SMART_FILL_IMAGE_BYTES: usize = 1024 * 1024;
const MAX_SMART_FILL_EXPLANATION_CHARS: usize = 2048;
/// Largest integer a browser number field represents exactly (2^53 - 1).
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
/// Length of a `YYYY-MM-DD` date.
const ISO_DATE_LEN: usize = 10;

const SMART_FILL_SYSTEM_PROMPT: &str = "You are a record editing assistant. Answer the user's editing request using the selected record context, current unsaved form draft and conversation history. Propose only fields that should change, using supplied editable scalar field codes. Never invent facts. Return exactly one propose_record_form_values tool call with a short explanation of changes and uncertainties. The values are draft-only and require explicit user application and a separate save; never claim they were saved. Do not propose relationship or file fields.";

fn smart_fill_definition() -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: ToolFunction {
            name: "propose_record_form_values",
            description: "Propose editable record form field values. This does not save catalog data.",
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

/// Asks the agent provider for draft values of a record form's editable
/// scalar fields. Nothing is saved; with a conversation, the request and the
/// proposal are appended to it. The caller has checked that the user may read
/// the record.
pub async fn propose_record_form_values(
    repository: &AttricatRepository,
    object_store: &dyn ObjectStore,
    config: &AgentProviderConfig,
    user: Uuid,
    input: SmartFillRequest,
) -> Result<SmartFillProposal, SmartFillError> {
    if (input.content.trim().is_empty() && input.attachment_ids.is_empty())
        || input.content.len() > MAX_CONVERSATION_MESSAGE_BYTES
        || input.draft_values.len() > MAX_SMART_FILL_DRAFT_VALUES
        || input
            .draft_values
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>()
            > MAX_CONVERSATION_MESSAGE_BYTES
    {
        return Err(SmartFillError::Invalid(format!(
            "content must be between 1 and {MAX_CONVERSATION_MESSAGE_BYTES} bytes"
        )));
    }
    let (record, values) = AttricatReadService::new(repository)
        .record_with_values(input.record_id)
        .await?;
    let blueprint = repository
        .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
        .await?
        .ok_or_else(|| SmartFillError::NotFound("blueprint version"))?;
    let reusable_attributes = repository
        .record_reusable_attributes(input.record_id)
        .await?;
    let reusable_values = repository.reusable_form_values(input.record_id).await?;
    let is_default_context = match input.context_id {
        Some(id) => {
            repository
                .get_context_by_id(id)
                .await?
                .ok_or_else(|| SmartFillError::Invalid("unknown context".to_owned()))?
                .code
                == DEFAULT_CONTEXT_CODE
        }
        None => true,
    };
    if input.is_default_context != is_default_context {
        return Err(SmartFillError::Invalid(
            "is_default_context does not match context_id".to_owned(),
        ));
    }
    let mut editable = Vec::new();
    for attribute in &blueprint.attributes {
        if smart_fill_editable(
            attribute.readonly,
            attribute.value_type,
            &attribute.context_editable,
            is_default_context,
        ) {
            editable.push(SmartFillField {
                code: attribute.code.clone(),
                name: attribute.code.clone(),
                value_type: attribute.value_type,
                value_schema: attribute.value_schema.clone(),
            });
        }
    }
    for attribute in &reusable_attributes {
        if smart_fill_editable(
            attribute.readonly,
            attribute.value_type,
            &attribute.context_editable,
            is_default_context,
        ) {
            editable.push(SmartFillField {
                code: attribute.code.clone(),
                name: attribute.name.clone(),
                value_type: attribute.value_type,
                value_schema: attribute.value_schema.clone(),
            });
        }
    }
    let allowed = editable
        .iter()
        .map(|field| field.code.as_str())
        .collect::<HashSet<_>>();
    let current_values = editable_scalar_values(
        values.iter().chain(reusable_values.iter()),
        input.context_id,
        &allowed,
    );
    if input.attachment_ids.len() > MAX_CONVERSATION_ATTACHMENTS
        || input.attachment_ids.iter().collect::<HashSet<_>>().len() != input.attachment_ids.len()
    {
        return Err(SmartFillError::Invalid("invalid attachments".into()));
    }
    if !input.attachment_ids.is_empty() && input.conversation_id.is_none() {
        return Err(SmartFillError::Invalid(
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
        if conversation.record_id != Some(input.record_id)
            || conversation.context_id != input.context_id
        {
            return Err(SmartFillError::Invalid(
                "conversation belongs to another record or context".into(),
            ));
        }
        let recent = repository.conversation_messages(id).await?;
        let mut remaining_attachment_bytes = SMART_FILL_HISTORY_ATTACHMENT_BYTES;
        let mut history = Vec::new();
        for message in recent
            .into_iter()
            .rev()
            .take(SMART_FILL_HISTORY_MESSAGES)
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
                    if let Ok(object) = object_store.get(&file.object_key).await
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
        "record_id": input.record_id,
        "context_id": input.context_id,
        "editable_fields": editable,
        "current_values": current_values,
        "unsaved_draft_values": input.draft_values.iter().filter(|(code, _)| allowed.contains(code.as_str())).collect::<BTreeMap<_, _>>(),
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
            if image.byte_size > MAX_SMART_FILL_IMAGE_BYTES as i64 {
                return Err(SmartFillError::Invalid("image is too large".into()));
            }
            let object = object_store
                .get(&image.object_key)
                .await
                .map_err(|_| SmartFillError::Unavailable("attachment unavailable"))?;
            if object.bytes.len() > MAX_SMART_FILL_IMAGE_BYTES {
                return Err(SmartFillError::Invalid("image is too large".into()));
            }
            parts.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}", image.mime_type, STANDARD.encode(object.bytes))}}));
        } else if file.mime_type.starts_with("text/") || file.mime_type == "application/json" {
            if file.byte_size > MAX_SMART_FILL_TEXT_ATTACHMENT_BYTES as i64 {
                return Err(SmartFillError::Invalid(
                    "text attachment is too large".into(),
                ));
            }
            let object = object_store
                .get(&file.object_key)
                .await
                .map_err(|_| SmartFillError::Unavailable("attachment unavailable"))?;
            if object.bytes.len() > MAX_SMART_FILL_TEXT_ATTACHMENT_BYTES {
                return Err(SmartFillError::Invalid(
                    "text attachment is too large".into(),
                ));
            }
            let text = std::str::from_utf8(&object.bytes)
                .map_err(|_| SmartFillError::Invalid("attachment is not UTF-8".into()))?;
            parts.push(json!({"type":"text","text":format!("Attachment {}:\n{text}",file.display_filename)}));
        } else {
            return Err(SmartFillError::Invalid(
                "only images and text attachments are supported for draft proposals".into(),
            ));
        }
    }
    let provider = OpenAiCompatibleClient::new(config)
        .map_err(|_| SmartFillError::Unavailable("agent provider is unavailable"))?;
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
        .map_err(|_| SmartFillError::Unavailable("agent provider is unavailable"))?;
    let call = completion
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.tool_calls.into_iter().next())
        .filter(|call| call.function.name == "propose_record_form_values")
        .ok_or_else(|| SmartFillError::BadGateway("agent did not return form values"))?;
    if call.function.arguments.len() > MAX_TOOL_CALL_ARGUMENT_BYTES {
        return Err(SmartFillError::BadGateway(
            "agent returned oversized form values",
        ));
    }
    let arguments = serde_json::from_str::<Value>(&call.function.arguments)
        .map_err(|_| SmartFillError::BadGateway("agent returned invalid form values"))?;
    let explanation = arguments
        .get("explanation")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(MAX_SMART_FILL_EXPLANATION_CHARS)
        .collect::<String>();
    let fields = Some(arguments)
        .and_then(|value| value.get("fields").cloned())
        .and_then(|value| serde_json::from_value::<BTreeMap<String, String>>(value).ok())
        .ok_or_else(|| SmartFillError::BadGateway("agent returned invalid form values"))?;
    let fields = fields
        .into_iter()
        .filter(|(code, value)| {
            editable
                .iter()
                .any(|field| field.code == *code && smart_fill_value_is_valid(field, value))
        })
        .collect::<BTreeMap<_, _>>();
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
            .collect::<BTreeMap<_, _>>();
        repository.append_conversation_message(id, None, "assistant", json!({"draft_proposal":{"fields":fields,"explanation":explanation,"base_values":base_values}})).await?;
        let title_repository = repository.clone();
        tokio::spawn(async move {
            crate::conversation_title::maybe_generate_title(&title_repository, &provider, id).await;
        });
    }
    Ok(SmartFillProposal {
        fields,
        explanation,
    })
}
/// Smart fill proposes only writable scalar values; non-default contexts may
/// change only attributes that are editable outside the default context.
fn smart_fill_editable(
    readonly: bool,
    value_type: ValueType,
    context_editable: &str,
    is_default_context: bool,
) -> bool {
    !readonly
        && value_type != ValueType::Relationship
        && value_type != ValueType::File
        && (is_default_context || context_editable != "default")
}

/// Parse a suggestion exactly as the browser will parse a scalar form field,
/// then apply the attribute's JSON schema before offering it to the user.
fn smart_fill_value_is_valid(field: &SmartFillField, proposed: &str) -> bool {
    let value = proposed.trim();
    if value.is_empty() {
        return false;
    }
    let native = match field.value_type {
        ValueType::String => Some(Value::String(value.to_owned())),
        ValueType::Number => value
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number),
        ValueType::Integer => value
            .parse::<i64>()
            .ok()
            .filter(|value| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(value))
            .map(|value| json!(value)),
        ValueType::Boolean => value.parse::<bool>().ok().map(|value| json!(value)),
        ValueType::Date => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|_| value.len() == ISO_DATE_LEN)
            .map(|_| json!(value)),
        ValueType::Datetime => DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|_| json!(value)),
        ValueType::Time => value.split_once(' ').and_then(|(time, zone)| {
            NaiveTime::parse_from_str(time, "%H:%M:%S%.f").ok()?;
            zone.parse::<chrono_tz::Tz>().ok()?;
            Some(json!({"time": time, "time_zone": zone}))
        }),
        ValueType::Json => serde_json::from_str(value).ok(),
        _ => None,
    };
    let Some(native) = native else { return false };
    field.value_schema.as_ref().is_none_or(|schema| {
        validate_json_schema(schema, &native).is_ok_and(|errors| errors.is_empty())
    })
}

fn editable_scalar_values<'a>(
    values: impl Iterator<Item = &'a FormAttributeValue>,
    context_id: Option<Uuid>,
    allowed: &HashSet<&str>,
) -> Vec<&'a FormAttributeValue> {
    values
        .filter(|value| matches!(value, FormAttributeValue::Scalar { attribute_code, context_id: value_context, .. }
            if *value_context == context_id && allowed.contains(attribute_code.as_str())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_editable_scalar_values_in_the_requested_context_reach_the_provider() {
        let context = Uuid::new_v4();
        let values = [
            FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(context),
                value: json!("ok"),
            },
            FormAttributeValue::Scalar {
                attribute_code: "private".into(),
                context_id: Some(context),
                value: json!("secret"),
            },
            FormAttributeValue::Scalar {
                attribute_code: "name".into(),
                context_id: Some(Uuid::new_v4()),
                value: json!("other"),
            },
        ];
        let allowed = HashSet::from(["name"]);
        let filtered = editable_scalar_values(values.iter(), Some(context), &allowed);
        assert_eq!(filtered.len(), 1);
        assert_eq!(serde_json::to_value(filtered[0]).unwrap()["value"], "ok");
    }

    #[test]
    fn suggestions_must_match_the_browser_scalar_type_and_attribute_schema() {
        let field = SmartFillField {
            code: "state".into(),
            name: "State".into(),
            value_type: ValueType::String,
            value_schema: Some(json!({"type": "string", "enum": ["active", "inactive"]})),
        };
        assert!(smart_fill_value_is_valid(&field, "active"));
        assert!(!smart_fill_value_is_valid(&field, "discontinued"));
        let integer = SmartFillField {
            value_type: ValueType::Integer,
            value_schema: None,
            ..field
        };
        assert!(smart_fill_value_is_valid(&integer, "42"));
        assert!(!smart_fill_value_is_valid(&integer, "1.5"));
        assert!(!smart_fill_value_is_valid(&integer, "9223372036854775808"));
        assert!(!smart_fill_value_is_valid(&integer, "-9223372036854775808"));
        let date = SmartFillField {
            value_type: ValueType::Date,
            value_schema: None,
            ..integer
        };
        assert!(smart_fill_value_is_valid(&date, "2026-09-23"));
        assert!(!smart_fill_value_is_valid(&date, "2026-9-23"));
    }
}
