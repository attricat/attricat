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
}
#[derive(Clone, Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub stream: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolFunctionCall,
}
#[derive(Clone, Debug, Deserialize)]
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
        let url = self
            .base_url
            .join("chat/completions")
            .map_err(|_| ProviderError::Malformed)?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: self.model.clone(),
                messages,
                tools,
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
        let url = self
            .base_url
            .join("chat/completions")
            .map_err(|_| ProviderError::Malformed)?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(&ChatRequest {
                model: self.model.clone(),
                messages,
                tools,
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
            let chunk = chunk.map_err(|_| ProviderError::Unavailable)?;
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
fn status(status: StatusCode) -> Result<(), ProviderError> {
    if status.is_success() {
        Ok(())
    } else if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        Err(ProviderError::Unavailable)
    } else {
        Err(ProviderError::Rejected)
    }
}
fn map_request_error(error: reqwest::Error) -> ProviderError {
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
