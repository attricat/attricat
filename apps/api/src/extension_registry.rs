//! Trusted extension-registry index discovery.
//!
//! A registry is a GitHub repository containing `registry.json`. Its index
//! points to extension repositories; only those index-authorized repositories
//! may have their README and release assets resolved. No index, README,
//! release catalogue, or archive is persisted.

use std::str::FromStr;

use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_OFFICIAL_REGISTRY: &str = "attricat/attricat-extensions";
const GITHUB_API_ORIGIN: &str = "https://api.github.com";
const GITHUB_RAW_ORIGIN: &str = "https://raw.githubusercontent.com";
const MAX_README_BYTES: usize = 256 * 1024;
const MAX_REGISTRY_INDEX_BYTES: usize = 1024 * 1024;
const MAX_REGISTRY_ENTRIES: usize = 1_000;
const MAX_REGISTRY_NAME_BYTES: usize = 256;
const MAX_REGISTRY_DESCRIPTION_BYTES: usize = 4 * 1024;
const MAX_REGISTRY_ICON_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GitHubRepository {
    pub owner: String,
    pub repository: String,
}
impl GitHubRepository {
    pub fn identity(&self) -> String {
        format!("github:{}/{}", self.owner, self.repository)
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
        let path = value.strip_prefix("github:").unwrap_or(value);
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
        && value.len() <= 39
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
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The public, versioned raw registry document at `registry.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryIndex {
    registry_version: u32,
    extensions: Vec<RegistryIndexEntry>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryIndexEntry {
    id: String,
    repository: String,
    name: String,
    description: String,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveredExtension {
    pub registry_source: String,
    pub id: String,
    pub repository: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
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
#[derive(Clone, Debug, Serialize)]
pub struct ReleaseAsset {
    pub id: u64,
    pub name: String,
    pub download_url: String,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry source must be a GitHub owner/repository identifier")]
    InvalidSource,
    #[error("registry request failed")]
    Unavailable,
    #[error("registry returned an invalid index or release response")]
    InvalidResponse,
    #[error("extension repository is not listed by a trusted registry")]
    UntrustedExtension,
}

#[derive(Clone)]
pub struct GitHubRegistry {
    client: Client,
    api_origin: Url,
    raw_origin: Url,
}
impl GitHubRegistry {
    pub fn new() -> Result<Self, RegistryError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .user_agent("catalog-extension-registry")
            .build()
            .map_err(|_| RegistryError::Unavailable)?;
        Ok(Self {
            client,
            api_origin: Url::parse(GITHUB_API_ORIGIN).expect("constant URL"),
            raw_origin: Url::parse(GITHUB_RAW_ORIGIN).expect("constant URL"),
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
            return Err(RegistryError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_REGISTRY_INDEX_BYTES as u64)
        {
            return Err(RegistryError::InvalidResponse);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if bytes.len() > MAX_REGISTRY_INDEX_BYTES {
            return Err(RegistryError::InvalidResponse);
        }
        let index: RegistryIndex =
            serde_json::from_slice(&bytes).map_err(|_| RegistryError::InvalidResponse)?;
        if index.registry_version != 1 || index.extensions.len() > MAX_REGISTRY_ENTRIES {
            return Err(RegistryError::InvalidResponse);
        }
        let mut ids = std::collections::HashSet::new();
        index
            .extensions
            .into_iter()
            .map(|entry| {
                if entry.id.is_empty()
                    || entry.id.len() > 128
                    || !entry.id.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
                    || entry.name.trim().is_empty()
                    || entry.name.len() > MAX_REGISTRY_NAME_BYTES
                    || entry.description.trim().is_empty()
                    || entry.description.len() > MAX_REGISTRY_DESCRIPTION_BYTES
                    || entry
                        .icon
                        .as_ref()
                        .is_some_and(|icon| icon.len() > MAX_REGISTRY_ICON_BYTES)
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
                })
            })
            .collect()
    }
    pub async fn extension_details(
        &self,
        extension: DiscoveredExtension,
    ) -> Result<ExtensionDetails, RegistryError> {
        let repository = extension
            .repository
            .strip_prefix("github:")
            .ok_or(RegistryError::InvalidResponse)?
            .parse::<GitHubRepository>()?;
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
        if !response.status().is_success() {
            return Err(RegistryError::Unavailable);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if bytes.len() > MAX_README_BYTES {
            return Err(RegistryError::InvalidResponse);
        }
        let readme =
            String::from_utf8(bytes.to_vec()).map_err(|_| RegistryError::InvalidResponse)?;
        let releases = self.releases(&repository).await?;
        Ok(ExtensionDetails {
            extension,
            readme,
            releases,
        })
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
            .trim_start_matches("github:")
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
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|size| size > crate::extensions::MAX_EXTENSION_ARCHIVE_BYTES as u64)
        {
            return Err(RegistryError::Unavailable);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| RegistryError::Unavailable)?;
        if bytes.len() > crate::extensions::MAX_EXTENSION_ARCHIVE_BYTES {
            return Err(RegistryError::InvalidResponse);
        }
        Ok(bytes.to_vec())
    }

    async fn releases(
        &self,
        source: &GitHubRepository,
    ) -> Result<Vec<DiscoveredRelease>, RegistryError> {
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
            return Err(RegistryError::Unavailable);
        }
        let releases: Vec<GitHubRelease> = response
            .json()
            .await
            .map_err(|_| RegistryError::InvalidResponse)?;
        Ok(releases
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
                    },
                })
            })
            .collect())
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
