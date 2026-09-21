//! Strict, side-effect-free validation for solution-pack v1 archives.
//!
//! A validated pack is still only portable input to a future workspace plan. This
//! module neither persists a pack nor applies its resources.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::{Cursor, Read},
    path::{Component, Path},
};

use catalog_blueprint::{
    BlueprintDefinition, BlueprintError, BlueprintKind, CompiledBlueprint, ResolvedInclude,
    ViewDefinition, ViewNode,
};
use catalog_validation::is_valid_code;
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::extensions::{
    ExtensionLayoutPlacement, classify_extension_layout_placement, valid_contribution_key,
};

pub const SOLUTION_PACK_MANIFEST_VERSION: u32 = 1;
pub const SOLUTION_PACK_RESOURCE_FORMAT_VERSION: u32 = 1;
pub const SOLUTION_PACK_MANIFEST_PATH: &str = "solution-pack.json";
pub const MAX_SOLUTION_PACK_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_EXPANDED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_SOLUTION_PACK_ARCHIVE_ENTRIES: usize = 256;
pub const MAX_SOLUTION_PACK_BLUEPRINTS: usize = 64;
pub const MAX_SOLUTION_PACK_WORKSPACE_SETTINGS: usize = 2;
pub const MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES: usize = 64;
pub const MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES: usize = 64;
pub const MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ROLES: usize = 16;
pub const MAX_SOLUTION_PACK_EXTENSION_REQUIREMENTS: usize = 64;
pub const MAX_SOLUTION_PACK_CHECKLIST_ITEMS: usize = 64;
pub const MAX_SOLUTION_PACK_CHECKS: usize = 64;
pub const MAX_SOLUTION_PACK_README_BYTES: usize = 64 * 1024;
pub const MAX_SOLUTION_PACK_RELEASE_NOTES_BYTES: usize = 32 * 1024;
pub const MAX_SOLUTION_PACK_CHECKLIST_BYTES: usize = 64 * 1024;
pub const MAX_SOLUTION_PACK_CHECKS_BYTES: usize = 128 * 1024;
pub const MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES: usize = 64 * 1024;
pub const MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH: usize = 16;
pub const MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS: usize = 256;
pub const MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES: usize = 128;
pub const MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES: usize = 4 * 1024;
pub const MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES: usize = 16;
pub const MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES: usize = 256;
pub const MAX_SOLUTION_PACK_TOTAL_BLUEPRINT_COMPLEXITY: usize = 4096;
pub const MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES: usize = 1024;
pub const MAX_SOLUTION_PACK_INSPECTION_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_SOLUTION_PACK_PLAN_RESPONSE_BYTES: usize = 1024 * 1024;
pub const MAX_SOLUTION_PACK_CHECK_RUN_RESPONSE_BYTES: usize = 512 * 1024;
pub const SOLUTION_PACK_PLAN_EXPIRY_HOURS: i64 = 24;
pub const MAX_SOLUTION_PACK_PREFIX_BYTES: usize = 32;
const MAX_ARCHIVE_PATH_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_NAME_BYTES: usize = 200;
const MAX_DESCRIPTION_BYTES: usize = 4096;
const MAX_VERSION_REQUIREMENT_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackManifest {
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub catalog: SolutionPackCatalog,
    pub resources: SolutionPackResources,
    #[serde(default)]
    pub extensions: Vec<SolutionPackExtensionRequirement>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub documentation: Option<SolutionPackDocumentation>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub checks: Option<SolutionPackFileRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackDocumentation {
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub readme: Option<SolutionPackFileRef>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub release_notes: Option<SolutionPackFileRef>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub setup_checklist: Option<SolutionPackFileRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackFileRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackSetupChecklist {
    pub format_version: u32,
    pub items: Vec<SolutionPackSetupChecklistItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackSetupChecklistItem {
    pub key: String,
    pub title: String,
    pub markdown: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub check: Option<String>,
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackChecksFile {
    pub format_version: u32,
    pub checks: Vec<SolutionPackCheckDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackCheckDefinition {
    pub key: String,
    pub title: String,
    pub predicate: SolutionPackCheckPredicate,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SolutionPackCheckPredicate {
    BlueprintPublished { blueprint: String },
    ExtensionInstalled { extension: String },
    ExtensionEnabled { extension: String },
    ExtensionConfigurationMatches { extension: String },
    ExploreNavigationEntryPresent { blueprint: String },
    WorkspaceExtensionLayoutPlacementPresent { contribution: String },
}

impl SolutionPackCheckPredicate {
    pub fn predicate_type(&self) -> &'static str {
        match self {
            Self::BlueprintPublished { .. } => "blueprint_published",
            Self::ExtensionInstalled { .. } => "extension_installed",
            Self::ExtensionEnabled { .. } => "extension_enabled",
            Self::ExtensionConfigurationMatches { .. } => "extension_configuration_matches",
            Self::ExploreNavigationEntryPresent { .. } => "explore_navigation_entry_present",
            Self::WorkspaceExtensionLayoutPlacementPresent { .. } => {
                "workspace_extension_layout_placement_present"
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackCatalog {
    pub host_api: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackResources {
    #[serde(default)]
    pub blueprints: Vec<SolutionPackResource>,
    #[serde(default)]
    pub workspace_settings: Vec<SolutionPackResource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackResource {
    pub key: String,
    pub path: String,
    pub required: bool,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackExtensionRequirement {
    pub key: String,
    pub id: String,
    pub version: String,
    pub required: bool,
    pub configuration_template: Option<SolutionPackConfigurationTemplateRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackConfigurationTemplateRef {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackExploreNavigation {
    pub format_version: u32,
    pub kind: String,
    pub entries: Vec<SolutionPackExploreNavigationEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackExploreNavigationEntry {
    pub blueprint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_to_role_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackExtensionLayout {
    pub format_version: u32,
    pub kind: String,
    pub entries: Vec<SolutionPackExtensionLayoutEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackExtensionLayoutEntry {
    pub contribution: String,
    pub outlet: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub promoted: bool,
    pub required: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SolutionPackError {
    #[error("solution-pack manifest_version {0} is unsupported")]
    UnsupportedManifestVersion(u32),
    #[error("{0}")]
    Invalid(String),
}

/// An archive whose container, manifest, declared files, content, and logical
/// references have all been validated without reading or mutating workspace state.
#[derive(Debug)]
pub struct ValidatedSolutionPack {
    manifest: SolutionPackManifest,
    archive_sha256: String,
    files: BTreeMap<String, Vec<u8>>,
    blueprints: BTreeMap<String, SolutionPackBlueprint>,
    explore_navigation: Option<SolutionPackExploreNavigation>,
    extension_layout: Option<SolutionPackExtensionLayout>,
    configuration_templates: BTreeMap<String, Value>,
    guidance: SolutionPackGuidance,
    checks: Vec<SolutionPackCheckDefinition>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SolutionPackGuidance {
    pub readme_markdown: Option<String>,
    pub release_notes_markdown: Option<String>,
    pub setup_checklist: Option<SolutionPackSetupChecklist>,
}

/// A validated portable blueprint source. References in this source remain
/// pack-local logical keys until a future workspace plan maps them to native
/// blueprint codes and revisions.
#[derive(Debug)]
pub struct SolutionPackBlueprint {
    key: String,
    code: String,
    source: String,
    includes: Vec<SolutionPackBlueprintInclude>,
    dependencies: BTreeSet<String>,
    table_path_dependencies: BTreeSet<String>,
    kind: BlueprintKind,
    extension_layout: Vec<BlueprintExtensionLayoutEntry>,
}

#[derive(Clone, Debug)]
struct BlueprintExtensionLayoutEntry {
    contribution: String,
    outlet: String,
}

#[derive(Debug)]
pub struct SolutionPackBlueprintInclude {
    alias: String,
    key: String,
}

impl SolutionPackBlueprint {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn includes(&self) -> &[SolutionPackBlueprintInclude] {
        &self.includes
    }

    pub fn dependencies(&self) -> &BTreeSet<String> {
        &self.dependencies
    }

    pub fn table_path_dependencies(&self) -> &BTreeSet<String> {
        &self.table_path_dependencies
    }

    pub fn kind(&self) -> BlueprintKind {
        self.kind.clone()
    }
}

impl SolutionPackBlueprintInclude {
    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Clone, Copy)]
struct ArchiveLimits {
    compressed_bytes: usize,
    expanded_bytes: usize,
    file_bytes: usize,
    manifest_bytes: usize,
    entries: usize,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            compressed_bytes: MAX_SOLUTION_PACK_ARCHIVE_BYTES,
            expanded_bytes: MAX_SOLUTION_PACK_EXPANDED_BYTES,
            file_bytes: MAX_SOLUTION_PACK_FILE_BYTES,
            manifest_bytes: MAX_SOLUTION_PACK_MANIFEST_BYTES,
            entries: MAX_SOLUTION_PACK_ARCHIVE_ENTRIES,
        }
    }
}

impl ValidatedSolutionPack {
    pub fn from_tar_zst(archive: &[u8]) -> Result<Self, SolutionPackError> {
        Self::from_tar_zst_with_limits(archive, ArchiveLimits::default())
    }

    fn from_tar_zst_with_limits(
        archive: &[u8],
        limits: ArchiveLimits,
    ) -> Result<Self, SolutionPackError> {
        if archive.len() > limits.compressed_bytes {
            return invalid("solution-pack archive exceeds the compressed size limit");
        }

        let decoder = zstd::stream::read::Decoder::new(Cursor::new(archive)).map_err(|_| {
            SolutionPackError::Invalid("solution-pack archive is not valid zstd data".into())
        })?;
        // The extra byte lets us distinguish an exact-limit stream from a stream
        // that would expand beyond the limit. Draining after tar parsing also
        // accounts for tar padding and trailing decompressed data.
        let bounded_decoder = decoder.take(limits.expanded_bytes as u64 + 1);
        let mut tar = tar::Archive::new(bounded_decoder);
        let mut files = BTreeMap::new();
        let mut seen_paths = HashSet::new();
        let mut total_file_bytes = 0usize;
        let mut entries = tar.entries().map_err(|_| {
            SolutionPackError::Invalid("solution-pack archive is not valid tar data".into())
        })?;

        for (index, entry) in entries.by_ref().enumerate() {
            if index >= limits.entries {
                return invalid("solution-pack archive has too many entries");
            }
            let mut entry = entry.map_err(|_| {
                SolutionPackError::Invalid("solution-pack archive has an invalid entry".into())
            })?;
            let path = entry
                .path()
                .map_err(|_| {
                    SolutionPackError::Invalid("solution-pack archive entry path is invalid".into())
                })?
                .to_str()
                .ok_or_else(|| {
                    SolutionPackError::Invalid(
                        "solution-pack archive entry paths must be UTF-8".into(),
                    )
                })?
                .to_owned();
            let entry_type = entry.header().entry_type();
            let normalized_path = if entry_type.is_dir() {
                path.strip_suffix('/').unwrap_or(&path)
            } else {
                &path
            };
            if !safe_archive_path(normalized_path) {
                return invalid("solution-pack archive has an unsafe entry path");
            }
            if !seen_paths.insert(normalized_path.to_owned()) {
                return invalid("solution-pack archive contains duplicate entry paths");
            }
            if entry_type.is_dir() {
                continue;
            }
            if !entry_type.is_file() {
                return invalid("solution-pack archive contains a non-file entry");
            }

            let declared_size = usize::try_from(entry.size()).map_err(|_| {
                SolutionPackError::Invalid("solution-pack archive entry is too large".into())
            })?;
            let per_file_limit = if path == SOLUTION_PACK_MANIFEST_PATH {
                limits.manifest_bytes
            } else {
                limits.file_bytes
            };
            if declared_size > per_file_limit {
                return invalid(if path == SOLUTION_PACK_MANIFEST_PATH {
                    "solution-pack manifest exceeds the size limit"
                } else {
                    "solution-pack archive file exceeds the per-file size limit"
                });
            }
            if total_file_bytes.saturating_add(declared_size) > limits.expanded_bytes {
                return invalid("solution-pack archive exceeds the expanded size limit");
            }

            let mut bytes = Vec::with_capacity(declared_size);
            entry
                .by_ref()
                .take(declared_size as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| {
                    SolutionPackError::Invalid(
                        "solution-pack archive entry could not be read".into(),
                    )
                })?;
            if bytes.len() != declared_size {
                return invalid("solution-pack archive entry size is invalid");
            }
            total_file_bytes += bytes.len();
            files.insert(path, bytes);
        }

        let mut bounded_decoder = tar.into_inner();
        let mut trailing = [0u8; 8192];
        loop {
            let count = bounded_decoder.read(&mut trailing).map_err(|_| {
                SolutionPackError::Invalid("solution-pack archive could not be decompressed".into())
            })?;
            if count == 0 {
                break;
            }
            if trailing[..count].iter().any(|byte| *byte != 0) {
                return invalid("solution-pack archive contains data after the tar end marker");
            }
        }
        if bounded_decoder.limit() == 0 {
            return invalid("solution-pack archive exceeds the expanded size limit");
        }

        let manifest_bytes = files.remove(SOLUTION_PACK_MANIFEST_PATH).ok_or_else(|| {
            SolutionPackError::Invalid(format!(
                "solution-pack archive must contain {SOLUTION_PACK_MANIFEST_PATH} at its root"
            ))
        })?;
        let manifest: SolutionPackManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|_| {
                SolutionPackError::Invalid(
                    "solution-pack.json is not a valid strict v1 manifest".into(),
                )
            })?;
        validate_manifest(&manifest)?;
        validate_declared_files(&manifest, &files)?;
        let blueprints = validate_content(&manifest, &files)?;
        let explore_navigation = validate_explore_navigation(&manifest, &files, &blueprints)?;
        let extension_layout = validate_extension_layout(&manifest, &files)?;
        validate_blueprint_extension_layouts(&manifest, &blueprints)?;
        let configuration_templates = validate_configuration_templates(&manifest, &files)?;
        let (guidance, checks) =
            validate_guidance_and_checks(&manifest, &files, &extension_layout)?;

        Ok(Self {
            manifest,
            archive_sha256: sha256_hex(archive),
            files,
            blueprints,
            explore_navigation,
            extension_layout,
            configuration_templates,
            guidance,
            checks,
        })
    }

    pub fn manifest(&self) -> &SolutionPackManifest {
        &self.manifest
    }

    pub fn archive_sha256(&self) -> &str {
        &self.archive_sha256
    }

    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
    }

    pub fn blueprint(&self, key: &str) -> Option<&SolutionPackBlueprint> {
        self.blueprints.get(key)
    }

    pub fn explore_navigation(&self) -> Option<&SolutionPackExploreNavigation> {
        self.explore_navigation.as_ref()
    }

    pub fn extension_layout(&self) -> Option<&SolutionPackExtensionLayout> {
        self.extension_layout.as_ref()
    }

    pub fn configuration_template(&self, key: &str) -> Option<&Value> {
        self.configuration_templates.get(key)
    }

    pub fn guidance(&self) -> &SolutionPackGuidance {
        &self.guidance
    }

    pub fn checks(&self) -> &[SolutionPackCheckDefinition] {
        &self.checks
    }
}

fn validate_manifest(manifest: &SolutionPackManifest) -> Result<(), SolutionPackError> {
    if manifest.manifest_version != SOLUTION_PACK_MANIFEST_VERSION {
        return Err(SolutionPackError::UnsupportedManifestVersion(
            manifest.manifest_version,
        ));
    }
    validate_pack_id(&manifest.id)?;
    Version::parse(&manifest.version)
        .map_err(|_| SolutionPackError::Invalid("version must be SemVer".into()))?;
    validate_bounded_text(&manifest.name, "name", MAX_NAME_BYTES)?;
    validate_bounded_text(&manifest.description, "description", MAX_DESCRIPTION_BYTES)?;
    let host = Version::parse(crate::extensions::SUPPORTED_HOST_API)
        .map_err(|_| SolutionPackError::Invalid("supported host API version is invalid".into()))?;
    let host_api = parse_version_req(&manifest.catalog.host_api).map_err(|_| {
        SolutionPackError::Invalid("catalog.host_api must be a SemVer range".into())
    })?;
    if !host_api.matches(&host) {
        return invalid("catalog.host_api is incompatible with this host");
    }

    if manifest.resources.blueprints.is_empty() && manifest.resources.workspace_settings.is_empty()
    {
        return invalid("at least one blueprint or workspace setting is required");
    }
    if manifest.resources.blueprints.len() > MAX_SOLUTION_PACK_BLUEPRINTS {
        return invalid("solution-pack manifest declares too many blueprints");
    }
    if manifest.resources.workspace_settings.len() > MAX_SOLUTION_PACK_WORKSPACE_SETTINGS {
        return invalid("solution-pack manifest declares too many workspace settings");
    }

    let mut keys = HashSet::new();
    let mut paths = HashSet::new();
    for (kind, resource) in manifest
        .resources
        .blueprints
        .iter()
        .map(|resource| ("blueprints", resource))
        .chain(
            manifest
                .resources
                .workspace_settings
                .iter()
                .map(|resource| ("workspace_settings", resource)),
        )
    {
        validate_resource(kind, resource)?;
        if !keys.insert(&resource.key) {
            return invalid(format!("duplicate resource key '{}'", resource.key));
        }
        if !paths.insert(&resource.path) {
            return invalid(format!("duplicate resource path '{}'", resource.path));
        }
    }

    if manifest.extensions.len() > MAX_SOLUTION_PACK_EXTENSION_REQUIREMENTS {
        return invalid("solution-pack manifest declares too many extension requirements");
    }
    let mut extension_ids = HashSet::new();
    for requirement in &manifest.extensions {
        validate_extension_requirement(requirement)?;
        if !keys.insert(&requirement.key) {
            return invalid(format!("duplicate resource key '{}'", requirement.key));
        }
        if !extension_ids.insert(&requirement.id) {
            return invalid(format!(
                "duplicate extension requirement id '{}'",
                requirement.id
            ));
        }
        if let Some(template) = &requirement.configuration_template
            && !paths.insert(&template.path)
        {
            return invalid(format!("duplicate resource path '{}'", template.path));
        }
    }
    if manifest
        .documentation
        .as_ref()
        .is_some_and(|documentation| {
            documentation.readme.is_none()
                && documentation.release_notes.is_none()
                && documentation.setup_checklist.is_none()
        })
    {
        return invalid("solution-pack documentation must declare at least one file");
    }
    for (label, reference, suffix) in guidance_file_references(manifest) {
        if !safe_archive_path(&reference.path) || !reference.path.ends_with(suffix) {
            return invalid(format!("solution-pack {label} path is invalid"));
        }
        parse_sha256(&reference.sha256).map_err(|()| {
            SolutionPackError::Invalid(format!(
                "solution-pack {label} sha256 must be 64 lowercase hexadecimal characters"
            ))
        })?;
        if !paths.insert(&reference.path) {
            return invalid(format!("duplicate resource path '{}'", reference.path));
        }
    }
    Ok(())
}

fn guidance_file_references(
    manifest: &SolutionPackManifest,
) -> Vec<(&'static str, &SolutionPackFileRef, &'static str)> {
    let mut references = Vec::new();
    if let Some(documentation) = &manifest.documentation {
        if let Some(reference) = &documentation.readme {
            references.push(("README", reference, ".md"));
        }
        if let Some(reference) = &documentation.release_notes {
            references.push(("release notes", reference, ".md"));
        }
        if let Some(reference) = &documentation.setup_checklist {
            references.push(("setup checklist", reference, ".json"));
        }
    }
    if let Some(reference) = &manifest.checks {
        references.push(("checks", reference, ".json"));
    }
    references
}

fn validate_extension_requirement(
    requirement: &SolutionPackExtensionRequirement,
) -> Result<(), SolutionPackError> {
    let Some(code) = requirement.key.strip_prefix("extensions/") else {
        return invalid(format!(
            "extension requirement key '{}' must be in the extensions/ namespace",
            requirement.key
        ));
    };
    if requirement.key.len() > MAX_IDENTIFIER_BYTES
        || !is_valid_stable_code(code)
        || code.contains('/')
    {
        return invalid(format!(
            "extension requirement key '{}' is invalid",
            requirement.key
        ));
    }
    crate::extensions::valid_id(&requirement.id, "extension requirement id")
        .map_err(|error| SolutionPackError::Invalid(error.to_string()))?;
    if requirement.version.is_empty()
        || requirement.version.len() > MAX_VERSION_REQUIREMENT_BYTES
        || requirement.version.trim() != requirement.version
    {
        return invalid(format!(
            "extension requirement '{}' version must be a trimmed SemVer range of at most {MAX_VERSION_REQUIREMENT_BYTES} bytes",
            requirement.key
        ));
    }
    parse_version_req(&requirement.version).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "extension requirement '{}' version must be a SemVer range",
            requirement.key
        ))
    })?;
    if let Some(template) = &requirement.configuration_template {
        if !safe_archive_path(&template.path)
            || !is_safe_configuration_template_path(&template.path)
        {
            return invalid(format!(
                "extension requirement '{}' configuration template path is invalid",
                requirement.key
            ));
        }
        parse_sha256(&template.sha256).map_err(|()| {
            SolutionPackError::Invalid(format!(
                "extension requirement '{}' configuration template sha256 must be 64 lowercase hexadecimal characters",
                requirement.key
            ))
        })?;
    }
    Ok(())
}

fn validate_resource(kind: &str, resource: &SolutionPackResource) -> Result<(), SolutionPackError> {
    if kind == "workspace_settings" {
        let fixed_pair = matches!(
            (resource.key.as_str(), resource.path.as_str()),
            (
                "workspace/explore-navigation",
                "workspace/explore-navigation.json"
            ) | (
                "workspace/extension-layout",
                "workspace/extension-layout.json"
            )
        );
        if !fixed_pair {
            return invalid("workspace setting must use a supported fixed workspace key and path");
        }
        parse_sha256(&resource.sha256).map_err(|()| {
            SolutionPackError::Invalid(format!(
                "resource '{}' sha256 must be 64 lowercase hexadecimal characters",
                resource.key
            ))
        })?;
        return Ok(());
    }
    let Some(code) = resource.key.strip_prefix(&format!("{kind}/")) else {
        return invalid(format!(
            "resource key '{}' must be in the {kind}/ namespace",
            resource.key
        ));
    };
    if resource.key.len() > MAX_IDENTIFIER_BYTES
        || !is_valid_stable_code(code)
        || code.contains('/')
    {
        return invalid(format!("resource key '{}' is invalid", resource.key));
    }
    if !safe_archive_path(&resource.path)
        || !resource.path.starts_with(&format!("{kind}/"))
        || resource.path == SOLUTION_PACK_MANIFEST_PATH
    {
        return invalid(format!("resource path '{}' is invalid", resource.path));
    }
    let expected_extension = ".toml";
    if !resource.path.ends_with(expected_extension) {
        return invalid(format!(
            "resource path '{}' must end in {expected_extension}",
            resource.path
        ));
    }
    parse_sha256(&resource.sha256).map_err(|()| {
        SolutionPackError::Invalid(format!(
            "resource '{}' sha256 must be 64 lowercase hexadecimal characters",
            resource.key
        ))
    })?;
    Ok(())
}

fn validate_declared_files(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), SolutionPackError> {
    let declared = manifest
        .resources
        .blueprints
        .iter()
        .chain(&manifest.resources.workspace_settings)
        .map(|resource| resource.path.as_str())
        .chain(
            manifest
                .extensions
                .iter()
                .filter_map(|requirement| requirement.configuration_template.as_ref())
                .map(|template| template.path.as_str()),
        )
        .chain(
            guidance_file_references(manifest)
                .into_iter()
                .map(|(_, reference, _)| reference.path.as_str()),
        )
        .collect::<BTreeSet<_>>();
    let actual = files.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if let Some(missing) = declared.difference(&actual).next() {
        return invalid(format!("declared file '{missing}' is missing"));
    }
    if let Some(extra) = actual.difference(&declared).next() {
        return invalid(format!("archive file '{extra}' is not declared"));
    }

    for (key, path, digest) in manifest
        .resources
        .blueprints
        .iter()
        .chain(&manifest.resources.workspace_settings)
        .map(|resource| {
            (
                resource.key.as_str(),
                resource.path.as_str(),
                resource.sha256.as_str(),
            )
        })
        .chain(manifest.extensions.iter().filter_map(|requirement| {
            requirement.configuration_template.as_ref().map(|template| {
                (
                    requirement.key.as_str(),
                    template.path.as_str(),
                    template.sha256.as_str(),
                )
            })
        }))
        .chain(
            guidance_file_references(manifest)
                .into_iter()
                .map(|(label, reference, _)| {
                    (label, reference.path.as_str(), reference.sha256.as_str())
                }),
        )
    {
        let bytes = &files[path];
        let expected = parse_sha256(digest).expect("digest validated with manifest");
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if expected.ct_eq(&actual).unwrap_u8() != 1 {
            return invalid(format!("resource '{key}' does not match its sha256 digest"));
        }
    }
    Ok(())
}

fn validate_explore_navigation(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
) -> Result<Option<SolutionPackExploreNavigation>, SolutionPackError> {
    let Some(resource) = manifest
        .resources
        .workspace_settings
        .iter()
        .find(|resource| resource.key == "workspace/explore-navigation")
    else {
        return Ok(None);
    };
    let mut navigation: SolutionPackExploreNavigation =
        serde_json::from_slice(&files[&resource.path]).map_err(|_| {
            SolutionPackError::Invalid(
                "workspace Explore navigation is not valid strict JSON".into(),
            )
        })?;
    if navigation.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION {
        return invalid(format!(
            "workspace Explore navigation has unsupported format_version {}",
            navigation.format_version
        ));
    }
    if navigation.kind != "explore_navigation" {
        return invalid("workspace Explore navigation kind must be 'explore_navigation'");
    }
    if navigation.entries.is_empty()
        || navigation.entries.len() > MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES
    {
        return invalid(format!(
            "workspace Explore navigation must contain 1-{MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES} entries"
        ));
    }
    let mut blueprint_keys = HashSet::new();
    for entry in &mut navigation.entries {
        if entry.blueprint.len() > MAX_IDENTIFIER_BYTES
            || !entry.blueprint.starts_with("blueprints/")
            || !blueprint_keys.insert(entry.blueprint.clone())
        {
            return invalid(format!(
                "workspace Explore navigation blueprint reference '{}' is invalid or duplicated",
                entry.blueprint
            ));
        }
        let Some(blueprint) = blueprints.get(&entry.blueprint) else {
            return invalid(format!(
                "workspace Explore navigation references undeclared blueprint '{}'",
                entry.blueprint
            ));
        };
        if blueprint.kind() != BlueprintKind::Entity {
            return invalid(format!(
                "workspace Explore navigation blueprint '{}' must be an entity blueprint",
                entry.blueprint
            ));
        }
        if entry.visible_to_role_codes.len() > MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ROLES {
            return invalid(format!(
                "workspace Explore navigation entry '{}' declares too many role codes",
                entry.blueprint
            ));
        }
        for role_code in &entry.visible_to_role_codes {
            if !is_valid_stable_code(role_code) {
                return invalid(format!(
                    "workspace Explore navigation role code '{role_code}' is invalid"
                ));
            }
        }
        entry.visible_to_role_codes.sort();
        if entry
            .visible_to_role_codes
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return invalid(format!(
                "workspace Explore navigation entry '{}' contains duplicate role codes",
                entry.blueprint
            ));
        }
    }
    Ok(Some(navigation))
}

fn validate_extension_layout(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<SolutionPackExtensionLayout>, SolutionPackError> {
    let Some(resource) = manifest
        .resources
        .workspace_settings
        .iter()
        .find(|resource| resource.key == "workspace/extension-layout")
    else {
        return Ok(None);
    };
    let layout: SolutionPackExtensionLayout = serde_json::from_slice(&files[&resource.path])
        .map_err(|_| {
            SolutionPackError::Invalid("workspace extension layout is not valid strict JSON".into())
        })?;
    if layout.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION {
        return invalid(format!(
            "workspace extension layout has unsupported format_version {}",
            layout.format_version
        ));
    }
    if layout.kind != "extension_layout" {
        return invalid("workspace extension layout kind must be 'extension_layout'");
    }
    if layout.entries.is_empty()
        || layout.entries.len() > MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES
    {
        return invalid(format!(
            "workspace extension layout must contain 1-{MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES} entries"
        ));
    }
    let requirements = manifest
        .extensions
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect::<HashSet<_>>();
    let mut contributions = HashSet::new();
    for entry in &layout.entries {
        let Some((extension_id, _)) = entry.contribution.split_once(':') else {
            return invalid(format!(
                "workspace extension layout contribution '{}' is invalid",
                entry.contribution
            ));
        };
        if !valid_contribution_key(&entry.contribution)
            || !contributions.insert(entry.contribution.as_str())
        {
            return invalid(format!(
                "workspace extension layout contribution '{}' is invalid or duplicated",
                entry.contribution
            ));
        }
        if !requirements.contains(extension_id) {
            return invalid(format!(
                "workspace extension layout contribution '{}' has no extension requirement",
                entry.contribution
            ));
        }
        if serde_json::from_value::<crate::extensions::UiOutlet>(Value::String(
            entry.outlet.clone(),
        ))
        .is_err()
        {
            return invalid(format!(
                "workspace extension layout outlet '{}' is invalid",
                entry.outlet
            ));
        }
        if entry.promoted && (entry.outlet != "navigation" || entry.hidden) {
            return invalid(format!(
                "workspace extension layout contribution '{}' may be promoted only as visible navigation",
                entry.contribution
            ));
        }
    }
    Ok(Some(layout))
}

fn validate_blueprint_extension_layouts(
    manifest: &SolutionPackManifest,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
) -> Result<(), SolutionPackError> {
    let requirements = manifest
        .extensions
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect::<HashSet<_>>();
    for blueprint in blueprints.values() {
        for entry in &blueprint.extension_layout {
            let extension_id = entry
                .contribution
                .split_once(':')
                .map(|(extension_id, _)| extension_id)
                .expect("blueprint compiler validated contribution key");
            if !requirements.contains(extension_id) {
                return invalid(format!(
                    "blueprint '{}' extension layout contribution '{}' has no extension requirement",
                    blueprint.key, entry.contribution
                ));
            }
        }
    }
    Ok(())
}

fn validate_guidance_and_checks(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
    extension_layout: &Option<SolutionPackExtensionLayout>,
) -> Result<(SolutionPackGuidance, Vec<SolutionPackCheckDefinition>), SolutionPackError> {
    let mut guidance = SolutionPackGuidance::default();
    if let Some(documentation) = &manifest.documentation {
        if let Some(reference) = &documentation.readme {
            guidance.readme_markdown = Some(validate_markdown_file(
                &files[&reference.path],
                MAX_SOLUTION_PACK_README_BYTES,
                "README",
            )?);
        }
        if let Some(reference) = &documentation.release_notes {
            guidance.release_notes_markdown = Some(validate_markdown_file(
                &files[&reference.path],
                MAX_SOLUTION_PACK_RELEASE_NOTES_BYTES,
                "release notes",
            )?);
        }
        if let Some(reference) = &documentation.setup_checklist {
            let bytes = &files[&reference.path];
            if bytes.len() > MAX_SOLUTION_PACK_CHECKLIST_BYTES {
                return invalid("solution-pack setup checklist exceeds the size limit");
            }
            let mut checklist: SolutionPackSetupChecklist =
                serde_json::from_slice(bytes).map_err(|_| {
                    SolutionPackError::Invalid(
                        "solution-pack setup checklist is not valid strict JSON".into(),
                    )
                })?;
            if checklist.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION
                || checklist.items.is_empty()
                || checklist.items.len() > MAX_SOLUTION_PACK_CHECKLIST_ITEMS
            {
                return invalid("solution-pack setup checklist format or item count is invalid");
            }
            let mut keys = HashSet::new();
            for item in &mut checklist.items {
                validate_guidance_key(&item.key, "checklist/")?;
                if !keys.insert(item.key.as_str()) {
                    return invalid(format!("duplicate setup checklist key '{}'", item.key));
                }
                validate_bounded_text(&item.title, "setup checklist title", MAX_NAME_BYTES)?;
                item.markdown =
                    validate_markdown(&item.markdown, 4096, "setup checklist markdown")?;
                if let Some(check) = &item.check {
                    validate_guidance_key(check, "checks/")?;
                }
            }
            if serde_json::to_vec(&checklist)
                .map_err(|_| {
                    SolutionPackError::Invalid(
                        "solution-pack setup checklist could not be normalized".into(),
                    )
                })?
                .len()
                > MAX_SOLUTION_PACK_CHECKLIST_BYTES
            {
                return invalid("normalized solution-pack setup checklist exceeds the size limit");
            }
            guidance.setup_checklist = Some(checklist);
        }
    }

    let checks = if let Some(reference) = &manifest.checks {
        let bytes = &files[&reference.path];
        if bytes.len() > MAX_SOLUTION_PACK_CHECKS_BYTES {
            return invalid("solution-pack checks file exceeds the size limit");
        }
        let checks_file: SolutionPackChecksFile = serde_json::from_slice(bytes).map_err(|_| {
            SolutionPackError::Invalid("solution-pack checks file is not valid strict JSON".into())
        })?;
        if checks_file.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION
            || checks_file.checks.is_empty()
            || checks_file.checks.len() > MAX_SOLUTION_PACK_CHECKS
        {
            return invalid("solution-pack checks format or check count is invalid");
        }
        let blueprint_keys = manifest
            .resources
            .blueprints
            .iter()
            .map(|resource| resource.key.as_str())
            .collect::<HashSet<_>>();
        let extensions = manifest
            .extensions
            .iter()
            .map(|requirement| (requirement.key.as_str(), requirement))
            .collect::<HashMap<_, _>>();
        let mut keys = HashSet::new();
        for check in &checks_file.checks {
            validate_guidance_key(&check.key, "checks/")?;
            if !keys.insert(check.key.as_str()) {
                return invalid(format!("duplicate check key '{}'", check.key));
            }
            validate_bounded_text(&check.title, "check title", MAX_NAME_BYTES)?;
            match &check.predicate {
                SolutionPackCheckPredicate::BlueprintPublished { blueprint }
                | SolutionPackCheckPredicate::ExploreNavigationEntryPresent { blueprint } => {
                    if !blueprint_keys.contains(blueprint.as_str()) {
                        return invalid(format!(
                            "check '{}' references undeclared blueprint '{}'",
                            check.key, blueprint
                        ));
                    }
                }
                SolutionPackCheckPredicate::ExtensionInstalled { extension }
                | SolutionPackCheckPredicate::ExtensionEnabled { extension } => {
                    if !extensions.contains_key(extension.as_str()) {
                        return invalid(format!(
                            "check '{}' references undeclared extension '{}'",
                            check.key, extension
                        ));
                    }
                }
                SolutionPackCheckPredicate::ExtensionConfigurationMatches { extension } => {
                    if !extensions
                        .get(extension.as_str())
                        .is_some_and(|requirement| requirement.configuration_template.is_some())
                    {
                        return invalid(format!(
                            "check '{}' requires an extension configuration template",
                            check.key
                        ));
                    }
                }
                SolutionPackCheckPredicate::WorkspaceExtensionLayoutPlacementPresent {
                    contribution,
                } => {
                    if !valid_contribution_key(contribution)
                        || extension_layout.as_ref().map_or(0, |layout| {
                            layout
                                .entries
                                .iter()
                                .filter(|entry| entry.contribution == *contribution)
                                .count()
                        }) != 1
                    {
                        return invalid(format!(
                            "check '{}' references no unique workspace extension-layout contribution",
                            check.key
                        ));
                    }
                }
            }
        }
        checks_file.checks
    } else {
        Vec::new()
    };

    let check_keys = checks
        .iter()
        .map(|check| check.key.as_str())
        .collect::<HashSet<_>>();
    if let Some(checklist) = &guidance.setup_checklist {
        for item in &checklist.items {
            if let Some(check) = &item.check
                && !check_keys.contains(check.as_str())
            {
                return invalid(format!(
                    "setup checklist item '{}' references undeclared check '{}'",
                    item.key, check
                ));
            }
        }
    }
    Ok((guidance, checks))
}

fn validate_guidance_key(value: &str, prefix: &str) -> Result<(), SolutionPackError> {
    let suffix = value.strip_prefix(prefix).unwrap_or_default();
    if value.len() > MAX_IDENTIFIER_BYTES
        || suffix.is_empty()
        || suffix.contains('/')
        || !is_valid_stable_code(suffix)
    {
        return invalid(format!("guidance key '{value}' is invalid"));
    }
    Ok(())
}

fn validate_markdown_file(
    bytes: &[u8],
    limit: usize,
    label: &str,
) -> Result<String, SolutionPackError> {
    if bytes.len() > limit {
        return invalid(format!("solution-pack {label} exceeds the size limit"));
    }
    let markdown = std::str::from_utf8(bytes)
        .map_err(|_| SolutionPackError::Invalid(format!("solution-pack {label} must be UTF-8")))?;
    validate_markdown(markdown, limit, label)
}

fn validate_markdown(
    markdown: &str,
    limit: usize,
    label: &str,
) -> Result<String, SolutionPackError> {
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() || normalized.len() > limit || normalized.trim() != normalized {
        return invalid(format!(
            "solution-pack {label} must be non-empty, trimmed, bounded Markdown"
        ));
    }
    if normalized.chars().any(|character| {
        (character.is_control() && !matches!(character, '\n' | '\t'))
            || ('\u{7f}'..='\u{9f}').contains(&character)
    }) {
        return invalid(format!("solution-pack {label} contains control characters"));
    }
    // Guidance is deliberately display-only. Reject the Markdown constructs that
    // can embed active content, fetch resources, or carry executable snippets.
    let lower = normalized.to_ascii_lowercase();
    if normalized.contains('<')
        || normalized.contains('>')
        || normalized.contains("![")
        || normalized.contains('`')
        || lower.contains("javascript:")
        || lower.contains("data:")
        || lower.contains("file:")
        || lower.contains("mailto:")
    {
        return invalid(format!("solution-pack {label} contains unsafe Markdown"));
    }
    for (_, destination) in markdown_destinations(&normalized) {
        if !destination.starts_with('#') || destination.len() == 1 {
            return invalid(format!(
                "solution-pack {label} links may only target same-document fragments"
            ));
        }
    }
    for line in normalized.lines() {
        let trimmed = line.trim_start();
        if (line.starts_with("    ") || line.starts_with('\t')) && !trimmed.is_empty()
            || trimmed.starts_with("~~~")
        {
            return invalid(format!("solution-pack {label} cannot contain code blocks"));
        }
        if let Some((_, destination)) = trimmed.split_once("]:")
            && !destination.trim().starts_with('#')
        {
            return invalid(format!(
                "solution-pack {label} links may only target same-document fragments"
            ));
        }
    }
    Ok(normalized)
}

fn markdown_destinations(markdown: &str) -> Vec<(usize, &str)> {
    let mut destinations = Vec::new();
    let bytes = markdown.as_bytes();
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] == b']' && bytes[index + 1] == b'(' {
            let start = index + 2;
            if let Some(relative_end) = markdown[start..].find(')') {
                let end = start + relative_end;
                destinations.push((start, markdown[start..end].trim()));
                index = end;
            }
        }
        index += 1;
    }
    destinations
}

fn validate_configuration_templates(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Value>, SolutionPackError> {
    let mut templates = BTreeMap::new();
    for requirement in &manifest.extensions {
        let Some(reference) = &requirement.configuration_template else {
            continue;
        };
        let bytes = &files[&reference.path];
        if bytes.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES {
            return invalid(format!(
                "extension requirement '{}' configuration template exceeds the size limit",
                requirement.key
            ));
        }
        let template: Value = serde_json::from_slice(bytes).map_err(|_| {
            SolutionPackError::Invalid(format!(
                "extension requirement '{}' configuration template must be valid JSON",
                requirement.key
            ))
        })?;
        if !template.is_object() {
            return invalid(format!(
                "extension requirement '{}' configuration template must be a JSON object",
                requirement.key
            ));
        }
        let mut items = 0;
        validate_configuration_template_value(&template, 1, &mut items, &requirement.key)?;
        templates.insert(requirement.key.clone(), template);
    }
    Ok(templates)
}

fn validate_configuration_template_value(
    value: &Value,
    depth: usize,
    items: &mut usize,
    requirement_key: &str,
) -> Result<(), SolutionPackError> {
    if depth > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH {
        return invalid(format!(
            "extension requirement '{requirement_key}' configuration template exceeds the depth limit"
        ));
    }
    match value {
        Value::Object(object) => {
            *items = items.saturating_add(object.len());
            if *items > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS {
                return invalid(format!(
                    "extension requirement '{requirement_key}' configuration template exceeds the item limit"
                ));
            }
            for (key, value) in object {
                if key.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES {
                    return invalid(format!(
                        "extension requirement '{requirement_key}' configuration template has an oversized key"
                    ));
                }
                let key = key.to_ascii_lowercase();
                if [
                    "password",
                    "secret",
                    "token",
                    "api_key",
                    "private_key",
                    "credential",
                    "authorization",
                ]
                .iter()
                .any(|suffix| key == *suffix || key.ends_with(suffix))
                {
                    return invalid(format!(
                        "extension requirement '{requirement_key}' configuration template contains a secret-like key"
                    ));
                }
                validate_configuration_template_value(value, depth + 1, items, requirement_key)?;
            }
        }
        Value::Array(array) => {
            *items = items.saturating_add(array.len());
            if *items > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS {
                return invalid(format!(
                    "extension requirement '{requirement_key}' configuration template exceeds the item limit"
                ));
            }
            for value in array {
                validate_configuration_template_value(value, depth + 1, items, requirement_key)?;
            }
        }
        Value::String(string)
            if string.len() > MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES =>
        {
            return invalid(format!(
                "extension requirement '{requirement_key}' configuration template has an oversized string"
            ));
        }
        _ => {}
    }
    Ok(())
}

pub fn json_deep_contains(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            expected.iter().all(|(key, expected)| {
                actual
                    .get(key)
                    .is_some_and(|actual| json_deep_contains(actual, expected))
            })
        }
        _ => actual == expected,
    }
}

type ValidatedContent = BTreeMap<String, SolutionPackBlueprint>;

struct PreparedBlueprint {
    portable: SolutionPackBlueprint,
    native_definition: BlueprintDefinition,
    native_source: String,
    include_keys: Vec<String>,
}

fn validate_content(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<ValidatedContent, SolutionPackError> {
    let blueprint_codes = manifest
        .resources
        .blueprints
        .iter()
        .map(|resource| (resource.key.as_str(), resource_code(&resource.key)))
        .collect::<HashMap<_, _>>();

    let mut prepared = BTreeMap::new();
    for resource in &manifest.resources.blueprints {
        let source = std::str::from_utf8(&files[&resource.path]).map_err(|_| {
            SolutionPackError::Invalid(format!("blueprint '{}' must be UTF-8 TOML", resource.key))
        })?;
        let blueprint = prepare_blueprint(&resource.key, source, &blueprint_codes)?;
        prepared.insert(resource.key.clone(), blueprint);
    }

    let total_blueprint_complexity = prepared.values().try_fold(0usize, |total, blueprint| {
        total
            .checked_add(blueprint.native_definition.attributes.len())
            .and_then(|total| total.checked_add(blueprint.native_definition.includes.len()))
            .ok_or_else(|| {
                SolutionPackError::Invalid("solution-pack blueprint complexity is too large".into())
            })
    })?;
    if total_blueprint_complexity > MAX_SOLUTION_PACK_TOTAL_BLUEPRINT_COMPLEXITY {
        return invalid("solution-pack blueprint complexity exceeds the total limit");
    }

    let blueprint_dependencies = prepared
        .iter()
        .map(|(key, blueprint)| {
            (
                key.as_str(),
                blueprint
                    .include_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    validate_acyclic(&blueprint_dependencies, "blueprint include")?;

    let mut compiled = HashMap::new();
    for key in prepared.keys() {
        compile_pack_blueprint(key, &prepared, &mut compiled)?;
    }
    validate_incoming_relationships(&compiled)?;
    let table_path_dependencies = validate_table_paths(&compiled)?;
    let blueprints = prepared
        .into_iter()
        .map(|(key, mut blueprint)| {
            blueprint.portable.table_path_dependencies = table_path_dependencies[&key].clone();
            (key, blueprint.portable)
        })
        .collect();

    Ok(blueprints)
}

fn prepare_blueprint(
    key: &str,
    source: &str,
    blueprint_codes: &HashMap<&str, &str>,
) -> Result<PreparedBlueprint, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(source).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    let table = value.as_table_mut().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    if table.contains_key("extensions") {
        return invalid(format!(
            "blueprint '{key}' cannot contain extension metadata in solution-pack v1"
        ));
    }

    let mut portable_includes = Vec::new();
    let mut include_keys = Vec::new();
    let mut dependencies = BTreeSet::new();
    if let Some(includes) = table.get_mut("includes") {
        let includes = includes.as_array_mut().ok_or_else(|| {
            SolutionPackError::Invalid(format!(
                "blueprint '{key}' includes must use portable logical references"
            ))
        })?;
        for include in includes {
            let include = include.as_table_mut().ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' includes must use portable logical references"
                ))
            })?;
            if include.contains_key("code") || include.contains_key("version") {
                return invalid(format!(
                    "blueprint '{key}' include cannot contain native code or revision fields"
                ));
            }
            let alias = include
                .get("alias")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "blueprint '{key}' include must declare an alias"
                    ))
                })?
                .to_owned();
            let dependency = include
                .remove("key")
                .and_then(|value| value.as_str().map(str::to_owned))
                .ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "blueprint '{key}' include must reference a logical key"
                    ))
                })?;
            let dependency_code = blueprint_codes.get(dependency.as_str()).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' references undeclared include '{dependency}'"
                ))
            })?;
            include.insert(
                "code".into(),
                toml::Value::String((*dependency_code).into()),
            );
            include.insert("version".into(), toml::Value::Integer(1));
            portable_includes.push(SolutionPackBlueprintInclude {
                alias,
                key: dependency.clone(),
            });
            dependencies.insert(dependency.clone());
            include_keys.push(dependency);
        }
    }

    if let Some(attributes) = table
        .get_mut("attributes")
        .and_then(toml::Value::as_array_mut)
    {
        for attribute in attributes {
            if let Some(attribute) = attribute.as_table_mut() {
                rewrite_blueprint_reference(
                    key,
                    attribute,
                    "target_blueprint",
                    "relationship target",
                    blueprint_codes,
                    Some(&mut dependencies),
                )?;
            }
        }
    }
    if let Some(views) = table.get_mut("views").and_then(toml::Value::as_table_mut) {
        for (_, view) in views.iter_mut() {
            if let Some(view) = view.as_table_mut() {
                rewrite_view_references(key, view, blueprint_codes, Some(&mut dependencies))?;
            }
        }
    }

    let native_source = toml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    let definition = catalog_blueprint::parse(&native_source).map_err(|error| {
        let message = match error {
            BlueprintError::Toml(_) => format!("blueprint '{key}' is invalid"),
            error => format!("blueprint '{key}' is invalid: {error}"),
        };
        SolutionPackError::Invalid(message)
    })?;
    reject_workspace_dependent_blueprint_constructs(key, &definition)?;
    if definition.includes.len() > MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES {
        return invalid(format!(
            "blueprint '{key}' exceeds the include limit of {MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES}"
        ));
    }
    if definition.attributes.len() > MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
        return invalid(format!(
            "blueprint '{key}' exceeds the attribute limit of {MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES}"
        ));
    }
    for include in &definition.includes {
        if include.alias.len() > MAX_IDENTIFIER_BYTES {
            return invalid(format!(
                "blueprint '{key}' include alias '{}' is too long",
                include.alias
            ));
        }
    }
    let expected_code = resource_code(key);
    if definition.code != expected_code {
        return invalid(format!("blueprint '{key}' code must be '{expected_code}'"));
    }

    let mut extension_layout = definition
        .views
        .get("extension_layout")
        .and_then(|view| match view {
            ViewDefinition::ExtensionLayout { outlets, .. } => Some(outlets),
            _ => None,
        })
        .into_iter()
        .flat_map(|outlets| outlets.iter())
        .flat_map(|(outlet, layout)| {
            layout
                .order
                .iter()
                .chain(&layout.hidden)
                .map(move |contribution| BlueprintExtensionLayoutEntry {
                    contribution: contribution.clone(),
                    outlet: outlet.clone(),
                })
        })
        .collect::<Vec<_>>();
    extension_layout.sort_by(|left, right| {
        left.outlet
            .cmp(&right.outlet)
            .then_with(|| left.contribution.cmp(&right.contribution))
    });

    Ok(PreparedBlueprint {
        portable: SolutionPackBlueprint {
            key: key.to_owned(),
            code: definition.code.clone(),
            source: source.to_owned(),
            includes: portable_includes,
            dependencies,
            table_path_dependencies: BTreeSet::new(),
            kind: definition.kind.clone(),
            extension_layout,
        },
        native_definition: definition,
        native_source,
        include_keys,
    })
}

fn reject_workspace_dependent_blueprint_constructs(
    key: &str,
    definition: &BlueprintDefinition,
) -> Result<(), SolutionPackError> {
    if !definition.publication.retain_on_edit_roles.is_empty() {
        return invalid(format!(
            "blueprint '{key}' cannot declare workspace roles in solution-pack v1"
        ));
    }
    for view in definition.views.values() {
        match view {
            ViewDefinition::Table {
                columns: Some(columns),
                ..
            } if columns.iter().any(|column| {
                column
                    .renderer
                    .as_ref()
                    .is_some_and(|renderer| !renderer.id.starts_with("catalog."))
            }) =>
            {
                return invalid(format!(
                    "blueprint '{key}' cannot declare extension table renderers in solution-pack v1"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn rewrite_blueprint_reference(
    owner_key: &str,
    table: &mut toml::map::Map<String, toml::Value>,
    field: &str,
    label: &str,
    blueprint_codes: &HashMap<&str, &str>,
    dependencies: Option<&mut BTreeSet<String>>,
) -> Result<(), SolutionPackError> {
    let Some(reference) = table.get_mut(field) else {
        return Ok(());
    };
    let logical_key = reference.as_str().ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{owner_key}' {label} must be a logical key"
        ))
    })?;
    let code = blueprint_codes.get(logical_key).ok_or_else(|| {
        SolutionPackError::Invalid(format!(
            "blueprint '{owner_key}' references undeclared {label} '{logical_key}'"
        ))
    })?;
    if let Some(dependencies) = dependencies {
        dependencies.insert(logical_key.to_owned());
    }
    *reference = toml::Value::String((*code).to_owned());
    Ok(())
}

fn rewrite_view_references(
    owner_key: &str,
    node: &mut toml::map::Map<String, toml::Value>,
    blueprint_codes: &HashMap<&str, &str>,
    mut dependencies: Option<&mut BTreeSet<String>>,
) -> Result<(), SolutionPackError> {
    if node.get("type").and_then(toml::Value::as_str) == Some("incoming_relationship_list")
        && let Some(relationships) = node
            .get_mut("relationships")
            .and_then(toml::Value::as_array_mut)
    {
        for relationship in relationships {
            if let Some(relationship) = relationship.as_table_mut() {
                rewrite_blueprint_reference(
                    owner_key,
                    relationship,
                    "source_blueprint",
                    "view source blueprint",
                    blueprint_codes,
                    dependencies.as_deref_mut(),
                )?;
            }
        }
    }

    for collection in ["children", "tabs", "sections"] {
        if let Some(children) = node.get_mut(collection).and_then(toml::Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_table_mut() {
                    rewrite_view_references(
                        owner_key,
                        child,
                        blueprint_codes,
                        dependencies.as_deref_mut(),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn compile_pack_blueprint(
    key: &str,
    blueprints: &BTreeMap<String, PreparedBlueprint>,
    compiled: &mut HashMap<String, CompiledBlueprint>,
) -> Result<(), SolutionPackError> {
    if compiled.contains_key(key) {
        return Ok(());
    }
    let blueprint = &blueprints[key];
    let mut resolved = Vec::with_capacity(blueprint.native_definition.includes.len());
    let mut resolved_attribute_count = 0usize;
    for (include, dependency_key) in blueprint
        .native_definition
        .includes
        .iter()
        .zip(&blueprint.include_keys)
    {
        compile_pack_blueprint(dependency_key, blueprints, compiled)?;
        let dependency = &compiled[dependency_key];
        if dependency.kind != BlueprintKind::Mixin {
            return invalid(format!(
                "blueprint '{key}' include '{dependency_key}' must reference a mixin"
            ));
        }
        resolved_attribute_count = resolved_attribute_count
            .checked_add(dependency.attributes.len())
            .ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "blueprint '{key}' resolved include attributes are too large"
                ))
            })?;
        if resolved_attribute_count > MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES {
            return invalid(format!(
                "blueprint '{key}' exceeds the resolved include attribute limit of {MAX_SOLUTION_PACK_RESOLVED_INCLUDE_ATTRIBUTES}"
            ));
        }
        resolved.push(ResolvedInclude {
            alias: include.alias.clone(),
            code: include.code.clone(),
            version: include.version,
            attributes: dependency.attributes.clone(),
        });
    }
    let definition = catalog_blueprint::compile(
        blueprint.native_definition.clone(),
        &resolved,
        &blueprint.native_source,
    )
    .map_err(|error| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' fails compiler validation: {error}"
        ))
    })?;
    compiled.insert(key.to_owned(), definition);
    Ok(())
}

fn validate_table_paths(
    blueprints: &HashMap<String, CompiledBlueprint>,
) -> Result<BTreeMap<String, BTreeSet<String>>, SolutionPackError> {
    let by_code = blueprints
        .iter()
        .map(|(key, blueprint)| (blueprint.code.as_str(), key.as_str()))
        .collect::<HashMap<_, _>>();
    let mut dependencies = BTreeMap::new();

    for (owner_key, owner) in blueprints {
        let mut owner_dependencies = BTreeSet::new();
        for view in owner.views.values() {
            let ViewDefinition::Table {
                columns: Some(columns),
                ..
            } = view
            else {
                continue;
            };
            for column in columns {
                let parts = column.field.split('.').collect::<Vec<_>>();
                if parts.len() == 1 {
                    continue;
                }
                let mut current = owner;
                for segment in &parts[..parts.len() - 1] {
                    let attribute = current
                        .attributes
                        .iter()
                        .find(|attribute| attribute.code == *segment)
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' table column '{}' path segment '{segment}' was not found",
                                column.field
                            ))
                        })?;
                    if attribute.value_type != "relationship" {
                        return invalid(format!(
                            "blueprint '{owner_key}' table column '{}' segment '{segment}' must be a relationship",
                            column.field
                        ));
                    }
                    let target_code = attribute.target_blueprint.as_deref().ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' segment '{segment}' has no target blueprint",
                            column.field
                        ))
                    })?;
                    let target_key = by_code.get(target_code).ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' target '{target_code}' is not declared by the pack",
                            column.field
                        ))
                    })?;
                    owner_dependencies.insert((*target_key).to_owned());
                    current = &blueprints[*target_key];
                }
                let leaf = parts.last().expect("multi-part table path has a leaf");
                let leaf_attribute = current
                    .attributes
                    .iter()
                    .find(|attribute| attribute.code == *leaf)
                    .ok_or_else(|| {
                        SolutionPackError::Invalid(format!(
                            "blueprint '{owner_key}' table column '{}' path segment '{leaf}' was not found",
                            column.field
                        ))
                    })?;
                if matches!(leaf_attribute.value_type.as_str(), "relationship" | "file") {
                    return invalid(format!(
                        "blueprint '{owner_key}' table column '{}' leaf must be scalar",
                        column.field
                    ));
                }
            }
        }
        dependencies.insert(owner_key.clone(), owner_dependencies);
    }
    Ok(dependencies)
}

fn validate_incoming_relationships(
    blueprints: &HashMap<String, CompiledBlueprint>,
) -> Result<(), SolutionPackError> {
    let by_code = blueprints
        .iter()
        .map(|(key, blueprint)| (blueprint.code.as_str(), (key.as_str(), blueprint)))
        .collect::<HashMap<_, _>>();

    for (owner_key, owner) in blueprints {
        for view in owner.views.values() {
            validate_incoming_relationships_in_view(owner_key, owner, view, &by_code)?;
        }
    }
    Ok(())
}

fn validate_incoming_relationships_in_view(
    owner_key: &str,
    owner: &CompiledBlueprint,
    view: &ViewDefinition,
    blueprints: &HashMap<&str, (&str, &CompiledBlueprint)>,
) -> Result<(), SolutionPackError> {
    match view {
        ViewDefinition::Stack { children, .. }
        | ViewDefinition::Grid { children, .. }
        | ViewDefinition::Section { children, .. } => {
            validate_incoming_relationships_in_nodes(owner_key, owner, children, blueprints)
        }
        ViewDefinition::Tabs { tabs, .. } => {
            for tab in tabs {
                validate_incoming_relationships_in_nodes(
                    owner_key,
                    owner,
                    &tab.children,
                    blueprints,
                )?;
            }
            Ok(())
        }
        ViewDefinition::Accordion { sections, .. } => {
            for section in sections {
                validate_incoming_relationships_in_nodes(
                    owner_key,
                    owner,
                    &section.children,
                    blueprints,
                )?;
            }
            Ok(())
        }
        ViewDefinition::DropdownOption { .. }
        | ViewDefinition::Table { .. }
        | ViewDefinition::ExtensionLayout { .. } => Ok(()),
    }
}

fn validate_incoming_relationships_in_nodes(
    owner_key: &str,
    owner: &CompiledBlueprint,
    nodes: &[ViewNode],
    blueprints: &HashMap<&str, (&str, &CompiledBlueprint)>,
) -> Result<(), SolutionPackError> {
    for node in nodes {
        match node {
            ViewNode::Stack { children, .. }
            | ViewNode::Grid { children, .. }
            | ViewNode::Section { children, .. } => {
                validate_incoming_relationships_in_nodes(owner_key, owner, children, blueprints)?
            }
            ViewNode::Tabs { tabs, .. } => {
                for tab in tabs {
                    validate_incoming_relationships_in_nodes(
                        owner_key,
                        owner,
                        &tab.children,
                        blueprints,
                    )?;
                }
            }
            ViewNode::Accordion { sections, .. } => {
                for section in sections {
                    validate_incoming_relationships_in_nodes(
                        owner_key,
                        owner,
                        &section.children,
                        blueprints,
                    )?;
                }
            }
            ViewNode::IncomingRelationshipList { relationships, .. } => {
                for relationship in relationships {
                    let (source_key, source) = blueprints
                        .get(relationship.source_blueprint.as_str())
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' incoming relationship references unavailable source blueprint '{}'",
                                relationship.source_blueprint
                            ))
                        })?;
                    let attribute = source
                        .attributes
                        .iter()
                        .find(|attribute| attribute.code == relationship.field)
                        .ok_or_else(|| {
                            SolutionPackError::Invalid(format!(
                                "blueprint '{owner_key}' incoming relationship source '{source_key}' has no field '{}'",
                                relationship.field
                            ))
                        })?;
                    if attribute.value_type != "relationship" {
                        return invalid(format!(
                            "blueprint '{owner_key}' incoming relationship source '{source_key}' field '{}' must be a relationship",
                            relationship.field
                        ));
                    }
                    if attribute.target_blueprint.as_deref() != Some(owner.code.as_str()) {
                        return invalid(format!(
                            "blueprint '{owner_key}' incoming relationship source '{source_key}' field '{}' must target '{}'",
                            relationship.field, owner.code
                        ));
                    }
                }
            }
            ViewNode::Heading { .. }
            | ViewNode::Text { .. }
            | ViewNode::Divider { .. }
            | ViewNode::Field { .. }
            | ViewNode::RelationshipList { .. } => {}
        }
    }
    Ok(())
}

fn validate_acyclic<'a>(
    graph: &HashMap<&'a str, Vec<&'a str>>,
    label: &str,
) -> Result<(), SolutionPackError> {
    fn visit<'a>(
        node: &'a str,
        graph: &HashMap<&'a str, Vec<&'a str>>,
        visiting: &mut HashSet<&'a str>,
        visited: &mut HashSet<&'a str>,
        label: &str,
    ) -> Result<(), SolutionPackError> {
        if visited.contains(node) {
            return Ok(());
        }
        if !visiting.insert(node) {
            return invalid(format!("{label} references contain a cycle at '{node}'"));
        }
        if let Some(dependencies) = graph.get(node) {
            for dependency in dependencies {
                visit(dependency, graph, visiting, visited, label)?;
            }
        }
        visiting.remove(node);
        visited.insert(node);
        Ok(())
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut visiting, &mut visited, label)?;
    }
    Ok(())
}

fn validate_pack_id(value: &str) -> Result<(), SolutionPackError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.split('.').count() < 2
        || !value.split('.').all(is_valid_id_segment)
    {
        return invalid("id must be a lowercase reverse-DNS-style stable identifier");
    }
    Ok(())
}

fn is_valid_id_segment(segment: &str) -> bool {
    segment
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !segment.ends_with('-')
}

fn is_valid_stable_code(value: &str) -> bool {
    value.len() <= MAX_IDENTIFIER_BYTES
        && is_valid_code(value)
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
        && uuid::Uuid::parse_str(value).is_err()
}

fn validate_bounded_text(
    value: &str,
    field: &str,
    max_bytes: usize,
) -> Result<(), SolutionPackError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max_bytes
        || value.chars().any(|character| {
            (character.is_control() && !matches!(character, '\n' | '\t'))
                || ('\u{7f}'..='\u{9f}').contains(&character)
        })
    {
        return invalid(format!(
            "{field} must be non-empty, trimmed, control-free, and at most {max_bytes} bytes"
        ));
    }
    Ok(())
}

fn is_safe_configuration_template_path(value: &str) -> bool {
    value.strip_prefix("extensions/").is_some_and(|path| {
        path.ends_with(".json")
            && path.split('/').all(|component| {
                component
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                    && component.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
            })
    })
}

fn safe_archive_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_ARCHIVE_PATH_BYTES
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains(['\\', '\0', ':'])
        || value
            .chars()
            .any(|character| character <= '\u{1f}' || ('\u{7f}'..='\u{9f}').contains(&character))
        || value.split('/').any(|part| part.is_empty())
    {
        return false;
    }
    Path::new(value)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn resource_code(key: &str) -> &str {
    key.rsplit_once('/').expect("resource key validated").1
}

pub(crate) fn parse_version_req(value: &str) -> Result<VersionReq, semver::Error> {
    VersionReq::parse(value).or_else(|original_error| {
        let comparators = value.split_ascii_whitespace().collect::<Vec<_>>();
        if comparators.len() < 2 || comparators.iter().any(|item| item.contains(',')) {
            return Err(original_error);
        }
        VersionReq::parse(&comparators.join(", "))
    })
}

fn parse_sha256(value: &str) -> Result<[u8; 32], ()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(());
    }
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = (hex_nibble(value.as_bytes()[offset])? << 4)
            | hex_nibble(value.as_bytes()[offset + 1])?;
    }
    Ok(digest)
}

fn hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn invalid<T>(message: impl Into<String>) -> Result<T, SolutionPackError> {
    Err(SolutionPackError::Invalid(message.into()))
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintPublication {
    Draft,
    Publish,
}

impl BlueprintPublication {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Publish => "publish",
        }
    }
}

#[derive(Clone, Debug)]
pub struct InstalledExtensionSnapshot {
    pub installed_release_id: uuid::Uuid,
    pub version: String,
    pub state: String,
    pub configuration: Value,
    pub policy_compatible: bool,
    /// Stable contribution key to its manifest-declared outlet.
    pub contributions: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanningExploreNavigationEntry {
    pub blueprint_code: String,
    pub visible_to_role_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlueprintMappingRequest {
    pub key: String,
    pub code: String,
}

#[derive(Clone, Debug)]
pub struct ExistingBlueprintSnapshot {
    pub id: uuid::Uuid,
    pub code: String,
    pub version: i64,
    pub kind: String,
    /// Hash of the stored definition after the same TOML parse/serialize
    /// canonicalization used for normalized pack definitions.
    pub canonical_definition_hash: String,
    /// Raw stored-source hash retained as immutable stale evidence.
    pub definition_hash: String,
}

#[derive(Debug)]
pub struct PlanningWorkspaceSnapshot {
    pub workspace_id: uuid::Uuid,
    pub physical_codes: BTreeSet<String>,
    /// Explicit selections keyed by pack-local blueprint key. Missing targets
    /// are rejected before the planner is called; the planner never searches.
    pub existing_blueprints: BTreeMap<String, ExistingBlueprintSnapshot>,
    pub installed_extensions: BTreeMap<String, InstalledExtensionSnapshot>,
    pub explore_navigation: Vec<PlanningExploreNavigationEntry>,
    pub explore_navigation_valid: bool,
    pub extension_layout: Value,
    pub extension_layout_valid: bool,
    pub role_codes: BTreeSet<String>,
    pub published_entity_codes: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct PlannedMapping {
    pub resource_kind: &'static str,
    pub logical_key: String,
    pub target_id: uuid::Uuid,
    pub target_code: String,
    pub target_version: Option<i64>,
    pub mapping_kind: &'static str,
    pub snapshot: Value,
}

#[derive(Clone, Debug)]
pub struct PlannedAction {
    pub resource_kind: &'static str,
    pub logical_key: String,
    pub action: &'static str,
    pub reason_code: &'static str,
    pub summary: Value,
    pub normalized_payload: Option<Value>,
    pub preconditions: Value,
}

#[derive(Clone, Debug)]
pub struct PlannedExtensionRequirement {
    pub logical_key: String,
    pub extension_id: String,
    pub version_requirement: String,
    pub required: bool,
    pub configuration_template_path: Option<String>,
    pub configuration_template_sha256: Option<String>,
    pub status: &'static str,
    pub reason_code: &'static str,
    pub installed_release_id: Option<uuid::Uuid>,
    pub installed_version: Option<String>,
    pub installed_state: Option<String>,
    pub configuration_matches: Option<bool>,
    pub evaluation_template: Value,
}

#[derive(Debug)]
pub struct SolutionPackPlanDraft {
    pub ready: bool,
    pub mappings: Vec<PlannedMapping>,
    pub actions: Vec<PlannedAction>,
    pub extension_requirements: Vec<PlannedExtensionRequirement>,
}

pub fn evaluate_extension_requirement(
    requirement: &SolutionPackExtensionRequirement,
    template: Option<&Value>,
    installed: Option<&InstalledExtensionSnapshot>,
) -> PlannedExtensionRequirement {
    let configuration_matches = installed.map(|installed| {
        template.is_none_or(|template| json_deep_contains(&installed.configuration, template))
    });
    let reason_code = match installed {
        None => "missing",
        Some(installed)
            if Version::parse(&installed.version).map_or(true, |version| {
                !parse_version_req(&requirement.version)
                    .expect("requirement version validated")
                    .matches(&version)
            }) =>
        {
            "incompatible_version"
        }
        Some(installed) if installed.state == "quarantined" => "quarantined",
        Some(installed) if !installed.policy_compatible => "policy_incompatible",
        Some(_) if configuration_matches != Some(true) => "configuration_mismatch",
        Some(_) => "satisfied",
    };
    let status = if reason_code == "satisfied" {
        "satisfied"
    } else if requirement.required {
        "blocked"
    } else {
        "skipped"
    };
    PlannedExtensionRequirement {
        logical_key: requirement.key.clone(),
        extension_id: requirement.id.clone(),
        version_requirement: requirement.version.clone(),
        required: requirement.required,
        configuration_template_path: requirement
            .configuration_template
            .as_ref()
            .map(|reference| reference.path.clone()),
        configuration_template_sha256: requirement
            .configuration_template
            .as_ref()
            .map(|reference| reference.sha256.clone()),
        status,
        reason_code,
        installed_release_id: installed.map(|installed| installed.installed_release_id),
        installed_version: installed.map(|installed| installed.version.clone()),
        installed_state: installed.map(|installed| installed.state.clone()),
        configuration_matches,
        evaluation_template: template.cloned().unwrap_or_else(|| serde_json::json!({})),
    }
}

fn extension_requirement_for_contribution<'a>(
    manifest: &'a SolutionPackManifest,
    contribution: &str,
) -> &'a SolutionPackExtensionRequirement {
    let extension_id = contribution
        .split_once(':')
        .map(|(extension_id, _)| extension_id)
        .expect("validated contribution key");
    manifest
        .extensions
        .iter()
        .find(|requirement| requirement.id == extension_id)
        .expect("validated contribution has an extension requirement")
}

fn contribution_unmet_reason(
    requirement: &SolutionPackExtensionRequirement,
    contribution: &str,
    outlet: &str,
    installed: Option<&InstalledExtensionSnapshot>,
) -> Option<&'static str> {
    let installed = installed?;
    let version_matches = Version::parse(&installed.version).is_ok_and(|version| {
        parse_version_req(&requirement.version)
            .expect("validated extension requirement")
            .matches(&version)
    });
    if !version_matches {
        return Some("incompatible_version");
    }
    if installed.state == "quarantined" {
        return Some("quarantined");
    }
    if !installed.policy_compatible {
        return Some("policy_incompatible");
    }
    match installed.contributions.get(contribution) {
        None => Some("contribution_missing"),
        Some(declared_outlet) if declared_outlet != outlet => Some("outlet_mismatch"),
        Some(_) => None,
    }
}

fn contribution_availability_reason(
    requirement: &SolutionPackExtensionRequirement,
    contribution: &str,
    outlet: &str,
    installed: Option<&InstalledExtensionSnapshot>,
) -> Option<&'static str> {
    if installed.is_none() {
        Some("missing")
    } else {
        contribution_unmet_reason(requirement, contribution, outlet, installed)
    }
}

pub fn validate_blueprint_mapping_requests(
    pack: &ValidatedSolutionPack,
    mappings: &[BlueprintMappingRequest],
) -> Result<(), SolutionPackError> {
    if mappings.len() > MAX_SOLUTION_PACK_BLUEPRINTS {
        return invalid("too many blueprint mappings");
    }
    let declared = pack
        .manifest()
        .resources
        .blueprints
        .iter()
        .map(|resource| resource.key.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for mapping in mappings {
        if !declared.contains(mapping.key.as_str()) {
            return invalid(format!("unknown blueprint mapping key '{}'", mapping.key));
        }
        if !seen.insert(mapping.key.as_str()) {
            return invalid(format!("duplicate blueprint mapping key '{}'", mapping.key));
        }
        if mapping.code.len() > MAX_IDENTIFIER_BYTES || !is_valid_stable_code(&mapping.code) {
            return invalid(format!(
                "invalid existing blueprint code for '{}'",
                mapping.key
            ));
        }
    }
    Ok(())
}

/// Builds a plan from an already validated local archive and an explicit set
/// of previously resolved existing-blueprint selections. No selection is ever
/// inferred from a code collision.
pub fn build_solution_pack_plan(
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    workspace: &PlanningWorkspaceSnapshot,
) -> Result<SolutionPackPlanDraft, SolutionPackError> {
    validate_plan_prefix(prefix)?;

    let manifest = pack.manifest();
    let resources = manifest
        .resources
        .blueprints
        .iter()
        .map(|resource| (resource.key.clone(), ("blueprint", resource)))
        .collect::<BTreeMap<_, _>>();

    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for resource in &manifest.resources.blueprints {
        dependencies.insert(
            resource.key.clone(),
            pack.blueprint(&resource.key)
                .expect("validated blueprint exists")
                .dependencies()
                .clone(),
        );
    }

    let mut mappings_by_key = BTreeMap::new();
    for (logical_key, (kind, _)) in &resources {
        let mapping = if let Some(existing) = workspace.existing_blueprints.get(logical_key) {
            PlannedMapping {
                resource_kind: kind,
                logical_key: logical_key.clone(),
                target_id: existing.id,
                target_code: existing.code.clone(),
                target_version: Some(existing.version),
                mapping_kind: "existing",
                snapshot: serde_json::json!({
                    "id": existing.id,
                    "code": existing.code,
                    "version": existing.version,
                    "kind": existing.kind,
                    "canonical_definition_hash": existing.canonical_definition_hash,
                    "definition_hash": existing.definition_hash,
                    "status": "published",
                    "deleted": false,
                }),
            }
        } else {
            let generated_code = format!("{prefix}_{}", resource_code(logical_key));
            if generated_code.len() > MAX_IDENTIFIER_BYTES || !is_valid_stable_code(&generated_code)
            {
                return invalid(format!(
                    "prefix produces an invalid physical code for '{logical_key}'"
                ));
            }
            PlannedMapping {
                resource_kind: kind,
                logical_key: logical_key.clone(),
                target_id: deterministic_target_id(
                    workspace.workspace_id,
                    pack,
                    prefix,
                    publication,
                    logical_key,
                ),
                target_code: generated_code.clone(),
                target_version: Some(1),
                mapping_kind: "create",
                snapshot: serde_json::json!({"code": generated_code, "version": 1}),
            }
        };
        mappings_by_key.insert(logical_key.clone(), mapping);
    }
    if pack.explore_navigation().is_some() {
        mappings_by_key.insert(
            "workspace/explore-navigation".to_owned(),
            PlannedMapping {
                resource_kind: "workspace_setting",
                logical_key: "workspace/explore-navigation".to_owned(),
                target_id: workspace.workspace_id,
                target_code: "explore_navigation".to_owned(),
                target_version: None,
                mapping_kind: "workspace",
                snapshot: serde_json::json!({"setting": "explore_navigation"}),
            },
        );
    }
    if pack.extension_layout().is_some() {
        mappings_by_key.insert(
            "workspace/extension-layout".to_owned(),
            PlannedMapping {
                resource_kind: "workspace_setting",
                logical_key: "workspace/extension-layout".to_owned(),
                target_id: workspace.workspace_id,
                target_code: "extension_layout".to_owned(),
                target_version: None,
                mapping_kind: "workspace",
                snapshot: serde_json::json!({"setting": "extension_layout"}),
            },
        );
    }

    // Includes constrain apply order. A published target is
    // also required before ordinary validation can resolve a relationship table
    // path. Other relationship references may legitimately be cyclic.
    let mut ordering_dependencies = BTreeMap::new();
    for resource in &manifest.resources.blueprints {
        let blueprint = pack
            .blueprint(&resource.key)
            .expect("validated blueprint exists");
        let mut resource_dependencies = blueprint
            .includes()
            .iter()
            .map(|include| include.key().to_owned())
            .collect::<BTreeSet<_>>();
        if publication == BlueprintPublication::Publish {
            resource_dependencies.extend(blueprint.table_path_dependencies().iter().cloned());
        }
        ordering_dependencies.insert(resource.key.clone(), resource_dependencies);
    }

    let mut generated_code_counts = HashMap::<&str, usize>::new();
    for (logical_key, (_, resource)) in &resources {
        if resource.required {
            *generated_code_counts
                .entry(&mappings_by_key[logical_key].target_code)
                .or_default() += 1;
        }
    }
    let mut blueprint_layout_allowed = HashMap::<String, HashSet<String>>::new();
    let mut blueprint_layout_evidence = HashMap::<String, Vec<Value>>::new();
    let mut blueprint_layout_blocked = HashMap::<String, &'static str>::new();
    for resource in &manifest.resources.blueprints {
        let blueprint = pack
            .blueprint(&resource.key)
            .expect("validated blueprint exists");
        let mut allowed = HashSet::new();
        let mut evidence = Vec::new();
        for entry in &blueprint.extension_layout {
            let requirement = extension_requirement_for_contribution(manifest, &entry.contribution);
            let reason = contribution_availability_reason(
                requirement,
                &entry.contribution,
                &entry.outlet,
                workspace.installed_extensions.get(&requirement.id),
            );
            let outcome = match reason {
                None => {
                    allowed.insert(entry.contribution.clone());
                    "satisfied"
                }
                Some(_) if requirement.required => {
                    blueprint_layout_blocked
                        .entry(resource.key.clone())
                        .or_insert("extension_contribution_unavailable");
                    "blocked"
                }
                Some(_) => "skip",
            };
            evidence.push(serde_json::json!({
                "contribution": entry.contribution,
                "outlet": entry.outlet,
                "required": requirement.required,
                "outcome": outcome,
                "reason_code": reason.unwrap_or("satisfied"),
            }));
        }
        blueprint_layout_allowed.insert(resource.key.clone(), allowed);
        blueprint_layout_evidence.insert(resource.key.clone(), evidence);
    }

    let mut outcomes = HashMap::<String, (&'static str, &'static str)>::new();
    for (logical_key, (_, resource)) in &resources {
        let mapping = &mappings_by_key[logical_key];
        let explicitly_mapped = mapping.mapping_kind == "existing";
        let mapping_compatible = if explicitly_mapped {
            let existing = &workspace.existing_blueprints[logical_key];
            let normalized = normalized_blueprint_payload(
                pack.blueprint(logical_key)
                    .expect("validated blueprint exists"),
                &mappings_by_key,
                publication,
                &blueprint_layout_allowed[logical_key],
                workspace,
                manifest,
            )?;
            let definition = normalized["definition"]
                .as_str()
                .expect("normalized blueprint definition is a string");
            existing.canonical_definition_hash == catalog_blueprint::raw_hash(definition)
                && existing.kind
                    == blueprint_kind_name(
                        pack.blueprint(logical_key)
                            .expect("validated blueprint exists")
                            .kind(),
                    )
        } else {
            false
        };
        outcomes.insert(
            logical_key.clone(),
            if !resource.required && !explicitly_mapped {
                ("skip", "optional_not_selected")
            } else if let Some(reason) = blueprint_layout_blocked.get(logical_key) {
                ("blocked", *reason)
            } else if explicitly_mapped && !mapping_compatible {
                ("conflict", "existing_blueprint_incompatible")
            } else if explicitly_mapped {
                ("map", "exact_blueprint_match")
            } else if generated_code_counts[mapping.target_code.as_str()] > 1 {
                ("conflict", "duplicate_target_code")
            } else if workspace.physical_codes.contains(&mapping.target_code) {
                ("conflict", "target_code_exists")
            } else if publication == BlueprintPublication::Draft
                && pack
                    .blueprint(logical_key)
                    .is_some_and(|blueprint| !blueprint.table_path_dependencies().is_empty())
            {
                ("blocked", "draft_table_path_target_unpublished")
            } else {
                ("create", "target_absent")
            },
        );
    }
    loop {
        let newly_blocked = outcomes
            .iter()
            .filter(|(_, (action, _))| matches!(*action, "create" | "map"))
            .filter(|(key, _)| {
                dependencies[*key]
                    .iter()
                    .any(|dependency| !matches!(outcomes[dependency].0, "create" | "map"))
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        if newly_blocked.is_empty() {
            break;
        }
        for key in newly_blocked {
            outcomes.insert(key, ("blocked", "dependency_not_creatable"));
        }
    }

    // Existing blueprints already exist, so their outbound publication-time
    // dependencies impose no creation order. Keep them as dependency targets
    // for resources that will be created, but clear their own outbound edges.
    for (key, dependencies) in &mut ordering_dependencies {
        if outcomes[key].0 != "create" {
            dependencies.clear();
        }
    }
    let ordered_keys = topological_resource_order(&ordering_dependencies)?;
    let mut actions = Vec::with_capacity(ordered_keys.len());
    for logical_key in ordered_keys {
        let (kind, resource) = resources
            .get(&logical_key)
            .expect("dependency graph contains declared resources");
        let mapping = &mappings_by_key[&logical_key];
        let (action, reason_code) = outcomes[&logical_key];
        let normalized_payload = if action == "create" {
            Some(normalized_blueprint_payload(
                pack.blueprint(&logical_key)
                    .expect("validated blueprint exists"),
                &mappings_by_key,
                publication,
                &blueprint_layout_allowed[&logical_key],
                workspace,
                manifest,
            )?)
        } else {
            None
        };
        let preconditions = if action == "map" {
            let existing = &workspace.existing_blueprints[&logical_key];
            serde_json::json!([{
                "kind": "existing_blueprint",
                "id": existing.id,
                "code": existing.code,
                "version": existing.version,
                "definition_hash": existing.definition_hash,
                "canonical_definition_hash": existing.canonical_definition_hash,
                "blueprint_kind": existing.kind,
                "status": "published",
                "deleted": false
            }])
        } else if matches!(action, "create" | "conflict") && mapping.mapping_kind == "create" {
            serde_json::json!([{"kind": "target_absent", "resource_kind": kind, "code": mapping.target_code}])
        } else {
            serde_json::json!([])
        };
        actions.push(PlannedAction {
            resource_kind: kind,
            logical_key: logical_key.clone(),
            action,
            reason_code,
            summary: serde_json::json!({
                "target_code": mapping.target_code,
                "target_version": mapping.target_version,
                "required": resource.required,
                "dependencies": dependencies[&logical_key],
                "extension_layout": blueprint_layout_evidence.get(&logical_key).cloned().unwrap_or_default(),
            }),
            normalized_payload,
            preconditions,
        });
    }

    if let Some(navigation) = pack.explore_navigation() {
        let resource = manifest
            .resources
            .workspace_settings
            .iter()
            .find(|resource| resource.key == "workspace/explore-navigation")
            .expect("validated navigation has a manifest resource");
        if !workspace.explore_navigation_valid {
            actions.push(PlannedAction {
                resource_kind: "workspace_setting",
                logical_key: resource.key.clone(),
                action: "conflict",
                reason_code: "invalid_current_navigation",
                summary: serde_json::json!({
                    "setting": "explore_navigation",
                    "required": resource.required,
                    "entries": [],
                }),
                normalized_payload: None,
                preconditions: serde_json::json!([]),
            });
            let extension_requirements = manifest
                .extensions
                .iter()
                .map(|requirement| {
                    evaluate_extension_requirement(
                        requirement,
                        pack.configuration_template(&requirement.key),
                        workspace.installed_extensions.get(&requirement.id),
                    )
                })
                .collect::<Vec<_>>();
            return Ok(SolutionPackPlanDraft {
                ready: false,
                mappings: mappings_by_key.into_values().collect(),
                actions,
                extension_requirements,
            });
        }

        let mut evidence = Vec::with_capacity(navigation.entries.len());
        let mut payload_entries = Vec::with_capacity(navigation.entries.len());
        let mut unmet_reason = None;
        let mut visibility_conflict = false;
        let current_by_code = workspace
            .explore_navigation
            .iter()
            .map(|entry| {
                let mut roles = entry.visible_to_role_codes.clone();
                roles.sort();
                (entry.blueprint_code.as_str(), roles)
            })
            .collect::<HashMap<_, _>>();
        for entry in &navigation.entries {
            let mapping = &mappings_by_key[&entry.blueprint];
            let roles_available = entry
                .visible_to_role_codes
                .iter()
                .all(|role| workspace.role_codes.contains(role));
            let blueprint_available = outcomes.get(&entry.blueprint).is_some_and(|(action, _)| {
                *action == "map"
                    || (*action == "create" && publication == BlueprintPublication::Publish)
            });
            let (outcome, reason) = if !roles_available {
                unmet_reason.get_or_insert("unknown_role_code");
                ("unmet", "unknown_role_code")
            } else if let Some(current_roles) = current_by_code.get(mapping.target_code.as_str()) {
                if *current_roles != entry.visible_to_role_codes {
                    visibility_conflict = true;
                    ("conflict", "visibility_mismatch")
                } else if workspace
                    .published_entity_codes
                    .contains(&mapping.target_code)
                {
                    ("satisfied", "exact_match")
                } else {
                    unmet_reason.get_or_insert("blueprint_not_published");
                    ("unmet", "blueprint_not_published")
                }
            } else if !blueprint_available {
                let reason = if publication != BlueprintPublication::Publish {
                    "blueprint_not_published"
                } else {
                    "blueprint_not_creatable"
                };
                unmet_reason.get_or_insert(reason);
                ("unmet", reason)
            } else {
                ("append", "target_absent")
            };
            let evidence_outcome = if !resource.required && matches!(outcome, "unmet" | "conflict")
            {
                "skip"
            } else {
                outcome
            };
            evidence.push(serde_json::json!({
                "blueprint": entry.blueprint,
                "blueprint_code": mapping.target_code,
                "visible_to_role_codes": entry.visible_to_role_codes,
                "outcome": evidence_outcome,
                "reason_code": reason,
            }));
            if matches!(outcome, "append" | "satisfied") {
                payload_entries.push(serde_json::json!({
                    "blueprint_key": entry.blueprint,
                    "blueprint_code": mapping.target_code,
                    "visible_to_role_codes": entry.visible_to_role_codes,
                }));
            }
        }
        let has_append = evidence.iter().any(|entry| entry["outcome"] == "append");
        let (action, reason_code, normalized_payload) = if resource.required && visibility_conflict
        {
            ("conflict", "visibility_mismatch", None)
        } else if resource.required
            && let Some(reason) = unmet_reason
        {
            ("blocked", reason, None)
        } else if payload_entries.is_empty() {
            ("skip", unmet_reason.unwrap_or("visibility_mismatch"), None)
        } else if has_append {
            (
                "append",
                "target_absent",
                Some(serde_json::json!({"entries": payload_entries})),
            )
        } else {
            (
                "satisfied",
                "exact_match",
                Some(serde_json::json!({"entries": payload_entries})),
            )
        };
        actions.push(PlannedAction {
            resource_kind: "workspace_setting",
            logical_key: resource.key.clone(),
            action,
            reason_code,
            summary: serde_json::json!({
                "setting": "explore_navigation",
                "required": resource.required,
                "entries": evidence,
            }),
            normalized_payload,
            preconditions: serde_json::json!([]),
        });
    }

    if let Some(layout) = pack.extension_layout() {
        let resource = manifest
            .resources
            .workspace_settings
            .iter()
            .find(|resource| resource.key == "workspace/extension-layout")
            .expect("validated extension layout has a manifest resource");
        if !workspace.extension_layout_valid {
            actions.push(PlannedAction {
                resource_kind: "workspace_setting",
                logical_key: resource.key.clone(),
                action: "conflict",
                reason_code: "invalid_current_extension_layout",
                summary: serde_json::json!({
                    "setting": "extension_layout",
                    "required": resource.required,
                    "entries": [],
                }),
                normalized_payload: None,
                preconditions: serde_json::json!([]),
            });
        } else {
            let mut evidence = Vec::with_capacity(layout.entries.len());
            let mut payload_entries = Vec::new();
            let mut required_unmet = None;
            let mut required_conflict = false;
            let mut has_append = false;
            for entry in &layout.entries {
                let requirement =
                    extension_requirement_for_contribution(manifest, &entry.contribution);
                let installed = workspace.installed_extensions.get(&requirement.id);
                let availability = contribution_availability_reason(
                    requirement,
                    &entry.contribution,
                    &entry.outlet,
                    installed,
                );
                let placement = availability.is_none().then(|| {
                    classify_extension_layout_placement(
                        &workspace.extension_layout,
                        &entry.contribution,
                        &entry.outlet,
                        entry.hidden,
                        entry.promoted,
                    )
                });
                let (outcome, reason) = if let Some(reason) = availability {
                    if entry.required {
                        required_unmet.get_or_insert(reason);
                        ("blocked", reason)
                    } else {
                        ("skip", reason)
                    }
                } else {
                    match placement.expect("available contribution has placement") {
                        ExtensionLayoutPlacement::Exact => ("satisfied", "exact_match"),
                        ExtensionLayoutPlacement::Absent => {
                            has_append = true;
                            ("append", "target_absent")
                        }
                        ExtensionLayoutPlacement::Conflict if entry.required => {
                            required_conflict = true;
                            ("conflict", "placement_mismatch")
                        }
                        ExtensionLayoutPlacement::Conflict => ("skip", "placement_mismatch"),
                    }
                };
                evidence.push(serde_json::json!({
                    "contribution": entry.contribution,
                    "outlet": entry.outlet,
                    "hidden": entry.hidden,
                    "promoted": entry.promoted,
                    "required": entry.required,
                    "outcome": outcome,
                    "reason_code": reason,
                }));
                if matches!(outcome, "append" | "satisfied") {
                    let installed = installed.expect("available contribution is installed");
                    payload_entries.push(serde_json::json!({
                        "contribution": entry.contribution,
                        "outlet": entry.outlet,
                        "hidden": entry.hidden,
                        "promoted": entry.promoted,
                        "installed_release_id": installed.installed_release_id,
                        "installed_version": installed.version,
                    }));
                }
            }
            let (action, reason_code, normalized_payload) = if required_conflict {
                ("conflict", "placement_mismatch", None)
            } else if let Some(reason) = required_unmet {
                ("blocked", reason, None)
            } else if payload_entries.is_empty() {
                ("skip", "optional_contributions_unmet", None)
            } else if has_append {
                (
                    "append",
                    "target_absent",
                    Some(serde_json::json!({"entries": payload_entries})),
                )
            } else {
                (
                    "satisfied",
                    "exact_match",
                    Some(serde_json::json!({"entries": payload_entries})),
                )
            };
            actions.push(PlannedAction {
                resource_kind: "workspace_setting",
                logical_key: resource.key.clone(),
                action,
                reason_code,
                summary: serde_json::json!({
                    "setting": "extension_layout",
                    "required": resource.required,
                    "entries": evidence,
                }),
                normalized_payload,
                preconditions: serde_json::json!([]),
            });
        }
    }

    let extension_requirements = manifest
        .extensions
        .iter()
        .map(|requirement| {
            evaluate_extension_requirement(
                requirement,
                pack.configuration_template(&requirement.key),
                workspace.installed_extensions.get(&requirement.id),
            )
        })
        .collect::<Vec<_>>();
    let ready = actions.iter().all(|action| {
        matches!(
            action.action,
            "create" | "map" | "append" | "satisfied" | "skip"
        )
    }) && extension_requirements
        .iter()
        .all(|requirement| requirement.status != "blocked");

    Ok(SolutionPackPlanDraft {
        ready,
        mappings: mappings_by_key.into_values().collect(),
        actions,
        extension_requirements,
    })
}

fn blueprint_kind_name(kind: BlueprintKind) -> &'static str {
    match kind {
        BlueprintKind::Entity => "entity",
        BlueprintKind::Mixin => "mixin",
    }
}

fn deterministic_target_id(
    workspace_id: uuid::Uuid,
    pack: &ValidatedSolutionPack,
    prefix: &str,
    publication: BlueprintPublication,
    logical_key: &str,
) -> uuid::Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"attricat.solution-pack.target-id.v1\0");
    hasher.update(workspace_id.as_bytes());
    for value in [
        pack.archive_sha256(),
        prefix,
        publication.as_str(),
        logical_key,
    ] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub fn validate_plan_prefix(prefix: &str) -> Result<(), SolutionPackError> {
    if prefix.is_empty()
        || prefix.len() > MAX_SOLUTION_PACK_PREFIX_BYTES
        || !prefix
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || prefix.ends_with('_')
    {
        return invalid(format!(
            "prefix must be 1-{MAX_SOLUTION_PACK_PREFIX_BYTES} lowercase ASCII letters, digits, or underscores, start with a letter, and not end with an underscore"
        ));
    }
    Ok(())
}

fn topological_resource_order(
    dependencies: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, SolutionPackError> {
    fn visit(
        key: &str,
        dependencies: &BTreeMap<String, BTreeSet<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        ordered: &mut Vec<String>,
    ) -> Result<(), SolutionPackError> {
        if visited.contains(key) {
            return Ok(());
        }
        if !visiting.insert(key.to_owned()) {
            return invalid(format!("resource dependencies contain a cycle at '{key}'"));
        }
        for dependency in &dependencies[key] {
            visit(dependency, dependencies, visiting, visited, ordered)?;
        }
        visiting.remove(key);
        visited.insert(key.to_owned());
        ordered.push(key.to_owned());
        Ok(())
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::with_capacity(dependencies.len());
    for key in dependencies.keys() {
        visit(key, dependencies, &mut visiting, &mut visited, &mut ordered)?;
    }
    Ok(ordered)
}

fn normalized_blueprint_payload(
    blueprint: &SolutionPackBlueprint,
    mappings: &BTreeMap<String, PlannedMapping>,
    publication: BlueprintPublication,
    allowed_contributions: &HashSet<String>,
    workspace: &PlanningWorkspaceSnapshot,
    manifest: &SolutionPackManifest,
) -> Result<Value, SolutionPackError> {
    let mut value: toml::Value = toml::from_str(blueprint.source()).map_err(|_| {
        SolutionPackError::Invalid(format!("blueprint '{}' is invalid", blueprint.key()))
    })?;
    let table = value
        .as_table_mut()
        .expect("validated blueprint is a table");
    table.insert(
        "code".to_owned(),
        toml::Value::String(mappings[blueprint.key()].target_code.clone()),
    );
    if let Some(includes) = table
        .get_mut("includes")
        .and_then(toml::Value::as_array_mut)
    {
        for include in includes {
            let include = include
                .as_table_mut()
                .expect("validated include is a table");
            let key = include
                .remove("key")
                .and_then(|value| value.as_str().map(str::to_owned))
                .expect("validated include has a key");
            include.insert(
                "code".to_owned(),
                toml::Value::String(mappings[&key].target_code.clone()),
            );
            include.insert(
                "version".to_owned(),
                toml::Value::Integer(
                    mappings[&key]
                        .target_version
                        .expect("blueprint mappings have a revision"),
                ),
            );
        }
    }
    if let Some(attributes) = table
        .get_mut("attributes")
        .and_then(toml::Value::as_array_mut)
    {
        for attribute in attributes {
            if let Some(attribute) = attribute.as_table_mut() {
                normalize_reference(attribute, "target_blueprint", mappings);
            }
        }
    }
    if let Some(views) = table.get_mut("views").and_then(toml::Value::as_table_mut) {
        for (name, view) in views.iter_mut() {
            if let Some(view) = view.as_table_mut() {
                normalize_view_references(view, mappings);
                if name == "extension_layout"
                    && let Some(outlets) =
                        view.get_mut("outlets").and_then(toml::Value::as_table_mut)
                {
                    for (_, outlet) in outlets.iter_mut() {
                        if let Some(outlet) = outlet.as_table_mut() {
                            for list in ["order", "hidden"] {
                                if let Some(entries) =
                                    outlet.get_mut(list).and_then(toml::Value::as_array_mut)
                                {
                                    entries.retain(|entry| {
                                        entry.as_str().is_some_and(|entry| {
                                            allowed_contributions.contains(entry)
                                        })
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let extension_contributions = blueprint
        .extension_layout
        .iter()
        .filter(|entry| allowed_contributions.contains(&entry.contribution))
        .map(|entry| {
            let requirement = extension_requirement_for_contribution(manifest, &entry.contribution);
            let installed = workspace
                .installed_extensions
                .get(&requirement.id)
                .expect("allowed contribution is installed");
            serde_json::json!({
                "contribution": entry.contribution,
                "outlet": entry.outlet,
                "installed_release_id": installed.installed_release_id,
                "installed_version": installed.version,
            })
        })
        .collect::<Vec<_>>();
    let definition = toml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!("blueprint '{}' is invalid", blueprint.key()))
    })?;
    Ok(serde_json::json!({
        "definition": definition,
        "version": 1,
        "publication": publication.as_str(),
        "extension_contributions": extension_contributions,
    }))
}

fn normalize_reference(
    table: &mut toml::map::Map<String, toml::Value>,
    field: &str,
    mappings: &BTreeMap<String, PlannedMapping>,
) {
    if let Some(reference) = table.get_mut(field) {
        let key = reference
            .as_str()
            .expect("validated reference is a string")
            .to_owned();
        *reference = toml::Value::String(mappings[&key].target_code.clone());
    }
}

fn normalize_view_references(
    node: &mut toml::map::Map<String, toml::Value>,
    mappings: &BTreeMap<String, PlannedMapping>,
) {
    if node.get("type").and_then(toml::Value::as_str) == Some("incoming_relationship_list")
        && let Some(relationships) = node
            .get_mut("relationships")
            .and_then(toml::Value::as_array_mut)
    {
        for relationship in relationships {
            if let Some(relationship) = relationship.as_table_mut() {
                normalize_reference(relationship, "source_blueprint", mappings);
            }
        }
    }
    for collection in ["children", "tabs", "sections"] {
        if let Some(children) = node.get_mut(collection).and_then(toml::Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_table_mut() {
                    normalize_view_references(child, mappings);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use tar::{Builder, EntryType, Header};

    use super::*;

    const PRODUCT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "blueprints/category"
"#;
    const CATEGORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[views.detail]
type = "stack"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Relationships"

[[views.detail.children.tabs.children]]
type = "grid"

[[views.detail.children.tabs.children.children]]
type = "incoming_relationship_list"
label = "Products"
relationships = [{ source_blueprint = "blueprints/product", field = "categories" }]
page_size = 10

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "products"
value_type = "relationship"
target_blueprint = "blueprints/product"
"#;
    const EXPLORE_NAVIGATION: &[u8] = br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","visible_to_role_codes":["viewer","editor"]}]}"#;

    fn digest(bytes: &[u8]) -> String {
        sha256_hex(bytes)
    }

    fn resource(key: &str, path: &str, bytes: &[u8]) -> Value {
        json!({"key":key,"path":path,"required":true,"sha256":digest(bytes)})
    }

    fn manifest_value() -> Value {
        json!({
            "manifest_version": 1,
            "id": "attricat.ecommerce",
            "name": "Ecommerce",
            "version": "1.2.0",
            "description": "Starter catalog",
            "catalog": {"host_api": ">=1.0.0, <2.0.0"},
            "resources": {
                "blueprints": [
                    resource("blueprints/product", "blueprints/product.toml", PRODUCT_BLUEPRINT),
                    resource("blueprints/category", "blueprints/category.toml", CATEGORY_BLUEPRINT)
                ]
            }
        })
    }

    fn valid_files() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("blueprints/product.toml", PRODUCT_BLUEPRINT),
            ("blueprints/category.toml", CATEGORY_BLUEPRINT),
        ]
    }

    fn archive_with_explore_navigation(required: bool) -> Vec<u8> {
        let mut manifest = manifest_value();
        manifest["resources"]["workspace_settings"] = json!([{
            "key": "workspace/explore-navigation",
            "path": "workspace/explore-navigation.json",
            "required": required,
            "sha256": digest(EXPLORE_NAVIGATION),
        }]);
        let mut files = valid_files();
        files.push(("workspace/explore-navigation.json", EXPLORE_NAVIGATION));
        archive(&manifest, &files)
    }

    fn archive_with_extension_layout(layout: &[u8], requirement_required: bool) -> Vec<u8> {
        let mut manifest = manifest_value();
        manifest["resources"]["workspace_settings"] = json!([{
            "key": "workspace/extension-layout",
            "path": "workspace/extension-layout.json",
            "required": true,
            "sha256": digest(layout),
        }]);
        manifest["extensions"] = json!([{
            "key": "extensions/shop",
            "id": "acme.shop",
            "version": "^1.0",
            "required": requirement_required,
        }]);
        let mut files = valid_files();
        files.push(("workspace/extension-layout.json", layout));
        archive(&manifest, &files)
    }

    fn archive_with_configuration_template(template: &[u8]) -> Vec<u8> {
        let mut manifest = manifest_value();
        manifest["extensions"] = json!([{
            "key": "extensions/shopify",
            "id": "acme.shopify",
            "version": ">=2.1.0 <3.0.0",
            "required": true,
            "configuration_template": {
                "path": "extensions/shopify.json",
                "sha256": digest(template)
            }
        }]);
        let mut files = valid_files();
        files.push(("extensions/shopify.json", template));
        archive(&manifest, &files)
    }

    fn archive(manifest: &Value, files: &[(&str, &[u8])]) -> Vec<u8> {
        let manifest = serde_json::to_vec(manifest).unwrap();
        let mut tar_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_bytes);
            append_file(&mut builder, SOLUTION_PACK_MANIFEST_PATH, &manifest);
            for (path, bytes) in files {
                append_file(&mut builder, path, bytes);
            }
            builder.finish().unwrap();
        }
        zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
    }

    fn custom_archive(entries: &[(&[u8], EntryType, &[u8])]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_bytes);
            for (path, kind, bytes) in entries {
                let mut header = Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o644);
                header.set_entry_type(*kind);
                header.as_mut_bytes()[..path.len()].copy_from_slice(path);
                header.set_cksum();
                builder.append(&header, *bytes).unwrap();
            }
            builder.finish().unwrap();
        }
        zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
    }

    fn append_file(builder: &mut Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
        let mut header = Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, bytes).unwrap();
    }

    fn assert_invalid(archive: &[u8], expected: &str) {
        let error = ValidatedSolutionPack::from_tar_zst(archive).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected '{expected}' in '{error}'"
        );
    }

    #[test]
    fn validates_a_complete_archive_and_exposes_validated_content() {
        let archive_bytes = archive(&manifest_value(), &valid_files());
        let pack = ValidatedSolutionPack::from_tar_zst(&archive_bytes).unwrap();

        assert_eq!(pack.manifest().id, "attricat.ecommerce");
        assert_eq!(pack.manifest().version, "1.2.0");
        assert_eq!(pack.archive_sha256(), digest(&archive_bytes));
        assert_eq!(pack.files().count(), 2);
        assert_eq!(
            pack.blueprint("blueprints/product").unwrap().code(),
            "product"
        );

        let mut illustrative_range = manifest_value();
        illustrative_range["catalog"]["host_api"] = json!(">=1.0.0 <2.0.0");
        ValidatedSolutionPack::from_tar_zst(&archive(&illustrative_range, &valid_files())).unwrap();
    }

    #[test]
    fn validates_and_normalizes_public_guidance_and_informational_checks() {
        let readme = b"# Setup\r\n\r\nSee [details](#details).";
        let checklist = br#"{"format_version":1,"items":[{"key":"checklist/publish","title":"Publish product","markdown":"Publish the product.","check":"checks/product-published"}]}"#;
        let checks = br#"{"format_version":1,"checks":[{"key":"checks/product-published","title":"Product is published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}]}"#;
        let mut manifest = manifest_value();
        manifest["documentation"] = json!({
            "readme":{"path":"README.md","sha256":digest(readme)},
            "setup_checklist":{"path":"setup/checklist.json","sha256":digest(checklist)}
        });
        manifest["checks"] = json!({"path":"checks/checks.json","sha256":digest(checks)});
        let mut files = valid_files();
        files.extend([
            ("README.md", readme.as_slice()),
            ("setup/checklist.json", checklist.as_slice()),
            ("checks/checks.json", checks.as_slice()),
        ]);
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        assert_eq!(
            pack.guidance().readme_markdown.as_deref(),
            Some("# Setup\n\nSee [details](#details).")
        );
        assert_eq!(
            pack.guidance()
                .setup_checklist
                .as_ref()
                .unwrap()
                .items
                .len(),
            1
        );
        assert_eq!(
            pack.checks()[0].predicate.predicate_type(),
            "blueprint_published"
        );

        let unsafe_readme = b"![remote](https://example.test/image.png)";
        manifest["documentation"]["readme"]["sha256"] = json!(digest(unsafe_readme));
        files[2] = ("README.md", unsafe_readme);
        assert_invalid(&archive(&manifest, &files), "unsafe Markdown");
    }

    #[test]
    fn guidance_text_bounds_count_utf8_bytes() {
        assert!(validate_bounded_text(&"é".repeat(100), "title", 200).is_ok());
        assert!(validate_bounded_text(&"é".repeat(101), "title", 200).is_err());
        assert!(validate_markdown(&"é".repeat(2048), 4096, "markdown").is_ok());
        assert!(validate_markdown(&"é".repeat(2049), 4096, "markdown").is_err());
    }

    #[test]
    fn check_predicates_are_a_closed_host_defined_catalogue() {
        for (value, expected) in [
            (
                json!({"type":"blueprint_published","blueprint":"blueprints/product"}),
                "blueprint_published",
            ),
            (
                json!({"type":"extension_installed","extension":"extensions/shop"}),
                "extension_installed",
            ),
            (
                json!({"type":"extension_enabled","extension":"extensions/shop"}),
                "extension_enabled",
            ),
            (
                json!({"type":"extension_configuration_matches","extension":"extensions/shop"}),
                "extension_configuration_matches",
            ),
            (
                json!({"type":"explore_navigation_entry_present","blueprint":"blueprints/product"}),
                "explore_navigation_entry_present",
            ),
            (
                json!({"type":"workspace_extension_layout_placement_present","contribution":"acme.shop:nav"}),
                "workspace_extension_layout_placement_present",
            ),
        ] {
            let predicate: SolutionPackCheckPredicate = serde_json::from_value(value).unwrap();
            assert_eq!(predicate.predicate_type(), expected);
        }
        assert!(
            serde_json::from_value::<SolutionPackCheckPredicate>(
                json!({"type":"query","sql":"select 1"})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<SolutionPackCheckPredicate>(json!({"type":"blueprint_published","blueprint":"blueprints/product","required":true})).is_err());
    }

    #[test]
    fn rejects_unknown_or_unresolvable_guidance_and_check_fields() {
        let checklist = br#"{"format_version":1,"items":[{"key":"checklist/setup","title":"Setup","markdown":"Do setup.","check":"checks/missing"}]}"#;
        let checks = br#"{"format_version":1,"checks":[{"key":"checks/missing","title":"Missing","predicate":{"type":"blueprint_published","blueprint":"blueprints/missing"}}]}"#;
        let mut manifest = manifest_value();
        manifest["documentation"] =
            json!({"setup_checklist":{"path":"setup.json","sha256":digest(checklist)}});
        manifest["checks"] = json!({"path":"checks.json","sha256":digest(checks)});
        let mut files = valid_files();
        files.extend([
            ("setup.json", checklist.as_slice()),
            ("checks.json", checks.as_slice()),
        ]);
        assert_invalid(&archive(&manifest, &files), "undeclared blueprint");

        let context_reference = br#"{"format_version":1,"checks":[{"key":"checks/context","title":"Context","predicate":{"type":"blueprint_published","blueprint":"contexts/web"}}]}"#;
        manifest["checks"]["sha256"] = json!(digest(context_reference));
        files[3] = ("checks.json", context_reference);
        assert_invalid(
            &archive(&manifest, &files),
            "undeclared blueprint 'contexts/web'",
        );

        let unknown = br#"{"format_version":1,"checks":[],"query":"select *"}"#;
        manifest["checks"]["sha256"] = json!(digest(unknown));
        files[3] = ("checks.json", unknown);
        assert_invalid(&archive(&manifest, &files), "not valid strict JSON");
    }

    #[test]
    fn validates_strict_explore_navigation_contract() {
        let pack =
            ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(true)).unwrap();
        let navigation = pack.explore_navigation().unwrap();
        assert_eq!(navigation.entries.len(), 1);
        assert_eq!(
            navigation.entries[0].visible_to_role_codes,
            ["editor", "viewer"]
        );

        for (invalid, expected) in [
            (
                br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","visible_to_role_codes":["editor","editor"]}]}"#.as_slice(),
                "duplicate role codes",
            ),
            (
                br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","unknown":true}]}"#.as_slice(),
                "not valid strict JSON",
            ),
        ] {
            let mut manifest = manifest_value();
            manifest["resources"]["workspace_settings"] = json!([resource(
                "workspace/explore-navigation",
                "workspace/explore-navigation.json",
                invalid,
            )]);
            let mut files = valid_files();
            files.push(("workspace/explore-navigation.json", invalid));
            assert_invalid(&archive(&manifest, &files), expected);
        }

        for (navigation, expected) in [
            (
                json!({
                    "format_version": 1,
                    "kind": "explore_navigation",
                    "entries": (0..=MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES)
                        .map(|index| json!({"blueprint": format!("blueprints/product_{index}")}))
                        .collect::<Vec<_>>(),
                }),
                "must contain 1-64 entries",
            ),
            (
                json!({
                    "format_version": 1,
                    "kind": "explore_navigation",
                    "entries": [{
                        "blueprint": "blueprints/product",
                        "visible_to_role_codes": (0..=MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ROLES)
                            .map(|index| format!("role_{index}"))
                            .collect::<Vec<_>>(),
                    }],
                }),
                "too many role codes",
            ),
        ] {
            let navigation = serde_json::to_vec(&navigation).unwrap();
            let mut manifest = manifest_value();
            manifest["resources"]["workspace_settings"] = json!([resource(
                "workspace/explore-navigation",
                "workspace/explore-navigation.json",
                &navigation,
            )]);
            let mut files: Vec<(&str, &[u8])> = valid_files();
            files.push(("workspace/explore-navigation.json", navigation.as_slice()));
            assert_invalid(&archive(&manifest, &files), expected);
        }
    }

    #[test]
    fn validates_and_plans_extension_layout_item_level_merge() {
        let layout = br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","promoted":true,"required":true}]}"#;
        let pack =
            ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(layout, true))
                .unwrap();
        assert_eq!(pack.extension_layout().unwrap().entries.len(), 1);

        let installed = InstalledExtensionSnapshot {
            installed_release_id: uuid::Uuid::from_u128(7),
            version: "1.2.0".into(),
            state: "disabled".into(),
            configuration: json!({}),
            policy_compatible: true,
            contributions: BTreeMap::from([("acme.shop:nav".to_owned(), "navigation".to_owned())]),
        };
        let workspace = |extension_layout| PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            installed_extensions: BTreeMap::from([("acme.shop".to_owned(), installed.clone())]),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout,
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
        };
        let action_for = |current| {
            build_solution_pack_plan(
                &pack,
                "shop",
                BlueprintPublication::Draft,
                &workspace(current),
            )
            .unwrap()
            .actions
            .into_iter()
            .find(|action| action.logical_key == "workspace/extension-layout")
            .unwrap()
        };
        assert_eq!(
            action_for(json!({"version":1,"outlets":{}})).action,
            "append"
        );
        assert_eq!(
            action_for(json!({"version":1,"outlets":{"navigation":{"order":["acme.shop:nav"],"hidden":[],"promoted":["acme.shop:nav"]}}})).action,
            "satisfied"
        );
        assert_eq!(
            action_for(json!({"version":1,"outlets":{"navigation":{"order":[],"hidden":["acme.shop:nav"],"promoted":[]}}})).action,
            "conflict"
        );
        assert_eq!(
            action_for(json!({"version":1,"outlets":{"navigation":{"order":[],"hidden":[],"promoted":["acme.shop:nav"]}}})).action,
            "conflict"
        );
        assert_eq!(
            action_for(json!({"version":1,"outlets":{
                "navigation":{"order":[],"hidden":[],"promoted":["acme.shop:nav"]},
                "entity_action":{"order":["acme.shop:nav"],"hidden":[]}
            }}))
            .action,
            "conflict"
        );

        let unavailable_action = |installed: Option<InstalledExtensionSnapshot>| {
            let plan = build_solution_pack_plan(
                &pack,
                "shop",
                BlueprintPublication::Draft,
                &PlanningWorkspaceSnapshot {
                    workspace_id: uuid::Uuid::nil(),
                    physical_codes: BTreeSet::from(["default".to_owned()]),
                    existing_blueprints: BTreeMap::new(),
                    installed_extensions: installed
                        .map(|installed| BTreeMap::from([("acme.shop".to_owned(), installed)]))
                        .unwrap_or_default(),
                    explore_navigation: Vec::new(),
                    explore_navigation_valid: true,
                    extension_layout: json!({"version":1,"outlets":{}}),
                    extension_layout_valid: true,
                    role_codes: BTreeSet::new(),
                    published_entity_codes: BTreeSet::new(),
                },
            )
            .unwrap();
            let action = plan
                .actions
                .iter()
                .find(|action| action.logical_key == "workspace/extension-layout")
                .unwrap();
            (action.action, action.reason_code)
        };
        let mut missing_contribution = installed.clone();
        missing_contribution.contributions.clear();
        let mut wrong_outlet = installed.clone();
        wrong_outlet
            .contributions
            .insert("acme.shop:nav".to_owned(), "entity_action".to_owned());
        let mut quarantined = installed.clone();
        quarantined.state = "quarantined".to_owned();
        let mut policy_incompatible = installed.clone();
        policy_incompatible.policy_compatible = false;
        let mut incompatible_version = installed.clone();
        incompatible_version.version = "2.0.0".to_owned();
        for (candidate, reason) in [
            (None, "missing"),
            (Some(missing_contribution), "contribution_missing"),
            (Some(wrong_outlet), "outlet_mismatch"),
            (Some(quarantined), "quarantined"),
            (Some(policy_incompatible), "policy_incompatible"),
            (Some(incompatible_version), "incompatible_version"),
        ] {
            assert_eq!(unavailable_action(candidate), ("blocked", reason));
        }

        let optional_layout = br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":false}]}"#;
        let optional_pack = ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(
            optional_layout,
            false,
        ))
        .unwrap();
        let optional_plan = build_solution_pack_plan(
            &optional_pack,
            "shop",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        assert_eq!(
            optional_plan
                .actions
                .iter()
                .find(|action| action.logical_key == "workspace/extension-layout")
                .unwrap()
                .action,
            "skip"
        );
    }

    #[test]
    fn blueprint_layout_skips_optional_unavailable_contributions_and_blocks_required() {
        let blueprint = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                br#"
[views.extension_layout]
type = "extension_layout"
version = 1
[views.extension_layout.outlets.entity_action]
order = ["acme.shop:action"]
hidden = []
"#
                .iter()
                .copied(),
            )
            .collect::<Vec<_>>();
        for (required, expected_action) in [(false, "create"), (true, "blocked")] {
            let mut manifest = manifest_value();
            manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&blueprint));
            manifest["extensions"] = json!([{
                "key": "extensions/shop",
                "id": "acme.shop",
                "version": "^1.0",
                "required": required,
            }]);
            let mut files = valid_files();
            files[0] = (files[0].0, &blueprint);
            let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
            let plan = build_solution_pack_plan(
                &pack,
                "shop",
                BlueprintPublication::Draft,
                &PlanningWorkspaceSnapshot {
                    workspace_id: uuid::Uuid::nil(),
                    physical_codes: BTreeSet::from(["default".to_owned()]),
                    existing_blueprints: BTreeMap::new(),
                    installed_extensions: BTreeMap::new(),
                    explore_navigation: Vec::new(),
                    explore_navigation_valid: true,
                    extension_layout: json!({"version":1,"outlets":{}}),
                    extension_layout_valid: true,
                    role_codes: BTreeSet::new(),
                    published_entity_codes: BTreeSet::new(),
                },
            )
            .unwrap();
            let action = plan
                .actions
                .iter()
                .find(|action| action.logical_key == "blueprints/product")
                .unwrap();
            assert_eq!(action.action, expected_action);
            if !required {
                assert!(
                    !action.normalized_payload.as_ref().unwrap()["definition"]
                        .as_str()
                        .unwrap()
                        .contains("acme.shop:action")
                );
                assert_eq!(action.summary["extension_layout"][0]["outcome"], "skip");
            } else {
                for contributions in [
                    BTreeMap::new(),
                    BTreeMap::from([(
                        "acme.shop:action".to_owned(),
                        "entity_preview_panel".to_owned(),
                    )]),
                ] {
                    let mapped = build_solution_pack_plan(
                        &pack,
                        "shop",
                        BlueprintPublication::Draft,
                        &PlanningWorkspaceSnapshot {
                            workspace_id: uuid::Uuid::nil(),
                            physical_codes: BTreeSet::from(["shop_product".to_owned()]),
                            existing_blueprints: BTreeMap::from([(
                                "blueprints/product".to_owned(),
                                ExistingBlueprintSnapshot {
                                    id: uuid::Uuid::from_u128(100),
                                    code: "shop_product".to_owned(),
                                    version: 1,
                                    kind: "entity".to_owned(),
                                    canonical_definition_hash: "0".repeat(64),
                                    definition_hash: "0".repeat(64),
                                },
                            )]),
                            installed_extensions: BTreeMap::from([(
                                "acme.shop".to_owned(),
                                InstalledExtensionSnapshot {
                                    installed_release_id: uuid::Uuid::from_u128(101),
                                    version: "1.0.0".to_owned(),
                                    state: "enabled".to_owned(),
                                    configuration: json!({}),
                                    policy_compatible: true,
                                    contributions,
                                },
                            )]),
                            explore_navigation: Vec::new(),
                            explore_navigation_valid: true,
                            extension_layout: json!({"version":1,"outlets":{}}),
                            extension_layout_valid: true,
                            role_codes: BTreeSet::new(),
                            published_entity_codes: BTreeSet::from(["shop_product".to_owned()]),
                        },
                    )
                    .unwrap();
                    let mapped_product = mapped
                        .actions
                        .iter()
                        .find(|action| action.logical_key == "blueprints/product")
                        .unwrap();
                    assert_eq!(
                        (mapped_product.action, mapped_product.reason_code),
                        ("blocked", "extension_contribution_unavailable")
                    );
                    assert_eq!(mapped.extension_requirements[0].status, "satisfied");
                    assert!(!mapped.ready);
                }
            }
        }
    }

    #[test]
    fn workspace_layout_accepts_every_manifest_outlet_and_both_primary_lists() {
        for outlet in [
            "navigation",
            "entity_preview_panel",
            "blueprint_attribute_configuration",
            "entity_attribute_decoration",
            "entity_action",
            "explorer_row_action",
            "explorer_table_cell",
            "blueprint_detail_panel",
            "explorer_action",
            "explorer_bulk_action",
            "entity_header_action",
            "entity_attribute_panel",
            "blueprint_panel",
            "blueprint_publish_check",
            "file_panel",
            "audit_event_panel",
            "data_health_card",
        ] {
            for hidden in [false, true] {
                let layout = serde_json::to_vec(&json!({
                    "format_version": 1,
                    "kind": "extension_layout",
                    "entries": [{
                        "contribution": "acme.shop:item",
                        "outlet": outlet,
                        "hidden": hidden,
                        "required": true,
                    }],
                }))
                .unwrap();
                ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(&layout, true))
                    .unwrap();
            }
        }
    }

    #[test]
    fn blueprint_layout_contribution_evidence_order_is_deterministic() {
        let blueprint = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                br#"
[views.extension_layout]
type = "extension_layout"
version = 1
[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.shop:preview"]
hidden = []
[views.extension_layout.outlets.entity_action]
order = ["acme.shop:z_action"]
hidden = ["acme.shop:a_action"]
"#
                .iter()
                .copied(),
            )
            .collect::<Vec<_>>();
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&blueprint));
        manifest["extensions"] = json!([{
            "key": "extensions/shop",
            "id": "acme.shop",
            "version": "^1.0",
            "required": true,
        }]);
        let mut files = valid_files();
        files[0] = (files[0].0, &blueprint);
        let archive = archive(&manifest, &files);
        let expected = vec![
            ("entity_action", "acme.shop:a_action"),
            ("entity_action", "acme.shop:z_action"),
            ("entity_preview_panel", "acme.shop:preview"),
        ];
        for _ in 0..20 {
            let pack = ValidatedSolutionPack::from_tar_zst(&archive).unwrap();
            let actual = pack
                .blueprint("blueprints/product")
                .unwrap()
                .extension_layout
                .iter()
                .map(|entry| (entry.outlet.as_str(), entry.contribution.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn rejects_invalid_extension_layout_contract() {
        for invalid in [
            br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"entity_action","promoted":true,"required":true}]}"#.as_slice(),
            br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","hidden":true,"promoted":true,"required":true}]}"#.as_slice(),
            br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":true,"unknown":true}]}"#.as_slice(),
            br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":true},{"contribution":"acme.shop:nav","outlet":"navigation","required":true}]}"#.as_slice(),
        ] {
            assert!(ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(invalid, true)).is_err());
        }
        let too_many = serde_json::to_vec(&json!({
            "format_version": 1,
            "kind": "extension_layout",
            "entries": (0..=MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES)
                .map(|index| json!({
                    "contribution": format!("acme.shop:item_{index}"),
                    "outlet": "entity_action",
                    "required": true,
                }))
                .collect::<Vec<_>>(),
        }))
        .unwrap();
        assert!(
            ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(&too_many, true))
                .is_err()
        );
    }

    #[test]
    fn planner_appends_satisfies_and_conflicts_explore_navigation() {
        let pack =
            ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(true)).unwrap();
        let workspace = |navigation| PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: navigation,
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::from(["editor".to_owned(), "viewer".to_owned()]),
            published_entity_codes: BTreeSet::from(["ecom_product".to_owned()]),
        };
        let appended = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &workspace(Vec::new()),
        )
        .unwrap();
        let action = appended.actions.last().unwrap();
        assert_eq!(
            (action.action, action.reason_code),
            ("append", "target_absent")
        );
        assert!(appended.ready);

        let exact = PlanningExploreNavigationEntry {
            blueprint_code: "ecom_product".to_owned(),
            visible_to_role_codes: vec!["viewer".to_owned(), "editor".to_owned()],
        };
        let satisfied = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &workspace(vec![exact]),
        )
        .unwrap();
        assert_eq!(satisfied.actions.last().unwrap().action, "satisfied");

        let conflicting = PlanningExploreNavigationEntry {
            blueprint_code: "ecom_product".to_owned(),
            visible_to_role_codes: vec!["viewer".to_owned()],
        };
        let conflicted = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &workspace(vec![conflicting]),
        )
        .unwrap();
        assert_eq!(conflicted.actions.last().unwrap().action, "conflict");
        assert!(!conflicted.ready);
    }

    #[test]
    fn planner_blocks_required_navigation_and_skips_optional_unmet_navigation() {
        for (required, expected) in [(true, "blocked"), (false, "skip")] {
            let pack =
                ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(required))
                    .unwrap();
            let plan = build_solution_pack_plan(
                &pack,
                "ecom",
                BlueprintPublication::Draft,
                &PlanningWorkspaceSnapshot {
                    workspace_id: uuid::Uuid::nil(),
                    physical_codes: BTreeSet::from(["default".to_owned()]),
                    existing_blueprints: BTreeMap::new(),
                    installed_extensions: BTreeMap::new(),
                    explore_navigation: Vec::new(),
                    explore_navigation_valid: true,
                    extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                    extension_layout_valid: true,
                    role_codes: BTreeSet::from(["editor".to_owned(), "viewer".to_owned()]),
                    published_entity_codes: BTreeSet::new(),
                },
            )
            .unwrap();
            assert_eq!(plan.actions.last().unwrap().action, expected);
        }

        let navigation = br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product"},{"blueprint":"blueprints/category","visible_to_role_codes":["missing_role"]}]}"#;
        let mut manifest = manifest_value();
        manifest["resources"]["workspace_settings"] = json!([{
            "key": "workspace/explore-navigation",
            "path": "workspace/explore-navigation.json",
            "required": false,
            "sha256": digest(navigation),
        }]);
        let files = [
            ("blueprints/product.toml", PRODUCT_BLUEPRINT),
            ("blueprints/category.toml", CATEGORY_BLUEPRINT),
            ("workspace/explore-navigation.json", navigation.as_slice()),
        ];
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        let action = plan.actions.last().unwrap();
        assert_eq!(action.action, "append");
        assert_eq!(action.summary["entries"][1]["outcome"], "skip");
        assert_eq!(
            action.normalized_payload.as_ref().unwrap()["entries"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn validates_bounded_non_secret_extension_configuration_templates() {
        const TEMPLATE: &[u8] = br#"{"endpoint":"https://example.test","features":{"sync":true}}"#;
        let pack =
            ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(TEMPLATE))
                .unwrap();
        assert_eq!(pack.manifest().extensions.len(), 1);
        assert_eq!(
            pack.configuration_template("extensions/shopify"),
            Some(&json!({"endpoint":"https://example.test","features":{"sync":true}}))
        );

        for template in [
            br#"[]"#.as_slice(),
            br#"{"api_token":"public-looking-but-forbidden"}"#,
            br#"{"nested":{"PASSWORD":"forbidden"}}"#,
            br#"{"unterminated":true"#,
        ] {
            assert!(
                ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(template))
                    .is_err()
            );
        }

        let exact_size = format!(
            "{{}}{}",
            " ".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES - 2)
        );
        ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(
            exact_size.as_bytes(),
        ))
        .unwrap();
        assert_invalid(
            &archive_with_configuration_template(format!("{exact_size} ").as_bytes()),
            "size limit",
        );

        fn validate_value(value: &Value) -> Result<(), SolutionPackError> {
            let mut items = 0;
            validate_configuration_template_value(value, 1, &mut items, "extensions/test")
        }

        let mut maximum_depth = json!(true);
        for _ in 0..(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH - 1) {
            maximum_depth = json!({"level": maximum_depth});
        }
        validate_value(&maximum_depth).unwrap();
        assert!(validate_value(&json!({"level": maximum_depth})).is_err());

        let maximum_items = Value::Object(
            (0..MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS)
                .map(|index| (format!("field_{index}"), Value::Null))
                .collect(),
        );
        validate_value(&maximum_items).unwrap();
        let excessive_items = Value::Object(
            (0..=MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS)
                .map(|index| (format!("field_{index}"), Value::Null))
                .collect(),
        );
        assert!(validate_value(&excessive_items).is_err());

        validate_value(&json!({
            "k".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES): true
        }))
        .unwrap();
        assert!(
            validate_value(&json!({
                "k".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES + 1): true
            }))
            .is_err()
        );
        validate_value(&json!({
            "value": "v".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES)
        }))
        .unwrap();
        assert!(
            validate_value(&json!({
                "value": "v".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES + 1)
            }))
            .is_err()
        );
    }

    #[test]
    fn extension_requirement_evaluation_is_deterministic_and_does_not_require_enablement() {
        let requirement = SolutionPackExtensionRequirement {
            key: "extensions/shopify".into(),
            id: "acme.shopify".into(),
            version: ">=2.1.0 <3.0.0".into(),
            required: true,
            configuration_template: None,
        };
        let installed = InstalledExtensionSnapshot {
            installed_release_id: uuid::Uuid::nil(),
            version: "2.2.0".into(),
            state: "disabled".into(),
            configuration: json!({"endpoint":"https://example.test","unrelated_secret":"not exposed"}),
            policy_compatible: true,
            contributions: BTreeMap::new(),
        };
        let template = json!({"endpoint":"https://example.test"});
        let satisfied =
            evaluate_extension_requirement(&requirement, Some(&template), Some(&installed));
        assert_eq!(satisfied.status, "satisfied");
        assert_eq!(satisfied.reason_code, "satisfied");

        let mismatch = evaluate_extension_requirement(
            &requirement,
            Some(&json!({"endpoint":"https://other.test"})),
            Some(&installed),
        );
        assert_eq!(mismatch.status, "blocked");
        assert_eq!(mismatch.reason_code, "configuration_mismatch");

        let incompatible = InstalledExtensionSnapshot {
            version: "3.0.0".into(),
            ..installed.clone()
        };
        let blocked = evaluate_extension_requirement(&requirement, None, Some(&incompatible));
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.reason_code, "incompatible_version");

        let mut optional = requirement.clone();
        optional.required = false;
        let missing = evaluate_extension_requirement(&optional, None, None);
        assert_eq!(missing.status, "skipped");
        assert_eq!(missing.reason_code, "missing");
        let incompatible = evaluate_extension_requirement(&optional, None, Some(&incompatible));
        assert_eq!(incompatible.status, "skipped");
        assert_eq!(incompatible.reason_code, "incompatible_version");
        let optional_mismatch = evaluate_extension_requirement(
            &optional,
            Some(&json!({"endpoint":"https://other.test"})),
            Some(&installed),
        );
        assert_eq!(optional_mismatch.status, "skipped");
        assert_eq!(optional_mismatch.reason_code, "configuration_mismatch");
        let optional_satisfied =
            evaluate_extension_requirement(&optional, Some(&template), Some(&installed));
        assert_eq!(optional_satisfied.status, "satisfied");

        let quarantined = InstalledExtensionSnapshot {
            state: "quarantined".into(),
            ..installed
        };
        let blocked = evaluate_extension_requirement(&requirement, None, Some(&quarantined));
        assert_eq!(blocked.reason_code, "quarantined");
        let skipped = evaluate_extension_requirement(&optional, None, Some(&quarantined));
        assert_eq!(skipped.status, "skipped");
        assert_eq!(skipped.reason_code, "quarantined");
    }

    #[test]
    fn configuration_matching_uses_recursive_object_containment_and_exact_arrays() {
        assert!(json_deep_contains(
            &json!({"nested":{"enabled":true,"extra":1},"list":[1,2],"extra":true}),
            &json!({"nested":{"enabled":true},"list":[1,2]})
        ));
        assert!(!json_deep_contains(
            &json!({"list":[1,2,3]}),
            &json!({"list":[1,2]})
        ));
        assert!(!json_deep_contains(
            &json!({"value":"1"}),
            &json!({"value":1})
        ));
    }

    #[test]
    fn accepts_safe_directory_entries() {
        let manifest = serde_json::to_vec(&manifest_value()).unwrap();
        let files = valid_files();
        let mut entries = vec![(
            b"blueprints/".as_slice(),
            EntryType::Directory,
            b"".as_slice(),
        )];
        entries.push((b"solution-pack.json", EntryType::Regular, &manifest));
        entries.extend(
            files
                .iter()
                .map(|(path, bytes)| (path.as_bytes(), EntryType::Regular, *bytes)),
        );
        ValidatedSolutionPack::from_tar_zst(&custom_archive(&entries)).unwrap();
    }

    #[test]
    fn published_manifest_schema_accepts_the_v1_fixture_and_is_strict() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/solution-pack-manifest-v1.schema.json"
        ))
        .unwrap();
        catalog_validation::validate_json_schema_definition(&schema).unwrap();
        assert!(
            catalog_validation::validate_json_schema(&schema, &manifest_value())
                .unwrap()
                .is_empty()
        );
        for contexts in [json!([]), json!([{"key": "contexts/web"}])] {
            let mut with_contexts = manifest_value();
            with_contexts["resources"]["contexts"] = contexts;
            assert!(
                !catalog_validation::validate_json_schema(&schema, &with_contexts)
                    .unwrap()
                    .is_empty()
            );
        }
        let mut with_navigation = manifest_value();
        with_navigation["resources"]["workspace_settings"] = json!([{
            "key": "workspace/explore-navigation",
            "path": "workspace/explore-navigation.json",
            "required": true,
            "sha256": "0".repeat(64),
        }]);
        assert!(
            catalog_validation::validate_json_schema(&schema, &with_navigation)
                .unwrap()
                .is_empty()
        );
        let mut duplicate_navigation = with_navigation.clone();
        duplicate_navigation["resources"]["workspace_settings"] = json!([
            with_navigation["resources"]["workspace_settings"][0].clone(),
            with_navigation["resources"]["workspace_settings"][0].clone(),
        ]);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &duplicate_navigation)
                .unwrap()
                .is_empty()
        );
        let checklist_schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/solution-pack-setup-checklist-v1.schema.json"
        ))
        .unwrap();
        let checks_schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/solution-pack-checks-v1.schema.json"
        ))
        .unwrap();
        catalog_validation::validate_json_schema_definition(&checklist_schema).unwrap();
        catalog_validation::validate_json_schema_definition(&checks_schema).unwrap();
        let checklist = json!({"format_version":1,"items":[{"key":"checklist/publish","title":"Publish","markdown":"Publish it.","check":"checks/published"}]});
        let checks = json!({"format_version":1,"checks":[{"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}]});
        assert!(
            catalog_validation::validate_json_schema(&checklist_schema, &checklist)
                .unwrap()
                .is_empty()
        );
        assert!(
            catalog_validation::validate_json_schema(&checks_schema, &checks)
                .unwrap()
                .is_empty()
        );
        let duplicate_checks = json!({"format_version":1,"checks":[
            {"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}},
            {"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}
        ]});
        assert!(
            !catalog_validation::validate_json_schema(&checks_schema, &duplicate_checks)
                .unwrap()
                .is_empty()
        );
        let contribution_256 = format!("a:{}", "b".repeat(254));
        let contribution_257 = format!("a:{}", "b".repeat(255));
        for (contribution, valid) in [(contribution_256, true), (contribution_257, false)] {
            assert_eq!(valid_contribution_key(&contribution), valid);
            let value = json!({"format_version":1,"checks":[{"key":"checks/layout","title":"Layout","predicate":{"type":"workspace_extension_layout_placement_present","contribution":contribution}}]});
            assert_eq!(
                catalog_validation::validate_json_schema(&checks_schema, &value)
                    .unwrap()
                    .is_empty(),
                valid
            );
        }
        let mut unknown_check = checks;
        unknown_check["checks"][0]["predicate"]["query"] = json!("select 1");
        assert!(
            !catalog_validation::validate_json_schema(&checks_schema, &unknown_check)
                .unwrap()
                .is_empty()
        );

        let duplicate_checklist = json!({"format_version":1,"items":[
            {"key":"checklist/publish","title":"Publish","markdown":"Publish it."},
            {"key":"checklist/publish","title":"Publish","markdown":"Publish it."}
        ]});
        assert!(
            !catalog_validation::validate_json_schema(&checklist_schema, &duplicate_checklist)
                .unwrap()
                .is_empty()
        );
        let mut manifest_with_guidance = manifest_value();
        manifest_with_guidance["documentation"] =
            json!({"readme":{"path":"README.md","sha256":"0".repeat(64)}});
        assert!(
            catalog_validation::validate_json_schema(&schema, &manifest_with_guidance)
                .unwrap()
                .is_empty()
        );
        for unsafe_path in [
            "/README.md",
            "../README.md",
            "docs/../README.md",
            "C:README.md",
            "docs\\README.md",
            "docs/README\n.md",
            "docs/README\u{85}.md",
        ] {
            manifest_with_guidance["documentation"]["readme"]["path"] = json!(unsafe_path);
            assert!(
                !catalog_validation::validate_json_schema(&schema, &manifest_with_guidance)
                    .unwrap()
                    .is_empty(),
                "schema accepted unsafe path {unsafe_path}"
            );
        }

        let navigation_schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/solution-pack-explore-navigation-v1.schema.json"
        ))
        .unwrap();
        catalog_validation::validate_json_schema_definition(&navigation_schema).unwrap();
        let navigation: Value = serde_json::from_slice(EXPLORE_NAVIGATION).unwrap();
        assert!(
            catalog_validation::validate_json_schema(&navigation_schema, &navigation)
                .unwrap()
                .is_empty()
        );
        let mut unknown_navigation = navigation;
        unknown_navigation["unknown"] = json!(true);
        assert!(
            !catalog_validation::validate_json_schema(&navigation_schema, &unknown_navigation)
                .unwrap()
                .is_empty()
        );

        let layout_schema: Value = serde_json::from_str(include_str!(
            "../../../contracts/solution-pack-extension-layout-v1.schema.json"
        ))
        .unwrap();
        catalog_validation::validate_json_schema_definition(&layout_schema).unwrap();
        let layout_entry = json!({
            "contribution": "acme.shop:nav",
            "outlet": "navigation",
            "required": true,
        });
        let valid_layout = json!({
            "format_version": 1,
            "kind": "extension_layout",
            "entries": [layout_entry.clone()],
        });
        assert!(
            catalog_validation::validate_json_schema(&layout_schema, &valid_layout)
                .unwrap()
                .is_empty()
        );
        let promoted_layout = json!({
            "format_version": 1,
            "kind": "extension_layout",
            "entries": [{
                "contribution": "acme.shop:nav",
                "outlet": "navigation",
                "hidden": false,
                "promoted": true,
                "required": true,
            }],
        });
        assert!(
            catalog_validation::validate_json_schema(&layout_schema, &promoted_layout)
                .unwrap()
                .is_empty()
        );
        for invalid_layout in [
            json!({
                "format_version": 1,
                "kind": "extension_layout",
                "entries": [layout_entry.clone(), layout_entry.clone()],
            }),
            json!({
                "format_version": 1,
                "kind": "extension_layout",
                "entries": [{
                    "contribution": "acme.shop:nav",
                    "outlet": "navigation",
                    "hidden": true,
                    "promoted": true,
                    "required": true,
                }],
            }),
        ] {
            assert!(
                !catalog_validation::validate_json_schema(&layout_schema, &invalid_layout)
                    .unwrap()
                    .is_empty()
            );
        }

        let mut unknown = manifest_value();
        unknown["unknown"] = json!(true);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &unknown)
                .unwrap()
                .is_empty()
        );

        let mut with_extension = manifest_value();
        with_extension["extensions"] = json!([{
            "key":"extensions/shopify",
            "id":"acme.shopify",
            "version":">=2.1.0 <3.0.0",
            "required":true,
            "configuration_template": {
                "path":"extensions/shopify.json",
                "sha256":"0".repeat(64)
            }
        }]);
        assert!(
            catalog_validation::validate_json_schema(&schema, &with_extension)
                .unwrap()
                .is_empty()
        );
        with_extension["extensions"][0]["unknown"] = json!(true);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &with_extension)
                .unwrap()
                .is_empty()
        );

        for (pointer, invalid_value, runtime_error) in [
            ("/extensions/0/version", "not-a-range", "SemVer range"),
            (
                "/extensions/0/configuration_template/path",
                "extensions/../template.json",
                "configuration template path is invalid",
            ),
            (
                "/extensions/0/key",
                "extensions/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "extension requirement key",
            ),
        ] {
            let mut invalid = manifest_value();
            invalid["extensions"] = json!([{
                "key":"extensions/shopify",
                "id":"acme.shopify",
                "version":"^1.0",
                "required":true,
                "configuration_template": {
                    "path":"extensions/shopify.json",
                    "sha256":"0".repeat(64)
                }
            }]);
            *invalid.pointer_mut(pointer).unwrap() = json!(invalid_value);
            assert!(
                !catalog_validation::validate_json_schema(&schema, &invalid)
                    .unwrap()
                    .is_empty(),
                "schema accepted invalid extension value '{invalid_value}'"
            );
            assert_invalid(&archive(&invalid, &valid_files()), runtime_error);
        }

        for (field, invalid_value, runtime_error) in [
            ("id", "attricat.foo-", "reverse-DNS-style"),
            ("version", "1.0.0-01", "version must be SemVer"),
        ] {
            let mut invalid = manifest_value();
            invalid[field] = json!(invalid_value);
            assert!(
                !catalog_validation::validate_json_schema(&schema, &invalid)
                    .unwrap()
                    .is_empty(),
                "schema accepted invalid {field} '{invalid_value}'"
            );
            assert_invalid(&archive(&invalid, &valid_files()), runtime_error);
        }
    }

    #[test]
    fn rejects_unknown_fields_at_each_manifest_contract_level() {
        for pointer in ["", "/catalog", "/resources", "/resources/blueprints/0"] {
            let mut manifest = manifest_value();
            manifest
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unknown".into(), json!(true));
            assert_invalid(&archive(&manifest, &valid_files()), "strict v1 manifest");
        }
    }

    #[test]
    fn rejects_invalid_container_manifest_and_package_confusion() {
        assert!(ValidatedSolutionPack::from_tar_zst(b"not zstd").is_err());

        let only_extension_manifest =
            custom_archive(&[(b"manifest.json", EntryType::Regular, br#"{}"#)]);
        assert_invalid(&only_extension_manifest, "must contain solution-pack.json");

        let nested =
            custom_archive(&[(b"nested/solution-pack.json", EntryType::Regular, br#"{}"#)]);
        assert_invalid(&nested, "must contain solution-pack.json");

        let manifest = serde_json::to_vec(&manifest_value()).unwrap();
        let duplicate = custom_archive(&[
            (b"solution-pack.json", EntryType::Regular, &manifest),
            (b"solution-pack.json", EntryType::Regular, &manifest),
        ]);
        assert_invalid(&duplicate, "duplicate entry paths");
    }

    #[test]
    fn rejects_unsafe_non_utf8_and_non_file_entries() {
        let manifest = serde_json::to_vec(&manifest_value()).unwrap();
        for path in [
            b"../solution-pack.json".as_slice(),
            b"/solution-pack.json",
            b"C:/solution-pack.json",
            b"dir\\solution-pack.json",
            b"dir/solution-pack\n.json",
            b"dir/solution-pack\x7f.json",
        ] {
            let archive = custom_archive(&[(path, EntryType::Regular, &manifest)]);
            assert_invalid(&archive, "unsafe entry path");
        }
        let archive = custom_archive(&[(&[0xff], EntryType::Regular, b"x")]);
        assert_invalid(&archive, "paths must be UTF-8");

        for kind in [EntryType::Symlink, EntryType::Link, EntryType::Char] {
            let archive = custom_archive(&[(b"solution-pack.json", kind, b"")]);
            assert_invalid(&archive, "non-file entry");
        }
    }

    #[test]
    fn enforces_compressed_expanded_per_file_manifest_and_entry_limits() {
        let valid = archive(&manifest_value(), &valid_files());
        let limits = ArchiveLimits {
            compressed_bytes: valid.len() - 1,
            ..ArchiveLimits::default()
        };
        assert!(ValidatedSolutionPack::from_tar_zst_with_limits(&valid, limits).is_err());

        let limits = ArchiveLimits {
            file_bytes: PRODUCT_BLUEPRINT.len() - 1,
            ..ArchiveLimits::default()
        };
        assert_invalid_with_limits(&valid, limits, "per-file size limit");

        let limits = ArchiveLimits {
            manifest_bytes: 10,
            ..ArchiveLimits::default()
        };
        assert_invalid_with_limits(&valid, limits, "manifest exceeds");

        let limits = ArchiveLimits {
            entries: 2,
            ..ArchiveLimits::default()
        };
        assert_invalid_with_limits(&valid, limits, "too many entries");

        let mut tar_bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_bytes);
            append_file(&mut builder, SOLUTION_PACK_MANIFEST_PATH, b"{}");
            builder.finish().unwrap();
        }
        tar_bytes.write_all(&vec![0; 4096]).unwrap();
        let trailing = zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap();
        let limits = ArchiveLimits {
            expanded_bytes: 2048,
            file_bytes: 1024,
            ..ArchiveLimits::default()
        };
        assert_invalid_with_limits(&trailing, limits, "expanded size limit");
    }

    #[test]
    fn rejects_non_zero_data_after_the_tar_end_marker() {
        let valid = archive(&manifest_value(), &valid_files());
        let mut tar_bytes = zstd::stream::decode_all(Cursor::new(valid)).unwrap();
        tar_bytes.extend_from_slice(b"hidden payload");
        let archive = zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap();

        assert_invalid(&archive, "data after the tar end marker");
    }

    fn assert_invalid_with_limits(archive: &[u8], limits: ArchiveLimits, expected: &str) {
        let error = ValidatedSolutionPack::from_tar_zst_with_limits(archive, limits).unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected '{expected}' in '{error}'"
        );
    }

    #[test]
    fn rejects_missing_undeclared_and_digest_mismatched_files() {
        let files = valid_files();
        assert_invalid(&archive(&manifest_value(), &files[..1]), "is missing");

        let mut extra = files.clone();
        extra.push(("README.md", b"undeclared"));
        assert_invalid(&archive(&manifest_value(), &extra), "is not declared");

        let mut changed = files;
        changed[0] = (changed[0].0, b"changed");
        assert_invalid(&archive(&manifest_value(), &changed), "sha256 digest");
    }

    #[test]
    fn rejects_invalid_manifest_versions_ids_keys_paths_and_digests() {
        let cases = [
            ("/manifest_version", json!(2), "manifest_version 2"),
            ("/id", json!("Ecommerce"), "reverse-DNS"),
            ("/version", json!("latest"), "SemVer"),
            ("/catalog/host_api", json!(">=2"), "incompatible"),
            (
                "/resources/blueprints/0/key",
                json!("contexts/product"),
                "blueprints/ namespace",
            ),
            (
                "/resources/blueprints/0/path",
                json!("contexts/product.toml"),
                "resource path",
            ),
            (
                "/resources/blueprints/0/sha256",
                json!("ABC"),
                "64 lowercase",
            ),
        ];
        for (pointer, replacement, expected) in cases {
            let mut manifest = manifest_value();
            *manifest.pointer_mut(pointer).unwrap() = replacement;
            assert_invalid(&archive(&manifest, &valid_files()), expected);
        }

        let mut duplicate_key = manifest_value();
        duplicate_key["resources"]["blueprints"][1]["key"] = json!("blueprints/product");
        assert_invalid(
            &archive(&duplicate_key, &valid_files()),
            "duplicate resource key",
        );

        let mut duplicate_path = manifest_value();
        duplicate_path["resources"]["blueprints"][1]["path"] = json!("blueprints/product.toml");
        assert_invalid(
            &archive(&duplicate_path, &valid_files()),
            "duplicate resource path",
        );
    }

    #[test]
    fn validates_blueprint_content_and_logical_reference_closure() {
        let mut files = valid_files();
        let invalid = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
unknown = true
[[attributes]]
code = "name"
value_type = "string"
"#;
        files[0] = (files[0].0, invalid);
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(invalid));
        assert_invalid(
            &archive(&manifest, &files),
            "blueprint 'blueprints/product' is invalid",
        );

        let missing_target = CATEGORY_BLUEPRINT.replace_ascii(b"product", b"missing");
        let mut files = valid_files();
        files[1] = (files[1].0, &missing_target);
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&missing_target));
        assert_invalid(
            &archive(&manifest, &files),
            "undeclared relationship target",
        );
    }

    #[test]
    fn rejects_blueprints_that_fail_ordinary_compiler_validation() {
        let missing_dropdown = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
[[attributes]]
code = "name"
value_type = "string"
"#;
        assert_blueprint_error(missing_dropdown, "must define views.dropdown_option");

        let unknown_view_field =
            PRODUCT_BLUEPRINT.replace_ascii(b"fields = [\"name\"]", b"fields = [\"missing\"]");
        assert_blueprint_error(&unknown_view_field, "unknown attribute 'missing'");

        let malformed_component = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                br#"
[views.table]
type = "table"
fields = ["name"]
component = { id = "INVALID", version = 1 }
"#
                .iter()
                .copied(),
            )
            .collect::<Vec<_>>();
        assert_blueprint_error(&malformed_component, "component id");

        let product = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{ alias = "base", key = "blueprints/category" }]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
from = "base.name"
"#;
        let mixin = br#"
format_version = 1
code = "category"
name = "Category fields"
kind = "mixin"
[[attributes]]
code = "title"
value_type = "string"
"#;
        let files = vec![
            ("blueprints/product.toml", product.as_slice()),
            ("blueprints/category.toml", mixin.as_slice()),
        ];
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(product));
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(mixin));
        assert_invalid(&archive(&manifest, &files), "not exposed by include 'base'");
    }

    #[test]
    fn rejects_manifests_that_exceed_resource_count_limits() {
        let mut manifest: SolutionPackManifest = serde_json::from_value(manifest_value()).unwrap();
        let blueprint = manifest.resources.blueprints[0].clone();
        manifest.resources.blueprints = vec![blueprint; MAX_SOLUTION_PACK_BLUEPRINTS + 1];
        assert!(
            validate_manifest(&manifest)
                .unwrap_err()
                .to_string()
                .contains("too many blueprints")
        );

        let mut manifest: SolutionPackManifest = serde_json::from_value(manifest_value()).unwrap();
        let requirement: SolutionPackExtensionRequirement = serde_json::from_value(json!({
            "key":"extensions/example",
            "id":"acme.example",
            "version":"^1.0",
            "required":false
        }))
        .unwrap();
        manifest.extensions = vec![requirement; MAX_SOLUTION_PACK_EXTENSION_REQUIREMENTS + 1];
        assert!(
            validate_manifest(&manifest)
                .unwrap_err()
                .to_string()
                .contains("too many extension requirements")
        );
    }

    #[test]
    fn rejects_blueprints_that_exceed_structural_complexity_limits() {
        let includes = (0..=MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES)
            .map(|index| format!("{{ alias = \"m{index}\", key = \"blueprints/category\" }}"))
            .collect::<Vec<_>>()
            .join(", ");
        let excessive_includes = format!(
            r#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{includes}]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
        )
        .into_bytes();
        assert_blueprint_error(&excessive_includes, "exceeds the include limit");

        let mut excessive_attributes = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["field_0"]
"#
        .to_vec();
        for index in 0..=MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
            excessive_attributes.extend_from_slice(
                format!("[[attributes]]\ncode = \"field_{index}\"\nvalue_type = \"string\"\n")
                    .as_bytes(),
            );
        }
        assert_blueprint_error(&excessive_attributes, "exceeds the attribute limit");

        let includes = (0..5)
            .map(|index| format!("{{ alias = \"m{index}\", key = \"blueprints/category\" }}"))
            .collect::<Vec<_>>()
            .join(", ");
        let product = format!(
            r#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{includes}]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
        )
        .into_bytes();
        let mut mixin = br#"format_version = 1
code = "category"
name = "Category fields"
kind = "mixin"
"#
        .to_vec();
        for index in 0..MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
            mixin.extend_from_slice(
                format!("[[attributes]]\ncode = \"field_{index}\"\nvalue_type = \"string\"\n")
                    .as_bytes(),
            );
        }
        assert_blueprints_error(
            &product,
            &mixin,
            "exceeds the resolved include attribute limit",
        );

        let mut owned_files = Vec::new();
        let mut resources = Vec::new();
        for blueprint_index in 0..17 {
            let code = format!("mixin_{blueprint_index}");
            let path = format!("blueprints/{code}.toml");
            let mut source = format!(
                "format_version = 1\ncode = \"{code}\"\nname = \"Mixin {blueprint_index}\"\nkind = \"mixin\"\n"
            )
            .into_bytes();
            for attribute_index in 0..MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
                source.extend_from_slice(
                    format!(
                        "[[attributes]]\ncode = \"field_{attribute_index}\"\nvalue_type = \"string\"\n"
                    )
                    .as_bytes(),
                );
            }
            resources.push(resource(&format!("blueprints/{code}"), &path, &source));
            owned_files.push((path, source));
        }
        let file_refs = owned_files
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_slice()))
            .collect::<Vec<_>>();
        let manifest = json!({
            "manifest_version": 1,
            "id": "attricat.complex",
            "name": "Complex",
            "version": "1.0.0",
            "description": "Complex pack",
            "catalog": {"host_api": "^1.0"},
            "resources": {"blueprints": resources}
        });
        assert_invalid(
            &archive(&manifest, &file_refs),
            "complexity exceeds the total limit",
        );
    }

    #[test]
    fn validates_incoming_relationship_selectors_across_blueprints() {
        let missing =
            CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"missing\"");
        assert_blueprints_error(PRODUCT_BLUEPRINT, &missing, "has no field 'missing'");

        let scalar =
            CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"name\"");
        assert_blueprints_error(
            PRODUCT_BLUEPRINT,
            &scalar,
            "field 'name' must be a relationship",
        );

        let wrong_target = PRODUCT_BLUEPRINT.replace_ascii(
            b"target_blueprint = \"blueprints/category\"",
            b"target_blueprint = \"blueprints/product\"",
        );
        assert_blueprints_error(
            &wrong_target,
            CATEGORY_BLUEPRINT,
            "field 'categories' must target 'category'",
        );
    }

    fn assert_blueprints_error(product: &[u8], category: &[u8], expected: &str) {
        let files = vec![
            ("blueprints/product.toml", product),
            ("blueprints/category.toml", category),
        ];
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(product));
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(category));
        assert_invalid(&archive(&manifest, &files), expected);
    }

    fn assert_blueprint_error(blueprint: &[u8], expected: &str) {
        let mut files = valid_files();
        files[0] = (files[0].0, blueprint);
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(blueprint));
        assert_invalid(&archive(&manifest, &files), expected);
    }

    #[test]
    fn requires_portable_include_keys_instead_of_native_revisions() {
        let native_include = PRODUCT_BLUEPRINT.replace_ascii(
            b"kind = \"entity\"",
            b"kind = \"entity\"\nincludes = [{ alias = \"base\", code = \"category\", version = 1 }]",
        );
        assert_blueprint_error(&native_include, "native code or revision fields");

        let mixed_include = PRODUCT_BLUEPRINT.replace_ascii(
            b"kind = \"entity\"",
            b"kind = \"entity\"\nincludes = [{ alias = \"base\", key = \"blueprints/category\", version = 1 }]",
        );
        assert_blueprint_error(&mixed_include, "native code or revision fields");

        let portable = blueprint_with_include("product", "category");
        let mixin = blueprint_with_include_without_dependencies("category");
        let files = vec![
            ("blueprints/product.toml", portable.as_slice()),
            ("blueprints/category.toml", mixin.as_slice()),
        ];
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&portable));
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&mixin));
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        let include = &pack.blueprint("blueprints/product").unwrap().includes()[0];
        assert_eq!(include.alias(), "base");
        assert_eq!(include.key(), "blueprints/category");
    }

    #[test]
    fn rejects_blueprint_include_cycles_and_extension_metadata() {
        let first = blueprint_with_include("product", "category");
        let second = blueprint_with_include("category", "product");
        let files = vec![
            ("blueprints/product.toml", first.as_slice()),
            ("blueprints/category.toml", second.as_slice()),
        ];
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&first));
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&second));
        assert_invalid(
            &archive(&manifest, &files),
            "include references contain a cycle",
        );

        let extension_metadata = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                b"\n[extensions.\"acme.test\"]\nenabled = true\n"
                    .iter()
                    .copied(),
            )
            .collect::<Vec<_>>();
        let mut files = valid_files();
        files[0] = (files[0].0, &extension_metadata);
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&extension_metadata));
        assert_invalid(
            &archive(&manifest, &files),
            "cannot contain extension metadata",
        );
    }

    fn blueprint_with_include(code: &str, include: &str) -> Vec<u8> {
        format!(
            r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "mixin"
includes = [{{ alias = "base", key = "blueprints/{include}" }}]
[[attributes]]
code = "name"
value_type = "string"
"#
        )
        .into_bytes()
    }

    fn blueprint_with_include_without_dependencies(code: &str) -> Vec<u8> {
        format!(
            r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#
        )
        .into_bytes()
    }

    #[test]
    fn rejects_context_resources_as_unknown_manifest_content() {
        for contexts in [
            json!([]),
            json!([resource(
                "contexts/web",
                "contexts/web.json",
                br#"{"format_version":1,"code":"web","data":{}}"#,
            )]),
        ] {
            let mut manifest = manifest_value();
            manifest["resources"]["contexts"] = contexts;
            assert_invalid(
                &archive(&manifest, &valid_files()),
                "not a valid strict v1 manifest",
            );
        }
    }

    #[test]
    fn planner_rewrites_portable_references_and_orders_dependencies() {
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files()))
            .unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();

        assert!(plan.ready);
        assert_eq!(
            plan.actions
                .iter()
                .map(|action| action.logical_key.as_str())
                .collect::<Vec<_>>(),
            ["blueprints/category", "blueprints/product"]
        );
        assert!(plan.actions.iter().all(|action| action.action == "create"));
        let product = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        let definition = product.normalized_payload.as_ref().unwrap()["definition"]
            .as_str()
            .unwrap();
        assert!(definition.contains("code = \"ecom_product\""));
        assert!(definition.contains("target_blueprint = \"ecom_category\""));
        assert_eq!(
            product.normalized_payload.as_ref().unwrap()["publication"],
            "publish"
        );
        let category_definition = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/category")
            .unwrap()
            .normalized_payload
            .as_ref()
            .unwrap()["definition"]
            .as_str()
            .unwrap();
        assert!(category_definition.contains("source_blueprint = \"ecom_product\""));
        assert!(category_definition.contains("target_blueprint = \"ecom_product\""));
    }

    #[test]
    fn planner_rewrites_portable_includes_to_mapped_revision_one() {
        let entity = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{ alias = "base", key = "blueprints/base" }]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
from = "base.name"
"#;
        let mixin = br#"format_version = 1
code = "base"
name = "Base"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#;
        let manifest = json!({
            "manifest_version": 1,
            "id": "attricat.includes",
            "name": "Includes",
            "version": "1.0.0",
            "description": "Include rewrite",
            "catalog": {"host_api": "^1.0"},
            "resources": {"blueprints": [
                resource("blueprints/product", "blueprints/product.toml", entity),
                resource("blueprints/base", "blueprints/base.toml", mixin)
            ]}
        });
        let files = [
            ("blueprints/product.toml", entity.as_slice()),
            ("blueprints/base.toml", mixin.as_slice()),
        ];
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "mapped",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        assert_eq!(plan.actions[0].logical_key, "blueprints/base");
        let definition = plan.actions[1].normalized_payload.as_ref().unwrap()["definition"]
            .as_str()
            .unwrap();
        assert!(definition.contains("code = \"mapped_base\""));
        assert!(definition.contains("version = 1"));
        assert!(!definition.contains("key = \"blueprints/base\""));

        let base_definition = plan.actions[0].normalized_payload.as_ref().unwrap()["definition"]
            .as_str()
            .unwrap();
        let existing_id = uuid::Uuid::from_u128(42);
        let reused = build_solution_pack_plan(
            &pack,
            "mapped",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned(), "mapped_base".to_owned()]),
                existing_blueprints: BTreeMap::from([(
                    "blueprints/base".to_owned(),
                    ExistingBlueprintSnapshot {
                        id: existing_id,
                        code: "mapped_base".to_owned(),
                        version: 7,
                        kind: "mixin".to_owned(),
                        canonical_definition_hash: catalog_blueprint::raw_hash(base_definition),
                        definition_hash: catalog_blueprint::raw_hash(base_definition),
                    },
                )]),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        assert!(reused.ready);
        assert_eq!(reused.actions[0].action, "map");
        assert_eq!(reused.actions[0].reason_code, "exact_blueprint_match");
        assert!(reused.actions[0].normalized_payload.is_none());
        assert_eq!(reused.mappings[0].mapping_kind, "existing");
        assert_eq!(reused.mappings[0].target_id, existing_id);
        assert_eq!(reused.mappings[0].target_version, Some(7));
        let dependent = reused.actions[1].normalized_payload.as_ref().unwrap()["definition"]
            .as_str()
            .unwrap();
        assert!(dependent.contains("version = 7"));

        let incompatible = build_solution_pack_plan(
            &pack,
            "mapped",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["mapped_base".to_owned()]),
                existing_blueprints: BTreeMap::from([(
                    "blueprints/base".to_owned(),
                    ExistingBlueprintSnapshot {
                        id: existing_id,
                        code: "mapped_base".to_owned(),
                        version: 7,
                        kind: "mixin".to_owned(),
                        canonical_definition_hash: "0".repeat(64),
                        definition_hash: "0".repeat(64),
                    },
                )]),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        assert!(!incompatible.ready);
        assert_eq!(incompatible.actions[0].action, "conflict");
        assert_eq!(
            incompatible.actions[0].reason_code,
            "existing_blueprint_incompatible"
        );
    }

    #[test]
    fn rejects_invalid_explicit_blueprint_mapping_requests() {
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files()))
            .unwrap();
        assert!(
            validate_blueprint_mapping_requests(
                &pack,
                &[
                    BlueprintMappingRequest {
                        key: "blueprints/product".to_owned(),
                        code: "shared".to_owned(),
                    },
                    BlueprintMappingRequest {
                        key: "blueprints/product".to_owned(),
                        code: "other".to_owned(),
                    },
                ],
            )
            .unwrap_err()
            .to_string()
            .contains("duplicate")
        );
        assert!(
            validate_blueprint_mapping_requests(
                &pack,
                &vec![
                    BlueprintMappingRequest {
                        key: "blueprints/product".to_owned(),
                        code: "shared".to_owned(),
                    };
                    MAX_SOLUTION_PACK_BLUEPRINTS + 1
                ],
            )
            .unwrap_err()
            .to_string()
            .contains("too many")
        );
        for request in [
            BlueprintMappingRequest {
                key: "blueprints/unknown".to_owned(),
                code: "shared".to_owned(),
            },
            BlueprintMappingRequest {
                key: "blueprints/product".to_owned(),
                code: "Unsafe-code".to_owned(),
            },
        ] {
            assert!(validate_blueprint_mapping_requests(&pack, &[request]).is_err());
        }
    }

    #[test]
    fn fully_mapped_resource_does_not_validate_an_unused_generated_code() {
        let suffix = "a".repeat(117);
        let key = format!("blueprints/{suffix}");
        assert_eq!(key.len(), MAX_IDENTIFIER_BYTES);
        let blueprint = format!(
            r#"format_version = 1
code = "{suffix}"
name = "Portable"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
        )
        .into_bytes();
        let manifest = json!({
            "manifest_version": 1,
            "id": "attricat.long-key",
            "name": "Long key",
            "version": "1.0.0",
            "description": "Mapped long logical key",
            "catalog": {"host_api": "^1.0"},
            "resources": {"blueprints": [resource(&key, "blueprints/long.toml", &blueprint)]}
        });
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(
            &manifest,
            &[("blueprints/long.toml", blueprint.as_slice())],
        ))
        .unwrap();
        let mut canonical: toml::Value =
            toml::from_str(std::str::from_utf8(&blueprint).unwrap()).unwrap();
        canonical
            .as_table_mut()
            .unwrap()
            .insert("code".to_owned(), toml::Value::String("shared".to_owned()));
        let definition = toml::to_string(&canonical).unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "a2345678901234567890123456789012",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["shared".to_owned()]),
                existing_blueprints: BTreeMap::from([(
                    key.clone(),
                    ExistingBlueprintSnapshot {
                        id: uuid::Uuid::from_u128(102),
                        code: "shared".to_owned(),
                        version: 3,
                        kind: "entity".to_owned(),
                        canonical_definition_hash: catalog_blueprint::raw_hash(&definition),
                        definition_hash: catalog_blueprint::raw_hash(&definition),
                    },
                )]),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::from(["shared".to_owned()]),
            },
        )
        .unwrap();
        assert!(plan.ready);
        assert_eq!(plan.actions[0].action, "map");
    }

    #[test]
    fn planner_rejects_unsafe_prefixes_and_reports_collisions() {
        for prefix in [
            "",
            "Bad",
            "bad-",
            "bad_",
            "a23456789012345678901234567890123",
        ] {
            assert!(validate_plan_prefix(prefix).is_err(), "accepted {prefix:?}");
        }
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files()))
            .unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned(), "ecom_product".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        assert!(!plan.ready);
        let product = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(product.action, "conflict");
        assert_eq!(product.reason_code, "target_code_exists");
    }

    #[test]
    fn planner_blocks_required_resources_when_optional_dependencies_are_skipped() {
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][1]["required"] = json!(false);
        let pack =
            ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &valid_files())).unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        let category = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/category")
            .unwrap();
        let product = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(
            (category.action, category.reason_code),
            ("skip", "optional_not_selected")
        );
        assert_eq!(
            (product.action, product.reason_code),
            ("blocked", "dependency_not_creatable")
        );
        assert!(!plan.ready);

        let required_pack =
            ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files()))
                .unwrap();
        let baseline = build_solution_pack_plan(
            &required_pack,
            "ecom",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::new(),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            },
        )
        .unwrap();
        let product_definition = baseline
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap()
            .normalized_payload
            .as_ref()
            .unwrap()["definition"]
            .as_str()
            .unwrap();
        let mapped = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["ecom_product".to_owned()]),
                existing_blueprints: BTreeMap::from([(
                    "blueprints/product".to_owned(),
                    ExistingBlueprintSnapshot {
                        id: uuid::Uuid::from_u128(99),
                        code: "ecom_product".to_owned(),
                        version: 1,
                        kind: "entity".to_owned(),
                        canonical_definition_hash: catalog_blueprint::raw_hash(product_definition),
                        definition_hash: catalog_blueprint::raw_hash(product_definition),
                    },
                )]),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::from(["ecom_product".to_owned()]),
            },
        )
        .unwrap();
        let mapped_product = mapped
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(
            (mapped_product.action, mapped_product.reason_code),
            ("blocked", "dependency_not_creatable")
        );
        assert!(!mapped.ready);
    }

    #[test]
    fn mapped_blueprints_are_blocked_by_skipped_include_relationship_and_view_dependencies() {
        let cases = [
            (
                "include",
                br#"format_version = 1
code = "main"
name = "Main"
kind = "mixin"
includes = [{ alias = "base", key = "blueprints/dep" }]
[[attributes]]
code = "name"
from = "base.name"
"#
                .as_slice(),
                br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#
                .as_slice(),
                "mixin",
            ),
            (
                "relationship",
                br#"format_version = 1
code = "main"
name = "Main"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "dep"
value_type = "relationship"
target_blueprint = "blueprints/dep"
"#
                .as_slice(),
                br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
                .as_slice(),
                "entity",
            ),
            (
                "view",
                br#"format_version = 1
code = "main"
name = "Main"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.detail]
type = "stack"
[[views.detail.children]]
type = "tabs"
[[views.detail.children.tabs]]
label = "Relationships"
[[views.detail.children.tabs.children]]
type = "grid"
[[views.detail.children.tabs.children.children]]
type = "incoming_relationship_list"
label = "Dependencies"
relationships = [{ source_blueprint = "blueprints/dep", field = "main_ref" }]
page_size = 10
[[attributes]]
code = "name"
value_type = "string"
"#
                .as_slice(),
                br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "main_ref"
value_type = "relationship"
target_blueprint = "blueprints/main"
"#
                .as_slice(),
                "entity",
            ),
        ];
        for (case, main, dep, main_kind) in cases {
            let manifest_for = |dep_required| {
                let mut dep_resource = resource("blueprints/dep", "blueprints/dep.toml", dep);
                dep_resource["required"] = json!(dep_required);
                json!({
                    "manifest_version": 1,
                    "id": format!("attricat.dependency-{case}"),
                    "name": "Dependency",
                    "version": "1.0.0",
                    "description": "Mapped dependency closure",
                    "catalog": {"host_api": "^1.0"},
                    "resources": {"blueprints": [
                        resource("blueprints/main", "blueprints/main.toml", main),
                        dep_resource
                    ]}
                })
            };
            let files = [("blueprints/main.toml", main), ("blueprints/dep.toml", dep)];
            let required_pack =
                ValidatedSolutionPack::from_tar_zst(&archive(&manifest_for(true), &files))
                    .unwrap_or_else(|error| panic!("{case}: {error}"));
            let empty_workspace = PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::new(),
                existing_blueprints: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
            };
            let baseline = build_solution_pack_plan(
                &required_pack,
                "deps",
                BlueprintPublication::Draft,
                &empty_workspace,
            )
            .unwrap();
            let main_definition = baseline
                .actions
                .iter()
                .find(|action| action.logical_key == "blueprints/main")
                .unwrap()
                .normalized_payload
                .as_ref()
                .unwrap()["definition"]
                .as_str()
                .unwrap();
            let optional_pack =
                ValidatedSolutionPack::from_tar_zst(&archive(&manifest_for(false), &files))
                    .unwrap();
            let mapped = build_solution_pack_plan(
                &optional_pack,
                "deps",
                BlueprintPublication::Draft,
                &PlanningWorkspaceSnapshot {
                    workspace_id: uuid::Uuid::nil(),
                    physical_codes: BTreeSet::from(["deps_main".to_owned()]),
                    existing_blueprints: BTreeMap::from([(
                        "blueprints/main".to_owned(),
                        ExistingBlueprintSnapshot {
                            id: uuid::Uuid::new_v4(),
                            code: "deps_main".to_owned(),
                            version: 1,
                            kind: main_kind.to_owned(),
                            canonical_definition_hash: catalog_blueprint::raw_hash(main_definition),
                            definition_hash: catalog_blueprint::raw_hash(main_definition),
                        },
                    )]),
                    installed_extensions: BTreeMap::new(),
                    explore_navigation: Vec::new(),
                    explore_navigation_valid: true,
                    extension_layout: json!({"version":1,"outlets":{}}),
                    extension_layout_valid: true,
                    role_codes: BTreeSet::new(),
                    published_entity_codes: BTreeSet::from(["deps_main".to_owned()]),
                },
            )
            .unwrap();
            let main_action = mapped
                .actions
                .iter()
                .find(|action| action.logical_key == "blueprints/main")
                .unwrap();
            assert_eq!(
                (main_action.action, main_action.reason_code),
                ("blocked", "dependency_not_creatable"),
                "{case}"
            );
            assert!(!mapped.ready, "{case}");
        }
    }

    #[test]
    fn planner_blocks_draft_relationship_paths_and_orders_publish_targets() {
        let product = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "category.name" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "blueprints/category"
"#;
        let category = br#"format_version = 1
code = "category"
name = "Category"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#;
        let manifest = json!({
            "manifest_version": 1,
            "id": "attricat.paths",
            "name": "Paths",
            "version": "1.0.0",
            "description": "Relationship table paths",
            "catalog": {"host_api": "^1.0"},
            "resources": {"blueprints": [
                resource("blueprints/product", "blueprints/product.toml", product),
                resource("blueprints/category", "blueprints/category.toml", category)
            ]}
        });
        let files = [
            ("blueprints/product.toml", product.as_slice()),
            ("blueprints/category.toml", category.as_slice()),
        ];
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        let workspace = PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
        };

        let draft =
            build_solution_pack_plan(&pack, "paths", BlueprintPublication::Draft, &workspace)
                .unwrap();
        let product_action = draft
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(
            (product_action.action, product_action.reason_code),
            ("blocked", "draft_table_path_target_unpublished")
        );
        assert!(!draft.ready);

        let publish =
            build_solution_pack_plan(&pack, "paths", BlueprintPublication::Publish, &workspace)
                .unwrap();
        assert!(publish.ready);
        assert_eq!(
            publish
                .actions
                .iter()
                .map(|action| action.logical_key.as_str())
                .collect::<Vec<_>>(),
            ["blueprints/category", "blueprints/product"]
        );

        let cyclic_category = br#"format_version = 1
code = "category"
name = "Category"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "product.name" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "product"
value_type = "relationship"
target_blueprint = "blueprints/product"
"#;
        let mut cyclic_manifest = manifest;
        cyclic_manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(cyclic_category));
        let cyclic_files = [
            ("blueprints/product.toml", product.as_slice()),
            ("blueprints/category.toml", cyclic_category.as_slice()),
        ];
        let cyclic_pack =
            ValidatedSolutionPack::from_tar_zst(&archive(&cyclic_manifest, &cyclic_files)).unwrap();
        let error = build_solution_pack_plan(
            &cyclic_pack,
            "paths",
            BlueprintPublication::Publish,
            &workspace,
        )
        .unwrap_err();
        assert!(error.to_string().contains("dependencies contain a cycle"));

        let mut mapping_graph = BTreeMap::new();
        for (key, code, version, id) in [
            ("blueprints/category", "existing_category", 4, 103_u128),
            ("blueprints/product", "existing_product", 6, 104_u128),
        ] {
            mapping_graph.insert(
                key.to_owned(),
                PlannedMapping {
                    resource_kind: "blueprint",
                    logical_key: key.to_owned(),
                    target_id: uuid::Uuid::from_u128(id),
                    target_code: code.to_owned(),
                    target_version: Some(version),
                    mapping_kind: "existing",
                    snapshot: json!({}),
                },
            );
        }
        let mut mapped_workspace = PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from([
                "existing_category".to_owned(),
                "existing_product".to_owned(),
            ]),
            existing_blueprints: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::from([
                "existing_category".to_owned(),
                "existing_product".to_owned(),
            ]),
        };
        for (key, code, version, id) in [
            ("blueprints/category", "existing_category", 4, 103_u128),
            ("blueprints/product", "existing_product", 6, 104_u128),
        ] {
            let payload = normalized_blueprint_payload(
                cyclic_pack.blueprint(key).unwrap(),
                &mapping_graph,
                BlueprintPublication::Publish,
                &HashSet::new(),
                &mapped_workspace,
                cyclic_pack.manifest(),
            )
            .unwrap();
            mapped_workspace.existing_blueprints.insert(
                key.to_owned(),
                ExistingBlueprintSnapshot {
                    id: uuid::Uuid::from_u128(id),
                    code: code.to_owned(),
                    version,
                    kind: "entity".to_owned(),
                    canonical_definition_hash: catalog_blueprint::raw_hash(
                        payload["definition"].as_str().unwrap(),
                    ),
                    definition_hash: catalog_blueprint::raw_hash(
                        payload["definition"].as_str().unwrap(),
                    ),
                },
            );
        }
        let mapped = build_solution_pack_plan(
            &cyclic_pack,
            "paths",
            BlueprintPublication::Publish,
            &mapped_workspace,
        )
        .unwrap();
        assert!(mapped.ready);
        assert!(mapped.actions.iter().all(|action| action.action == "map"));
    }

    #[test]
    fn planner_uses_workspace_wide_codes_and_deterministic_target_ids() {
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files()))
            .unwrap();
        let workspace = PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::from_u128(1),
            physical_codes: BTreeSet::from(["default".to_owned(), "ecom_product".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
        };
        let first =
            build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace)
                .unwrap();
        let second =
            build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace)
                .unwrap();

        assert_eq!(
            first
                .mappings
                .iter()
                .map(|mapping| mapping.target_id)
                .collect::<Vec<_>>(),
            second
                .mappings
                .iter()
                .map(|mapping| mapping.target_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            first
                .actions
                .iter()
                .map(|action| &action.normalized_payload)
                .collect::<Vec<_>>(),
            second
                .actions
                .iter()
                .map(|action| &action.normalized_payload)
                .collect::<Vec<_>>()
        );
        let product = first
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(
            (product.action, product.reason_code),
            ("conflict", "target_code_exists")
        );
    }

    #[test]
    fn rejects_unsafe_workspace_dependent_blueprint_constructs_and_allows_layouts() {
        let role = PRODUCT_BLUEPRINT.replace_ascii(
            b"kind = \"entity\"",
            b"kind = \"entity\"\n[publication]\nretain_on_edit_roles = [\"editor\"]",
        );
        assert_blueprint_error(&role, "cannot declare workspace roles");

        let renderer = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                br#"
[views.table]
type = "table"
columns = [{ field = "name", renderer = { id = "acme.custom", version = 1 } }]
"#
                .iter()
                .copied(),
            )
            .collect::<Vec<_>>();
        assert_blueprint_error(&renderer, "cannot declare extension table renderers");

        let layout = PRODUCT_BLUEPRINT
            .iter()
            .copied()
            .chain(
                br#"
[views.extension_layout]
type = "extension_layout"
version = 1
outlets = {}
"#
                .iter()
                .copied(),
            )
            .collect::<Vec<_>>();
        let mut files = valid_files();
        files[0] = (files[0].0, &layout);
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&layout));
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    }

    trait ReplaceAscii {
        fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
    }

    impl ReplaceAscii for [u8] {
        fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
            let text = std::str::from_utf8(self).unwrap();
            text.replace(
                std::str::from_utf8(from).unwrap(),
                std::str::from_utf8(to).unwrap(),
            )
            .into_bytes()
        }
    }
}
