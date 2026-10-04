//! SQLx-backed catalog persistence and repository-coupled application services.

pub use catalog_domain::{account, agents, model, task_queue};
pub use catalog_events as domain_events;
pub use catalog_extension_manifest::{extension_policy, extensions};
pub use catalog_solution_pack::{solution_pack_sample_data, solution_pack_seeds, solution_packs};
pub use catalog_storage as storage;

mod blueprint_resolver;
pub mod catalog_read_service;
pub mod catalog_service;
pub mod constants;
pub mod extension_installer;
pub mod extension_registry;
pub mod file_access;
mod persistence_rows;
pub mod repository;
pub mod round_trips;
pub mod solution_pack_extensions;
