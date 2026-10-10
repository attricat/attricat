//! API application compatibility facade.
//!
//! Implementations live in focused workspace crates. This crate keeps the
//! historical `api::…` paths stable while the binaries remain composition roots.

pub use attricat_agent_runtime::agents;
pub use attricat_domain::{account, model, task_queue};

pub mod constants {
    //! Compatibility facade for constants now owned by their implementation layers.

    pub use attricat_domain::constants::*;
    pub use attricat_http::constants::{
        DEFAULT_HTTP_DEFAULT_BODY_BYTES, DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS,
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
    };
    pub use attricat_repository::constants::{REQUEST_POOL_CONNECTIONS, TASK_POOL_CONNECTIONS};

    pub const MAINTENANCE_POOL_CONNECTIONS: u32 = 1;
}
pub use attricat_agent_runtime::{agent_provider, agent_runner, agent_tools, agent_worker};
pub use attricat_events as domain_events;
pub use attricat_extension_manifest::{extension_policy, extensions};
pub use attricat_extension_runtime as extension_runtime;
pub use attricat_http::{http, mail, telemetry};
pub use attricat_repository::{
    attricat_read_service, attricat_service, extension_installer, extension_registry, file_access,
    repository, solution_pack_extensions,
};
pub use attricat_solution_pack::{solution_pack_sample_data, solution_pack_seeds, solution_packs};
pub use attricat_storage as storage;
pub use attricat_workers::{
    blueprint_migration_worker, event_dispatcher, file_worker, maintenance, rule_runtime,
    solution_pack_housekeeping, task_worker, workflow_runtime,
};

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
