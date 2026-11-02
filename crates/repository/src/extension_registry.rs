//! Trusted extension-registry index discovery.
//!
//! A registry is a GitHub repository containing `registry.json`. Its index
//! points to extension repositories; only those index-authorized repositories
//! may have their README and release assets resolved, and only the versions an
//! entry lists after review may be offered or installed, with the archive
//! digest it pins. No index, README, release catalogue, or archive is persisted.

use std::{str::FromStr, time::Duration};

use reqwest::{Client, Response, Url, redirect::Policy};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

pub const DEFAULT_OFFICIAL_REGISTRY: &str = "attricat/attricat-extensions";
/// The `registry_version` this host reads. Version 2 made `versions` required.
const REGISTRY_INDEX_VERSION: u32 = 2;
const GITHUB_API_ORIGIN: &str = "https://api.github.com";
const GITHUB_RAW_ORIGIN: &str = "https://raw.githubusercontent.com";
/// Scheme prefix of canonical GitHub registry and extension sources.
const GITHUB_SOURCE_PREFIX: &str = "github:";
const MAX_README_BYTES: usize = 256 * 1024;
const MAX_REGISTRY_INDEX_BYTES: usize = 1024 * 1024;
const MAX_RELEASE_CATALOGUE_BYTES: usize = 1024 * 1024;
const MAX_REGISTRY_ENTRIES: usize = 1_000;
const MAX_REGISTRY_NAME_BYTES: usize = 256;
const MAX_REGISTRY_DESCRIPTION_BYTES: usize = 4 * 1024;
const MAX_REGISTRY_ICON_BYTES: usize = 512;
const MAX_REGISTRY_ID_BYTES: usize = 128;
const MAX_REGISTRY_VERSIONS: usize = 100;
const MAX_REGISTRY_VERSION_BYTES: usize = 128;
const MAX_REGISTRY_HOST_API_BYTES: usize = 256;
/// Length of a SHA-256 digest in hexadecimal.
const SHA256_HEX_BYTES: usize = 64;
/// GitHub limits for account and repository names.
const MAX_GITHUB_OWNER_BYTES: usize = 39;
const MAX_GITHUB_REPOSITORY_BYTES: usize = 100;
/// Redirects followed from a release download to its short-lived asset URL.
const MAX_ASSET_REDIRECTS: usize = 5;
const SHA256_DIGEST_PREFIX: &str = "sha256:";
const REGISTRY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Archives are far larger than catalogue responses.
const ASSET_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const USER_AGENT: &str = "attricat-extension-registry";
/// How long READMEs and release lists are reused. GitHub allows an address 60
/// unauthenticated API requests an hour, and viewing and then installing an
/// extension would otherwise spend four of them.
const CATALOGUE_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const CATALOGUE_CACHE_ENTRIES: u64 = 1_024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GitHubRepository {
    pub owner: String,
    pub repository: String,
}
impl GitHubRepository {
    pub fn identity(&self) -> String {
        format!("{GITHUB_SOURCE_PREFIX}{}/{}", self.owner, self.repository)
    }
    fn release_asset_origin(&self) -> String {
        format!(
            "https://github.com/{}/{}/releases/download/",
            self.owner, self.repository
        )
    }
}
impl FromStr for GitHubRepository {
    type Err = RegistryError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        let path = value.strip_prefix(GITHUB_SOURCE_PREFIX).unwrap_or(value);
        let path = if path.starts_with("https://") {
            let url = Url::parse(path).map_err(|_| RegistryError::InvalidSource)?;
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.host_str() != Some("github.com")
                || url.port().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(RegistryError::InvalidSource);
            }
            url.path().trim_matches('/').to_owned()
        } else {
            path.trim_matches('/').to_owned()
        };
        let mut parts = path.split('/');
        let (Some(owner), Some(repository), None) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(RegistryError::InvalidSource);
        };
        if !valid_owner(owner) || !valid_repository(repository) {
            return Err(RegistryError::InvalidSource);
        }
        Ok(Self {
            owner: owner.to_ascii_lowercase(),
            repository: repository.to_ascii_lowercase(),
        })
    }
}
fn valid_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_GITHUB_OWNER_BYTES
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
fn valid_repository(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_GITHUB_REPOSITORY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The public, versioned raw registry document at `registry.json`. Unknown
/// fields are ignored so a registry can add one without breaking older hosts; a
/// breaking change raises `registry_version`.
#[derive(Deserialize)]
struct RegistryIndex {
    registry_version: u32,
    extensions: Vec<RegistryIndexEntry>,
}
#[derive(Clone, Deserialize)]
struct RegistryIndexEntry {
    id: String,
    repository: String,
    name: String,
    description: String,
    #[serde(default)]
    icon: Option<String>,
    versions: Vec<ReviewedVersion>,
}

/// One release a registry has reviewed: its version, the host ABIs it runs on,
/// and the lowercase hexadecimal SHA-256 of the archive that was reviewed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReviewedVersion {
    pub version: String,
    pub host_api: String,
    pub sha256: String,
}
impl ReviewedVersion {
    fn is_valid(&self) -> bool {
        self.version.len() <= MAX_REGISTRY_VERSION_BYTES
            && self.host_api.len() <= MAX_REGISTRY_HOST_API_BYTES
            && Version::parse(&self.version).is_ok()
            && VersionReq::parse(&self.host_api).is_ok()
            && self.sha256.len() == SHA256_HEX_BYTES
            && self
                .sha256
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    }
    /// The version, when its `host_api` range accepts this host.
    fn compatible_version(&self) -> Option<Version> {
        let host = Version::parse(crate::extensions::SUPPORTED_HOST_API).ok()?;
        VersionReq::parse(&self.host_api)
            .ok()
            .filter(|range| range.matches(&host))?;
        Version::parse(&self.version).ok()
    }
}
fn valid_reviewed_versions(versions: &[ReviewedVersion]) -> bool {
    let mut seen = std::collections::HashSet::new();
    !versions.is_empty()
        && versions.len() <= MAX_REGISTRY_VERSIONS
        && versions
            .iter()
            .all(|listed| listed.is_valid() && seen.insert(Version::parse(&listed.version).ok()))
}
/// Release tags are SemVer versions with an optional `v` prefix.
pub fn release_version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveredExtension {
    pub registry_source: String,
    pub id: String,
    pub repository: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub versions: Vec<ReviewedVersion>,
}
impl DiscoveredExtension {
    /// The release, pinned to the reviewed digest, when the entry lists the
    /// version its tag names. The pin replaces the digest GitHub reports, which
    /// would follow an asset replaced after review.
    fn reviewed_release(&self, mut release: DiscoveredRelease) -> Option<DiscoveredRelease> {
        let version = release_version(&release.tag_name)?;
        let listed = self
            .versions
            .iter()
            .find(|listed| Version::parse(&listed.version).ok().as_ref() == Some(&version))?;
        release.asset.digest = Some(format!("{SHA256_DIGEST_PREFIX}{}", listed.sha256));
        Some(release)
    }
    /// The highest listed version this host can run, when it is newer than the
    /// installed one.
    pub fn available_update(&self, installed_version: &str) -> Option<String> {
        let installed = Version::parse(installed_version).ok()?;
        self.versions
            .iter()
            .filter_map(ReviewedVersion::compatible_version)
            .max()
            .filter(|latest| *latest > installed)
            .map(|latest| latest.to_string())
    }
    fn github_repository(&self) -> Result<GitHubRepository, RegistryError> {
        self.repository
            .strip_prefix(GITHUB_SOURCE_PREFIX)
            .ok_or(RegistryError::InvalidResponse)?
            .parse()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ExtensionDetails {
    pub extension: DiscoveredExtension,
    pub readme: String,
    pub releases: Vec<DiscoveredRelease>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DiscoveredRelease {
    pub source: String,
    pub release_id: u64,
    pub tag_name: String,
    pub name: String,
    pub published_at: Option<String>,
    pub asset: ReleaseAsset,
}
impl DiscoveredRelease {
    /// Whether an archive's manifest version is the one its tag names. A listed
    /// tag vouches only for that version, so an archive declaring another one
    /// must not install under it.
    pub fn declares_version(&self, manifest_version: &str) -> bool {
        let version = release_version(&self.tag_name);
        version.is_some() && version == Version::parse(manifest_version).ok()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub download_url: String,
    /// The `sha256:<hex>` checked on download: the digest a registry reviewed
    /// for a listed release, otherwise the one GitHub reports.
    pub digest: Option<String>,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry source must be a GitHub owner/repository identifier")]
    InvalidSource,
    #[error("registry request failed")]
    Unavailable,
    #[error("registry repository or file was not found")]
    NotFound,
    #[error("GitHub's rate limit for this server's address is used up")]
    RateLimited,
    #[error("registry returned an invalid index or release response")]
    InvalidResponse,
    #[error("extension repository is not listed by a trusted registry")]
    UntrustedExtension,
    #[error("release archive does not match the digest its registry reviewed")]
    DigestMismatch,
}

#[derive(Clone)]
pub struct GitHubRegistry {
    client: Client,
    asset_client: Client,
    api_origin: Url,
    raw_origin: Url,
    readmes: moka::future::Cache<String, String>,
    releases: moka::future::Cache<String, Vec<DiscoveredRelease>>,
}
/// Why a catalogue response was not a success.
fn status_error(status: reqwest::StatusCode) -> RegistryError {
    match status {
        reqwest::StatusCode::NOT_FOUND => RegistryError::NotFound,
        reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::TOO_MANY_REQUESTS => {
            RegistryError::RateLimited
        }
        _ => RegistryError::Unavailable,
    }
}
fn catalogue_cache<V: Clone + Send + Sync + 'static>() -> moka::future::Cache<String, V> {
    moka::future::Cache::builder()
        .max_capacity(CATALOGUE_CACHE_ENTRIES)
        .time_to_live(CATALOGUE_CACHE_TTL)
        .build()
}
/// Reads a response no larger than `limit`, stopping as soon as it passes that,
/// so a body that does not declare its size cannot fill memory first.
async fn read_capped(mut response: Response, limit: usize) -> Result<Vec<u8>, RegistryError> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err(RegistryError::InvalidResponse);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| RegistryError::Unavailable)?
    {
        if bytes.len() + chunk.len() > limit {
            return Err(RegistryError::InvalidResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Whether bytes match an asset's digest. An asset without one, or in a form
/// this does not know, has nothing to check against.
fn matches_digest(bytes: &[u8], digest: Option<&str>) -> bool {
    digest
        .and_then(|digest| digest.strip_prefix(SHA256_DIGEST_PREFIX))
        .is_none_or(|expected| {
            format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(expected)
        })
}

/// Parses and validates a `registry.json` document. Any invalid entry rejects
/// the whole index, so a registry never lists less than it means to review.
fn parse_index(
    source: &GitHubRepository,
    bytes: &[u8],
) -> Result<Vec<DiscoveredExtension>, RegistryError> {
    let index: RegistryIndex =
        serde_json::from_slice(bytes).map_err(|_| RegistryError::InvalidResponse)?;
    if index.registry_version != REGISTRY_INDEX_VERSION
        || index.extensions.len() > MAX_REGISTRY_ENTRIES
    {
        return Err(RegistryError::InvalidResponse);
    }
    let mut ids = std::collections::HashSet::new();
    index
        .extensions
        .into_iter()
        .map(|entry| {
            if entry.id.is_empty()
                || entry.id.len() > MAX_REGISTRY_ID_BYTES
                || !entry
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
                || entry.name.trim().is_empty()
                || entry.name.len() > MAX_REGISTRY_NAME_BYTES
                || entry.description.trim().is_empty()
                || entry.description.len() > MAX_REGISTRY_DESCRIPTION_BYTES
                || entry
                    .icon
                    .as_ref()
                    .is_some_and(|icon| icon.len() > MAX_REGISTRY_ICON_BYTES)
                || !valid_reviewed_versions(&entry.versions)
                || !ids.insert(entry.id.clone())
            {
                return Err(RegistryError::InvalidResponse);
            }
            let repository = entry.repository.parse::<GitHubRepository>()?;
            Ok(DiscoveredExtension {
                registry_source: source.identity(),
                id: entry.id,
                repository: repository.identity(),
                name: entry.name,
                description: entry.description,
                icon: entry.icon,
                versions: entry.versions,
            })
        })
        .collect()
}

impl GitHubRegistry {
    pub fn new() -> Result<Self, RegistryError> {
        Self::with_origins(
            Url::parse(GITHUB_API_ORIGIN).expect("constant URL"),
            Url::parse(GITHUB_RAW_ORIGIN).expect("constant URL"),
        )
    }
    /// A registry reading its catalogue from other origins, for tests. Assets
    /// still have to come from GitHub.
    pub fn with_origins(api_origin: Url, raw_origin: Url) -> Result<Self, RegistryError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .user_agent(USER_AGENT)
            .connect_timeout(REGISTRY_CONNECT_TIMEOUT)
            .timeout(REGISTRY_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| RegistryError::Unavailable)?;
        // GitHub release download URLs redirect to a short-lived asset URL.
        // The initial URL is authenticated against the GitHub release response
        // and exact repository before this client follows that redirect.
        let asset_client = Client::builder()
            .redirect(Policy::limited(MAX_ASSET_REDIRECTS))
            .user_agent(USER_AGENT)
            .connect_timeout(REGISTRY_CONNECT_TIMEOUT)
            .timeout(ASSET_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| RegistryError::Unavailable)?;
        Ok(Self {
            client,
            asset_client,
            api_origin,
            raw_origin,
            readmes: catalogue_cache(),
            releases: catalogue_cache(),
        })
    }
    pub async fn discover(
        &self,
        source: &GitHubRepository,
    ) -> Result<Vec<DiscoveredExtension>, RegistryError> {
        let url = self
            .raw_origin
            .join(&format!(
                "{}/{}/HEAD/registry.json",
                source.owner, source.repository
            ))
            .expect("validated repository path");
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let bytes = read_capped(response, MAX_REGISTRY_INDEX_BYTES).await?;
        parse_index(source, &bytes)
    }
    pub async fn extension_details(
        &self,
        extension: DiscoveredExtension,
    ) -> Result<ExtensionDetails, RegistryError> {
        let repository = extension.github_repository()?;
        let readme = self.readme(&repository).await?;
        let releases = self.listed_releases(&extension).await?;
        Ok(ExtensionDetails {
            extension,
            readme,
            releases,
        })
    }
    /// The repository's README, empty if it has none.
    async fn readme(&self, repository: &GitHubRepository) -> Result<String, RegistryError> {
        let key = repository.identity();
        if let Some(readme) = self.readmes.get(&key).await {
            return Ok(readme);
        }
        let readme_url = self
            .api_origin
            .join(&format!(
                "repos/{}/{}/readme",
                repository.owner, repository.repository
            ))
            .expect("validated repository path");
        let response = self
            .client
            .get(readme_url)
            .header("Accept", "application/vnd.github.raw+json")
            .send()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        let readme = match response.status() {
            status if status.is_success() => {
                let bytes = read_capped(response, MAX_README_BYTES).await?;
                String::from_utf8(bytes).map_err(|_| RegistryError::InvalidResponse)?
            }
            reqwest::StatusCode::NOT_FOUND => String::new(),
            status => return Err(status_error(status)),
        };
        self.readmes.insert(key, readme.clone()).await;
        Ok(readme)
    }
    /// Lists the releases of an index-authorized extension that its entry
    /// reviewed, without loading its README.
    pub async fn extension_releases(
        &self,
        extension: &DiscoveredExtension,
    ) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        self.listed_releases(extension).await
    }
    /// Downloads only an asset that was returned by this registry's trusted
    /// release resolution. Callers never provide a URL, preventing this API
    /// from becoming an outbound fetch proxy.
    pub async fn download_release_asset(
        &self,
        release: &DiscoveredRelease,
    ) -> Result<Vec<u8>, RegistryError> {
        let url =
            Url::parse(&release.asset.download_url).map_err(|_| RegistryError::InvalidResponse)?;
        let source = release
            .source
            .trim_start_matches(GITHUB_SOURCE_PREFIX)
            .parse::<GitHubRepository>()
            .map_err(|_| RegistryError::InvalidResponse)?;
        if url.scheme() != "https"
            || !release
                .asset
                .download_url
                .starts_with(&source.release_asset_origin())
        {
            return Err(RegistryError::InvalidResponse);
        }
        let response = self
            .asset_client
            .get(url)
            .send()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if !response.status().is_success() {
            return Err(RegistryError::Unavailable);
        }
        let bytes = read_capped(response, crate::extensions::MAX_EXTENSION_ARCHIVE_BYTES).await?;
        if !matches_digest(&bytes, release.asset.digest.as_deref()) {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(bytes)
    }

    async fn listed_releases(
        &self,
        extension: &DiscoveredExtension,
    ) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        let repository = extension.github_repository()?;
        let releases = self.releases(&repository).await?;
        Ok(releases
            .into_iter()
            .filter_map(|release| extension.reviewed_release(release))
            .collect())
    }

    async fn releases(
        &self,
        source: &GitHubRepository,
    ) -> Result<Vec<DiscoveredRelease>, RegistryError> {
        let key = source.identity();
        if let Some(releases) = self.releases.get(&key).await {
            return Ok(releases);
        }
        let url = self
            .api_origin
            .join(&format!(
                "repos/{}/{}/releases",
                source.owner, source.repository
            ))
            .expect("validated repository path");
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let bytes = read_capped(response, MAX_RELEASE_CATALOGUE_BYTES).await?;
        let releases: Vec<GitHubRelease> =
            serde_json::from_slice(&bytes).map_err(|_| RegistryError::InvalidResponse)?;
        let releases: Vec<DiscoveredRelease> = releases
            .into_iter()
            .filter(|release| !release.draft && !release.prerelease)
            .filter_map(|release| {
                let asset = release
                    .assets
                    .into_iter()
                    .find(|asset| asset.name.ends_with(".tar.zst"))?;
                let download_url = Url::parse(&asset.browser_download_url).ok()?;
                if download_url.scheme() != "https"
                    || !asset
                        .browser_download_url
                        .starts_with(&source.release_asset_origin())
                {
                    return None;
                }
                Some(DiscoveredRelease {
                    source: source.identity(),
                    release_id: release.id,
                    tag_name: release.tag_name,
                    name: release.name.unwrap_or_default(),
                    published_at: release.published_at,
                    asset: ReleaseAsset {
                        id: asset.id,
                        name: asset.name,
                        download_url: asset.browser_download_url,
                        digest: asset.digest,
                    },
                })
            })
            .collect();
        self.releases.insert(key, releases.clone()).await;
        Ok(releases)
    }
}
#[derive(Deserialize)]
struct GitHubRelease {
    id: u64,
    tag_name: String,
    name: Option<String>,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<GitHubAsset>,
}
#[derive(Deserialize)]
struct GitHubAsset {
    id: u64,
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> GitHubRepository {
        "acme/registry".parse().unwrap()
    }
    const PIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn index(versions: &str) -> String {
        format!(
            r#"{{"registry_version": 2, "extensions": [{{
                "id": "acme.widget", "name": "Widget", "description": "A widget",
                "repository": "acme/widget", "versions": {versions}
            }}]}}"#
        )
    }
    fn listing(versions: &[(&str, &str)]) -> DiscoveredExtension {
        let versions = versions
            .iter()
            .map(|(version, host_api)| ReviewedVersion {
                version: (*version).to_owned(),
                host_api: (*host_api).to_owned(),
                sha256: PIN.to_owned(),
            })
            .collect::<Vec<_>>();
        parse_index(
            &source(),
            index(&serde_json::to_string(&versions).unwrap()).as_bytes(),
        )
        .unwrap()
        .remove(0)
    }
    fn release(tag_name: &str) -> DiscoveredRelease {
        DiscoveredRelease {
            source: "github:acme/widget".to_owned(),
            release_id: 1,
            tag_name: tag_name.to_owned(),
            name: String::new(),
            published_at: None,
            asset: ReleaseAsset {
                id: 1,
                name: "widget.tar.zst".to_owned(),
                download_url: String::new(),
                digest: None,
            },
        }
    }

    #[test]
    fn an_index_must_list_valid_reviewed_versions() {
        let valid = index(&format!(
            r#"[{{"version": "1.0.0", "host_api": "^1.1", "sha256": "{PIN}"}}]"#
        ));
        assert_eq!(
            parse_index(&source(), valid.as_bytes()).unwrap()[0].versions[0].version,
            "1.0.0"
        );
        for versions in [
            "[]".to_owned(),
            format!(r#"[{{"host_api": "^1.1", "sha256": "{PIN}"}}]"#),
            format!(r#"[{{"version": "1.0.0", "sha256": "{PIN}"}}]"#),
            r#"[{"version": "1.0.0", "host_api": "^1.1"}]"#.to_owned(),
            format!(r#"[{{"version": "v1.0.0", "host_api": "^1.1", "sha256": "{PIN}"}}]"#),
            format!(r#"[{{"version": "1.0.0", "host_api": "any", "sha256": "{PIN}"}}]"#),
            r#"[{"version": "1.0.0", "host_api": "^1.1", "sha256": "abc"}]"#.to_owned(),
            format!(
                r#"[{{"version": "1.0.0", "host_api": "^1.1", "sha256": "{}"}}]"#,
                PIN.to_ascii_uppercase()
            ),
            format!(
                r#"[{{"version": "1.0.0", "host_api": "^1.1", "sha256": "{PIN}"}},
                    {{"version": "1.0.0", "host_api": "^1", "sha256": "{PIN}"}}]"#
            ),
        ] {
            assert!(
                parse_index(&source(), index(&versions).as_bytes()).is_err(),
                "{versions}"
            );
        }
        let without_versions = r#"{"registry_version": 2, "extensions": [{"id": "acme.widget",
            "name": "Widget", "description": "A widget", "repository": "acme/widget"}]}"#;
        assert!(parse_index(&source(), without_versions.as_bytes()).is_err());
    }

    #[test]
    fn an_index_must_use_the_supported_registry_version() {
        let index = index(&format!(
            r#"[{{"version": "1.0.0", "host_api": "^1.1", "sha256": "{PIN}"}}]"#
        ))
        .replace(r#""registry_version": 2"#, r#""registry_version": 1"#);
        assert!(parse_index(&source(), index.as_bytes()).is_err());
    }

    #[test]
    fn an_index_may_add_fields_this_host_does_not_know() {
        let index = index(&format!(
            r#"[{{"version": "1.0.0", "host_api": "^1.1", "sha256": "{PIN}", "note": "ok"}}]"#
        ))
        .replace(r#""id":"#, r#""added": true, "id":"#);
        assert!(parse_index(&source(), index.as_bytes()).is_ok());
    }

    #[test]
    fn only_listed_versions_are_offered() {
        let extension = listing(&[("1.0.1", "^1.1")]);
        assert!(extension.reviewed_release(release("v1.0.1")).is_some());
        assert!(extension.reviewed_release(release("1.0.1")).is_some());
        assert!(extension.reviewed_release(release("v1.0.0")).is_none());
        assert!(extension.reviewed_release(release("v1.0.2")).is_none());
        assert!(extension.reviewed_release(release("latest")).is_none());
    }

    #[test]
    fn a_listed_release_is_checked_against_the_reviewed_digest_not_githubs() {
        let reviewed = format!("{:x}", Sha256::digest(b"reviewed"));
        let replaced = format!("sha256:{:x}", Sha256::digest(b"replaced"));
        let mut extension = listing(&[("1.0.1", "^1.1")]);
        extension.versions[0].sha256 = reviewed;
        let mut offered = release("v1.0.1");
        offered.asset.digest = Some(replaced);
        let digest = extension.reviewed_release(offered).unwrap().asset.digest;
        assert!(matches_digest(b"reviewed", digest.as_deref()));
        assert!(!matches_digest(b"replaced", digest.as_deref()));
    }

    #[test]
    fn a_release_must_declare_the_version_its_tag_names() {
        assert!(release("v1.0.1").declares_version("1.0.1"));
        assert!(!release("v1.0.1").declares_version("2.0.0"));
        assert!(!release("latest").declares_version("1.0.1"));
    }

    #[test]
    fn an_update_is_the_highest_compatible_version_newer_than_the_installed_one() {
        let incompatible = ">=99.0.0";
        let extension = listing(&[("0.1.9", "^1"), ("0.1.10", "^1"), ("0.2.0", incompatible)]);
        assert_eq!(
            extension.available_update("0.1.9").as_deref(),
            Some("0.1.10")
        );
        assert_eq!(extension.available_update("0.1.10"), None);
        assert_eq!(extension.available_update("0.3.0"), None);
        assert_eq!(extension.available_update("not-a-version"), None);
    }

    #[test]
    fn a_release_asset_must_match_the_digest_github_reports() {
        let good = format!("sha256:{:x}", Sha256::digest(b"archive"));
        assert!(matches_digest(b"archive", Some(&good)));
        assert!(!matches_digest(b"tampered", Some(&good)));
        assert!(matches_digest(b"archive", None));
    }

    /// Serves one chunked response, which declares no size, of `size` bytes.
    async fn undeclared_size_response(size: usize) -> Response {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let chunk = format!("{size:x}\r\n{}\r\n0\r\n\r\n", "x".repeat(size));
            let _ = socket.write_all(chunk.as_bytes()).await;
        });
        reqwest::get(format!("http://{address}/")).await.unwrap()
    }

    /// A registry whose origin answers one request, then refuses connections.
    async fn registry_answering_once(response: &'static str) -> GitHubRegistry {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            let _ = socket.write_all(response.as_bytes()).await;
        });
        GitHubRegistry::with_origins(origin.clone(), origin).unwrap()
    }
    const EMPTY_LIST: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]";
    const NOT_FOUND: &str =
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    const LIMITED: &str =
        "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    fn acme() -> GitHubRepository {
        "acme/example".parse().unwrap()
    }

    #[tokio::test]
    async fn release_lists_are_reused_rather_than_spending_the_rate_limit() {
        let registry = registry_answering_once(EMPTY_LIST).await;
        registry.releases(&acme()).await.unwrap();
        registry.releases(&acme()).await.unwrap();
    }

    #[tokio::test]
    async fn catalogue_failures_say_why_and_are_not_reused() {
        let registry = registry_answering_once(NOT_FOUND).await;
        assert!(matches!(
            registry.discover(&acme()).await,
            Err(RegistryError::NotFound)
        ));
        let registry = registry_answering_once(LIMITED).await;
        assert!(matches!(
            registry.releases(&acme()).await,
            Err(RegistryError::RateLimited)
        ));
        assert!(matches!(
            registry.releases(&acme()).await,
            Err(RegistryError::Unavailable)
        ));
    }

    #[tokio::test]
    async fn a_repository_without_a_readme_can_still_be_shown() {
        let registry = registry_answering_once(NOT_FOUND).await;
        assert_eq!(registry.readme(&acme()).await.unwrap(), "");
    }

    #[tokio::test]
    async fn a_body_past_the_limit_is_refused_without_a_declared_size() {
        const LIMIT: usize = 1024;
        assert_eq!(
            read_capped(undeclared_size_response(LIMIT).await, LIMIT)
                .await
                .unwrap()
                .len(),
            LIMIT
        );
        assert!(matches!(
            read_capped(undeclared_size_response(LIMIT + 1).await, LIMIT).await,
            Err(RegistryError::InvalidResponse)
        ));
    }

    #[test]
    fn canonicalizes_only_safe_github_sources() {
        let source: GitHubRepository = "https://github.com/Acme/Widget/".parse().unwrap();
        assert_eq!(source.identity(), "github:acme/widget");
        for invalid in [
            "http://github.com/acme/widget",
            "https://evil.test/acme/widget",
            "github:acme/widget/extra",
            "https://user@github.com/acme/widget",
            "https://github.com:444/acme/widget",
        ] {
            assert!(invalid.parse::<GitHubRepository>().is_err(), "{invalid}");
        }
    }
}
