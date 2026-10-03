//! API application compatibility facade.
//!
//! Implementations live in focused workspace crates. This crate keeps the
//! historical `api::…` paths stable while the binaries remain composition roots.

pub use catalog_agent_runtime::agents;
pub use catalog_domain::{account, model, task_queue};

pub mod constants {
    //! Compatibility facade for constants now owned by their implementation layers.

    pub use catalog_domain::constants::*;
    pub use catalog_http::constants::{
        DEFAULT_HTTP_DEFAULT_BODY_BYTES, DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
    };
    pub use catalog_repository::constants::{REQUEST_POOL_CONNECTIONS, TASK_POOL_CONNECTIONS};

    pub const MAINTENANCE_POOL_CONNECTIONS: u32 = 1;
}
pub use catalog_agent_runtime::{agent_provider, agent_runner, agent_tools, agent_worker};
pub use catalog_events as domain_events;
pub use catalog_extension_manifest::{extension_policy, extensions};
pub use catalog_extension_runtime as extension_runtime;
pub use catalog_http::{http, mail, telemetry};
pub use catalog_repository::{
    catalog_read_service, catalog_service, extension_installer, extension_registry, file_access,
    repository, solution_pack_extensions,
};
pub use catalog_solution_pack::{solution_pack_sample_data, solution_pack_seeds, solution_packs};
pub use catalog_storage as storage;
pub use catalog_workers::{
    blueprint_migration_worker, event_dispatcher, file_worker, maintenance, rule_runtime,
    solution_pack_housekeeping, task_worker, workflow_runtime,
};

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
