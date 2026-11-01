//! SQLx-backed catalog persistence and repository-coupled application services.

pub use attricat_domain::{account, agents, model, task_queue};
pub use attricat_events as domain_events;
pub use attricat_extension_manifest::{extension_policy, extensions};
pub use attricat_solution_pack::{solution_pack_sample_data, solution_pack_seeds, solution_packs};
pub use attricat_storage as storage;

pub mod attricat_read_service;
pub mod attricat_service;
mod blueprint_resolver;
pub mod constants;
pub mod extension_installer;
pub mod extension_registry;
pub mod file_access;
mod persistence_rows;
pub mod repository;
pub mod round_trips;
pub mod solution_pack_extensions;

/// The API migrations, which readiness compares with the database. Tests use
/// this too: a bare `#[sqlx::test]` embeds every migration again in each test.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../apps/api/migrations");
#[cfg(test)]
pub(crate) use MIGRATOR as TEST_MIGRATOR;
