//! Agent provider, tool execution, and durable agent-run task handler.

pub use catalog_domain::{constants, model};
pub use catalog_repository::{catalog_read_service, catalog_service, file_access, repository};
pub use catalog_storage as storage;
pub use catalog_workers::{task_queue, task_worker};

pub mod agent_provider;
pub mod agent_runner;
pub mod agents {
    //! Agent contracts and provider configuration.
    pub use crate::provider_config::{
        AgentConfigError, AgentProviderConfig, DEFAULT_LLM_BASE_URL, DEFAULT_LLM_MODEL,
    };
    pub use catalog_domain::agents::*;
}
pub mod agent_tools;
pub mod agent_worker;
pub mod provider_config;
