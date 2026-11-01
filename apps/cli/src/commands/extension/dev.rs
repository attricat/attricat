//! `acli extension dev`: rebuilds an extension's Worker bundle or server
//! component and hot-swaps it in a development server, keeping the installed
//! release and its grants.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use serde_json::json;

use crate::error::CliError;
use crate::output::print_line;
use crate::transport::{Api, JAVASCRIPT_MEDIA_TYPE, WASM_MEDIA_TYPE};

/// How often watched paths are checked for changes.
const WATCH_INTERVAL: Duration = Duration::from_millis(500);

/// Where `acli extension new` builds the Worker bundle.
pub(crate) const DEFAULT_BUNDLE_PATH: &str = "dist/ui.js";

pub(crate) struct DevOptions {
    pub(crate) extension_id: String,
    pub(crate) bundle: Option<PathBuf>,
    pub(crate) server: Option<PathBuf>,
    pub(crate) build: Option<String>,
    pub(crate) watch: Vec<PathBuf>,
    pub(crate) once: bool,
}

/// The newest modification time under `paths`, recursively.
fn newest_change(paths: &[PathBuf]) -> Option<SystemTime> {
    fn visit(path: &Path, newest: &mut Option<SystemTime>) {
        let Ok(metadata) = std::fs::metadata(path) else {
            return;
        };
        if let Ok(modified) = metadata.modified() {
            *newest = (*newest).max(Some(modified));
        }
        if metadata.is_dir()
            && let Ok(entries) = std::fs::read_dir(path)
        {
            for entry in entries.flatten() {
                visit(&entry.path(), newest);
            }
        }
    }
    let mut newest = None;
    for path in paths {
        visit(path, &mut newest);
    }
    newest
}

async fn build(command: &str) -> Result<(), CliError> {
    let status = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
        .await
        .map_err(|error| CliError::Input(format!("cannot run '{command}': {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::Input(format!("'{command}' failed with {status}")))
    }
}

async fn swap(api: &Api, options: &DevOptions) -> Result<(), CliError> {
    if let Some(command) = &options.build {
        build(command).await?;
    }
    let segment = crate::transport::segment(&options.extension_id);
    if let Some(bundle) = &options.bundle {
        let size = put(
            api,
            bundle,
            &format!("/extensions/{segment}/dev-artifact"),
            JAVASCRIPT_MEDIA_TYPE,
        )
        .await?;
        print_line(
            &json!({"extension_id": options.extension_id, "swapped_bytes": size}).to_string(),
        )?;
    }
    if let Some(server) = &options.server {
        let size = put(
            api,
            server,
            &format!("/extensions/{segment}/dev-server-artifact"),
            WASM_MEDIA_TYPE,
        )
        .await?;
        print_line(
            &json!({"extension_id": options.extension_id, "swapped_server_bytes": size})
                .to_string(),
        )?;
    }
    Ok(())
}

/// Sends a built file to a development route and returns its size.
async fn put(api: &Api, file: &Path, route: &str, media_type: &str) -> Result<usize, CliError> {
    let bytes = tokio::fs::read(file)
        .await
        .map_err(|error| CliError::Input(format!("cannot read {}: {error}", file.display())))?;
    let size = bytes.len();
    api.put_bytes(route, bytes, media_type).await?;
    Ok(size)
}

/// Swaps once, then again after every change under the watched paths. A
/// failed build is reported and the previous build keeps running.
pub(crate) async fn dev(api: &Api, options: DevOptions) -> Result<String, CliError> {
    swap(api, &options).await?;
    if options.once {
        return Ok(String::new());
    }
    let mut seen = newest_change(&options.watch);
    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        let current = newest_change(&options.watch);
        if current == seen {
            continue;
        }
        seen = current;
        if let Err(error) = swap(api, &options).await {
            eprintln!("{}", error.json());
        }
    }
}
