//! Strict, side-effect-free validation for solution-pack v1 archives.
//!
//! A validated pack is still only portable input to a future workspace plan. This
//! module neither persists a pack nor applies its resources.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::{Cursor, Read},
    path::{Component, Path},
};

use catalog_blueprint::BlueprintKind;
use catalog_validation::is_valid_code;
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

use crate::{
    extensions::valid_contribution_key,
    solution_pack_sample_data::{
        ValidatedSampleData, explicit_fact_attribute_codes, prohibited_attribute_code,
        prohibited_numeric, prohibited_scalar, validate_sample_data,
    },
    solution_pack_seeds::{
        SeedContext, SeedRule, SeedSavedSearch, SeedWorkflow, SolutionPackBlueprintReuse,
        SolutionPackPrerequisite, ValidatedSeeds, seed_resources, validate_seed_content,
        validate_seed_manifest,
    },
};

mod blueprints;
mod guidance;
mod plan;
mod presentation_assets;
#[cfg(test)]
mod tests;

use blueprints::validate_content;
use guidance::{validate_configuration_templates, validate_guidance_and_checks};
use presentation_assets::{validate_presentation_asset_resource, validate_presentation_assets};

pub use guidance::json_deep_contains;
pub use plan::*;
pub use presentation_assets::validate_presentation_asset_bytes;

pub const SOLUTION_PACK_MANIFEST_VERSION: u32 = 1;
pub const SOLUTION_PACK_RESOURCE_FORMAT_VERSION: u32 = 1;
pub const SOLUTION_PACK_MANIFEST_PATH: &str = "solution-pack.json";
pub const MAX_SOLUTION_PACK_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_EXPANDED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_SOLUTION_PACK_ARCHIVE_ENTRIES: usize = 256;
pub const MAX_SOLUTION_PACK_BLUEPRINTS: usize = 64;
pub const MAX_SOLUTION_PACK_PRESENTATION_ASSETS: usize = 64;
pub const MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_PRESENTATION_ASSET_TOTAL_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_SVG_BYTES: usize = 256 * 1024;
pub const MAX_SOLUTION_PACK_ASSET_DIMENSION: u32 = 4096;
pub const MAX_SOLUTION_PACK_ASSET_PIXELS: u64 = 16_000_000;
pub const MAX_SOLUTION_PACK_WORKSPACE_SETTINGS: usize = 3;
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
pub(crate) const MAX_IDENTIFIER_BYTES: usize = 128;
pub(crate) const MAX_NAME_BYTES: usize = 200;
const MAX_DESCRIPTION_BYTES: usize = 4096;
pub(crate) const MAX_VERSION_REQUIREMENT_BYTES: usize = 256;

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
    /// Seeds that must already be applied; see [`SolutionPackPrerequisite`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<SolutionPackPrerequisite>,
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
    #[serde(default)]
    pub presentation_assets: Vec<SolutionPackPresentationAssetResource>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub sample_data: Option<SolutionPackSampleDataResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<SolutionPackResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflows: Vec<SolutionPackResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_searches: Vec<SolutionPackResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contexts: Vec<SolutionPackResource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackSampleDataResource {
    pub key: String,
    pub path: String,
    pub sha256: String,
    /// Bundled files that sample entities attach through ordinary file storage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SolutionPackFileRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackPresentationAssetResource {
    pub key: String,
    pub path: String,
    pub required: bool,
    pub purpose: String,
    pub media_type: String,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct NormalizedPresentationAsset {
    pub bytes: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct ValidatedPresentationAsset {
    pub key: String,
    pub purpose: String,
    pub media_type: String,
    pub source_sha256: String,
    pub source_byte_size: usize,
    pub stored_sha256: String,
    pub stored_bytes: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackResource {
    pub key: String,
    pub path: String,
    pub required: bool,
    pub sha256: String,
    /// Blueprints only: reuse the exact blueprint a prerequisite seed installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse: Option<SolutionPackBlueprintReuse>,
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

/// Translations for `{{…}}` references in the pack's catalog labels, applied
/// as solution-pack lexicon entries that workspace entries override.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackLexicon {
    pub format_version: u32,
    pub kind: String,
    pub languages: Vec<SolutionPackLexiconLanguage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionPackLexiconLanguage {
    pub language: String,
    pub entries: Vec<catalog_lexicon::LexiconFileEntry>,
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
    lexicon: Option<Vec<catalog_lexicon::Entry>>,
    configuration_templates: BTreeMap<String, Value>,
    presentation_assets: BTreeMap<String, ValidatedPresentationAsset>,
    sample_data: Option<ValidatedSampleData>,
    guidance: SolutionPackGuidance,
    checks: Vec<SolutionPackCheckDefinition>,
    seeds: ValidatedSeeds,
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
    effective_attributes: Vec<catalog_blueprint::EffectiveAttribute>,
    unique_keys: Vec<catalog_blueprint::UniqueKeyDefinition>,
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

    pub fn effective_attributes(&self) -> &[catalog_blueprint::EffectiveAttribute] {
        &self.effective_attributes
    }

    pub fn unique_keys(&self) -> &[catalog_blueprint::UniqueKeyDefinition] {
        &self.unique_keys
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
        let seeds = validate_seed_content(&manifest, &files, &blueprints)?;
        let explore_navigation = validate_explore_navigation(&manifest, &files, &blueprints)?;
        let extension_layout = validate_extension_layout(&manifest, &files)?;
        let lexicon = validate_lexicon(&manifest, &files)?;
        validate_blueprint_extension_layouts(&manifest, &blueprints)?;
        let configuration_templates = validate_configuration_templates(&manifest, &files)?;
        let presentation_assets = validate_presentation_assets(&manifest, &files)?;
        let sample_data = validate_sample_resource(&manifest, &files, &blueprints, &seeds)?;
        let (guidance, checks) =
            validate_guidance_and_checks(&manifest, &files, &extension_layout)?;

        Ok(Self {
            manifest,
            archive_sha256: sha256_hex(archive),
            files,
            blueprints,
            explore_navigation,
            extension_layout,
            lexicon,
            configuration_templates,
            presentation_assets,
            sample_data,
            guidance,
            checks,
            seeds,
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

    /// Validated, normalized lexicon entries across all languages.
    pub fn lexicon(&self) -> Option<&[catalog_lexicon::Entry]> {
        self.lexicon.as_deref()
    }

    pub fn configuration_template(&self, key: &str) -> Option<&Value> {
        self.configuration_templates.get(key)
    }

    pub fn presentation_asset(&self, key: &str) -> Option<&ValidatedPresentationAsset> {
        self.presentation_assets.get(key)
    }

    pub fn presentation_assets(&self) -> impl Iterator<Item = &ValidatedPresentationAsset> {
        self.presentation_assets.values()
    }

    pub fn sample_data(&self) -> Option<&ValidatedSampleData> {
        self.sample_data.as_ref()
    }

    pub fn guidance(&self) -> &SolutionPackGuidance {
        &self.guidance
    }

    pub fn checks(&self) -> &[SolutionPackCheckDefinition] {
        &self.checks
    }

    /// A declared archive file, such as a bundled sample file.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    pub fn rule(&self, key: &str) -> Option<&SeedRule> {
        self.seeds.rules.get(key)
    }

    pub fn workflow(&self, key: &str) -> Option<&SeedWorkflow> {
        self.seeds.workflows.get(key)
    }

    pub fn saved_search(&self, key: &str) -> Option<&SeedSavedSearch> {
        self.seeds.saved_searches.get(key)
    }

    pub fn context(&self, key: &str) -> Option<&SeedContext> {
        self.seeds.contexts.get(key)
    }

    pub fn contexts(&self) -> impl Iterator<Item = &SeedContext> {
        self.seeds.contexts.values()
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

    if manifest.resources.blueprints.is_empty()
        && manifest.resources.workspace_settings.is_empty()
        && manifest.resources.presentation_assets.is_empty()
        && manifest.resources.sample_data.is_none()
        && seed_resources(manifest).next().is_none()
    {
        return invalid(
            "at least one blueprint, workspace setting, presentation asset, sample-data, rule, workflow, saved-search, or context resource is required",
        );
    }
    if manifest.resources.blueprints.len() > MAX_SOLUTION_PACK_BLUEPRINTS {
        return invalid("solution-pack manifest declares too many blueprints");
    }
    if manifest.resources.presentation_assets.len() > MAX_SOLUTION_PACK_PRESENTATION_ASSETS {
        return invalid("solution-pack manifest declares too many presentation assets");
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
    for resource in &manifest.resources.presentation_assets {
        validate_presentation_asset_resource(resource)?;
        if !keys.insert(&resource.key) {
            return invalid(format!("duplicate resource key '{}'", resource.key));
        }
        if !paths.insert(&resource.path) {
            return invalid(format!("duplicate resource path '{}'", resource.path));
        }
    }
    if let Some(resource) = &manifest.resources.sample_data {
        if resource.key != "sample-data/default" || resource.path != "sample-data/sample-data.json"
        {
            return invalid("sample-data must use the fixed key and path");
        }
        parse_sha256(&resource.sha256).map_err(|()| {
            SolutionPackError::Invalid(
                "sample-data sha256 must be 64 lowercase hexadecimal characters".into(),
            )
        })?;
        if !keys.insert(&resource.key) || !paths.insert(&resource.path) {
            return invalid("duplicate sample-data key or path");
        }
        if resource.files.len() > crate::solution_pack_sample_data::MAX_SAMPLE_FILES {
            return invalid("sample-data declares too many files");
        }
        for file in &resource.files {
            if !crate::solution_pack_sample_data::valid_sample_file_path(&file.path) {
                return invalid(format!("sample-data file path '{}' is invalid", file.path));
            }
            parse_sha256(&file.sha256).map_err(|()| {
                SolutionPackError::Invalid(format!(
                    "sample-data file '{}' sha256 must be 64 lowercase hexadecimal characters",
                    file.path
                ))
            })?;
            if !paths.insert(&file.path) {
                return invalid(format!("duplicate resource path '{}'", file.path));
            }
        }
    }
    validate_seed_manifest(manifest)?;
    for (_, _, resource) in seed_resources(manifest) {
        if !keys.insert(&resource.key) {
            return invalid(format!("duplicate resource key '{}'", resource.key));
        }
        if !paths.insert(&resource.path) {
            return invalid(format!("duplicate resource path '{}'", resource.path));
        }
    }
    for prerequisite in &manifest.prerequisites {
        if !keys.insert(&prerequisite.key) {
            return invalid(format!("duplicate resource key '{}'", prerequisite.key));
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
    if kind != "blueprints" && resource.reuse.is_some() {
        return invalid(format!(
            "resource '{}' cannot declare blueprint reuse",
            resource.key
        ));
    }
    if kind == "workspace_settings" {
        let fixed_pair = matches!(
            (resource.key.as_str(), resource.path.as_str()),
            (
                "workspace/explore-navigation",
                "workspace/explore-navigation.json"
            ) | (
                "workspace/extension-layout",
                "workspace/extension-layout.json"
            ) | ("workspace/lexicon", "workspace/lexicon.json")
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
                .resources
                .presentation_assets
                .iter()
                .map(|resource| resource.path.as_str()),
        )
        .chain(
            manifest
                .resources
                .sample_data
                .iter()
                .map(|resource| resource.path.as_str()),
        )
        .chain(
            manifest
                .resources
                .sample_data
                .iter()
                .flat_map(|resource| &resource.files)
                .map(|file| file.path.as_str()),
        )
        .chain(seed_resources(manifest).map(|(_, _, resource)| resource.path.as_str()))
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
        .chain(
            manifest
                .resources
                .presentation_assets
                .iter()
                .map(|resource| {
                    (
                        resource.key.as_str(),
                        resource.path.as_str(),
                        resource.sha256.as_str(),
                    )
                }),
        )
        .chain(manifest.resources.sample_data.iter().map(|resource| {
            (
                resource.key.as_str(),
                resource.path.as_str(),
                resource.sha256.as_str(),
            )
        }))
        .chain(
            manifest
                .resources
                .sample_data
                .iter()
                .flat_map(|resource| &resource.files)
                .map(|file| (file.path.as_str(), file.path.as_str(), file.sha256.as_str())),
        )
        .chain(seed_resources(manifest).map(|(_, _, resource)| {
            (
                resource.key.as_str(),
                resource.path.as_str(),
                resource.sha256.as_str(),
            )
        }))
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

fn validate_sample_resource(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
    seeds: &ValidatedSeeds,
) -> Result<Option<ValidatedSampleData>, SolutionPackError> {
    let Some(resource) = &manifest.resources.sample_data else {
        return Ok(None);
    };
    let declared_blueprints = manifest
        .resources
        .blueprints
        .iter()
        .map(|blueprint| blueprint.key.as_str())
        .collect::<BTreeSet<_>>();
    let declared_contexts = seeds
        .contexts
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let declared_files = resource
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let sample = validate_sample_data(
        &files[&resource.path],
        &declared_blueprints,
        &declared_contexts,
        &declared_files,
    )
    .map_err(SolutionPackError::Invalid)?;
    for (path, file) in &sample.files {
        crate::solution_pack_sample_data::validate_sample_file_bytes(
            &file.media_type,
            &files[path],
        )
        .map_err(|error| SolutionPackError::Invalid(format!("'{path}': {error}")))?;
    }
    validate_effective_sample_facts(&sample, blueprints, files)?;
    crate::solution_pack_sample_data::validate_sample_unique_keys(
        &sample,
        blueprints,
        &seeds.contexts,
    )
    .map_err(SolutionPackError::Invalid)?;
    Ok(Some(sample))
}

fn validate_effective_sample_facts(
    sample: &ValidatedSampleData,
    blueprints: &BTreeMap<String, SolutionPackBlueprint>,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), SolutionPackError> {
    for entity in &sample.declaration.entities {
        let blueprint = &blueprints[&entity.blueprint];
        if blueprint.kind() != BlueprintKind::Entity {
            return invalid(format!(
                "sample entity '{}' must reference an entity blueprint",
                entity.key
            ));
        }
        let attributes = blueprint
            .effective_attributes()
            .iter()
            .map(|attribute| (attribute.code.as_str(), attribute))
            .collect::<HashMap<_, _>>();
        let explicit_attributes = explicit_fact_attribute_codes(entity);
        for attribute in blueprint.effective_attributes() {
            prohibited_attribute_code(&attribute.code).map_err(SolutionPackError::Invalid)?;
            if let Some(default) = attribute
                .default_value
                .as_ref()
                .filter(|_| !explicit_attributes.contains(attribute.code.as_str()))
            {
                validate_sample_value_for_attribute(default, attribute, true)?;
            }
        }
        for fact in &entity.facts {
            let code = fact
                .attribute
                .rsplit('/')
                .next()
                .expect("validated attribute reference");
            let attribute = attributes.get(code).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "sample entity '{}' references unknown attribute '{}'",
                    entity.key, fact.attribute
                ))
            })?;
            if attribute.readonly
                || matches!(attribute.value_type.as_str(), "relationship" | "file")
            {
                return invalid(format!(
                    "sample entity '{}' fact '{}' is read-only or non-scalar",
                    entity.key, fact.attribute
                ));
            }
            // Status values follow workspace transitions and principals name
            // workspace users or teams, so samples leave them to defaults.
            if attribute.value_schema.as_ref().is_some_and(|schema| {
                schema.get(catalog_validation::status::STATUS_KEY).is_some()
                    || schema
                        .get(catalog_validation::principal::PRINCIPAL_KEY)
                        .is_some()
            }) {
                return invalid(format!(
                    "sample entity '{}' cannot set status or principal attribute '{}'",
                    entity.key, fact.attribute
                ));
            }
            validate_sample_context_editable(&entity.key, fact.context.as_ref(), attribute)?;
            validate_sample_value_for_attribute(&fact.value, attribute, false)?;
        }
        for relationship in &entity.relationships {
            let code = relationship
                .attribute
                .rsplit('/')
                .next()
                .expect("validated attribute reference");
            let attribute = attributes.get(code).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "sample entity '{}' references unknown relationship '{}'",
                    entity.key, relationship.attribute
                ))
            })?;
            if attribute.readonly || attribute.value_type != "relationship" {
                return invalid(format!(
                    "sample entity '{}' relationship '{}' is not writable relationship data",
                    entity.key, relationship.attribute
                ));
            }
            if attribute.cardinality.as_deref() == Some("one") && relationship.targets.len() != 1 {
                return invalid(format!(
                    "sample entity '{}' relationship '{}' exceeds cardinality one",
                    entity.key, relationship.attribute
                ));
            }
            validate_sample_context_editable(
                &entity.key,
                relationship.context.as_ref(),
                attribute,
            )?;
        }
        for value in &entity.files {
            let code = value
                .attribute
                .rsplit('/')
                .next()
                .expect("validated attribute reference");
            let attribute = attributes.get(code).ok_or_else(|| {
                SolutionPackError::Invalid(format!(
                    "sample entity '{}' references unknown file attribute '{}'",
                    entity.key, value.attribute
                ))
            })?;
            let policy = attribute
                .file_policy
                .as_ref()
                .filter(|_| attribute.value_type == "file" && !attribute.readonly)
                .ok_or_else(|| {
                    SolutionPackError::Invalid(format!(
                        "sample entity '{}' attribute '{}' is not a writable file attribute",
                        entity.key, value.attribute
                    ))
                })?;
            if policy.cardinality == "one" && value.files.len() != 1 {
                return invalid(format!(
                    "sample entity '{}' file attribute '{}' accepts one file",
                    entity.key, value.attribute
                ));
            }
            validate_sample_context_editable(&entity.key, value.context.as_ref(), attribute)?;
            for file in &value.files {
                if !policy.allows(
                    &file.media_type,
                    &file.filename,
                    files[&file.path].len() as u64,
                ) {
                    return invalid(format!(
                        "sample file '{}' is not allowed by the file policy of '{}'",
                        file.path, value.attribute
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_sample_context_editable(
    entity: &str,
    context: Option<&String>,
    attribute: &catalog_blueprint::EffectiveAttribute,
) -> Result<(), SolutionPackError> {
    if context.is_some() && attribute.context_editable == "default" {
        return invalid(format!(
            "sample entity '{entity}' sets attribute '{}' outside the default context",
            attribute.code
        ));
    }
    Ok(())
}

fn validate_sample_value_for_attribute(
    value: &Value,
    attribute: &catalog_blueprint::EffectiveAttribute,
    defaulted: bool,
) -> Result<(), SolutionPackError> {
    let exact_time = value.as_object().and_then(|object| {
        (object.len() == 2).then(|| {
            (
                object.get("time").and_then(Value::as_str),
                object.get("time_zone").and_then(Value::as_str),
            )
        })
    });
    let valid_type = validates_native_value(&attribute.value_type, value)
        && (value.is_object()
            == (attribute.value_type == "time"
                && exact_time
                    .is_some_and(|(time, time_zone)| time.is_some() && time_zone.is_some())));
    if !valid_type {
        return invalid(format!(
            "sample {} for attribute '{}' has the wrong native scalar type",
            if defaulted { "default" } else { "fact" },
            attribute.code
        ));
    }
    prohibited_attribute_code(&attribute.code).map_err(SolutionPackError::Invalid)?;
    match value {
        Value::String(value) if matches!(attribute.value_type.as_str(), "date" | "datetime") => {
            crate::solution_pack_sample_data::prohibited_temporal_scalar(&attribute.code, value)
                .map_err(SolutionPackError::Invalid)
        }
        Value::Object(object) => {
            let time = object["time"].as_str().expect("validated time string");
            let time_zone = object["time_zone"]
                .as_str()
                .expect("validated time-zone string");
            crate::solution_pack_sample_data::prohibited_temporal_scalar(&attribute.code, time)
                .map_err(SolutionPackError::Invalid)?;
            prohibited_scalar(&attribute.code, time_zone).map_err(SolutionPackError::Invalid)
        }
        Value::String(value) => {
            prohibited_scalar(&attribute.code, value).map_err(SolutionPackError::Invalid)
        }
        Value::Number(value) => prohibited_numeric(&attribute.code, &value.to_string())
            .map_err(SolutionPackError::Invalid),
        Value::Bool(_) => Ok(()),
        _ => unreachable!("type checked above"),
    }
}

fn validates_native_value(value_type: &str, value: &Value) -> bool {
    match value_type {
        "string" => value.is_string(),
        "number" => value
            .as_number()
            .is_some_and(|value| value.to_string().parse::<rust_decimal::Decimal>().is_ok()),
        "integer" => value.as_i64().is_some(),
        "boolean" => value.is_boolean(),
        "date" => value
            .as_str()
            .is_some_and(|value| value.parse::<chrono::NaiveDate>().is_ok()),
        "datetime" => value
            .as_str()
            .is_some_and(|value| value.parse::<chrono::DateTime<chrono::Utc>>().is_ok()),
        "time" => value.as_object().is_some_and(|value| {
            value
                .get("time")
                .and_then(Value::as_str)
                .is_some_and(|value| value.parse::<chrono::NaiveTime>().is_ok())
                && value
                    .get("time_zone")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.parse::<chrono_tz::Tz>().is_ok())
        }),
        "json" => true,
        "file" | "relationship" => false,
        _ => false,
    }
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

fn validate_lexicon(
    manifest: &SolutionPackManifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<Option<Vec<catalog_lexicon::Entry>>, SolutionPackError> {
    let Some(resource) = manifest
        .resources
        .workspace_settings
        .iter()
        .find(|resource| resource.key == "workspace/lexicon")
    else {
        return Ok(None);
    };
    let lexicon: SolutionPackLexicon =
        serde_json::from_slice(&files[&resource.path]).map_err(|_| {
            SolutionPackError::Invalid("workspace lexicon is not valid strict JSON".into())
        })?;
    if lexicon.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION {
        return invalid(format!(
            "workspace lexicon has unsupported format_version {}",
            lexicon.format_version
        ));
    }
    if lexicon.kind != "lexicon" {
        return invalid("workspace lexicon kind must be 'lexicon'");
    }
    if lexicon.languages.is_empty() {
        return invalid("workspace lexicon must contain at least one language");
    }
    let mut languages = HashSet::new();
    let mut entries = Vec::new();
    for language in lexicon.languages {
        let file = catalog_lexicon::LexiconFile {
            format_version: catalog_lexicon::LEXICON_FILE_FORMAT_VERSION,
            language: language.language,
            entries: language.entries,
        };
        let canonical = catalog_lexicon::canonical_language(&file.language).ok_or_else(|| {
            SolutionPackError::Invalid(format!(
                "workspace lexicon language '{}' is unsupported",
                file.language
            ))
        })?;
        if !languages.insert(canonical.clone()) {
            return invalid(format!(
                "workspace lexicon language '{canonical}' is duplicated"
            ));
        }
        if file.entries.is_empty() {
            return invalid(format!(
                "workspace lexicon language '{canonical}' must contain at least one entry"
            ));
        }
        entries.extend(file.into_entries().map_err(|error| {
            SolutionPackError::Invalid(format!(
                "workspace lexicon language '{canonical}' is invalid: {error}"
            ))
        })?);
        if entries.len() > catalog_lexicon::MAX_FILE_ENTRIES {
            return invalid(format!(
                "workspace lexicon must contain at most {} entries",
                catalog_lexicon::MAX_FILE_ENTRIES
            ));
        }
    }
    Ok(Some(entries))
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

pub(crate) fn validate_acyclic<'a>(
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

pub(crate) fn validate_pack_id(value: &str) -> Result<(), SolutionPackError> {
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

pub(crate) fn is_valid_stable_code(value: &str) -> bool {
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

pub(crate) fn validate_bounded_text(
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

pub(crate) fn safe_archive_path(value: &str) -> bool {
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

pub(crate) fn resource_code(key: &str) -> &str {
    key.rsplit_once('/').expect("resource key validated").1
}

pub fn parse_version_req(value: &str) -> Result<VersionReq, semver::Error> {
    VersionReq::parse(value).or_else(|original_error| {
        let comparators = value.split_ascii_whitespace().collect::<Vec<_>>();
        if comparators.len() < 2 || comparators.iter().any(|item| item.contains(',')) {
            return Err(original_error);
        }
        VersionReq::parse(&comparators.join(", "))
    })
}

pub(crate) fn parse_sha256(value: &str) -> Result<[u8; 32], ()> {
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

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

pub(crate) fn invalid<T>(message: impl Into<String>) -> Result<T, SolutionPackError> {
    Err(SolutionPackError::Invalid(message.into()))
}
