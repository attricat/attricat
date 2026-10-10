use crate::{
    agent_provider::{ChatMessage, OpenAiCompatibleClient},
    repository::AttricatRepository,
};
use serde_json::{Value, json};
use uuid::Uuid;

const TITLE_PROMPT: &str = "Give this catalogue conversation a concise, specific title describing the user's actual task or question. Use the language of the user. Respond with only the title, without quotes, markdown, a label, or a sentence. Do not include credentials or private identifiers.";
const MAX_TITLE_CHARS: usize = 72;
const MAX_CONTEXT_CHARS: usize = 2000;

fn normalize_title(raw: &str) -> Option<String> {
    let title = raw
        .trim()
        .trim_matches(['"', '\'', '`'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let title = title.trim_matches(['"', '\'', '`', '.', ':']).trim();
    if title.is_empty() || title.contains('{') || title.contains('}') {
        return None;
    }
    Some(title.chars().take(MAX_TITLE_CHARS).collect())
}

pub async fn maybe_generate_title(
    repository: &AttricatRepository,
    provider: &OpenAiCompatibleClient,
    conversation_id: Uuid,
) {
    let Ok(conversation) = repository.get_conversation(conversation_id).await else {
        return;
    };
    if conversation.title_source != "pending" {
        return;
    }
    let Ok(messages) = repository.conversation_messages(conversation_id).await else {
        return;
    };
    let first_request = messages.iter().find(|message| message.role == "user");
    let last_answer = messages.iter().rev().find(|message| {
        message.role == "assistant"
            && (message.content.is_string() || message.content.get("draft_proposal").is_some())
    });
    let Some(request) = first_request else {
        return;
    };
    let Some(answer) = last_answer else {
        return;
    };
    let answer_text = answer
        .content
        .as_str()
        .or_else(|| {
            answer
                .content
                .get("draft_proposal")?
                .get("explanation")?
                .as_str()
        })
        .unwrap_or("");
    let input = json!({
        "user_request": request.content.as_str().unwrap_or("").chars().take(MAX_CONTEXT_CHARS).collect::<String>(),
        "attachments": request.attachments.iter().map(|file| file.filename.as_str()).take(8).collect::<Vec<_>>(),
        "assistant_answer": answer_text.chars().take(MAX_CONTEXT_CHARS).collect::<String>(),
    });
    let result = provider
        .complete(
            vec![
                ChatMessage {
                    role: "system".into(),
                    content: Value::String(TITLE_PROMPT.into()),
                    tool_call_id: None,
                    tool_calls: None,
                },
                ChatMessage {
                    role: "user".into(),
                    content: input,
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
            Vec::new(),
        )
        .await;
    match result {
        Ok(completion) => {
            if let Some(title) = completion
                .choices
                .first()
                .and_then(|choice| choice.message.content.as_deref())
                .and_then(normalize_title)
                && let Err(error) = repository
                    .set_generated_conversation_title(conversation_id, &title)
                    .await
            {
                tracing::warn!(%conversation_id, %error, "conversation title could not be saved");
            }
        }
        Err(error) => {
            tracing::warn!(%conversation_id, %error, "conversation title generation failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_title;

    #[test]
    fn strips_wrappers_and_bounds_generated_titles() {
        assert_eq!(
            normalize_title("  \"Product price review\".\n"),
            Some("Product price review".into())
        );
        assert_eq!(normalize_title(" "), None);
        assert_eq!(
            normalize_title(&"a".repeat(200)).unwrap().chars().count(),
            72
        );
    }
}
