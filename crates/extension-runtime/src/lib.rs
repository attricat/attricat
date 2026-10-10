//! Sandboxed Wasmtime component execution for installed extensions.

pub use attricat_domain::{constants, model, task_queue};
pub use attricat_events as domain_events;
pub use attricat_extension_manifest::extensions;
pub use attricat_repository::{attricat_service, extension_installer, repository};
pub use attricat_storage as storage;
pub use attricat_workers::task_worker;

mod extension_runtime;
pub use extension_runtime::*;
