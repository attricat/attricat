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

use crate::{
    agent_tools::ToolDefinition,
    agents::{
        AgentProviderConfig, MAX_ASSISTANT_CONTENT_BYTES, MAX_PROVIDER_BODY_BYTES,
        MAX_PROVIDER_FRAME_BUFFER_BYTES, MAX_TOOL_CALL_ARGUMENT_BYTES, MAX_TOOL_CALLS,
    },
};

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
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PROVIDER_BODY_BYTES as u64)
        {
            return Err(ProviderError::Malformed);
        }
        let body = response.bytes().await.map_err(map_request_error)?;
        if body.len() > MAX_PROVIDER_BODY_BYTES {
            return Err(ProviderError::Malformed);
        }
        serde_json::from_slice(&body).map_err(|_| ProviderError::Malformed)
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
        let mut received_bytes: usize = 0;
        let mut content = String::new();
        let mut calls: Vec<PartialToolCall> = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| {
                tracing::warn!(error = %error, "agent provider stream failed");
                ProviderError::Unavailable
            })?;
            received_bytes = received_bytes
                .checked_add(chunk.len())
                .filter(|bytes| *bytes <= MAX_PROVIDER_BODY_BYTES)
                .ok_or(ProviderError::Malformed)?;
            if buffer
                .len()
                .checked_add(chunk.len())
                .is_none_or(|bytes| bytes > MAX_PROVIDER_FRAME_BUFFER_BYTES)
            {
                return Err(ProviderError::Malformed);
            }
            buffer.push_str(std::str::from_utf8(&chunk).map_err(|_| ProviderError::Malformed)?);
            while let Some((end, separator_length)) = sse_frame_end(&buffer) {
                let frame = buffer[..end].to_owned();
                buffer.drain(..end + separator_length);
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
                        if content
                            .len()
                            .checked_add(text.len())
                            .is_none_or(|bytes| bytes > MAX_ASSISTANT_CONTENT_BYTES)
                        {
                            return Err(ProviderError::Malformed);
                        }
                        content.push_str(&text);
                        on_delta(&text);
                    }
                    for call in choice.delta.tool_calls {
                        if call.index >= MAX_TOOL_CALLS {
                            return Err(ProviderError::Malformed);
                        }
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
                                if slot
                                    .arguments
                                    .len()
                                    .checked_add(args.len())
                                    .is_none_or(|bytes| bytes > MAX_TOOL_CALL_ARGUMENT_BYTES)
                                {
                                    return Err(ProviderError::Malformed);
                                }
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
fn sse_frame_end(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|index| (index, 2));
    let crlf = buffer.find("\r\n\r\n").map(|index| (index, 4));
    match (lf, crlf) {
        (Some(lf), Some(crlf)) => Some(if lf.0 < crlf.0 { lf } else { crlf }),
        (Some(frame), None) | (None, Some(frame)) => Some(frame),
        (None, None) => None,
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
    use super::{
        ChatRequest, OpenAiCompatibleClient, PartialToolCall, ProviderError, chat_completions_url,
        finish_calls,
    };
    use crate::agents::{
        AgentProviderConfig, MAX_ASSISTANT_CONTENT_BYTES, MAX_PROVIDER_BODY_BYTES,
        MAX_TOOL_CALL_ARGUMENT_BYTES, MAX_TOOL_CALLS,
    };
    use axum::{
        Router,
        body::Body,
        http::{Response, header},
        routing::post,
    };
    use url::Url;

    async fn mock_client(body: String) -> (OpenAiCompatibleClient, tokio::task::JoinHandle<()>) {
        let frames = body
            .split_inclusive("\n\n")
            .map(|frame| bytes::Bytes::copy_from_slice(frame.as_bytes()))
            .collect::<Vec<_>>();
        let app = Router::new().route(
            "/v1/chat/completions",
            post(move || async move {
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from_stream(futures_util::stream::iter(
                        frames.into_iter().map(Ok::<_, std::convert::Infallible>),
                    )))
                    .unwrap()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let config = AgentProviderConfig::from_values(|name| match name {
            "LLM_API_KEY" => Some("test-key".to_owned()),
            "LLM_BASE_URL" => Some(format!("http://{address}/v1")),
            _ => None,
        })
        .unwrap()
        .unwrap();
        (OpenAiCompatibleClient::new(&config).unwrap(), server)
    }

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
    fn accepts_crlf_delimited_sse_frames() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
            let (client, server) = mock_client(body.to_owned()).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert_eq!(result.unwrap().content.as_deref(), Some("hello"));
        });
    }

    #[test]
    fn ignores_large_reasoning_streams_within_provider_byte_limit() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let frame = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"x\"}}]}\n\n";
            let body = format!(
                "{}data: [DONE]\n\n",
                frame.repeat(128 * 1024 / frame.len() + 1)
            );
            assert!(body.len() > 64 * 1024);
            let (client, server) = mock_client(body).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn rejects_many_small_frames_after_total_provider_byte_limit() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let frame = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n";
            let body = frame.repeat(MAX_PROVIDER_BODY_BYTES / frame.len() + 1);
            let (client, server) = mock_client(body).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert!(matches!(result, Err(ProviderError::Malformed)));
        });
    }

    #[test]
    fn rejects_oversized_fragmented_tool_arguments() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_entity\",\"arguments\":\"\"}}]}}]}\n\n".to_owned();
            let fragment = "x".repeat(1024);
            let frame = format!(
                "data: {{\"choices\":[{{\"delta\":{{\"tool_calls\":[{{\"index\":0,\"function\":{{\"arguments\":\"{fragment}\"}}}}]}}}}]}}\n\n"
            );
            body.push_str(&frame.repeat(MAX_TOOL_CALL_ARGUMENT_BYTES / fragment.len() + 1));
            let (client, server) = mock_client(body).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert!(matches!(result, Err(ProviderError::Malformed)));
        });
    }

    #[test]
    fn rejects_too_many_tool_calls() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":INDEX}]}}]}\n\n"
                .replace("INDEX", &MAX_TOOL_CALLS.to_string());
            let (client, server) = mock_client(body).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert!(matches!(result, Err(ProviderError::Malformed)));
        });
    }

    #[test]
    fn rejects_oversized_accumulated_assistant_text() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fragment = "x".repeat(1024);
            let frame =
                format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{fragment}\"}}}}]}}\n\n");
            let body = format!(
                "{}data: [DONE]\n\n",
                frame.repeat(MAX_ASSISTANT_CONTENT_BYTES / fragment.len() + 1)
            );
            let (client, server) = mock_client(body).await;
            let result = client.stream(vec![], vec![], |_| {}).await;
            server.abort();
            assert!(matches!(result, Err(ProviderError::Malformed)));
        });
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
