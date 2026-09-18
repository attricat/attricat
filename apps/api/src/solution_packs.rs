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
    BlueprintDefinition, BlueprintKind, CompiledBlueprint, ResolvedInclude, ViewDefinition,
    ViewNode,
};
use catalog_validation::is_valid_code;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

pub const SOLUTION_PACK_MANIFEST_VERSION: u32 = 1;
pub const SOLUTION_PACK_RESOURCE_FORMAT_VERSION: u32 = 1;
pub const SOLUTION_PACK_MANIFEST_PATH: &str = "solution-pack.json";
pub const SYSTEM_DEFAULT_CONTEXT_KEY: &str = "system/default";
pub const MAX_SOLUTION_PACK_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_EXPANDED_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOLUTION_PACK_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_SOLUTION_PACK_ARCHIVE_ENTRIES: usize = 256;
const MAX_ARCHIVE_PATH_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_NAME_BYTES: usize = 200;
const MAX_DESCRIPTION_BYTES: usize = 4096;

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
    pub contexts: Vec<SolutionPackResource>,
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
pub struct SolutionPackContext {
    pub format_version: u32,
    pub code: String,
    pub data: Value,
    pub parent: String,
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
    contexts: BTreeMap<String, SolutionPackContext>,
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
        let (blueprints, contexts) = validate_content(&manifest, &files)?;

        Ok(Self {
            manifest,
            archive_sha256: sha256_hex(archive),
            files,
            blueprints,
            contexts,
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

    pub fn context(&self, key: &str) -> Option<&SolutionPackContext> {
        self.contexts.get(key)
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

    if manifest.resources.blueprints.is_empty() && manifest.resources.contexts.is_empty() {
        return invalid("at least one blueprint or context is required");
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
                .contexts
                .iter()
                .map(|resource| ("contexts", resource)),
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
    Ok(())
}

fn validate_resource(kind: &str, resource: &SolutionPackResource) -> Result<(), SolutionPackError> {
    if kind == "contexts" && resource.key == SYSTEM_DEFAULT_CONTEXT_KEY {
        return invalid("the system default context cannot be declared by a pack");
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
    let expected_extension = if kind == "blueprints" {
        ".toml"
    } else {
        ".json"
    };
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
        .chain(&manifest.resources.contexts)
        .map(|resource| resource.path.as_str())
        .collect::<BTreeSet<_>>();
    let actual = files.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if let Some(missing) = declared.difference(&actual).next() {
        return invalid(format!("declared file '{missing}' is missing"));
    }
    if let Some(extra) = actual.difference(&declared).next() {
        return invalid(format!("archive file '{extra}' is not declared"));
    }

    for resource in manifest
        .resources
        .blueprints
        .iter()
        .chain(&manifest.resources.contexts)
    {
        let bytes = &files[&resource.path];
        let expected = parse_sha256(&resource.sha256).expect("digest validated with manifest");
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if expected.ct_eq(&actual).unwrap_u8() != 1 {
            return invalid(format!(
                "resource '{}' does not match its sha256 digest",
                resource.key
            ));
        }
    }
    Ok(())
}

type ValidatedContent = (
    BTreeMap<String, SolutionPackBlueprint>,
    BTreeMap<String, SolutionPackContext>,
);

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
    let blueprints = prepared
        .into_iter()
        .map(|(key, blueprint)| (key, blueprint.portable))
        .collect();

    let context_keys = manifest
        .resources
        .contexts
        .iter()
        .map(|resource| resource.key.as_str())
        .collect::<HashSet<_>>();
    let mut context_dependencies: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut contexts = BTreeMap::new();
    for resource in &manifest.resources.contexts {
        if resource.key == SYSTEM_DEFAULT_CONTEXT_KEY {
            return invalid("the system default context cannot be declared by a pack");
        }
        let context: SolutionPackContext =
            serde_json::from_slice(&files[&resource.path]).map_err(|_| {
                SolutionPackError::Invalid(format!(
                    "context '{}' is not valid strict context JSON",
                    resource.key
                ))
            })?;
        if context.format_version != SOLUTION_PACK_RESOURCE_FORMAT_VERSION {
            return invalid(format!(
                "context '{}' has unsupported format_version {}",
                resource.key, context.format_version
            ));
        }
        let expected_code = resource_code(&resource.key);
        if context.code != expected_code {
            return invalid(format!(
                "context '{}' code must be '{expected_code}'",
                resource.key
            ));
        }
        if !context.data.is_object() {
            return invalid(format!("context '{}' data must be an object", resource.key));
        }
        let dependencies = if context.parent == SYSTEM_DEFAULT_CONTEXT_KEY {
            Vec::new()
        } else if let Some(parent) = context_keys.get(context.parent.as_str()) {
            vec![*parent]
        } else {
            return invalid(format!(
                "context '{}' references undeclared parent '{}'",
                resource.key, context.parent
            ));
        };
        context_dependencies.insert(&resource.key, dependencies);
        contexts.insert(resource.key.clone(), context);
    }
    validate_acyclic(&context_dependencies, "context parent")?;

    Ok((blueprints, contexts))
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
                )?;
            }
        }
    }
    if let Some(views) = table.get_mut("views").and_then(toml::Value::as_table_mut) {
        for (_, view) in views.iter_mut() {
            if let Some(view) = view.as_table_mut() {
                rewrite_view_references(key, view, blueprint_codes)?;
            }
        }
    }

    let native_source = toml::to_string(&value).map_err(|_| {
        SolutionPackError::Invalid(format!(
            "blueprint '{key}' is not valid strict blueprint TOML"
        ))
    })?;
    let definition = catalog_blueprint::parse(&native_source).map_err(|error| {
        SolutionPackError::Invalid(format!("blueprint '{key}' is invalid: {error}"))
    })?;
    let expected_code = resource_code(key);
    if definition.code != expected_code {
        return invalid(format!("blueprint '{key}' code must be '{expected_code}'"));
    }

    Ok(PreparedBlueprint {
        portable: SolutionPackBlueprint {
            key: key.to_owned(),
            code: definition.code.clone(),
            source: source.to_owned(),
            includes: portable_includes,
        },
        native_definition: definition,
        native_source,
        include_keys,
    })
}

fn rewrite_blueprint_reference(
    owner_key: &str,
    table: &mut toml::map::Map<String, toml::Value>,
    field: &str,
    label: &str,
    blueprint_codes: &HashMap<&str, &str>,
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
    *reference = toml::Value::String((*code).to_owned());
    Ok(())
}

fn rewrite_view_references(
    owner_key: &str,
    node: &mut toml::map::Map<String, toml::Value>,
    blueprint_codes: &HashMap<&str, &str>,
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
                )?;
            }
        }
    }

    for collection in ["children", "tabs", "sections"] {
        if let Some(children) = node.get_mut(collection).and_then(toml::Value::as_array_mut) {
            for child in children {
                if let Some(child) = child.as_table_mut() {
                    rewrite_view_references(owner_key, child, blueprint_codes)?;
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
    if value.is_empty() || value.trim() != value || value.len() > max_bytes {
        return invalid(format!(
            "{field} must be non-empty, trimmed, and at most {max_bytes} bytes"
        ));
    }
    Ok(())
}

fn safe_archive_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > MAX_ARCHIVE_PATH_BYTES
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains(['\\', '\0', ':'])
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

fn parse_version_req(value: &str) -> Result<VersionReq, semver::Error> {
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
    const WEB_CONTEXT: &[u8] =
        br#"{"format_version":1,"code":"web","data":{"channel":"web"},"parent":"system/default"}"#;

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
                ],
                "contexts": [resource("contexts/web", "contexts/web.json", WEB_CONTEXT)]
            }
        })
    }

    fn valid_files() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("blueprints/product.toml", PRODUCT_BLUEPRINT),
            ("blueprints/category.toml", CATEGORY_BLUEPRINT),
            ("contexts/web.json", WEB_CONTEXT),
        ]
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
        assert_eq!(pack.files().count(), 3);
        assert_eq!(
            pack.blueprint("blueprints/product").unwrap().code(),
            "product"
        );
        assert_eq!(pack.context("contexts/web").unwrap().code, "web");

        let mut illustrative_range = manifest_value();
        illustrative_range["catalog"]["host_api"] = json!(">=1.0.0 <2.0.0");
        ValidatedSolutionPack::from_tar_zst(&archive(&illustrative_range, &valid_files())).unwrap();
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

        let mut unknown = manifest_value();
        unknown["unknown"] = json!(true);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &unknown)
                .unwrap()
                .is_empty()
        );

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
        assert_invalid(&archive(&manifest_value(), &files[..2]), "is missing");

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
            ("contexts/web.json", WEB_CONTEXT),
        ];
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(product));
        manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(mixin));
        assert_invalid(&archive(&manifest, &files), "not exposed by include 'base'");
    }

    #[test]
    fn validates_incoming_relationship_selectors_across_blueprints() {
        let missing =
            CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"missing\"");
        assert_incoming_relationship_error(PRODUCT_BLUEPRINT, &missing, "has no field 'missing'");

        let scalar =
            CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"name\"");
        assert_incoming_relationship_error(
            PRODUCT_BLUEPRINT,
            &scalar,
            "field 'name' must be a relationship",
        );

        let wrong_target = PRODUCT_BLUEPRINT.replace_ascii(
            b"target_blueprint = \"blueprints/category\"",
            b"target_blueprint = \"blueprints/product\"",
        );
        assert_incoming_relationship_error(
            &wrong_target,
            CATEGORY_BLUEPRINT,
            "field 'categories' must target 'category'",
        );
    }

    fn assert_incoming_relationship_error(product: &[u8], category: &[u8], expected: &str) {
        let files = vec![
            ("blueprints/product.toml", product),
            ("blueprints/category.toml", category),
            ("contexts/web.json", WEB_CONTEXT),
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
            ("contexts/web.json", WEB_CONTEXT),
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
            ("contexts/web.json", WEB_CONTEXT),
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
    fn validates_context_content_parent_references_and_cycles() {
        let unknown_field = br#"{"format_version":1,"code":"web","data":{},"parent":"system/default","unknown":true}"#;
        assert_context_error(unknown_field, "strict context JSON");

        let unknown_parent =
            br#"{"format_version":1,"code":"web","data":{},"parent":"contexts/missing"}"#;
        assert_context_error(unknown_parent, "undeclared parent");

        let bad_data = br#"{"format_version":1,"code":"web","data":[],"parent":"system/default"}"#;
        assert_context_error(bad_data, "data must be an object");

        let bad_version =
            br#"{"format_version":2,"code":"web","data":{},"parent":"system/default"}"#;
        assert_context_error(bad_version, "unsupported format_version");

        let bad_code =
            br#"{"format_version":1,"code":"other","data":{},"parent":"system/default"}"#;
        assert_context_error(bad_code, "code must be 'web'");

        let default =
            br#"{"format_version":1,"code":"default","data":{},"parent":"system/default"}"#;
        let default_manifest = json!({
            "manifest_version":1,"id":"attricat.test","name":"Test","version":"1.0.0",
            "description":"Test pack","catalog":{"host_api":"^1.0"},
            "resources":{"blueprints":[],"contexts":[resource("system/default", "contexts/default.json", default)]}
        });
        assert_invalid(
            &archive(&default_manifest, &[("contexts/default.json", default)]),
            "system default context cannot be declared",
        );

        let a = br#"{"format_version":1,"code":"a","data":{},"parent":"contexts/b"}"#;
        let b = br#"{"format_version":1,"code":"b","data":{},"parent":"contexts/a"}"#;
        let files = vec![
            ("contexts/a.json", a.as_slice()),
            ("contexts/b.json", b.as_slice()),
        ];
        let manifest = json!({
            "manifest_version":1,"id":"attricat.test","name":"Test","version":"1.0.0",
            "description":"Test pack","catalog":{"host_api":"^1.0"},
            "resources":{"blueprints":[],"contexts":[
                resource("contexts/a", "contexts/a.json", a),
                resource("contexts/b", "contexts/b.json", b)
            ]}
        });
        assert_invalid(
            &archive(&manifest, &files),
            "parent references contain a cycle",
        );
    }

    fn assert_context_error(context: &[u8], expected: &str) {
        let files = vec![("contexts/web.json", context)];
        let manifest = json!({
            "manifest_version":1,"id":"attricat.test","name":"Test","version":"1.0.0",
            "description":"Test pack","catalog":{"host_api":"^1.0"},
            "resources":{"blueprints":[],"contexts":[resource("contexts/web", "contexts/web.json", context)]}
        });
        assert_invalid(&archive(&manifest, &files), expected);
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
