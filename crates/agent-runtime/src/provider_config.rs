//! Server-only configuration for an OpenAI-compatible agent provider.
//!
//! Provider credentials and deployment parsing belong with the worker-owned
//! provider adapter rather than dependency-light domain contracts.

use std::{env, time::Duration};

use catalog_domain::agents::MAX_AGENT_MODEL_BYTES;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use url::Url;

pub const DEFAULT_LLM_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";

const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_RUN_TIMEOUT_SECONDS: u64 = 300;
const MAX_TIMEOUT_SECONDS: u64 = 3_600;

#[derive(Clone)]
pub struct AgentProviderConfig {
    api_key: SecretString,
    pub base_url: Url,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub request_timeout: Duration,
    pub run_timeout: Duration,
}

impl AgentProviderConfig {
    /// Loads a configured provider. A missing or blank API key means agents are
    /// unavailable; malformed settings are deployment errors rather than a
    /// reason to silently select another provider.
    pub fn from_env() -> Result<Option<Self>, AgentConfigError> {
        Self::from_values(|name| env::var(name).ok())
    }

    pub fn from_values(
        value: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, AgentConfigError> {
        let Some(api_key) = value("LLM_API_KEY").filter(|key| !key.trim().is_empty()) else {
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
            effort.trim() != effort || effort.len() > 64 || effort.chars().any(char::is_whitespace)
        }) {
            return Err(AgentConfigError::InvalidReasoningEffort);
        }
        Ok(Some(Self {
            api_key: SecretString::from(api_key),
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
    #[error("LLM_MODEL must be non-empty and at most 512 characters")]
    InvalidModel,
    #[error("LLM_REASONING_EFFORT must be a single token of at most 64 characters")]
    InvalidReasoningEffort,
    #[error("{0} must be an integer between 1 and 3600 seconds")]
    InvalidDuration(&'static str),
}

#[cfg(test)]
mod tests {
    use super::{AgentConfigError, AgentProviderConfig, DEFAULT_LLM_MODEL};

    #[test]
    fn missing_or_blank_key_disables_agents() {
        assert!(
            AgentProviderConfig::from_values(|_| None)
                .unwrap()
                .is_none()
        );
        assert!(
            AgentProviderConfig::from_values(|name| {
                (name == "LLM_API_KEY").then(|| "  ".to_owned())
            })
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn validates_provider_settings_and_uses_safe_defaults() {
        let configured = AgentProviderConfig::from_values(|name| match name {
            "LLM_API_KEY" => Some("secret".to_owned()),
            _ => None,
        })
        .unwrap()
        .unwrap();
        assert_eq!(configured.base_url.as_str(), "https://api.openai.com/v1/");
        assert_eq!(configured.model, DEFAULT_LLM_MODEL);
        assert_eq!(configured.reasoning_effort, None);
        let configured = AgentProviderConfig::from_values(|name| match name {
            "LLM_API_KEY" => Some("secret".to_owned()),
            "LLM_REASONING_EFFORT" => Some("none".to_owned()),
            _ => None,
        })
        .unwrap()
        .unwrap();
        assert_eq!(configured.reasoning_effort.as_deref(), Some("none"));
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
        ] {
            let result = AgentProviderConfig::from_values(|key| match key {
                "LLM_API_KEY" => Some("secret".to_owned()),
                key if key == name => Some(value.to_owned()),
                _ => None,
            });
            assert_eq!(result.err(), Some(expected));
        }
    }
}
