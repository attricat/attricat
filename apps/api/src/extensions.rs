//! Shared extension manifest contracts and host-side policy validation.
//!
//! This module intentionally does not load packages or execute components. Those
//! concerns belong to the registry and runtime follow-on work; the installer
//! accepts trusted release archives and applies only structural safety limits.

use std::{
    collections::{BTreeMap, HashSet},
    io::{Cursor, Read},
    net::IpAddr,
    path::{Component, Path},
};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const MANIFEST_VERSION: u32 = 1;
pub const SUPPORTED_HOST_API: &str = "1.0.0";
pub const MAX_EXTENSION_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EXTENSION_UNPACKED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_EXTENSION_ARCHIVE_ENTRIES: usize = 256;
pub const CAPABILITIES: &[&str] = &[
    "catalog.read",
    "catalog.write",
    "events.subscribe",
    "events.emit",
    "storage.extension",
    "configuration.read",
    "secrets.read",
    "logging.write",
    "client.navigation",
    "client.notification",
    "client.events",
    "network.request",
    "webhooks.receive",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icons: BTreeMap<String, String>,
    pub catalog: CatalogIdentity,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub optional_permissions: Vec<String>,
    #[serde(default)]
    pub host_permissions: Vec<HostPermission>,
    #[serde(default)]
    pub optional_host_permissions: Vec<HostPermission>,
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub configuration: Option<Configuration>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub server: Option<Server>,
    #[serde(default)]
    pub ui: Vec<UiContribution>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogIdentity {
    pub id: String,
    pub host_api: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    ServerWasm,
    ClientComponent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostPermission {
    pub id: String,
    pub matches: Vec<String>,
    pub methods: Vec<String>,
    #[serde(default = "default_request_limit")]
    pub max_request_bytes: u64,
    #[serde(default = "default_response_limit")]
    pub max_response_bytes: u64,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}
fn default_request_limit() -> u64 {
    65_536
}
fn default_response_limit() -> u64 {
    1_048_576
}
fn default_timeout() -> u64 {
    10_000
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub version: u32,
    pub schema: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Webhook {
    pub id: String,
    pub event_type: String,
    pub handler: String,
    pub methods: Vec<String>,
    pub authentication: WebhookAuthentication,
    pub max_body_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookAuthentication {
    #[serde(rename = "type")]
    pub kind: WebhookAuthenticationKind,
    pub signature_header: String,
    pub timestamp_header: String,
    pub max_age_seconds: u64,
    pub secret: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WebhookAuthenticationKind {
    HmacSha256,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiContribution {
    pub id: String,
    pub version: u32,
    pub kind: String,
    pub artifact: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest_version {0} is unsupported")]
    UnsupportedManifestVersion(u32),
    #[error("{0}")]
    Invalid(String),
}

impl Manifest {
    /// Validates only stable v1 contracts. Package retrieval and component
    /// execution deliberately stay outside this shared layer.
    pub fn validate(&self, host_api: &str) -> Result<(), ManifestError> {
        if self.manifest_version != MANIFEST_VERSION {
            return Err(ManifestError::UnsupportedManifestVersion(
                self.manifest_version,
            ));
        }
        let host = Version::parse(host_api)
            .map_err(|_| ManifestError::Invalid("host API version is invalid".into()))?;
        let range = VersionReq::parse(&self.catalog.host_api).map_err(|_| {
            ManifestError::Invalid("catalog.host_api must be a SemVer range".into())
        })?;
        if !range.matches(&host) {
            return Err(ManifestError::Invalid(
                "catalog.host_api is incompatible with this host".into(),
            ));
        }
        Version::parse(&self.version)
            .map_err(|_| ManifestError::Invalid("version must be SemVer".into()))?;
        if self.name.trim().is_empty()
            || self.description.trim().is_empty()
            || self.icons.is_empty()
        {
            return Err(ManifestError::Invalid(
                "name, description, and at least one icon are required".into(),
            ));
        }
        valid_id(&self.catalog.id, "catalog.id")?;
        let mut ids = HashSet::new();
        for id in self.permissions.iter().chain(&self.optional_permissions) {
            if !CAPABILITIES.contains(&id.as_str()) {
                return Err(ManifestError::Invalid(format!("unknown capability '{id}'")));
            }
            if !ids.insert(id) {
                return Err(ManifestError::Invalid(format!(
                    "duplicate capability '{id}'"
                )));
            }
        }
        let requires_network = self.permissions.iter().any(|p| p == "network.request");
        let may_request_network = requires_network
            || self
                .optional_permissions
                .iter()
                .any(|p| p == "network.request");
        if !self.host_permissions.is_empty() && !requires_network {
            return Err(ManifestError::Invalid(
                "required host_permissions require required network.request".into(),
            ));
        }
        if !self.optional_host_permissions.is_empty() && !may_request_network {
            return Err(ManifestError::Invalid(
                "optional_host_permissions require network.request".into(),
            ));
        }
        for permission in self
            .host_permissions
            .iter()
            .chain(&self.optional_host_permissions)
        {
            permission.validate()?;
        }
        unique(
            self.host_permissions
                .iter()
                .chain(&self.optional_host_permissions)
                .map(|p| &p.id),
            "host permission",
        )?;
        if self.artifacts.is_empty() {
            return Err(ManifestError::Invalid(
                "at least one artifact is required".into(),
            ));
        }
        unique(self.artifacts.iter().map(|a| &a.id), "artifact")?;
        for artifact in &self.artifacts {
            artifact.validate()?;
        }
        unique(self.dependencies.iter().map(|d| &d.id), "dependency")?;
        for dependency in &self.dependencies {
            valid_id(&dependency.id, "dependency id")?;
            VersionReq::parse(&dependency.version).map_err(|_| {
                ManifestError::Invalid(format!(
                    "dependency '{}' has an invalid SemVer range",
                    dependency.id
                ))
            })?;
        }
        if let Some(configuration) = &self.configuration {
            if configuration.version == 0 || !configuration.schema.is_object() {
                return Err(ManifestError::Invalid(
                    "configuration requires a positive version and object JSON Schema".into(),
                ));
            }
        }
        if let Some(server) = &self.server {
            if !server.webhooks.is_empty()
                && !self
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.kind == ArtifactKind::ServerWasm)
            {
                return Err(ManifestError::Invalid(
                    "webhooks require a server_wasm artifact".into(),
                ));
            }
            for webhook in &server.webhooks {
                webhook.validate(&self.permissions)?;
            }
        }
        for contribution in &self.ui {
            valid_id(&contribution.id, "UI contribution id")?;
            if contribution.version == 0
                || !self.artifacts.iter().any(|a| {
                    a.id == contribution.artifact && a.kind == ArtifactKind::ClientComponent
                })
            {
                return Err(ManifestError::Invalid(format!(
                    "UI contribution '{}' must reference a client_component artifact",
                    contribution.id
                )));
            }
        }
        Ok(())
    }

    /// Validates a configuration value against the intentionally small v1 JSON
    /// Schema subset: object/properties/required/additionalProperties and the
    /// primitive `type` values. More schema vocabulary requires a new contract.
    pub fn validate_configuration(&self, value: &Value) -> Result<(), ManifestError> {
        let Some(configuration) = &self.configuration else {
            return if value.is_null() {
                Ok(())
            } else {
                Err(ManifestError::Invalid(
                    "this extension does not declare configuration".into(),
                ))
            };
        };
        validate_schema(&configuration.schema, value)
    }
}

impl Artifact {
    fn validate(&self) -> Result<(), ManifestError> {
        valid_id(&self.id, "artifact id")?;
        if self.path.starts_with('/')
            || self
                .path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(ManifestError::Invalid(format!(
                "artifact '{}' has an unsafe path",
                self.id
            )));
        }
        Ok(())
    }
}
impl HostPermission {
    fn validate(&self) -> Result<(), ManifestError> {
        valid_id(&self.id, "host permission id")?;
        if self.matches.is_empty()
            || self.methods.is_empty()
            || self.max_request_bytes == 0
            || self.max_response_bytes == 0
            || self.timeout_ms == 0
        {
            return Err(ManifestError::Invalid(format!(
                "host permission '{}' has empty or unbounded rules",
                self.id
            )));
        }
        for pattern in &self.matches {
            validate_url_pattern(pattern)?;
        }
        for method in &self.methods {
            if !matches!(
                method.as_str(),
                "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE"
            ) {
                return Err(ManifestError::Invalid(format!(
                    "host permission '{}' has unsupported method '{method}'",
                    self.id
                )));
            }
        }
        Ok(())
    }
}
impl Webhook {
    fn validate(&self, permissions: &[String]) -> Result<(), ManifestError> {
        valid_id(&self.id, "webhook id")?;
        valid_id(&self.handler, "webhook handler")?;
        if !permissions.iter().any(|p| p == "webhooks.receive") {
            return Err(ManifestError::Invalid(format!(
                "webhook '{}' requires webhooks.receive",
                self.id
            )));
        }
        if self.methods.as_slice() != ["POST"]
            || self.max_body_bytes == 0
            || self.authentication.max_age_seconds == 0
            || self.authentication.signature_header.trim().is_empty()
            || self.authentication.timestamp_header.trim().is_empty()
        {
            return Err(ManifestError::Invalid(format!(
                "webhook '{}' must use bounded POST HMAC authentication",
                self.id
            )));
        }
        valid_id(&self.authentication.secret, "webhook secret name")?;
        if !self
            .event_type
            .rsplit_once(".v")
            .is_some_and(|(_, version)| version.parse::<u32>().is_ok_and(|v| v > 0))
        {
            return Err(ManifestError::Invalid(format!(
                "webhook '{}' event_type must end in .vN",
                self.id
            )));
        }
        Ok(())
    }
}

fn unique<'a>(values: impl Iterator<Item = &'a String>, label: &str) -> Result<(), ManifestError> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ManifestError::Invalid(format!(
                "duplicate {label} '{value}'"
            )));
        }
    }
    Ok(())
}
fn valid_id(value: &str, label: &str) -> Result<(), ManifestError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        Err(ManifestError::Invalid(format!(
            "{label} must contain only ASCII letters, numbers, '.', '_' or '-'"
        )))
    } else {
        Ok(())
    }
}

fn validate_url_pattern(pattern: &str) -> Result<(), ManifestError> {
    if pattern.contains(|character| matches!(character, '?' | '#' | '@'))
        || !pattern.ends_with("/*")
    {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' must be a query-free path prefix ending in /*"
        )));
    }
    let (scheme, rest) = pattern.split_once("://").ok_or_else(|| {
        ManifestError::Invalid(format!("URL pattern '{pattern}' needs an exact scheme"))
    })?;
    if !matches!(scheme, "http" | "https") || rest.contains("//") {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an unsafe scheme or path"
        )));
    }
    let (authority, path) = rest.split_once('/').ok_or_else(|| {
        ManifestError::Invalid(format!("URL pattern '{pattern}' needs a path prefix"))
    })?;
    if authority.is_empty() || path != "*" && !path.ends_with("/*") {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an invalid path"
        )));
    }
    let host = authority
        .strip_prefix("*.")
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("");
    if host.is_empty()
        || host == "*"
        || authority.matches(':').count() > 1
        || host.eq_ignore_ascii_case("localhost")
    {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an invalid host or port"
        )));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_destination(ip) {
            return Err(ManifestError::Invalid(format!(
                "URL pattern '{pattern}' targets an unsafe destination"
            )));
        }
    }
    if !host.split('.').all(|part| {
        !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    }) {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an invalid host"
        )));
    }
    if let Some((_, port)) = authority.rsplit_once(':') {
        if port.parse::<u16>().ok().filter(|p| *p > 0).is_none() {
            return Err(ManifestError::Invalid(format!(
                "URL pattern '{pattern}' has an invalid port"
            )));
        }
    }
    Ok(())
}

/// A validated selected release archive. Registry discovery supplies the bytes;
/// installation extracts only declared artifacts to Catalog object storage.
pub struct ExtensionPackage {
    manifest: Manifest,
    artifacts: BTreeMap<String, Vec<u8>>,
}

impl ExtensionPackage {
    pub fn from_tar_zst(archive: &[u8]) -> Result<Self, ManifestError> {
        if archive.len() > MAX_EXTENSION_ARCHIVE_BYTES {
            return Err(ManifestError::Invalid(
                "extension archive exceeds the compressed size limit".into(),
            ));
        }
        let decoder = zstd::stream::read::Decoder::new(Cursor::new(archive)).map_err(|_| {
            ManifestError::Invalid("extension archive is not valid zstd data".into())
        })?;
        let mut tar = tar::Archive::new(decoder);
        let mut manifest_bytes = None;
        let mut files = BTreeMap::new();
        let mut total_bytes = 0usize;
        let mut entries = tar.entries().map_err(|_| {
            ManifestError::Invalid("extension archive is not valid tar data".into())
        })?;
        for (index, entry) in entries.by_ref().enumerate() {
            if index >= MAX_EXTENSION_ARCHIVE_ENTRIES {
                return Err(ManifestError::Invalid(
                    "extension archive has too many entries".into(),
                ));
            }
            let mut entry = entry.map_err(|_| {
                ManifestError::Invalid("extension archive has an invalid entry".into())
            })?;
            let path = entry
                .path()
                .map_err(|_| {
                    ManifestError::Invalid("extension archive entry path is invalid".into())
                })?
                .to_str()
                .ok_or_else(|| {
                    ManifestError::Invalid("extension archive entry paths must be UTF-8".into())
                })?
                .to_owned();
            if !safe_archive_path(Path::new(&path)) {
                return Err(ManifestError::Invalid(
                    "extension archive has an unsafe entry path".into(),
                ));
            }
            if entry.header().entry_type().is_dir() {
                continue;
            }
            if !entry.header().entry_type().is_file() {
                return Err(ManifestError::Invalid(
                    "extension archive contains a non-file entry".into(),
                ));
            }
            let declared_size = usize::try_from(entry.size()).map_err(|_| {
                ManifestError::Invalid("extension archive entry is too large".into())
            })?;
            if declared_size > MAX_EXTENSION_UNPACKED_BYTES
                || total_bytes.saturating_add(declared_size) > MAX_EXTENSION_UNPACKED_BYTES
            {
                return Err(ManifestError::Invalid(
                    "extension archive exceeds the unpacked size limit".into(),
                ));
            }
            let mut bytes = Vec::with_capacity(declared_size);
            entry
                .by_ref()
                .take(declared_size as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| {
                    ManifestError::Invalid("extension archive entry could not be read".into())
                })?;
            if bytes.len() != declared_size {
                return Err(ManifestError::Invalid(
                    "extension archive entry size is invalid".into(),
                ));
            }
            total_bytes += bytes.len();
            if path == "manifest.json" {
                if manifest_bytes.replace(bytes).is_some() {
                    return Err(ManifestError::Invalid(
                        "extension archive contains multiple manifest.json files".into(),
                    ));
                }
            } else if files.insert(path.to_owned(), bytes).is_some() {
                return Err(ManifestError::Invalid(
                    "extension archive contains duplicate file paths".into(),
                ));
            }
        }
        let manifest_bytes = manifest_bytes.ok_or_else(|| {
            ManifestError::Invalid("extension archive must contain manifest.json".into())
        })?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|_| {
            ManifestError::Invalid("manifest.json is not a valid strict manifest".into())
        })?;
        manifest.validate(SUPPORTED_HOST_API)?;
        let artifacts = manifest
            .artifacts
            .iter()
            .map(|artifact| {
                files
                    .get(&artifact.path)
                    .map(|bytes| (artifact.path.clone(), bytes.clone()))
                    .ok_or_else(|| {
                        ManifestError::Invalid(format!("artifact '{}' is missing", artifact.path))
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            manifest,
            artifacts,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn artifacts(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.artifacts
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
    }
}

fn safe_archive_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_public_destination(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            !(ip.is_loopback()
                || ip.is_private()
                || ip.is_unspecified()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_documentation()
                || octets[0] == 0
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
                || octets[0] >= 240)
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

fn validate_schema(schema: &Value, value: &Value) -> Result<(), ManifestError> {
    let object = schema
        .as_object()
        .ok_or_else(|| ManifestError::Invalid("configuration schema must be an object".into()))?;
    if let Some(kind) = object.get("type").and_then(Value::as_str) {
        let matches = match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => {
                return Err(ManifestError::Invalid(format!(
                    "unsupported configuration schema type '{kind}'"
                )));
            }
        };
        if !matches {
            return Err(ManifestError::Invalid(format!(
                "configuration must be {kind}"
            )));
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        let input = value
            .as_object()
            .ok_or_else(|| ManifestError::Invalid("configuration must be an object".into()))?;
        for key in required {
            let key = key.as_str().ok_or_else(|| {
                ManifestError::Invalid("configuration required entries must be strings".into())
            })?;
            if !input.contains_key(key) {
                return Err(ManifestError::Invalid(format!(
                    "configuration requires '{key}'"
                )));
            }
        }
    }
    if let (Some(properties), Some(input)) = (
        object.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        for (key, item) in input {
            if let Some(child) = properties.get(key) {
                validate_schema(child, item)?;
            } else if object.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                return Err(ManifestError::Invalid(format!(
                    "configuration field '{key}' is not allowed"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        serde_json::from_value(serde_json::json!({"manifest_version":1,"name":"Acme","version":"1.2.3","description":"test extension","icons":{"48":"icon.png"},"catalog":{"id":"acme.test","host_api":"^1.0"},"permissions":["network.request","webhooks.receive"],"host_permissions":[{"id":"acme","matches":["https://api.acme.example/v1/*"],"methods":["GET"]}],"artifacts":[{"id":"server","kind":"server_wasm","path":"server.wasm"}],"configuration":{"version":1,"schema":{"type":"object","required":["url"],"properties":{"url":{"type":"string"}},"additionalProperties":false}},"server":{"webhooks":[{"id":"events","event_type":"webhook.acme.events.v1","handler":"handle_events_v1","methods":["POST"],"authentication":{"type":"hmac-sha256","signature_header":"X-Signature","timestamp_header":"X-Timestamp","max_age_seconds":300,"secret":"webhook_secret"},"max_body_bytes":1024}]}})).unwrap()
    }
    #[test]
    fn validates_contract_boundaries() {
        let value = manifest();
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        let mut invalid = value.clone();
        invalid.catalog.host_api = "^2".into();
        assert!(invalid.validate(SUPPORTED_HOST_API).is_err());
        invalid = value.clone();
        invalid.host_permissions[0].matches = vec!["https://127.0.0.1/*".into()];
        assert!(invalid.validate(SUPPORTED_HOST_API).is_err());
        invalid = value.clone();
        invalid.host_permissions[0].matches = vec!["https://224.0.0.1/*".into()];
        assert!(invalid.validate(SUPPORTED_HOST_API).is_err());
        invalid = value;
        invalid.permissions.clear();
        invalid.optional_permissions = vec!["network.request".into(), "webhooks.receive".into()];
        assert!(invalid.validate(SUPPORTED_HOST_API).is_err());
    }
    #[test]
    fn validates_configuration() {
        let value = manifest();
        assert!(
            value
                .validate_configuration(&serde_json::json!({"url":"https://example.test"}))
                .is_ok()
        );
        assert!(
            value
                .validate_configuration(&serde_json::json!({"url":1}))
                .is_err()
        );
    }
}
