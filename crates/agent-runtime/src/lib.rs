//! Agent provider, tool execution, and durable agent-run task handler.

pub use attricat_domain::{constants, model};
pub use attricat_repository::{attricat_read_service, attricat_service, file_access, repository};
pub use attricat_storage as storage;
pub use attricat_workers::{task_queue, task_worker};

pub mod agent_provider;
pub mod agent_runner;
pub mod agents {
    //! Agent contracts and provider configuration.
    pub use crate::provider_config::{
        AgentConfigError, AgentProviderConfig, DEFAULT_LLM_BASE_URL, DEFAULT_LLM_MODEL,
    };
    pub use attricat_domain::agents::*;
}
pub mod agent_tools;
pub mod agent_worker;
pub mod conversation_title;
pub mod provider_config;
mod search_filters;
