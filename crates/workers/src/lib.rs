//! Durable background execution and provider adapters.

pub use catalog_domain::{constants, model, task_queue};
pub use catalog_events as domain_events;
pub use catalog_repository::{catalog_read_service, catalog_service, file_access, repository};
pub use catalog_storage as storage;

pub mod agent_provider;
pub mod agent_runner;
pub mod agents {
    //! Compatibility facade combining domain invariants with worker-owned provider configuration.

    pub use crate::provider_config::{
        AgentConfigError, AgentProviderConfig, DEFAULT_LLM_BASE_URL, DEFAULT_LLM_MODEL,
    };
    pub use catalog_domain::agents::*;
}
pub mod agent_tools;
pub mod agent_worker;
pub mod blueprint_migration_worker;
pub mod event_dispatcher;
pub mod file_worker;
pub mod provider_config;
pub mod rule_runtime;
pub mod task_worker;
pub mod workflow_runtime;
