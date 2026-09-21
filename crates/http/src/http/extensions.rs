use std::str::FromStr;

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{AppState, auth::ScopedRepository, error::ApiError, extractors::ApiJson};
use crate::{
    constants::DEFAULT_LIST_PAGE_SIZE,
    extension_installer::ExtensionInstaller,
    extension_registry::{DiscoveredRelease, GitHubRepository},
    extensions::{
        ExtensionPackage, MAX_EXTENSION_IDENTIFIER_BYTES, UiContributionKind, UiOutlet,
        validate_schema,
    },
    repository::{
        ExtensionGrant, ExtensionInstallation, ExtensionLifecycleRecord, ExtensionOperationRun,
        ExtensionStorageError, InstalledExtension, StartExtensionOperation,
    },
    storage::ObjectStoreError,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CommandRequest {
    release_id: Uuid,
    command_id: String,
    payload: Value,
}

#[derive(Deserialize)]
pub(super) struct RuntimeQuery {
    blueprint_id: Option<Uuid>,
    blueprint_version: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct RuntimeContribution {
    contribution_key: String,
    display_order: u32,
    navigation_group: Option<String>,
    extension_id: String,
    extension_name: String,
    release_id: Uuid,
    configuration: Value,
    capabilities: Vec<String>,
    id: String,
    version: u32,
    kind: UiContributionKind,
    outlet: Option<UiOutlet>,
    route: Option<String>,
    title: Option<String>,
}

#[derive(Serialize)]
pub(super) struct InstallationResponse {
    id: Uuid,
    extension_id: String,
    installed_release_id: Uuid,
    state: String,
    configuration: Value,
    configuration_version: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    version: String,
    manifest: Value,
    manifest_sha256: String,
    source: String,
}
impl From<InstalledExtension> for InstallationResponse {
    fn from(value: InstalledExtension) -> Self {
        Self {
            id: value.id,
            extension_id: value.extension_id,
            installed_release_id: value.installed_release_id,
            state: value.state,
            configuration: value.configuration,
            configuration_version: value.configuration_version,
            created_at: value.created_at,
            updated_at: value.updated_at,
            version: value.version,
            manifest: value.manifest,
            manifest_sha256: value.manifest_sha256,
            source: value.source,
        }
    }
}
impl From<ExtensionInstallation> for InstallationResponse {
    fn from(value: ExtensionInstallation) -> Self {
        // Mutation responses intentionally omit immutable metadata. The client
        // invalidates and reloads the management projection after a mutation.
        Self {
            id: value.id,
            extension_id: value.extension_id,
            installed_release_id: value.installed_release_id,
            state: value.state,
            configuration: value.configuration,
            configuration_version: value.configuration_version,
            created_at: value.created_at,
            updated_at: value.updated_at,
            version: String::new(),
            manifest: Value::Null,
            manifest_sha256: String::new(),
            source: String::new(),
        }
    }
}
#[derive(Serialize)]
struct GrantResponse {
    grant_kind: String,
    grant_id: String,
    granted_at: chrono::DateTime<chrono::Utc>,
}
impl From<ExtensionGrant> for GrantResponse {
    fn from(value: ExtensionGrant) -> Self {
        Self {
            grant_kind: value.grant_kind,
            grant_id: value.grant_id,
            granted_at: value.granted_at,
        }
    }
}
#[derive(Serialize)]
struct LifecycleResponse {
    id: Uuid,
    operation: String,
    prior_state: Option<String>,
    new_state: Option<String>,
    outcome: String,
    actor_user_id: Option<Uuid>,
    actor_token_id: Option<Uuid>,
    source: Option<String>,
    diagnostics: Value,
    created_at: chrono::DateTime<chrono::Utc>,
}
impl From<ExtensionLifecycleRecord> for LifecycleResponse {
    fn from(value: ExtensionLifecycleRecord) -> Self {
        Self {
            id: value.id,
            operation: value.operation,
            prior_state: value.prior_state,
            new_state: value.new_state,
            outcome: value.outcome,
            actor_user_id: value.actor_user_id,
            actor_token_id: value.actor_token_id,
            source: value.source,
            diagnostics: value.diagnostics,
            created_at: value.created_at,
        }
    }
}
#[derive(Serialize)]
pub(super) struct ExtensionDetailResponse {
    installation: InstallationResponse,
    grants: Vec<GrantResponse>,
    lifecycle: Vec<LifecycleResponse>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseRequest {
    owner: String,
    repository: String,
    release_id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfigurationRequest {
    configuration: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrantRequest {
    grant_kind: String,
    grant_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuarantineRequest {
    diagnostic_code: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceExtensionsModeRequest {
    enabled: bool,
}

#[derive(Serialize)]
pub(super) struct WorkspaceExtensionsModeResponse {
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartOperationRequest {
    operation_id: String,
    input: Value,
    #[serde(default = "empty_object")]
    source_reference: Value,
    #[serde(default = "empty_object")]
    destination_reference: Value,
    idempotency_key: String,
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Serialize)]
pub(super) struct OperationRunResponse {
    id: Uuid,
    extension_id: String,
    installed_release_id: Uuid,
    abi_version: String,
    operation_id: String,
    status: String,
    progress: Value,
    attempts: i32,
    last_error_code: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ExtensionOperationRun> for OperationRunResponse {
    fn from(value: ExtensionOperationRun) -> Self {
        Self {
            id: value.id,
            extension_id: value.extension_id,
            installed_release_id: value.installed_release_id,
            abi_version: value.abi_version,
            operation_id: value.operation_id,
            status: value.status,
            progress: value.progress,
            attempts: value.attempts,
            last_error_code: value.last_error_code,
            created_at: value.created_at,
            completed_at: value.completed_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum StorageRequest {
    Get {
        key: String,
    },
    Set {
        key: String,
        value: Value,
        expected_revision: Option<i64>,
    },
    Delete {
        key: String,
        expected_revision: Option<i64>,
    },
    List {
        prefix: Option<String>,
        cursor: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Serialize)]
struct StorageEntryResponse {
    key: String,
    value: Value,
    revision: i64,
}
#[derive(Serialize)]
struct StoragePageResponse {
    entries: Vec<StorageEntryResponse>,
    cursor: Option<String>,
}

fn storage_error(error: ExtensionStorageError) -> ApiError {
    match error {
        ExtensionStorageError::Denied => ApiError::forbidden(),
        ExtensionStorageError::Conflict => ApiError::storage_conflict(),
        ExtensionStorageError::QuotaExceeded => ApiError::storage_quota_exceeded(),
        ExtensionStorageError::InvalidKey
        | ExtensionStorageError::InvalidValue
        | ExtensionStorageError::InvalidLimit => ApiError::invalid_input(error.to_string()),
        ExtensionStorageError::Database(_) | ExtensionStorageError::Audit(_) => {
            ApiError::internal("extension storage operation failed")
        }
    }
}

/// Changes the workspace emergency gate without changing installation state or
/// grants. The audit middleware records the operator and request context.
pub(super) async fn set_workspace_mode(
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<WorkspaceExtensionsModeRequest>,
) -> Result<Json<WorkspaceExtensionsModeResponse>, ApiError> {
    repository
        .set_workspace_extensions_enabled(input.enabled)
        .await?;
    Ok(Json(WorkspaceExtensionsModeResponse {
        enabled: input.enabled,
    }))
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<InstallationResponse>>, ApiError> {
    Ok(Json(
        repository
            .installed_extensions()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}
pub(super) async fn detail(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
) -> Result<Json<ExtensionDetailResponse>, ApiError> {
    let installation = repository.installed_extension(&extension_id).await?;
    let grants = repository.extension_grants(&extension_id).await?;
    let lifecycle = repository
        .extension_lifecycle_history(&extension_id)
        .await?;
    Ok(Json(ExtensionDetailResponse {
        installation: installation.into(),
        grants: grants.into_iter().map(Into::into).collect(),
        lifecycle: lifecycle.into_iter().map(Into::into).collect(),
    }))
}

async fn selected_release(
    state: &AppState,
    repository: &crate::repository::CatalogRepository,
    input: &ReleaseRequest,
) -> Result<DiscoveredRelease, ApiError> {
    let target = GitHubRepository::from_str(&format!("{}/{}", input.owner, input.repository))
        .map_err(|error| ApiError::invalid_input(error.to_string()))?
        .identity();
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
    for source in sources {
        let entries =
            state.registry.discover(&source).await.map_err(|_| {
                ApiError::service_unavailable("extension registry discovery failed")
            })?;
        if let Some(extension) = entries.into_iter().find(|entry| entry.repository == target) {
            let details = state
                .registry
                .extension_details(extension)
                .await
                .map_err(|_| {
                    ApiError::service_unavailable("extension repository could not be resolved")
                })?;
            return details
                .releases
                .into_iter()
                .find(|release| release.release_id == input.release_id)
                .ok_or_else(|| ApiError::not_found("trusted extension release"));
        }
    }
    Err(ApiError::not_found("trusted extension repository"))
}

/// Installs a locally supplied archive. The archive goes through the exact
/// same manifest validation, bounded unpacking, artifact staging, and
/// lifecycle recording as a registry release.
pub(super) async fn sideload(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    headers: HeaderMap,
    archive: Bytes,
) -> Result<(StatusCode, Json<InstallationResponse>), ApiError> {
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if media_type != Some("application/zstd") {
        return Err(ApiError::invalid_input(
            "extension archives must use application/zstd".into(),
        ));
    }
    let installation = ExtensionInstaller::new(repository, state.object_store.clone())
        .install("sideload", &archive)
        .await
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(installation.into())))
}

pub(super) async fn install(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiJson(input): ApiJson<ReleaseRequest>,
) -> Result<(StatusCode, Json<InstallationResponse>), ApiError> {
    let release = selected_release(&state, &repository, &input).await?;
    let archive = state
        .registry
        .download_release_asset(&release)
        .await
        .map_err(|_| ApiError::service_unavailable("extension archive could not be downloaded"))?;
    let source = format!("{}@{}", release.source, release.tag_name);
    let installation = ExtensionInstaller::new(repository, state.object_store.clone())
        .install(&source, &archive)
        .await
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(installation.into())))
}
pub(super) async fn upgrade(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
    ApiJson(input): ApiJson<ReleaseRequest>,
) -> Result<Json<InstallationResponse>, ApiError> {
    let release = selected_release(&state, &repository, &input).await?;
    let archive = state
        .registry
        .download_release_asset(&release)
        .await
        .map_err(|_| ApiError::service_unavailable("extension archive could not be downloaded"))?;
    // Verify the package identity before passing it to the installer. Checking
    // only its returned installation would allow a selected archive to mutate
    // a different installed extension before this handler rejects the request.
    let package = ExtensionPackage::from_tar_zst(&archive)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    if package.manifest().catalog.id != extension_id {
        return Err(ApiError::invalid_input(
            "selected release has a different extension ID".to_owned(),
        ));
    }
    let source = format!("{}@{}", release.source, release.tag_name);
    let installation = ExtensionInstaller::new(repository, state.object_store.clone())
        .upgrade(&source, &archive)
        .await
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    Ok(Json(installation.into()))
}
pub(super) async fn configure(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
    ApiJson(input): ApiJson<ConfigurationRequest>,
) -> Result<Json<InstallationResponse>, ApiError> {
    Ok(Json(
        repository
            .configure_extension(&extension_id, input.configuration)
            .await?
            .into(),
    ))
}
pub(super) async fn grant(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
    ApiJson(input): ApiJson<GrantRequest>,
) -> Result<StatusCode, ApiError> {
    repository
        .grant_extension(&extension_id, &input.grant_kind, &input.grant_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn revoke(
    ScopedRepository(repository): ScopedRepository,
    Path((extension_id, grant_kind, grant_id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    repository
        .revoke_extension_grant(&extension_id, &grant_kind, &grant_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn enable(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
) -> Result<Json<InstallationResponse>, ApiError> {
    Ok(Json(
        repository.enable_extension(&extension_id).await?.into(),
    ))
}
pub(super) async fn disable(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
) -> Result<Json<InstallationResponse>, ApiError> {
    Ok(Json(
        repository.disable_extension(&extension_id).await?.into(),
    ))
}
pub(super) async fn quarantine(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
    ApiJson(input): ApiJson<QuarantineRequest>,
) -> Result<Json<InstallationResponse>, ApiError> {
    Ok(Json(
        repository
            .quarantine_extension(&extension_id, &input.diagnostic_code)
            .await?
            .into(),
    ))
}
pub(super) async fn remove(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    repository.remove_extension(&extension_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Starts a release-pinned operation. Only bounded object input crosses this
/// endpoint; operator projections intentionally never return that input.
pub(super) async fn start_operation(
    ScopedRepository(repository): ScopedRepository,
    Path(extension_id): Path<String>,
    ApiJson(input): ApiJson<StartOperationRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.operation_id.len() > MAX_EXTENSION_IDENTIFIER_BYTES {
        return Err(ApiError::invalid_input(
            "invalid extension operation request".into(),
        ));
    }
    let installation = repository
        .runtime_extension_installation(
            &extension_id,
            repository
                .installed_extension(&extension_id)
                .await?
                .installed_release_id,
        )
        .await?
        .ok_or_else(ApiError::forbidden)?;
    let operation = installation
        .manifest
        .server
        .as_ref()
        .and_then(|server| {
            server
                .operations
                .iter()
                .find(|operation| operation.id == input.operation_id)
        })
        .ok_or_else(|| ApiError::not_found("extension operation"))?;
    validate_schema(&operation.request_schema, &input.input)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let id = repository
        .start_extension_operation(StartExtensionOperation {
            extension_id,
            expected_release_id: installation.installed_release_id,
            operation_id: input.operation_id,
            input: input.input,
            source_reference: input.source_reference,
            destination_reference: input.destination_reference,
            idempotency_key: input.idempotency_key,
        })
        .await?;
    Ok((StatusCode::ACCEPTED, Json(json!({"id": id}))))
}

pub(super) async fn list_operation_runs(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<OperationRunResponse>>, ApiError> {
    Ok(Json(
        repository
            .list_extension_operation_runs()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

pub(super) async fn cancel_operation(
    ScopedRepository(repository): ScopedRepository,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repository.cancel_extension_operation(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("extension operation run"))
    }
}

pub(super) async fn replay_operation(
    ScopedRepository(repository): ScopedRepository,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repository.replay_extension_operation(id).await? {
        Ok(StatusCode::ACCEPTED)
    } else {
        Err(ApiError::not_found("dead-lettered extension operation run"))
    }
}

/// Returns only contributions from currently enabled installations.
pub(super) async fn workspace_extension_layout(
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(repository.workspace_extension_layout().await?))
}

pub(super) async fn update_workspace_extension_layout(
    ScopedRepository(repository): ScopedRepository,
    ApiJson(layout): ApiJson<Value>,
) -> Result<StatusCode, ApiError> {
    repository.update_workspace_extension_layout(layout).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn runtime(
    Query(query): Query<RuntimeQuery>,
    ScopedRepository(repository): ScopedRepository,
) -> Result<Json<Vec<RuntimeContribution>>, ApiError> {
    let blueprint = match (query.blueprint_id, query.blueprint_version) {
        (Some(id), Some(version)) if version > 0 => Some((id, version)),
        (None, None) => None,
        _ => {
            return Err(ApiError::invalid_input(
                "invalid extension runtime scope".into(),
            ));
        }
    };
    let contributions = repository
        .client_extension_contributions_for_blueprint(blueprint)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        contributions
            .into_iter()
            .map(|item| RuntimeContribution {
                contribution_key: item.contribution_key,
                display_order: item.display_order,
                navigation_group: item.navigation_group,
                extension_id: item.extension_id,
                extension_name: item.extension_name,
                release_id: item.installed_release_id,
                configuration: item.configuration,
                capabilities: item.capabilities,
                id: item.id,
                version: item.version,
                kind: item.kind,
                outlet: item.outlet,
                route: item.route,
                title: item.title,
            })
            .collect(),
    ))
}

/// The frame broker is the only client storage path. Resolving the contribution
/// on every request ties storage to an enabled current release, not IDs supplied
/// by the opaque-origin frame.
/// Resolves a manifest-declared command through the same enabled-release gate
/// as artifacts and storage. The command request is intentionally mediated;
/// components never receive browser credentials or a direct endpoint.
pub(super) async fn command(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    Path((extension_id, contribution_id)): Path<(String, String)>,
    ApiJson(input): ApiJson<CommandRequest>,
) -> Result<Json<Value>, ApiError> {
    if input.command_id.len() > MAX_EXTENSION_IDENTIFIER_BYTES {
        return Err(ApiError::invalid_input(
            "invalid extension command request".into(),
        ));
    }
    let contribution = repository
        .client_extension_contribution(&extension_id, &contribution_id)
        .await?;
    if contribution.installed_release_id != input.release_id
        || contribution.kind == UiContributionKind::Panel
        || !contribution
            .capabilities
            .iter()
            .any(|capability| capability == "client.commands")
    {
        return Err(ApiError::forbidden());
    }
    let installation = repository
        .runtime_extension_installation(&extension_id, input.release_id)
        .await?
        .ok_or_else(ApiError::forbidden)?;
    let command = installation
        .manifest
        .server
        .as_ref()
        .and_then(|server| {
            server
                .commands
                .iter()
                .find(|command| command.id == input.command_id)
        })
        .ok_or_else(|| ApiError::not_found("extension command"))?;
    validate_schema(&command.request_schema, &input.payload)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let request = serde_json::to_string(&input.payload)
        .map_err(|_| ApiError::invalid_input("invalid extension command payload".into()))?;
    if request.len() > command.max_request_bytes as usize {
        return Err(ApiError::invalid_input(
            "extension command request exceeds its declared byte limit".into(),
        ));
    }
    let response = state
        .extension_runtime
        .invoke_command(
            &installation,
            repository.for_extension(&extension_id),
            &command.handler,
            &request,
            command.max_response_bytes,
        )
        .await
        .map_err(|_| ApiError::service_unavailable("extension command failed"))?;
    let response: Value = serde_json::from_str(&response)
        .map_err(|_| ApiError::service_unavailable("extension command returned invalid JSON"))?;
    validate_schema(&command.response_schema, &response)
        .map_err(|_| ApiError::service_unavailable("extension command returned invalid data"))?;
    Ok(Json(response))
}

pub(super) async fn storage(
    ScopedRepository(repository): ScopedRepository,
    Path((extension_id, contribution_id, release_id)): Path<(String, String, Uuid)>,
    ApiJson(input): ApiJson<StorageRequest>,
) -> Result<Json<Value>, ApiError> {
    let contribution = repository
        .client_extension_contribution(&extension_id, &contribution_id)
        .await?;
    if contribution.installed_release_id != release_id
        || !contribution
            .capabilities
            .iter()
            .any(|capability| capability == "storage.extension")
    {
        return Err(ApiError::forbidden());
    }
    let response = match input {
        StorageRequest::Get { key } => serde_json::to_value(repository.extension_storage_get(&extension_id, release_id, &key).await.map_err(storage_error)?.map(|entry| StorageEntryResponse { key: entry.key, value: entry.value, revision: entry.revision })).expect("storage response serializes"),
        StorageRequest::Set { key, value, expected_revision } => serde_json::to_value(json!({"revision": repository.extension_storage_set(&extension_id, release_id, &key, value, expected_revision).await.map_err(storage_error)?})).expect("storage response serializes"),
        StorageRequest::Delete { key, expected_revision } => { repository.extension_storage_delete(&extension_id, release_id, &key, expected_revision).await.map_err(storage_error)?; json!(null) },
        StorageRequest::List { prefix, cursor, limit } => {
            let page = repository.extension_storage_list(&extension_id, release_id, prefix.as_deref(), cursor.as_deref(), limit.unwrap_or(DEFAULT_LIST_PAGE_SIZE)).await.map_err(storage_error)?;
            serde_json::to_value(StoragePageResponse { entries: page.entries.into_iter().map(|entry| StorageEntryResponse { key: entry.key, value: entry.value, revision: entry.revision }).collect(), cursor: page.cursor }).expect("storage response serializes")
        }
    };
    Ok(Json(response))
}

pub(super) async fn artifact(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    Path((extension_id, contribution_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let contribution = repository
        .client_extension_contribution(&extension_id, &contribution_id)
        .await
        .map_err(ApiError::from)?;
    let artifact_key = contribution
        .artifact_key
        .ok_or_else(|| ApiError::not_found("extension artifact"))?;
    let object =
        state
            .object_store
            .get_stream(&artifact_key)
            .await
            .map_err(|error| match error {
                ObjectStoreError::Unavailable | ObjectStoreError::TimedOut(_) => {
                    ApiError::service_unavailable("extension artifact storage is unavailable")
                }
                ObjectStoreError::Operation(_) => {
                    ApiError::internal("extension artifact could not be loaded")
                }
            })?;
    let mut response = Response::new(Body::from_stream(object.stream));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
