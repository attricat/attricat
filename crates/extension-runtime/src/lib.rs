//! Sandboxed Wasmtime component execution for installed extensions.

pub use catalog_domain::{constants, model, task_queue};
pub use catalog_events as domain_events;
pub use catalog_extension_manifest::extensions;
pub use catalog_repository::{catalog_service, extension_installer, repository};
pub use catalog_storage as storage;
pub use catalog_workers::task_worker;

mod extension_runtime;
pub use extension_runtime::*;
