//! `acli extension validate`: the checks an upload runs, without a server.

use std::path::Path;

use attricat_extension_manifest::extensions::{
    ArtifactKind, ExtensionPackage, Manifest, NATIVE_BUILD_RECORD_SUFFIX, SUPPORTED_HOST_API,
};
use serde_json::json;

use crate::error::CliError;

/// The name of an extension's manifest, at the root of its directory and archive.
const MANIFEST_FILE: &str = "manifest.json";
/// The file name suffix of a packed extension.
const ARCHIVE_SUFFIX: &str = ".tar.zst";

fn invalid(error: impl std::fmt::Display) -> CliError {
    CliError::Input(format!("invalid extension: {error}"))
}

fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    std::fs::read(path)
        .map_err(|error| CliError::Input(format!("cannot read {}: {error}", path.display())))
}

/// A directory's manifest, with every file it declares present. Built artifacts
/// may be missing before the first build, so run this after building.
fn validate_directory(directory: &Path) -> Result<Manifest, CliError> {
    let manifest: Manifest =
        serde_json::from_slice(&read(&directory.join(MANIFEST_FILE))?).map_err(invalid)?;
    manifest.validate(SUPPORTED_HOST_API).map_err(invalid)?;
    let mut declared: Vec<String> = manifest.icons.values().cloned().collect();
    for artifact in &manifest.artifacts {
        declared.push(artifact.path.clone());
        if artifact.kind == ArtifactKind::ClientNative {
            declared.push(format!("{}{NATIVE_BUILD_RECORD_SUFFIX}", artifact.path));
        }
    }
    for path in declared {
        if !directory.join(&path).is_file() {
            return Err(invalid(format!(
                "'{path}' is declared but missing; build the extension first"
            )));
        }
    }
    Ok(manifest)
}

pub(crate) fn validate(path: &Path) -> Result<String, CliError> {
    let manifest = if path.is_dir() {
        validate_directory(path)?
    } else if path.to_string_lossy().ends_with(ARCHIVE_SUFFIX) {
        ExtensionPackage::from_tar_zst(&read(path)?)
            .map_err(invalid)?
            .manifest()
            .clone()
    } else {
        return Err(CliError::Input(format!(
            "{} is not a directory or a {ARCHIVE_SUFFIX} archive",
            path.display()
        )));
    };
    Ok(format!(
        "{}\n",
        json!({
            "valid": true,
            "extension_id": manifest.attricat.id,
            "version": manifest.version,
            "host_api": SUPPORTED_HOST_API,
        })
    ))
}
