use std::str::FromStr;

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    AppState,
    auth::ScopedRepository,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    extension_registry::{DiscoveredExtension, GitHubRepository, RegistryError},
    repository::{AttricatRepository, ExtensionRegistrySource},
};

/// Separates the repository from the release tag in an installation's source.
const RELEASE_TAG_SEPARATOR: char = '@';

/// The API's answer to a failed lookup of a repository's catalogue.
pub(super) fn catalogue_error(error: RegistryError) -> ApiError {
    match error {
        RegistryError::NotFound => ApiError::not_found("extension repository"),
        RegistryError::RateLimited => ApiError::service_unavailable(
            "GitHub's rate limit for this server is used up; try again within an hour",
        ),
        _ => ApiError::service_unavailable("extension repository could not be resolved"),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateSourceRequest {
    source: String,
}
#[derive(Serialize)]
pub(super) struct SourceResponse {
    id: Option<Uuid>,
    kind: &'static str,
    source: String,
    official: bool,
}
impl SourceResponse {
    fn official(source: GitHubRepository) -> Self {
        Self {
            id: None,
            kind: "github_repository",
            source: source.identity(),
            official: true,
        }
    }
    fn custom(source: ExtensionRegistrySource) -> Self {
        Self {
            id: Some(source.id),
            kind: "github_repository",
            source: format!("github:{}/{}", source.owner, source.repository),
            official: false,
        }
    }
}

pub(super) async fn list(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<SourceResponse>>, ApiError> {
    let mut sources = vec![SourceResponse::official(state.official_registry.clone())];
    sources.extend(
        repository
            .extension_registry_sources()
            .await?
            .into_iter()
            .map(SourceResponse::custom),
    );
    Ok(Json(sources))
}
pub(super) async fn create(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateSourceRequest>,
) -> Result<(StatusCode, Json<SourceResponse>), ApiError> {
    if state.demo_mode {
        return Err(ApiError::disabled_in_demo());
    }
    let source = GitHubRepository::from_str(&input.source)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    if source == state.official_registry {
        return Err(ApiError::conflict(
            "the official registry is always included",
        ));
    }
    // A source that lists nothing would only make later lookups fail.
    match state.registry.discover(&source).await {
        Ok(_) => {}
        Err(RegistryError::NotFound) => {
            return Err(ApiError::invalid_input(format!(
                "{} has no registry.json on its default branch",
                source.identity()
            )));
        }
        Err(RegistryError::InvalidResponse | RegistryError::InvalidSource) => {
            return Err(ApiError::invalid_input(format!(
                "the registry.json of {} isn't valid",
                source.identity()
            )));
        }
        Err(_) => {
            return Err(ApiError::service_unavailable(
                "the registry could not be checked; try again later",
            ));
        }
    }
    Ok((
        StatusCode::CREATED,
        Json(SourceResponse::custom(
            repository.add_extension_registry_source(&source).await?,
        )),
    ))
}
pub(super) async fn remove(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    repository.remove_extension_registry_source(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn discover(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<crate::extension_registry::DiscoveredExtension>>, ApiError> {
    Ok(Json(discovered_extensions(&state, &repository).await?))
}

/// An installed registry extension's newest version its registry reviewed.
#[derive(Serialize)]
pub(super) struct ExtensionUpdate {
    extension_id: String,
    installed_version: String,
    latest_version: String,
    repository: String,
    registry_source: String,
}

/// Compares installations with the registry indexes, without calling GitHub's
/// release API. A registry that cannot be read reports no updates.
pub(super) async fn updates(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<ExtensionUpdate>>, ApiError> {
    let installations = repository.installed_extensions().await?;
    let listed = discovered_extensions(&state, &repository).await?;
    Ok(Json(
        installations
            .into_iter()
            .filter_map(|installation| {
                let (source, _) = installation.source.split_once(RELEASE_TAG_SEPARATOR)?;
                let entry = listed.iter().find(|entry| entry.repository == source)?;
                let latest_version = entry.available_update(&installation.version)?;
                Some(ExtensionUpdate {
                    extension_id: installation.extension_id,
                    installed_version: installation.version,
                    latest_version,
                    repository: entry.repository.clone(),
                    registry_source: entry.registry_source.clone(),
                })
            })
            .collect(),
    ))
}

/// Every extension the readable registries list.
async fn discovered_extensions(
    state: &AppState,
    repository: &AttricatRepository,
) -> Result<Vec<DiscoveredExtension>, ApiError> {
    let mut extensions: Vec<DiscoveredExtension> = Vec::new();
    for source in registry_sources(state, repository).await? {
        match state.registry.discover(&source).await {
            // Sources come in trust order, so the first to list a repository
            // keeps it.
            Ok(found) => {
                for extension in found {
                    if !extensions
                        .iter()
                        .any(|listed| listed.repository == extension.repository)
                    {
                        extensions.push(extension);
                    }
                }
            }
            Err(error) => {
                tracing::warn!(source = %source.identity(), %error, "extension registry discovery failed")
            }
        }
    }
    Ok(extensions)
}

/// Resolves a repository only after its current trusted registry index lists it.
pub(super) async fn extension_details(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiPath((owner, repository_name)): ApiPath<(String, String)>,
) -> Result<Json<crate::extension_registry::ExtensionDetails>, ApiError> {
    let target = GitHubRepository::from_str(&format!("{owner}/{repository_name}"))
        .map_err(|error| ApiError::invalid_input(error.to_string()))?
        .identity();
    let entry = find_trusted_extension(&state, &repository, &target).await?;
    Ok(Json(
        state
            .registry
            .extension_details(entry)
            .await
            .map_err(catalogue_error)?,
    ))
}

/// Registry sources in trust order: the official registry, then the
/// workspace's configured registries.
async fn registry_sources(
    state: &AppState,
    repository: &AttricatRepository,
) -> Result<Vec<GitHubRepository>, ApiError> {
    let mut sources = vec![state.official_registry.clone()];
    sources.extend(
        repository
            .extension_registry_sources()
            .await?
            .into_iter()
            .map(|source| GitHubRepository {
                owner: source.owner,
                repository: source.repository,
            }),
    );
    Ok(sources)
}

/// Finds `target` in the first trusted registry that lists it. A failing
/// source is skipped so one broken registry cannot block lookups served by the
/// others; the lookup is unavailable only if the target was not found and a
/// source could not be checked. A source that no longer exists lists nothing,
/// which is an answer.
pub(super) async fn find_trusted_extension(
    state: &AppState,
    repository: &AttricatRepository,
    target: &str,
) -> Result<DiscoveredExtension, ApiError> {
    let mut unavailable = false;
    for source in registry_sources(state, repository).await? {
        match state.registry.discover(&source).await {
            Ok(entries) => {
                if let Some(entry) = entries.into_iter().find(|entry| entry.repository == target) {
                    return Ok(entry);
                }
            }
            Err(error) => {
                tracing::warn!(source = %source.identity(), %error, "extension registry discovery failed");
                unavailable |= !matches!(error, RegistryError::NotFound);
            }
        }
    }
    Err(if unavailable {
        ApiError::service_unavailable("extension registry discovery failed")
    } else {
        ApiError::not_found("trusted extension repository")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension_registry::DEFAULT_OFFICIAL_REGISTRY;

    #[test]
    fn official_default_registry_is_valid() {
        GitHubRepository::from_str(DEFAULT_OFFICIAL_REGISTRY).unwrap();
    }
}
