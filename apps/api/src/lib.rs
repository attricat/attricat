//! The API application: its binaries' configuration and process runtime,
//! the database migrations, and one `api::…` import root over the workspace
//! crates for the binaries and the integration tests.

pub use attricat_agent_runtime::agents;
pub use attricat_domain::{account, model, permissions, states, task_queue, value_type};

pub mod constants {
    //! Constants of the layers the binaries compose, plus the maintenance pool size.

    pub use attricat_domain::constants::*;
    pub use attricat_http::constants::{
        DEFAULT_HTTP_DEFAULT_BODY_BYTES, DEFAULT_HTTP_EVENT_STREAM_LIFETIME_SECONDS,
        DEFAULT_HTTP_MAX_CONCURRENT_REQUESTS, DEFAULT_HTTP_MAX_EVENT_STREAMS,
        DEFAULT_HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL, DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
        READINESS_PROBE_PERMITS,
    };
    pub use attricat_repository::constants::{REQUEST_POOL_CONNECTIONS, TASK_POOL_CONNECTIONS};

    pub const MAINTENANCE_POOL_CONNECTIONS: u32 = 1;
}
pub use attricat_agent_runtime::{
    agent_provider, agent_runner, agent_tools, agent_worker, bulk_edits, smart_fill,
};
pub use attricat_events as domain_events;
pub use attricat_extension_manifest::{extension_policy, extensions};
pub use attricat_extension_runtime as extension_runtime;
pub use attricat_http::{http, mail, telemetry};
pub use attricat_repository::{
    attricat_service, extension_installer, extension_registry, file_access, repository,
    solution_pack_extensions,
};
pub use attricat_solution_pack::solution_packs;
pub use attricat_storage as storage;
pub use attricat_workers::{
    blueprint_migration_worker, event_dispatcher, file_worker, maintenance, rule_runtime,
    solution_pack_housekeeping, task_worker, workflow_runtime,
};

pub mod config;
pub mod runtime;

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
