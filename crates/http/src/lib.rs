//! Axum transport, request authorization, mail delivery, and API telemetry.

pub use attricat_agent_runtime::agents;
pub use attricat_domain::{account, model};
pub use attricat_extension_manifest::extensions;
pub use attricat_extension_runtime as extension_runtime;
pub use attricat_repository::{
    attricat_read_service, attricat_service, extension_installer, extension_registry, file_access,
    repository, solution_pack_extensions,
};
pub use attricat_solution_pack::{solution_pack_sample_data, solution_pack_seeds, solution_packs};
pub use attricat_storage as storage;

pub mod constants;
pub mod http;
pub mod mail;
pub mod telemetry;
