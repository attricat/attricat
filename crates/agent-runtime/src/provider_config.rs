//! Server-only configuration for an OpenAI-compatible agent provider.
//!
//! Provider credentials and deployment parsing belong with the worker-owned
//! provider adapter rather than dependency-light domain contracts.

use std::time::Duration;

use attricat_domain::agents::MAX_AGENT_MODEL_BYTES;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use url::Url;

pub const DEFAULT_LLM_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";

const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_RUN_TIMEOUT_SECONDS: u64 = 300;
const MAX_TIMEOUT_SECONDS: u64 = 3_600;
const DEFAULT_MAX_TOOL_ROUNDS: u16 = 25;
const MAX_TOOL_ROUNDS: u16 = 100;
const MAX_REASONING_EFFORT_BYTES: usize = 64;

#[derive(Clone)]
pub struct AgentProviderConfig {
    api_key: SecretString,
    pub base_url: Url,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub request_timeout: Duration,
    pub run_timeout: Duration,
    /// Model and tool round trips one run may take between human approvals.
    pub max_tool_rounds: u16,
    /// Offers the model read tools only, so it can never propose a change.
    /// Public demo deployments set it; it is not read from `LLM_*` settings.
    pub read_only: bool,
}

impl AgentProviderConfig {
    /// Builds a configured provider from `LLM_*` settings and the API key
    /// secret. A missing or blank API key means agents are unavailable;
    /// malformed settings are deployment errors rather than a reason to
    /// silently select another provider.
    pub fn from_values(
        value: impl Fn(&str) -> Option<String>,
        api_key: Option<SecretString>,
    ) -> Result<Option<Self>, AgentConfigError> {
        let Some(api_key) = api_key.filter(|key| !key.expose_secret().trim().is_empty()) else {
            return Ok(None);
        };
        let base_url = value("LLM_BASE_URL").unwrap_or_else(|| DEFAULT_LLM_BASE_URL.to_owned());
        let mut base_url = Url::parse(&base_url).map_err(|_| AgentConfigError::InvalidBaseUrl)?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(AgentConfigError::InvalidBaseUrl);
        }
        // `Url::join` treats a path without a trailing slash as a file. Provider
        // base URLs conventionally end in `/v1`, so normalize it before the
        // client appends `chat/completions`.
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let model = value("LLM_MODEL").unwrap_or_else(|| DEFAULT_LLM_MODEL.to_owned());
        if model.trim().is_empty() || model.len() > MAX_AGENT_MODEL_BYTES {
            return Err(AgentConfigError::InvalidModel);
        }
        let reasoning_effort = value("LLM_REASONING_EFFORT").filter(|effort| !effort.is_empty());
        if reasoning_effort.as_ref().is_some_and(|effort| {
            effort.trim() != effort
                || effort.len() > MAX_REASONING_EFFORT_BYTES
                || effort.chars().any(char::is_whitespace)
        }) {
            return Err(AgentConfigError::InvalidReasoningEffort);
        }
        Ok(Some(Self {
            api_key,
            base_url,
            model,
            reasoning_effort,
            request_timeout: duration_value(
                &value,
                "LLM_REQUEST_TIMEOUT_SECONDS",
                DEFAULT_REQUEST_TIMEOUT_SECONDS,
            )?,
            run_timeout: duration_value(
                &value,
                "LLM_RUN_TIMEOUT_SECONDS",
                DEFAULT_RUN_TIMEOUT_SECONDS,
            )?,
            max_tool_rounds: match value("LLM_MAX_TOOL_ROUNDS") {
                Some(raw) => raw
                    .parse::<u16>()
                    .ok()
                    .filter(|rounds| (1..=MAX_TOOL_ROUNDS).contains(rounds))
                    .ok_or(AgentConfigError::InvalidToolRounds)?,
                None => DEFAULT_MAX_TOOL_ROUNDS,
            },
            read_only: false,
        }))
    }

    /// Only the provider adapter may expose the credential. HTTP state,
    /// responses, logs, and persistence never receive its plaintext value.
    pub(crate) fn api_key(&self) -> &str {
        self.api_key.expose_secret()
    }
}

fn duration_value(
    value: &impl Fn(&str) -> Option<String>,
    name: &'static str,
    default: u64,
) -> Result<Duration, AgentConfigError> {
    let seconds = match value(name) {
        Some(raw) => raw
            .parse::<u64>()
            .map_err(|_| AgentConfigError::InvalidDuration(name))?,
        None => default,
    };
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&seconds) {
        return Err(AgentConfigError::InvalidDuration(name));
    }
    Ok(Duration::from_secs(seconds))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentConfigError {
    #[error("LLM_BASE_URL must be an absolute HTTP(S) URL")]
    InvalidBaseUrl,
    #[error(
        "LLM_MODEL must be non-empty and at most {max} characters",
        max = MAX_AGENT_MODEL_BYTES
    )]
    InvalidModel,
    #[error(
        "LLM_REASONING_EFFORT must be a single token of at most {max} characters",
        max = MAX_REASONING_EFFORT_BYTES
    )]
    InvalidReasoningEffort,
    #[error("{0} must be an integer between 1 and {max} seconds", max = MAX_TIMEOUT_SECONDS)]
    InvalidDuration(&'static str),
    #[error(
        "LLM_MAX_TOOL_ROUNDS must be an integer between 1 and {max}",
        max = MAX_TOOL_ROUNDS
    )]
    InvalidToolRounds,
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::{
        AgentConfigError, AgentProviderConfig, DEFAULT_LLM_MODEL, DEFAULT_MAX_TOOL_ROUNDS,
    };

    fn key(value: &str) -> Option<SecretString> {
        Some(SecretString::from(value))
    }

    #[test]
    fn missing_or_blank_key_disables_agents() {
        assert!(
            AgentProviderConfig::from_values(|_| None, None)
                .unwrap()
                .is_none()
        );
        assert!(
            AgentProviderConfig::from_values(|_| None, key("  "))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn validates_provider_settings_and_uses_safe_defaults() {
        let configured = AgentProviderConfig::from_values(|_| None, key("secret"))
            .unwrap()
            .unwrap();
        assert_eq!(configured.base_url.as_str(), "https://api.openai.com/v1/");
        assert_eq!(configured.model, DEFAULT_LLM_MODEL);
        assert_eq!(configured.reasoning_effort, None);
        assert_eq!(configured.max_tool_rounds, DEFAULT_MAX_TOOL_ROUNDS);
        let configured = AgentProviderConfig::from_values(
            |name| (name == "LLM_REASONING_EFFORT").then(|| "none".to_owned()),
            key("secret"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(configured.reasoning_effort.as_deref(), Some("none"));
        let configured = AgentProviderConfig::from_values(
            |name| (name == "LLM_MAX_TOOL_ROUNDS").then(|| "40".to_owned()),
            key("secret"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(configured.max_tool_rounds, 40);
        for (name, value, expected) in [
            (
                "LLM_BASE_URL",
                "ftp://provider.test",
                AgentConfigError::InvalidBaseUrl,
            ),
            ("LLM_MODEL", " ", AgentConfigError::InvalidModel),
            (
                "LLM_REASONING_EFFORT",
                "not valid",
                AgentConfigError::InvalidReasoningEffort,
            ),
            (
                "LLM_REQUEST_TIMEOUT_SECONDS",
                "0",
                AgentConfigError::InvalidDuration("LLM_REQUEST_TIMEOUT_SECONDS"),
            ),
            (
                "LLM_MAX_TOOL_ROUNDS",
                "0",
                AgentConfigError::InvalidToolRounds,
            ),
            (
                "LLM_MAX_TOOL_ROUNDS",
                "101",
                AgentConfigError::InvalidToolRounds,
            ),
            (
                "LLM_MAX_TOOL_ROUNDS",
                "many",
                AgentConfigError::InvalidToolRounds,
            ),
        ] {
            let result = AgentProviderConfig::from_values(
                |key| (key == name).then(|| value.to_owned()),
                key("secret"),
            );
            assert_eq!(result.err(), Some(expected));
        }
    }
}
