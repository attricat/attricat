//! `acli extension dev`: rebuilds a native extension's Worker bundle and
//! hot-swaps it in a development server, keeping the installed release and
//! its grants.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use serde_json::json;

use crate::error::CliError;
use crate::output::print_line;
use crate::transport::{Api, JAVASCRIPT_MEDIA_TYPE};

/// How often watched paths are checked for changes.
const WATCH_INTERVAL: Duration = Duration::from_millis(500);

pub(crate) struct DevOptions {
    pub(crate) extension_id: String,
    pub(crate) bundle: PathBuf,
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
    let bytes = tokio::fs::read(&options.bundle).await.map_err(|error| {
        CliError::Input(format!("cannot read {}: {error}", options.bundle.display()))
    })?;
    let size = bytes.len();
    api.put_bytes(
        &format!(
            "/extensions/{}/dev-artifact",
            crate::transport::segment(&options.extension_id)
        ),
        bytes,
        JAVASCRIPT_MEDIA_TYPE,
    )
    .await?;
    print_line(&json!({"extension_id": options.extension_id, "swapped_bytes": size}).to_string())?;
    Ok(())
}

/// Swaps once, then again after every change under the watched paths. A
/// failed build is reported and the previous Worker keeps running.
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
