//! Small OpenAI-compatible Chat Completions client.
//!
//! Only normalized, bounded data crosses this boundary. In particular API keys
//! and arbitrary provider response bodies are never persisted as run events.

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::{agent_tools::ToolDefinition, agents::AgentProviderConfig};

pub const MAX_PROVIDER_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    client: Client,
    base_url: Url,
    api_key: String,
    model: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    /// Agent approvals are sequential, so a response must contain at most one
    /// call. This prevents an unresolved call from invalidating the next turn.
    pub parallel_tool_calls: bool,
    pub stream: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunctionCall,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolFunctionCall {
    pub name: String,
    pub arguments: String,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletion {
    pub choices: Vec<Choice>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Choice {
    pub message: AssistantMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AssistantMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider request timed out")]
    Timeout,
    #[error("provider is unavailable")]
    Unavailable,
    #[error("provider rejected the request")]
    Rejected,
    #[error("provider returned malformed data")]
    Malformed,
}

impl OpenAiCompatibleClient {
    pub fn new(config: &AgentProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|_| ProviderError::Unavailable)?;
        Ok(Self {
            client,
            base_url: config.base_url.clone(),
            api_key: config.api_key().to_owned(),
            model: config.model.clone(),
        })
    }
    pub async fn complete(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<ChatCompletion, ProviderError> {
        let url = chat_completions_url(&self.base_url);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: self.model.clone(),
                messages,
                tools,
                parallel_tool_calls: false,
                stream: false,
            })
            .send()
            .await
            .map_err(map_request_error)?;
        status(response.status())?;
        response.json().await.map_err(|_| ProviderError::Malformed)
    }
    /// Reads OpenAI SSE frames and invokes `on_delta` for each non-empty text
    /// delta. Tool calls are returned after the terminal frame.
    pub async fn stream(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
        mut on_delta: impl FnMut(&str),
    ) -> Result<AssistantMessage, ProviderError> {
        let url = chat_completions_url(&self.base_url);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: self.model.clone(),
                messages,
                tools,
                parallel_tool_calls: false,
                stream: true,
            })
            .send()
            .await
            .map_err(map_request_error)?;
        status(response.status())?;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut calls: Vec<PartialToolCall> = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                tracing::warn!(error = %error, "agent provider stream failed");
                ProviderError::Unavailable
            })?;
            if buffer.len() + chunk.len() > MAX_PROVIDER_BODY_BYTES {
                return Err(ProviderError::Malformed);
            }
            buffer.push_str(std::str::from_utf8(&chunk).map_err(|_| ProviderError::Malformed)?);
            while let Some(end) = buffer.find("\n\n") {
                let frame = buffer[..end].to_owned();
                buffer.drain(..end + 2);
                let data = frame
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                if data == "[DONE]" {
                    return Ok(AssistantMessage {
                        content: (!content.is_empty()).then_some(content),
                        tool_calls: finish_calls(calls)?,
                    });
                }
                let event: StreamChunk =
                    serde_json::from_str(&data).map_err(|_| ProviderError::Malformed)?;
                for choice in event.choices {
                    if let Some(text) = choice.delta.content {
                        content.push_str(&text);
                        on_delta(&text);
                    }
                    for call in choice.delta.tool_calls {
                        while calls.len() <= call.index {
                            calls.push(PartialToolCall::default());
                        }
                        let slot = &mut calls[call.index];
                        if let Some(id) = call.id {
                            slot.id = id;
                        }
                        if let Some(kind) = call.kind {
                            slot.kind = kind;
                        }
                        if let Some(function) = call.function {
                            if let Some(name) = function.name {
                                slot.name = name;
                            }
                            if let Some(args) = function.arguments {
                                slot.arguments.push_str(&args);
                            }
                        }
                    }
                }
            }
        }
        Err(ProviderError::Malformed)
    }
}
fn chat_completions_url(base_url: &Url) -> Url {
    let mut url = base_url.clone();
    url.set_path(&format!(
        "{}/chat/completions",
        url.path().trim_end_matches('/')
    ));
    url
}

fn status(status: StatusCode) -> Result<(), ProviderError> {
    if status.is_success() {
        Ok(())
    } else if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        tracing::warn!(%status, "agent provider returned an unavailable status");
        Err(ProviderError::Unavailable)
    } else {
        tracing::warn!(%status, "agent provider rejected the request");
        Err(ProviderError::Rejected)
    }
}
fn map_request_error(error: reqwest::Error) -> ProviderError {
    tracing::warn!(error = %format!("{error:#}"), "agent provider HTTP request failed");
    if error.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Unavailable
    }
}
#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}
#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}
#[derive(Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<StreamToolCall>,
}
#[derive(Deserialize)]
struct StreamToolCall {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    function: Option<StreamFunction>,
}
#[derive(Deserialize)]
struct StreamFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}
#[derive(Default)]
struct PartialToolCall {
    id: String,
    kind: String,
    name: String,
    arguments: String,
}
fn finish_calls(calls: Vec<PartialToolCall>) -> Result<Vec<ToolCall>, ProviderError> {
    calls
        .into_iter()
        .map(|c| {
            if c.id.is_empty() || c.name.is_empty() {
                Err(ProviderError::Malformed)
            } else {
                Ok(ToolCall {
                    id: c.id,
                    kind: if c.kind.is_empty() {
                        "function".into()
                    } else {
                        c.kind
                    },
                    function: ToolFunctionCall {
                        name: c.name,
                        arguments: c.arguments,
                    },
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ChatRequest, PartialToolCall, ProviderError, chat_completions_url, finish_calls};
    use url::Url;

    #[test]
    fn preserves_the_provider_version_path_when_building_chat_url() {
        assert_eq!(
            chat_completions_url(&Url::parse("http://provider.test/v1").unwrap()).as_str(),
            "http://provider.test/v1/chat/completions"
        );
    }

    #[test]
    fn disables_parallel_tool_calls_in_provider_requests() {
        let request = ChatRequest {
            model: "test".into(),
            messages: vec![],
            tools: vec![],
            parallel_tool_calls: false,
            stream: true,
        };
        assert_eq!(
            serde_json::to_value(request).unwrap()["parallel_tool_calls"],
            false
        );
    }

    #[test]
    fn completes_fragmented_tool_calls_and_rejects_incomplete_ones() {
        let calls = finish_calls(vec![PartialToolCall {
            id: "call_1".into(),
            kind: String::new(),
            name: "get_entity".into(),
            arguments: r#"{"entity_id":"abc"}"#.into(),
        }])
        .unwrap();
        assert_eq!(calls[0].kind, "function");
        assert_eq!(calls[0].function.name, "get_entity");
        assert!(matches!(
            finish_calls(vec![PartialToolCall::default()]),
            Err(ProviderError::Malformed)
        ));
    }
}
