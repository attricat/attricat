//! Server-only configuration for an OpenAI-compatible agent provider.
//!
//! This module deliberately contains configuration only. It neither sends
//! requests nor serializes credentials, so callers can safely use it to decide
//! whether agent features are available.

use std::{env, time::Duration};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use url::Url;

pub const DEFAULT_LLM_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_RUN_TIMEOUT_SECONDS: u64 = 300;
const MAX_TIMEOUT_SECONDS: u64 = 3_600;

#[allow(dead_code)]
#[derive(Clone)]
pub struct AgentProviderConfig {
    api_key: SecretString,
    pub base_url: Url,
    pub model: String,
    pub request_timeout: Duration,
    pub run_timeout: Duration,
    pub scheduler_poll_interval: Duration,
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
        let base_url = Url::parse(&base_url).map_err(|_| AgentConfigError::InvalidBaseUrl)?;
        if !matches!(base_url.scheme(), "http" | "https") || base_url.host_str().is_none() {
            return Err(AgentConfigError::InvalidBaseUrl);
        }
        let model = value("LLM_MODEL").unwrap_or_else(|| DEFAULT_LLM_MODEL.to_owned());
        if model.trim().is_empty() || model.len() > 512 {
            return Err(AgentConfigError::InvalidModel);
        }
        Ok(Some(Self {
            api_key: SecretString::from(api_key),
            base_url,
            model,
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
            scheduler_poll_interval: duration_value(&value, "AGENT_SCHEDULER_POLL_SECONDS", 15)?,
        }))
    }

    /// This is intentionally crate-private: only the future provider client may
    /// read it. HTTP state, responses, logs, and persistence never receive it.
    #[allow(dead_code)]
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
    #[error("{0} must be an integer between 1 and 3600 seconds")]
    InvalidDuration(&'static str),
}
