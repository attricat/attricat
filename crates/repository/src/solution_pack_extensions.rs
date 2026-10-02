//! Official-registry extensions that a solution pack installs for itself.
//!
//! Solution packs are first-party, so applying a pack may install, configure,
//! grant, and enable the extensions it requires without separate operator
//! approval. A pack names only an extension ID and version range: code always
//! comes from the official registry, never from the pack archive or from a
//! workspace-managed registry.
//!
//! Planning resolves and validates one pinned release per missing extension
//! without persisting the archive. Apply downloads that exact release again and
//! refuses it if its archive or manifest changed since review.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use semver::{Version, VersionReq};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    extension_installer::{ExtensionInstallError, ExtensionInstaller},
    extension_registry::{
        DiscoveredRelease, GitHubRegistry, GitHubRepository, RegistryError, ReleaseAsset,
    },
    extensions::{ExtensionPackage, Manifest},
    repository::{
        CatalogRepository, InstalledExtension, PlannedExtensionInstall, RepositoryError,
        extension_manifest_sha256, required_extension_grants,
    },
    solution_packs::{ValidatedSolutionPack, parse_version_req},
    storage::ObjectStore,
};

/// Release lookup in the official extension registry.
#[async_trait]
pub trait OfficialExtensionReleases: Send + Sync {
    /// Installable releases of the extension the official registry lists
    /// under `extension_id`, or none when it is not listed.
    async fn releases(&self, extension_id: &str) -> Result<Vec<DiscoveredRelease>, RegistryError>;
    async fn download(&self, release: &DiscoveredRelease) -> Result<Vec<u8>, RegistryError>;
}

pub struct OfficialExtensionRegistry {
    registry: Arc<GitHubRegistry>,
    source: GitHubRepository,
}

impl OfficialExtensionRegistry {
    pub fn new(registry: Arc<GitHubRegistry>, source: GitHubRepository) -> Self {
        Self { registry, source }
    }
}

#[async_trait]
impl OfficialExtensionReleases for OfficialExtensionRegistry {
    async fn releases(&self, extension_id: &str) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        let Some(extension) = self
            .registry
            .discover(&self.source)
            .await?
            .into_iter()
            .find(|extension| extension.id == extension_id)
        else {
            return Ok(Vec::new());
        };
        self.registry.extension_releases(&extension).await
    }

    async fn download(&self, release: &DiscoveredRelease) -> Result<Vec<u8>, RegistryError> {
        self.registry.download_release_asset(release).await
    }
}

/// Development stand-in for the official registry: serves
/// `<root>/<extension-id>/<version>.tar.zst` as release `v<version>`, so packs
/// can be tried against locally built extensions. Never use it in production.
pub struct LocalExtensionReleases {
    root: std::path::PathBuf,
}

impl LocalExtensionReleases {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn archive_path(&self, extension_id: &str, version: &str) -> std::path::PathBuf {
        self.root
            .join(extension_id)
            .join(format!("{version}.tar.zst"))
    }
}

#[async_trait]
impl OfficialExtensionReleases for LocalExtensionReleases {
    async fn releases(&self, extension_id: &str) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        // Registry IDs never contain path separators; reject anything else.
        if extension_id.is_empty()
            || !extension_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || extension_id.starts_with('.')
        {
            return Ok(Vec::new());
        }
        let Ok(mut entries) = tokio::fs::read_dir(self.root.join(extension_id)).await else {
            return Ok(Vec::new());
        };
        let mut releases = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|_| RegistryError::Unavailable)?
        {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some(version) = file_name
                .strip_suffix(".tar.zst")
                .filter(|version| Version::parse(version).is_ok())
                .map(str::to_owned)
            else {
                continue;
            };
            let id = releases.len() as u64 + 1;
            releases.push(DiscoveredRelease {
                source: format!("local:{extension_id}"),
                release_id: id,
                tag_name: format!("v{version}"),
                name: file_name.clone(),
                published_at: None,
                asset: ReleaseAsset {
                    id,
                    name: file_name,
                    download_url: format!("local:{extension_id}/{version}.tar.zst"),
                },
            });
        }
        Ok(releases)
    }

    async fn download(&self, release: &DiscoveredRelease) -> Result<Vec<u8>, RegistryError> {
        let extension_id = release
            .source
            .strip_prefix("local:")
            .ok_or(RegistryError::InvalidResponse)?;
        let version = release
            .tag_name
            .strip_prefix('v')
            .filter(|version| Version::parse(version).is_ok())
            .ok_or(RegistryError::InvalidResponse)?;
        if self.releases(extension_id).await?.is_empty() {
            return Err(RegistryError::InvalidResponse);
        }
        tokio::fs::read(self.archive_path(extension_id, version))
            .await
            .map_err(|_| RegistryError::Unavailable)
    }
}

#[derive(Debug, Error)]
pub enum SolutionPackExtensionError {
    #[error("the official extension registry is unavailable")]
    RegistryUnavailable,
    #[error(
        "official release {tag} of '{extension_id}' cannot be installed by this pack: {reason}"
    )]
    InvalidRelease {
        extension_id: String,
        tag: String,
        reason: String,
    },
    #[error(
        "'{extension_id}' depends on '{dependency}', which is neither enabled in this workspace nor installed by this pack"
    )]
    UnmetDependency {
        extension_id: String,
        dependency: String,
    },
    #[error("extensions installed by this pack have a dependency cycle")]
    DependencyCycle,
    #[error("the official release of '{0}' changed after the plan was reviewed")]
    ReleaseChanged(String),
    #[error(transparent)]
    Install(#[from] ExtensionInstallError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// A validated official release that a plan will install.
#[derive(Clone, Debug)]
pub struct ResolvedExtensionRelease {
    pub extension_id: String,
    /// Reserved while planning so reviewed contribution evidence can name the
    /// installation that apply creates.
    pub installed_release_id: Uuid,
    pub release: DiscoveredRelease,
    pub archive_sha256: String,
    pub manifest: Manifest,
    pub manifest_sha256: String,
    /// The pack's configuration template, or `{}` without one.
    pub configuration: Value,
}

/// Resolves the highest official release in range for every required or
/// optional extension that is not installed yet, in dependency order. An
/// extension the official registry does not offer in range is left out, so
/// the planner reports it as missing.
pub async fn resolve_official_extensions(
    repository: &CatalogRepository,
    releases: &dyn OfficialExtensionReleases,
    pack: &ValidatedSolutionPack,
) -> Result<Vec<ResolvedExtensionRelease>, SolutionPackExtensionError> {
    let requirements = &pack.manifest().extensions;
    if requirements.is_empty() {
        return Ok(Vec::new());
    }
    let installed = repository
        .installed_extensions()
        .await?
        .into_iter()
        .map(|installation| (installation.extension_id.clone(), installation))
        .collect::<HashMap<_, _>>();
    let mut resolved = BTreeMap::new();
    for requirement in requirements {
        if installed.contains_key(&requirement.id) {
            continue;
        }
        let range = parse_version_req(&requirement.version).expect("validated version range");
        let Some((version, release)) = releases
            .releases(&requirement.id)
            .await
            .map_err(registry_error)?
            .into_iter()
            .filter_map(|release| {
                release_version(&release.tag_name)
                    .filter(|version| range.matches(version))
                    .map(|version| (version, release))
            })
            .max_by(|left, right| left.0.cmp(&right.0))
        else {
            continue;
        };
        let invalid = |reason: String| SolutionPackExtensionError::InvalidRelease {
            extension_id: requirement.id.clone(),
            tag: release.tag_name.clone(),
            reason,
        };
        let archive = releases.download(&release).await.map_err(registry_error)?;
        let archive_sha256 = sha256_hex(&archive);
        let manifest = unpack(archive)
            .await
            .map_err(|error| invalid(error.to_string()))?
            .manifest()
            .clone();
        if manifest.catalog.id != requirement.id {
            return Err(invalid(format!(
                "its manifest declares '{}'",
                manifest.catalog.id
            )));
        }
        if Version::parse(&manifest.version).ok() != Some(version) {
            return Err(invalid(format!(
                "its manifest version {} does not match the release tag",
                manifest.version
            )));
        }
        let configuration = pack
            .configuration_template(&requirement.key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        manifest
            .validate_configuration(&configuration)
            .map_err(|error| {
                invalid(format!(
                    "the pack's configuration template is not a valid configuration: {error}"
                ))
            })?;
        let manifest_sha256 = extension_manifest_sha256(&manifest)?;
        resolved.insert(
            requirement.id.clone(),
            ResolvedExtensionRelease {
                extension_id: requirement.id.clone(),
                installed_release_id: Uuid::new_v4(),
                release,
                archive_sha256,
                manifest,
                manifest_sha256,
                configuration,
            },
        );
    }
    dependency_order(resolved, &installed)
}

/// Orders releases so dependencies are enabled first. Every dependency must be
/// installed by this pack or already enabled in the workspace, because
/// enablement requires enabled compatible dependencies.
fn dependency_order(
    mut pending: BTreeMap<String, ResolvedExtensionRelease>,
    installed: &HashMap<String, InstalledExtension>,
) -> Result<Vec<ResolvedExtensionRelease>, SolutionPackExtensionError> {
    for release in pending.values() {
        for dependency in &release.manifest.dependencies {
            let matches = |version: &str| {
                VersionReq::parse(&dependency.version)
                    .ok()
                    .zip(Version::parse(version).ok())
                    .is_some_and(|(range, version)| range.matches(&version))
            };
            let available = match (pending.get(&dependency.id), installed.get(&dependency.id)) {
                (Some(planned), _) => matches(&planned.manifest.version),
                (None, Some(existing)) => existing.state == "enabled" && matches(&existing.version),
                (None, None) => false,
            };
            if !available {
                return Err(SolutionPackExtensionError::UnmetDependency {
                    extension_id: release.extension_id.clone(),
                    dependency: dependency.id.clone(),
                });
            }
        }
    }
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let next = pending
            .iter()
            .find(|(_, release)| {
                release
                    .manifest
                    .dependencies
                    .iter()
                    .all(|dependency| !pending.contains_key(&dependency.id))
            })
            .map(|(extension_id, _)| extension_id.clone())
            .ok_or(SolutionPackExtensionError::DependencyCycle)?;
        ordered.extend(pending.remove(&next));
    }
    Ok(ordered)
}

/// Installs, configures, grants, and enables every extension a reviewed plan
/// installs. Each step checks current state first, so a retried apply resumes
/// where an interrupted one stopped. An installation that no longer matches
/// the plan is left alone; apply's revalidation then reports the plan stale.
pub async fn install_planned_extensions(
    repository: &CatalogRepository,
    object_store: Arc<dyn ObjectStore>,
    releases: &dyn OfficialExtensionReleases,
    plan_id: Uuid,
) -> Result<(), SolutionPackExtensionError> {
    for planned in repository.solution_pack_extension_installs(plan_id).await? {
        install_planned_extension(repository, object_store.clone(), releases, &planned).await?;
    }
    Ok(())
}

async fn install_planned_extension(
    repository: &CatalogRepository,
    object_store: Arc<dyn ObjectStore>,
    releases: &dyn OfficialExtensionReleases,
    planned: &PlannedExtensionInstall,
) -> Result<(), SolutionPackExtensionError> {
    let installation = match repository.installed_extension(&planned.extension_id).await {
        Ok(installation) => installation,
        Err(RepositoryError::NotFound(_)) => {
            let release = DiscoveredRelease {
                source: planned.repository.clone(),
                release_id: planned.release_id as u64,
                tag_name: planned.tag_name.clone(),
                name: String::new(),
                published_at: None,
                asset: ReleaseAsset {
                    id: planned.asset_id as u64,
                    name: planned.asset_name.clone(),
                    download_url: planned.download_url.clone(),
                },
            };
            let changed =
                || SolutionPackExtensionError::ReleaseChanged(planned.extension_id.clone());
            let archive = releases.download(&release).await.map_err(registry_error)?;
            if sha256_hex(&archive) != planned.archive_sha256 {
                return Err(changed());
            }
            let package = unpack(archive).await.map_err(|_| changed())?;
            if extension_manifest_sha256(package.manifest())? != planned.manifest_sha256 {
                return Err(changed());
            }
            let source = format!("{}@{}", planned.repository, planned.tag_name);
            match ExtensionInstaller::new(repository.clone(), object_store)
                .install_package_as(&source, package, planned.installed_release_id)
                .await
            {
                Ok(_)
                | Err(ExtensionInstallError::Repository(
                    RepositoryError::ExtensionAlreadyInstalled,
                )) => {}
                Err(error) => return Err(error.into()),
            }
            repository
                .installed_extension(&planned.extension_id)
                .await?
        }
        Err(error) => return Err(error.into()),
    };
    // Only a disabled installation of the planned release is ours to finish.
    // An enabled one is done; anything else changed outside this plan.
    if installation.installed_release_id != planned.installed_release_id
        || installation.state != "disabled"
    {
        return Ok(());
    }
    if installation.configuration != planned.configuration {
        repository
            .configure_extension(&planned.extension_id, planned.configuration.clone())
            .await?;
    }
    let manifest = serde_json::from_value::<Manifest>(installation.manifest)
        .map_err(|error| RepositoryError::InvalidExtension(error.to_string()))?;
    let granted = repository
        .extension_grants(&planned.extension_id)
        .await?
        .into_iter()
        .map(|grant| (grant.grant_kind, grant.grant_id))
        .collect::<HashSet<_>>();
    for (grant_kind, grant_id) in required_extension_grants(&manifest) {
        if !granted.contains(&(grant_kind.to_owned(), grant_id.clone())) {
            repository
                .grant_extension(&planned.extension_id, grant_kind, &grant_id)
                .await?;
        }
    }
    repository.enable_extension(&planned.extension_id).await?;
    Ok(())
}

fn registry_error(_: RegistryError) -> SolutionPackExtensionError {
    SolutionPackExtensionError::RegistryUnavailable
}

/// Release tags are SemVer versions with an optional `v` prefix.
fn release_version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Decompresses and validates an archive off the async executor.
async fn unpack(archive: Vec<u8>) -> Result<ExtensionPackage, crate::extensions::ManifestError> {
    tokio::task::spawn_blocking(move || ExtensionPackage::from_tar_zst(&archive))
        .await
        .expect("extension unpacking does not panic")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tags_accept_an_optional_v_prefix() {
        assert_eq!(release_version("v1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(release_version("1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(release_version("release-1"), None);
    }
}
