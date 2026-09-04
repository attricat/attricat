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
    extension_registry::{DEFAULT_OFFICIAL_REGISTRY, GitHubRepository},
    repository::ExtensionRegistrySource,
};

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
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<CreateSourceRequest>,
) -> Result<(StatusCode, Json<SourceResponse>), ApiError> {
    let source = GitHubRepository::from_str(&input.source)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let created = repository.add_extension_registry_source(&source).await?;
    Ok((StatusCode::CREATED, Json(SourceResponse::custom(created))))
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
) -> Result<Json<Vec<crate::extension_registry::DiscoveredRelease>>, ApiError> {
    let mut configured = vec![state.official_registry.clone()];
    configured.extend(
        repository
            .extension_registry_sources()
            .await?
            .into_iter()
            .map(|source| GitHubRepository {
                owner: source.owner,
                repository: source.repository,
            }),
    );
    let mut releases = Vec::new();
    for source in configured {
        match state.registry.discover(&source).await {
            Ok(found) => releases.extend(found),
            // One unavailable registry must not hide releases from the remaining
            // configured trusted sources. Marketplace UX can surface per-source
            // health when it is introduced in #148.
            Err(error) => {
                tracing::warn!(source = %source.identity(), %error, "extension registry discovery failed")
            }
        }
    }
    Ok(Json(releases))
}

#[allow(dead_code)]
fn _official_default_is_valid() {
    let _ = GitHubRepository::from_str(DEFAULT_OFFICIAL_REGISTRY)
        .expect("official registry must be valid");
}
