//! Draft-only record form proposals ("smart fill"): the agent provider
//! suggests values for a record's editable scalar fields from the user's
//! request, the unsaved draft, attachments and conversation history.
use std::collections::{BTreeMap, HashSet};

use attricat_validation::files::is_text_extraction_mime;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    agent_provider::{AssistantMessage, ChatMessage, OpenAiCompatibleClient, ToolChoice},
    agent_runner::language_prompt,
    agents::{
        MAX_CONVERSATION_ATTACHMENTS, MAX_CONVERSATION_MESSAGE_BYTES, MAX_TOOL_CALL_ARGUMENT_BYTES,
    },
    documents::{self, DocumentTextError},
    provider_config::AgentProviderConfig,
    record_form::{self, RecordForm, RecordFormError},
    repository::{AttricatRepository, FileObject, RepositoryError},
};
use attricat_storage::ObjectStore;

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
    /// The sender's BCP 47 interface language; the explanation is written in it.
    #[serde(default)]
    pub language: Option<String>,
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

/// Most recent conversation messages replayed to the provider as history.
const SMART_FILL_HISTORY_MESSAGES: usize = 16;
/// Combined size of the text attachments inlined from conversation history.
const SMART_FILL_HISTORY_ATTACHMENT_BYTES: usize = 64 * 1024;
const MAX_SMART_FILL_TEXT_ATTACHMENT_BYTES: usize = 64 * 1024;
const MAX_SMART_FILL_IMAGE_BYTES: usize = 1024 * 1024;

const SMART_FILL_SYSTEM_PROMPT: &str = "You are a record editing assistant. Answer the user's editing request using the selected record context, current unsaved form draft and conversation history. Propose only fields that should change, using supplied editable scalar field codes. Never invent facts. Return exactly one propose_record_form_values tool call with a short explanation of changes and uncertainties. If the user asks a question instead of requesting an edit, answer it in the explanation and propose no fields. The values are draft-only and require explicit user application and a separate save; never claim they were saved. Do not propose relationship or file fields.";

/// A document's text within `max_bytes`, marked when cut. A document without
/// readable text is described rather than rejected, so the request can still
/// use the rest of its context.
async fn document_attachment_text(
    repository: &AttricatRepository,
    object_store: &dyn ObjectStore,
    file_id: Uuid,
    file: &FileObject,
    max_bytes: usize,
) -> Result<String, SmartFillError> {
    match documents::load(repository, object_store, file_id, file).await {
        Ok(document) => {
            let text = documents::render(&document.sections);
            let window = documents::window(&text, 0, usize::MAX, max_bytes);
            Ok(if window.end < window.total {
                format!("{}\n[the rest of the document was cut]", window.text)
            } else {
                window.text
            })
        }
        Err(DocumentTextError::Unavailable(failure)) => {
            Ok(format!("[no readable text: {}]", failure.as_str()))
        }
        Err(DocumentTextError::Storage) => {
            Err(SmartFillError::Unavailable("attachment unavailable"))
        }
        Err(DocumentTextError::Repository(error)) => Err(error.into()),
    }
}

const MISSING_FORM_VALUES: &str = "agent did not return form values";
const INVALID_FORM_VALUES: &str = "agent returned invalid form values";

/// The proposal's arguments. A provider that ignores the required tool may
/// still answer in text, such as about a long attachment; that answer becomes
/// the explanation of a proposal without fields, as for a question.
fn smart_fill_arguments(message: Option<AssistantMessage>) -> Result<Value, SmartFillError> {
    let message = message.ok_or(SmartFillError::BadGateway(MISSING_FORM_VALUES))?;
    let Some(call) = message
        .tool_calls
        .into_iter()
        .next()
        .filter(|call| call.function.name == record_form::PROPOSE_TOOL)
    else {
        return message
            .content
            .filter(|content| !content.trim().is_empty())
            .map(|content| json!({"fields": {}, "explanation": content}))
            .ok_or(SmartFillError::BadGateway(MISSING_FORM_VALUES));
    };
    if call.function.arguments.len() > MAX_TOOL_CALL_ARGUMENT_BYTES {
        return Err(SmartFillError::BadGateway(
            "agent returned oversized form values",
        ));
    }
    serde_json::from_str::<Value>(&call.function.arguments)
        .map_err(|_| SmartFillError::BadGateway(INVALID_FORM_VALUES))
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
    {
        return Err(SmartFillError::Invalid(format!(
            "content must be between 1 and {MAX_CONVERSATION_MESSAGE_BYTES} bytes"
        )));
    }
    record_form::check_draft_values(&input.draft_values).map_err(SmartFillError::Invalid)?;
    let form = RecordForm::load(
        repository,
        input.record_id,
        input.context_id,
        input.draft_values,
    )
    .await
    .map_err(|error| match error {
        RecordFormError::Invalid(message) => SmartFillError::Invalid(message),
        RecordFormError::NotFound(what) => SmartFillError::NotFound(what),
        RecordFormError::Repository(error) => SmartFillError::Repository(error),
    })?;
    if input.is_default_context != form.is_default_context {
        return Err(SmartFillError::Invalid(
            "is_default_context does not match context_id".to_owned(),
        ));
    }
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
                if is_text_extraction_mime(&file.mime_type) && remaining_attachment_bytes > 0 {
                    // Like an unreadable text file, a document that cannot be
                    // read now is noted rather than failing the request.
                    match document_attachment_text(
                        repository,
                        object_store,
                        attachment.id,
                        &file,
                        remaining_attachment_bytes,
                    )
                    .await
                    {
                        Ok(text) => {
                            remaining_attachment_bytes =
                                remaining_attachment_bytes.saturating_sub(text.len());
                            prior_files
                                .push(json!({"filename":file.display_filename,"text":text}));
                        }
                        Err(_) => prior_files.push(json!({"filename":file.display_filename,"note":"Reattach this file if its contents are needed."})),
                    }
                } else if (file.mime_type.starts_with("text/")
                    || file.mime_type == "application/json")
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
    let mut prompt = form.describe();
    prompt["conversation_history"] = Value::Array(history);
    prompt["user_request"] = Value::String(input.content.clone());
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
        } else if is_text_extraction_mime(&file.mime_type) {
            let text = document_attachment_text(
                repository,
                object_store,
                *file_id,
                &file,
                MAX_SMART_FILL_TEXT_ATTACHMENT_BYTES,
            )
            .await?;
            parts.push(json!({"type":"text","text":format!("Attachment {}:\n{text}",file.display_filename)}));
        } else {
            return Err(SmartFillError::Invalid(
                "only images, text, PDF, Word and Excel attachments are supported for draft proposals".into(),
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
                    content: Value::String(format!(
                        "{SMART_FILL_SYSTEM_PROMPT}{}",
                        language_prompt(input.language.as_deref())
                    )),
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
            vec![record_form::propose_tool_definition()],
            Some(ToolChoice::function(record_form::PROPOSE_TOOL)),
        )
        .await
        .map_err(|_| SmartFillError::Unavailable("agent provider is unavailable"))?;
    let arguments = smart_fill_arguments(
        completion
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message),
    )?;
    let proposal = form
        .proposal(&arguments)
        .ok_or(SmartFillError::BadGateway(INVALID_FORM_VALUES))?;
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
        // Without values to apply, the explanation is an ordinary reply.
        let content = if proposal.fields.is_empty() {
            Value::String(proposal.explanation.clone())
        } else {
            form.proposal_message(&proposal)
        };
        repository
            .append_conversation_message(id, None, "assistant", content)
            .await?;
        let title_repository = repository.clone();
        tokio::spawn(async move {
            crate::conversation_title::maybe_generate_title(&title_repository, &provider, id).await;
        });
    }
    Ok(SmartFillProposal {
        fields: proposal.fields,
        explanation: proposal.explanation,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    fn assistant_message(content: Option<&str>, call: Option<(&str, &str)>) -> AssistantMessage {
        serde_json::from_value(json!({
            "content": content,
            "tool_calls": call.map(|(name, arguments)| vec![json!({
                "id": "call_1",
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            })]).unwrap_or_default(),
        }))
        .unwrap()
    }

    #[test]
    fn proposals_come_from_the_tool_call() {
        let arguments = smart_fill_arguments(Some(assistant_message(
            Some("ignored"),
            Some((
                record_form::PROPOSE_TOOL,
                r#"{"fields":{"name":"Golf"},"explanation":"From the brochure."}"#,
            )),
        )))
        .unwrap();
        assert_eq!(arguments["fields"]["name"], "Golf");
    }

    #[test]
    fn a_text_answer_without_the_tool_proposes_no_fields() {
        let arguments = smart_fill_arguments(Some(assistant_message(
            Some("The PDF does not name a model year."),
            None,
        )))
        .unwrap();
        assert_eq!(arguments["fields"], json!({}));
        assert_eq!(
            arguments["explanation"],
            "The PDF does not name a model year."
        );
    }

    #[test]
    fn an_empty_answer_or_another_tool_is_rejected() {
        for message in [
            None,
            Some(assistant_message(Some("  "), None)),
            Some(assistant_message(None, Some(("other_tool", "{}")))),
        ] {
            assert!(matches!(
                smart_fill_arguments(message),
                Err(SmartFillError::BadGateway(MISSING_FORM_VALUES))
            ));
        }
    }
}
