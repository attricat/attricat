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
/// The newest host contract accepted by manifests. Components importing
/// `catalog:host@1.0.0` remain supported by the unchanged v1 WIT package.
pub const SUPPORTED_HOST_API: &str = "1.1.0";
pub const MAX_EXTENSION_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EXTENSION_UNPACKED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_EXTENSION_ARCHIVE_ENTRIES: usize = 256;

pub(crate) fn valid_contribution_key(value: &str) -> bool {
    let Some((extension_id, contribution_id)) = value.split_once(':') else {
        return false;
    };
    value.len() <= 256
        && !extension_id.is_empty()
        && !contribution_id.is_empty()
        && !contribution_id.contains(':')
        && value
            .chars()
            .filter(|character| *character != ':')
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtensionLayoutPlacement {
    Exact,
    Absent,
    Conflict,
}

/// Classifies one stable contribution key without interpreting unrelated
/// layout data. Planning and application share this to keep merge semantics
/// identical across the immutable snapshot and the locked current row.
pub(crate) fn classify_extension_layout_placement(
    layout: &Value,
    contribution: &str,
    outlet: &str,
    hidden: bool,
    promoted: bool,
) -> ExtensionLayoutPlacement {
    let mut found: Option<(&str, bool, bool)> = None;
    let mut promoted_seen = false;
    let Some(outlets) = layout.get("outlets").and_then(Value::as_object) else {
        return ExtensionLayoutPlacement::Conflict;
    };
    for (current_outlet, item) in outlets {
        let Some(item) = item.as_object() else {
            return ExtensionLayoutPlacement::Conflict;
        };
        let is_promoted = item
            .get("promoted")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|value| value.as_str() == Some(contribution))
            });
        promoted_seen |= is_promoted;
        for (list, is_hidden) in [("order", false), ("hidden", true)] {
            if item
                .get(list)
                .and_then(Value::as_array)
                .is_some_and(|values| {
                    values
                        .iter()
                        .any(|value| value.as_str() == Some(contribution))
                })
            {
                if found.is_some() {
                    return ExtensionLayoutPlacement::Conflict;
                }
                found = Some((current_outlet, is_hidden, is_promoted));
            }
        }
    }
    if promoted_seen && !matches!(found, Some(("navigation", false, true))) {
        return ExtensionLayoutPlacement::Conflict;
    }
    match found {
        None => ExtensionLayoutPlacement::Absent,
        Some((current_outlet, current_hidden, current_promoted))
            if current_outlet == outlet
                && current_hidden == hidden
                && current_promoted == promoted =>
        {
            ExtensionLayoutPlacement::Exact
        }
        Some(_) => ExtensionLayoutPlacement::Conflict,
    }
}
pub const DEFAULT_HOST_REQUEST_BYTES: u64 = 64 * 1024;
pub const DEFAULT_HOST_RESPONSE_BYTES: u64 = 1024 * 1024;
pub const DEFAULT_HOST_TIMEOUT_MILLIS: u64 = 10_000;
pub const MAX_HOST_REQUEST_BYTES: u64 = DEFAULT_HOST_REQUEST_BYTES;
pub const MAX_HOST_RESPONSE_BYTES: u64 = DEFAULT_HOST_REQUEST_BYTES;
pub const MAX_EXTENSION_IDENTIFIER_BYTES: usize = 128;
pub const CAPABILITIES: &[&str] = &[
    "catalog.read",
    "catalog.write",
    "events.subscribe",
    "events.emit",
    "storage.extension",
    "configuration.read",
    "configuration.write",
    "client.commands",
    "client.blueprint_configuration",
    "client.entity_decoration",
    "client.entity_action",
    "client.explorer_row_action",
    "client.explorer_table_cell",
    "client.blueprint_detail_panel",
    "secrets.read",
    "logging.write",
    "client.navigation",
    "client.notification",
    "client.events",
    "client.refresh",
    "client.confirmation",
    "client.download",
    "client.external_navigation",
    "client.files.read",
    "client.files.upload",
    "client.search",
    "client.live_updates",
    "client.clipboard",
    "client.theme.read",
    "client.locale.read",
    "client.explorer_action",
    "client.explorer_bulk_action",
    "client.entity_header_action",
    "client.entity_attribute_panel",
    "client.blueprint_panel",
    "client.blueprint_publish_check",
    "client.file_panel",
    "client.audit_event_panel",
    "client.data_health_card",
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
    /// Extension-owned values associated with Catalog objects rather than the
    /// installation. These values never modify the blueprint definition.
    #[serde(default)]
    pub scoped_configuration: Option<ScopedConfiguration>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Versioned extension-owned event contracts. They are the only supported
    /// inter-extension communication primitive in host API 1.1.
    #[serde(default)]
    pub event_contracts: EventContracts,
    #[serde(default)]
    pub server: Option<Server>,
    #[serde(default)]
    pub ui: Vec<UiContribution>,
    /// Declarative cell renderers available to Explorer table columns. Each
    /// renderer is paired with a sandboxed `explorer_table_cell` contribution.
    #[serde(default)]
    pub cell_renderers: Vec<CellRenderer>,
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
    DEFAULT_HOST_REQUEST_BYTES
}
fn default_response_limit() -> u64 {
    DEFAULT_HOST_RESPONSE_BYTES
}
fn default_timeout() -> u64 {
    DEFAULT_HOST_TIMEOUT_MILLIS
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub version: u32,
    pub schema: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedConfiguration {
    pub version: u32,
    pub schema: Value,
    pub scopes: Vec<ConfigurationScope>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationScope {
    Blueprint,
    Attribute,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    pub version: String,
}

/// Exports are owned by this extension; imports name both a provider and its
/// stable contract ID. Contract versions are independent of package versions.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventContracts {
    #[serde(default)]
    pub exports: Vec<EventContractExport>,
    #[serde(default)]
    pub consumes: Vec<EventContractConsume>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventContractExport {
    pub id: String,
    pub version: String,
    pub event_type: String,
    pub schema: Value,
    pub max_payload_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventContractConsume {
    pub provider: String,
    pub contract: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
    /// Durable catalog event subscriptions delivered to named component exports.
    #[serde(default)]
    pub event_handlers: Vec<EventHandler>,
    /// Client-mediated component calls. Commands have no route or ambient
    /// browser access; the host resolves and authorizes each invocation.
    #[serde(default)]
    pub commands: Vec<ServerCommand>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServerCommand {
    pub id: String,
    pub handler: String,
    pub request_schema: Value,
    pub response_schema: Value,
    #[serde(default = "default_command_bytes")]
    pub max_request_bytes: u64,
    #[serde(default = "default_command_bytes")]
    pub max_response_bytes: u64,
}
fn default_command_bytes() -> u64 {
    DEFAULT_HOST_REQUEST_BYTES
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventHandler {
    pub id: String,
    pub event_types: Vec<String>,
    pub handler: String,
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

/// A client contribution is either a namespaced full page or an embedded view
/// at a host-owned outlet. The host never accepts arbitrary route paths or DOM
/// selectors from an extension.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellRenderer {
    pub id: String,
    pub version: u32,
    pub value_types: Vec<String>,
    #[serde(default)]
    pub allowed_props: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiContribution {
    pub id: String,
    pub version: u32,
    pub kind: UiContributionKind,
    pub artifact: String,
    #[serde(default)]
    pub outlet: Option<UiOutlet>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiContributionKind {
    Route,
    /// Generic embedded contribution. New surfaces must use `Action` or
    /// `Panel`, so the host can own their compact/action or read-only layout.
    Embedded,
    Action,
    Panel,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum UiOutlet {
    Navigation,
    EntityPreviewPanel,
    BlueprintAttributeConfiguration,
    EntityAttributeDecoration,
    EntityAction,
    /// A compact, per-entity explorer menu action. The host provides only the
    /// entity and immutable blueprint revision identifiers.
    ExplorerRowAction,
    /// A sandboxed renderer for a visible Explorer table cell.
    ExplorerTableCell,
    /// A read-only panel on a blueprint revision detail page.
    BlueprintDetailPanel,
    ExplorerAction,
    ExplorerBulkAction,
    EntityHeaderAction,
    EntityAttributePanel,
    BlueprintPanel,
    BlueprintPublishCheck,
    FilePanel,
    AuditEventPanel,
    DataHealthCard,
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
        unique(
            self.event_contracts.exports.iter().map(|item| &item.id),
            "event export",
        )?;
        for contract in &self.event_contracts.exports {
            contract.validate(&self.catalog.id)?;
        }
        let consumed_contracts = self
            .event_contracts
            .consumes
            .iter()
            .map(|item| (&item.provider, &item.contract))
            .collect::<HashSet<_>>();
        if consumed_contracts.len() != self.event_contracts.consumes.len() {
            return Err(ManifestError::Invalid(
                "duplicate event contract consumption".into(),
            ));
        }
        for contract in &self.event_contracts.consumes {
            contract.validate(&self.catalog.id)?;
        }
        if !self.event_contracts.exports.is_empty()
            && !self.permissions.iter().any(|item| item == "events.emit")
        {
            return Err(ManifestError::Invalid(
                "event exports require events.emit".into(),
            ));
        }
        if !self.event_contracts.consumes.is_empty()
            && !self
                .permissions
                .iter()
                .any(|item| item == "events.subscribe")
        {
            return Err(ManifestError::Invalid(
                "event contract consumption requires events.subscribe".into(),
            ));
        }
        if let Some(configuration) = &self.configuration
            && (configuration.version == 0 || !configuration.schema.is_object())
        {
            return Err(ManifestError::Invalid(
                "configuration requires a positive version and object JSON Schema".into(),
            ));
        }
        if let Some(configuration) = &self.scoped_configuration {
            require_next_host_api(&range)?;
            if configuration.version == 0
                || !configuration.schema.is_object()
                || configuration.scopes.is_empty()
            {
                return Err(ManifestError::Invalid(
                    "scoped_configuration requires a positive version, object JSON Schema, and scopes".into(),
                ));
            }
            let scopes = configuration.scopes.iter().collect::<HashSet<_>>();
            if scopes.len() != configuration.scopes.len() {
                return Err(ManifestError::Invalid(
                    "scoped_configuration scopes must be unique".into(),
                ));
            }
            if !self
                .permissions
                .iter()
                .chain(&self.optional_permissions)
                .any(|item| item == "configuration.write")
            {
                return Err(ManifestError::Invalid(
                    "scoped_configuration requires configuration.write".into(),
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
            unique(
                server.event_handlers.iter().map(|handler| &handler.id),
                "event handler",
            )?;
            for handler in &server.event_handlers {
                handler.validate(&self.permissions)?;
            }
            if !server.commands.is_empty() {
                require_next_host_api(&range)?;
            }
            unique(
                server.commands.iter().map(|command| &command.id),
                "server command",
            )?;
            for command in &server.commands {
                command.validate(&self.permissions)?;
            }
            if (!server.event_handlers.is_empty() || !server.commands.is_empty())
                && !self
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.kind == ArtifactKind::ServerWasm)
            {
                return Err(ManifestError::Invalid(
                    "event handlers require a server_wasm artifact".into(),
                ));
            }
        }
        unique(
            self.ui.iter().map(|contribution| &contribution.id),
            "UI contribution",
        )?;
        let mut outlets = HashSet::new();
        unique(
            self.cell_renderers.iter().map(|renderer| &renderer.id),
            "cell renderer",
        )?;
        if !self.cell_renderers.is_empty()
            && !self
                .permissions
                .iter()
                .chain(&self.optional_permissions)
                .any(|item| item == "client.explorer_table_cell")
        {
            return Err(ManifestError::Invalid(
                "cell renderers require client.explorer_table_cell".into(),
            ));
        }
        for renderer in &self.cell_renderers {
            valid_id(&renderer.id, "cell renderer")?;
            if renderer.version == 0 || renderer.value_types.is_empty() {
                return Err(ManifestError::Invalid(format!(
                    "cell renderer '{}' requires a positive version and value types",
                    renderer.id
                )));
            }
            for value_type in &renderer.value_types {
                if !matches!(
                    value_type.as_str(),
                    "string" | "number" | "integer" | "boolean" | "date" | "datetime" | "time"
                ) {
                    return Err(ManifestError::Invalid(format!(
                        "cell renderer '{}' has unsupported value type '{value_type}'",
                        renderer.id
                    )));
                }
            }
            unique(renderer.allowed_props.iter(), "cell renderer prop")?;
        }
        for contribution in &self.ui {
            valid_id(&contribution.id, "UI contribution id")?;
            let contribution_key = format!("{}:{}", self.catalog.id, contribution.id);
            if !valid_contribution_key(&contribution_key) {
                return Err(ManifestError::Invalid(format!(
                    "UI contribution '{}' stable key exceeds the layout key limit",
                    contribution.id
                )));
            }
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
            if let Some(outlet) = &contribution.outlet {
                if !matches!(outlet, UiOutlet::Navigation | UiOutlet::EntityPreviewPanel) {
                    require_next_host_api(&range)?;
                }
                let required = match outlet {
                    UiOutlet::Navigation | UiOutlet::EntityPreviewPanel => None,
                    UiOutlet::BlueprintAttributeConfiguration => {
                        Some("client.blueprint_configuration")
                    }
                    UiOutlet::EntityAttributeDecoration => Some("client.entity_decoration"),
                    UiOutlet::EntityAction => Some("client.entity_action"),
                    UiOutlet::ExplorerRowAction => Some("client.explorer_row_action"),
                    UiOutlet::ExplorerTableCell => Some("client.explorer_table_cell"),
                    UiOutlet::BlueprintDetailPanel => Some("client.blueprint_detail_panel"),
                    UiOutlet::ExplorerAction => Some("client.explorer_action"),
                    UiOutlet::ExplorerBulkAction => Some("client.explorer_bulk_action"),
                    UiOutlet::EntityHeaderAction => Some("client.entity_header_action"),
                    UiOutlet::EntityAttributePanel => Some("client.entity_attribute_panel"),
                    UiOutlet::BlueprintPanel => Some("client.blueprint_panel"),
                    UiOutlet::BlueprintPublishCheck => Some("client.blueprint_publish_check"),
                    UiOutlet::FilePanel => Some("client.file_panel"),
                    UiOutlet::AuditEventPanel => Some("client.audit_event_panel"),
                    UiOutlet::DataHealthCard => Some("client.data_health_card"),
                };
                if required.is_some_and(|capability| {
                    !self
                        .permissions
                        .iter()
                        .chain(&self.optional_permissions)
                        .any(|item| item == capability)
                }) {
                    return Err(ManifestError::Invalid(format!(
                        "UI outlet requires capability '{}'",
                        required.unwrap()
                    )));
                }
            }
            match (&contribution.kind, &contribution.outlet) {
                (UiContributionKind::Route, None) => {}
                (UiContributionKind::Embedded, Some(outlet))
                | (UiContributionKind::Action, Some(outlet))
                | (UiContributionKind::Panel, Some(outlet)) => {
                    // Extensions may each contribute once to an outlet. A
                    // single extension cannot rely on duplicate ordering.
                    if !outlets.insert(outlet) {
                        return Err(ManifestError::Invalid(
                            "only one contribution may target each UI outlet".into(),
                        ));
                    }
                    let valid_kind = matches!(
                        (&contribution.kind, outlet),
                        (UiContributionKind::Action, UiOutlet::ExplorerRowAction)
                            | (UiContributionKind::Embedded, UiOutlet::ExplorerTableCell)
                            | (UiContributionKind::Action, UiOutlet::ExplorerAction)
                            | (UiContributionKind::Action, UiOutlet::ExplorerBulkAction)
                            | (UiContributionKind::Action, UiOutlet::EntityHeaderAction)
                            | (UiContributionKind::Panel, UiOutlet::BlueprintDetailPanel)
                            | (UiContributionKind::Panel, UiOutlet::EntityAttributePanel)
                            | (UiContributionKind::Panel, UiOutlet::BlueprintPanel)
                            | (UiContributionKind::Panel, UiOutlet::BlueprintPublishCheck)
                            | (UiContributionKind::Panel, UiOutlet::FilePanel)
                            | (UiContributionKind::Panel, UiOutlet::AuditEventPanel)
                            | (UiContributionKind::Panel, UiOutlet::DataHealthCard)
                            | (UiContributionKind::Embedded, UiOutlet::Navigation)
                            | (UiContributionKind::Embedded, UiOutlet::EntityPreviewPanel)
                            | (
                                UiContributionKind::Embedded,
                                UiOutlet::BlueprintAttributeConfiguration
                            )
                            | (
                                UiContributionKind::Embedded,
                                UiOutlet::EntityAttributeDecoration
                            )
                            | (UiContributionKind::Embedded, UiOutlet::EntityAction)
                    );
                    if !valid_kind {
                        return Err(ManifestError::Invalid(
                            "this UI outlet requires its explicit contribution kind".into(),
                        ));
                    }
                }
                (UiContributionKind::Route, Some(_)) => {
                    return Err(ManifestError::Invalid(
                        "route UI contributions cannot declare an outlet".into(),
                    ));
                }
                (UiContributionKind::Embedded, None)
                | (UiContributionKind::Action, None)
                | (UiContributionKind::Panel, None) => {
                    return Err(ManifestError::Invalid(
                        "embedded UI contributions require an outlet".into(),
                    ));
                }
            }
            if matches!(contribution.kind, UiContributionKind::Route)
                && contribution
                    .title
                    .as_deref()
                    .is_none_or(|title| title.trim().is_empty())
            {
                return Err(ManifestError::Invalid(
                    "route UI contributions require a non-empty title".into(),
                ));
            }
        }
        for renderer in &self.cell_renderers {
            if !self.ui.iter().any(|contribution| {
                contribution.outlet == Some(UiOutlet::ExplorerTableCell)
                    && contribution.id == renderer.id
                    && contribution.version == renderer.version
            }) {
                return Err(ManifestError::Invalid(format!(
                    "cell renderer '{}@{}' requires a matching explorer_table_cell contribution",
                    renderer.id, renderer.version
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
impl EventContractExport {
    fn validate(&self, extension_id: &str) -> Result<(), ManifestError> {
        valid_id(&self.id, "event contract id")?;
        Version::parse(&self.version).map_err(|_| {
            ManifestError::Invalid(format!(
                "event contract '{}' version must be SemVer",
                self.id
            ))
        })?;
        if !self
            .event_type
            .starts_with(&format!("plugin.{extension_id}."))
            || !self
                .event_type
                .rsplit_once(".v")
                .is_some_and(|(_, version)| version.parse::<u32>().is_ok_and(|version| version > 0))
            || !self.schema.is_object()
            || self.max_payload_bytes == 0
            || self.max_payload_bytes > MAX_HOST_REQUEST_BYTES
        {
            return Err(ManifestError::Invalid(format!(
                "event contract '{}' requires an owned versioned type, object schema, and 1-65536 byte payload limit",
                self.id
            )));
        }
        Ok(())
    }
}

impl EventContractConsume {
    fn validate(&self, extension_id: &str) -> Result<(), ManifestError> {
        valid_id(&self.provider, "event contract provider")?;
        valid_id(&self.contract, "event contract id")?;
        if self.provider == extension_id {
            return Err(ManifestError::Invalid(
                "an extension cannot consume its own event contract".into(),
            ));
        }
        VersionReq::parse(&self.version).map_err(|_| {
            ManifestError::Invalid(format!(
                "event contract '{}:{}' has an invalid SemVer range",
                self.provider, self.contract
            ))
        })?;
        Ok(())
    }
}

impl EventHandler {
    fn validate(&self, permissions: &[String]) -> Result<(), ManifestError> {
        valid_id(&self.id, "event handler id")?;
        if self.handler != "handle-event" {
            return Err(ManifestError::Invalid(format!(
                "event handler '{}' must use the v1 handle-event export",
                self.id
            )));
        }
        if !permissions
            .iter()
            .any(|permission| permission == "events.subscribe")
        {
            return Err(ManifestError::Invalid(format!(
                "event handler '{}' requires events.subscribe",
                self.id
            )));
        }
        if self.event_types.is_empty()
            || self.event_types.iter().any(|event_type| {
                event_type.is_empty()
                    || event_type.len() > MAX_EXTENSION_IDENTIFIER_BYTES
                    || !event_type.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
                    || !event_type.rsplit_once(".v").is_some_and(|(_, version)| {
                        version.parse::<u32>().is_ok_and(|version| version > 0)
                    })
            })
        {
            return Err(ManifestError::Invalid(format!(
                "event handler '{}' must declare versioned event types",
                self.id
            )));
        }
        Ok(())
    }
}

impl ServerCommand {
    fn validate(&self, permissions: &[String]) -> Result<(), ManifestError> {
        valid_id(&self.id, "server command id")?;
        valid_id(&self.handler, "server command handler")?;
        if !permissions
            .iter()
            .any(|permission| permission == "client.commands")
        {
            return Err(ManifestError::Invalid(format!(
                "server command '{}' requires client.commands",
                self.id
            )));
        }
        if !self.request_schema.is_object()
            || !self.response_schema.is_object()
            || self.max_request_bytes == 0
            || self.max_request_bytes > MAX_HOST_REQUEST_BYTES
            || self.max_response_bytes == 0
            || self.max_response_bytes > MAX_HOST_RESPONSE_BYTES
        {
            return Err(ManifestError::Invalid(format!(
                "server command '{}' requires bounded object request and response schemas",
                self.id
            )));
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

fn require_next_host_api(range: &VersionReq) -> Result<(), ManifestError> {
    if range.matches(&Version::new(1, 1, 0)) && !range.matches(&Version::new(1, 0, 0)) {
        Ok(())
    } else {
        Err(ManifestError::Invalid(
            "this declaration requires catalog.host_api compatible with 1.1 but not 1.0".into(),
        ))
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
pub(crate) fn valid_id(value: &str, label: &str) -> Result<(), ManifestError> {
    if value.is_empty()
        || value.len() > MAX_EXTENSION_IDENTIFIER_BYTES
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
    if pattern.contains(['?', '#', '@']) || !pattern.ends_with("/*") {
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
    if let Ok(ip) = host.parse::<IpAddr>()
        && !is_public_destination(ip)
    {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' targets an unsafe destination"
        )));
    }
    if !host.split('.').all(|part| {
        !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    }) {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an invalid host"
        )));
    }
    if let Some((_, port)) = authority.rsplit_once(':')
        && port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
    {
        return Err(ManifestError::Invalid(format!(
            "URL pattern '{pattern}' has an invalid port"
        )));
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

pub(crate) fn validate_schema(schema: &Value, value: &Value) -> Result<(), ManifestError> {
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

    #[test]
    fn shared_layout_placement_classifier_covers_plan_and_apply_states() {
        let layout = serde_json::json!({
            "version": 1,
            "outlets": {
                "navigation": {
                    "order": ["acme.shop:nav"],
                    "hidden": [],
                    "promoted": ["acme.shop:nav"]
                },
                "entity_action": {
                    "order": [],
                    "hidden": ["acme.shop:hidden"]
                }
            }
        });
        for (contribution, outlet, hidden, promoted, expected) in [
            (
                "acme.shop:nav",
                "navigation",
                false,
                true,
                ExtensionLayoutPlacement::Exact,
            ),
            (
                "acme.shop:hidden",
                "entity_action",
                true,
                false,
                ExtensionLayoutPlacement::Exact,
            ),
            (
                "acme.shop:absent",
                "entity_action",
                false,
                false,
                ExtensionLayoutPlacement::Absent,
            ),
            (
                "acme.shop:nav",
                "navigation",
                false,
                false,
                ExtensionLayoutPlacement::Conflict,
            ),
            (
                "acme.shop:hidden",
                "navigation",
                true,
                false,
                ExtensionLayoutPlacement::Conflict,
            ),
        ] {
            assert_eq!(
                classify_extension_layout_placement(
                    &layout,
                    contribution,
                    outlet,
                    hidden,
                    promoted,
                ),
                expected
            );
        }

        for malformed in [
            serde_json::json!({
                "version":1,
                "outlets": {
                    "navigation": {
                        "order":[],
                        "hidden":[],
                        "promoted":["acme.shop:item"]
                    }
                }
            }),
            serde_json::json!({
                "version":1,
                "outlets": {
                    "navigation": {
                        "order":[],
                        "hidden":[],
                        "promoted":["acme.shop:item"]
                    },
                    "entity_action": {
                        "order":["acme.shop:item"],
                        "hidden":[]
                    }
                }
            }),
        ] {
            assert_eq!(
                classify_extension_layout_placement(
                    &malformed,
                    "acme.shop:item",
                    "entity_action",
                    false,
                    false,
                ),
                ExtensionLayoutPlacement::Conflict
            );
        }
    }

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
    fn ui_contribution_stable_key_enforces_combined_length_boundary() {
        let mut value = manifest();
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.ui.push(UiContribution {
            id: "b".repeat(128),
            version: 1,
            kind: UiContributionKind::Embedded,
            artifact: "client".into(),
            outlet: Some(UiOutlet::Navigation),
            title: None,
        });
        value.catalog.id = "a".repeat(127);
        assert_eq!(
            format!("{}:{}", value.catalog.id, value.ui[0].id).len(),
            256
        );
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());

        value.catalog.id = "a".repeat(128);
        assert_eq!(
            format!("{}:{}", value.catalog.id, value.ui[0].id).len(),
            257
        );
        let error = value.validate(SUPPORTED_HOST_API).unwrap_err();
        assert!(error.to_string().contains("layout key limit"));

        let mut invalid_contribution = value.clone();
        invalid_contribution.catalog.id = "acme.test".into();
        invalid_contribution.ui[0].id = "b".repeat(129);
        assert!(
            invalid_contribution
                .validate(SUPPORTED_HOST_API)
                .unwrap_err()
                .to_string()
                .contains("UI contribution id")
        );
        let mut invalid_extension = value;
        invalid_extension.catalog.id = "a".repeat(129);
        assert!(
            invalid_extension
                .validate(SUPPORTED_HOST_API)
                .unwrap_err()
                .to_string()
                .contains("catalog.id")
        );
    }

    #[test]
    fn event_handlers_require_subscription_capability_and_versioned_types() {
        let mut value = manifest();
        value.permissions.push("events.subscribe".into());
        value.server.as_mut().unwrap().event_handlers = vec![EventHandler {
            id: "on-entity".into(),
            event_types: vec!["entity.updated.v1".into()],
            handler: "handle-event".into(),
        }];
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value
            .permissions
            .retain(|permission| permission != "events.subscribe");
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
        value.permissions.push("events.subscribe".into());
        value.server.as_mut().unwrap().event_handlers[0].event_types =
            vec!["entity.updated".into()];
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
    }

    #[test]
    fn validates_event_contracts_and_requires_scoped_capabilities() {
        let mut value = manifest();
        value.permissions.push("events.emit".into());
        value.event_contracts.exports.push(EventContractExport {
            id: "inventory.changed".into(),
            version: "1.0.0".into(),
            event_type: "plugin.acme.test.inventory_changed.v1".into(),
            schema: serde_json::json!({"type":"object","required":["sku"]}),
            max_payload_bytes: 1024,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value.event_contracts.exports[0].event_type = "plugin.other.changed.v1".into();
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
        value.event_contracts.exports[0].event_type =
            "plugin.acme.test.inventory_changed.v1".into();
        value
            .permissions
            .retain(|permission| permission != "events.emit");
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
    }

    #[test]
    fn validates_client_ui_contributions() {
        let mut value = manifest();
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.ui.push(UiContribution {
            id: "panel".into(),
            version: 1,
            kind: UiContributionKind::Embedded,
            artifact: "client".into(),
            outlet: Some(UiOutlet::EntityPreviewPanel),
            title: None,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
    }
    #[test]
    fn validates_mediated_capabilities_and_matching_placement() {
        let mut value = manifest();
        value.catalog.host_api = ">=1.1.0, <2.0.0".into();
        value.permissions.push("client.confirmation".into());
        value.permissions.push("client.explorer_action".into());
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.ui.push(UiContribution {
            id: "explorer-action".into(),
            version: 1,
            kind: UiContributionKind::Action,
            artifact: "client".into(),
            outlet: Some(UiOutlet::ExplorerAction),
            title: None,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value
            .permissions
            .retain(|permission| permission != "client.explorer_action");
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
    }

    #[test]
    fn validates_explorer_table_cell_renderer_contract() {
        let mut value = manifest();
        value.catalog.host_api = ">=1.1.0, <2.0.0".into();
        value.permissions.push("client.explorer_table_cell".into());
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.cell_renderers.push(CellRenderer {
            id: "example.currency".into(),
            version: 1,
            value_types: vec!["number".into()],
            allowed_props: vec!["currency".into()],
        });
        value.ui.push(UiContribution {
            id: "example.currency".into(),
            version: 1,
            kind: UiContributionKind::Embedded,
            artifact: "client".into(),
            outlet: Some(UiOutlet::ExplorerTableCell),
            title: None,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value.permissions.clear();
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
        value.permissions.push("client.explorer_table_cell".into());
        value.ui[0].version = 2;
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
    }

    #[test]
    fn validates_generic_scoped_configuration_and_commands() {
        let mut value = manifest();
        value.catalog.host_api = ">=1.1.0, <2.0.0".into();
        value.permissions.extend([
            "configuration.write".into(),
            "client.commands".into(),
            "client.entity_action".into(),
        ]);
        value.scoped_configuration = Some(ScopedConfiguration {
            version: 1,
            schema: serde_json::json!({"type":"object","additionalProperties":false}),
            scopes: vec![ConfigurationScope::Attribute],
        });
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.server.as_mut().unwrap().commands.push(ServerCommand {
            id: "refresh".into(),
            handler: "refresh".into(),
            request_schema: serde_json::json!({"type":"object"}),
            response_schema: serde_json::json!({"type":"object"}),
            max_request_bytes: 64,
            max_response_bytes: 64,
        });
        value.ui.push(UiContribution {
            id: "action".into(),
            version: 1,
            kind: UiContributionKind::Embedded,
            artifact: "client".into(),
            outlet: Some(UiOutlet::EntityAction),
            title: None,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value
            .permissions
            .retain(|permission| permission != "client.entity_action");
        assert!(value.validate(SUPPORTED_HOST_API).is_err());

        value.permissions.push("client.entity_action".into());
        value.server.as_mut().unwrap().commands[0].handler = "".into();
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
        value.server.as_mut().unwrap().commands[0].handler = "refresh-handler".into();
        value.server.as_mut().unwrap().commands[0].max_response_bytes = 65_537;
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
    }

    #[test]
    fn validates_explicit_context_outlet_kinds_and_capabilities() {
        let mut value = manifest();
        value.catalog.host_api = ">=1.1.0, <2.0.0".into();
        value.permissions.push("client.explorer_row_action".into());
        value.artifacts.push(Artifact {
            id: "client".into(),
            kind: ArtifactKind::ClientComponent,
            path: "client.js".into(),
        });
        value.ui.push(UiContribution {
            id: "explorer-action".into(),
            version: 1,
            kind: UiContributionKind::Action,
            artifact: "client".into(),
            outlet: Some(UiOutlet::ExplorerRowAction),
            title: None,
        });
        assert!(value.validate(SUPPORTED_HOST_API).is_ok());
        value.ui[0].kind = UiContributionKind::Panel;
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
        value.ui[0].kind = UiContributionKind::Action;
        value
            .permissions
            .retain(|permission| permission != "client.explorer_row_action");
        assert!(value.validate(SUPPORTED_HOST_API).is_err());
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
