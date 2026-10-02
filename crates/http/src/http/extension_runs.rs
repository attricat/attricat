//! End-user extension runs and extension annotation management.
//!
//! These routes are deliberately separate from the operator management API.
//! A run is visible to its initiator while that user can still read every
//! selected entity, and to workspace operators holding `extensions.manage`.

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    extensions::{
        MAX_EXTENSION_IDENTIFIER_BYTES, SELECTION_ACTION_CONTRIBUTION_VERSION, UiOutlet,
        selection_action_outlet, validate_schema,
    },
    repository::{
        AuthorizationActor, CatalogRepository, ExtensionAnnotationNamespace,
        ExtensionAnnotationPatch, ExtensionAnnotations, InteractiveRun, InteractiveRunArtifact,
        RepositoryError, StartInteractiveOperation,
    },
    storage::ObjectStoreError,
};

/// The frame never supplies a workspace, actor or installation; the host
/// derives those from the session and the resolved contribution. The
/// selection comes from the host-owned outlet context captured by the parent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartInteractiveRequest {
    release_id: Uuid,
    operation_id: String,
    input: Value,
    idempotency_key: String,
    selection: SelectionRequest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectionRequest {
    blueprint_id: Uuid,
    blueprint_version: i64,
    context_id: Option<Uuid>,
    entity_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
pub(super) struct RunListQuery {
    extension_id: Option<String>,
}

#[derive(Serialize)]
pub(super) struct RunDetailResponse {
    #[serde(flatten)]
    run: InteractiveRun,
    artifacts: Vec<InteractiveRunArtifact>,
}

fn actor(principal: AuthenticatedPrincipal) -> AuthorizationActor {
    AuthorizationActor {
        user_id: principal.0,
        token_id: principal.1,
    }
}

/// Resolves a run the caller may inspect. Other users' runs are reported as
/// missing rather than forbidden so run IDs cannot be probed.
async fn visible_run(
    state: &AppState,
    repository: &CatalogRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    run_id: Uuid,
) -> Result<InteractiveRun, ApiError> {
    let not_found = || ApiError::not_found("extension run");
    let run = repository
        .interactive_extension_run(run_id)
        .await?
        .ok_or_else(not_found)?;
    if run.actor_user_id == Some(principal.0) {
        // Frozen membership is not permission: the initiator must still be
        // able to read every member that may have contributed to the output.
        return match repository
            .ensure_principal_may_read_run_selection(actor(principal), run_id)
            .await
        {
            Ok(()) => Ok(run),
            Err(RepositoryError::ActorNotAuthorized) => Err(ApiError::forbidden()),
            Err(error) => Err(error.into()),
        };
    }
    let operator = state
        .repository
        .is_authorized(principal.0, workspace.0, "extensions.manage", None, None)
        .await?
        && match principal.1 {
            Some(token_id) => {
                state
                    .repository
                    .personal_api_token_permits(token_id, "extensions.manage")
                    .await?
            }
            None => true,
        };
    if operator { Ok(run) } else { Err(not_found()) }
}

/// Starts an interactive operation from a selection-aware contribution.
pub(super) async fn start(
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    ApiPath((extension_id, contribution_id)): ApiPath<(String, String)>,
    ApiJson(input): ApiJson<StartInteractiveRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.operation_id.len() > MAX_EXTENSION_IDENTIFIER_BYTES {
        return Err(ApiError::invalid_input(
            "invalid extension operation request".into(),
        ));
    }
    let contribution = repository
        .client_extension_contribution(&extension_id, &contribution_id)
        .await?;
    let selection_surface = match &contribution.outlet {
        Some(UiOutlet::ActionDialog) => true,
        Some(outlet) => {
            selection_action_outlet(outlet)
                && contribution.version == SELECTION_ACTION_CONTRIBUTION_VERSION
        }
        None => false,
    };
    if contribution.installed_release_id != input.release_id
        || !selection_surface
        || !contribution
            .capabilities
            .iter()
            .any(|capability| capability == "client.operations.start")
    {
        return Err(ApiError::forbidden());
    }
    let installation = repository
        .runtime_extension_installation(&extension_id, input.release_id)
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
        .filter(|operation| operation.interactive.is_some())
        .ok_or_else(|| ApiError::not_found("interactive extension operation"))?;
    validate_schema(&operation.request_schema, &input.input)
        .map_err(|error| ApiError::invalid_input(error.to_string()))?;
    let run_id = repository
        .start_interactive_extension_operation(StartInteractiveOperation {
            extension_id,
            contribution_id,
            expected_release_id: input.release_id,
            operation_id: input.operation_id,
            input: input.input,
            idempotency_key: input.idempotency_key,
            entity_ids: input.selection.entity_ids,
            blueprint_id: input.selection.blueprint_id,
            blueprint_version: input.selection.blueprint_version,
            context_id: input.selection.context_id,
            actor: actor(principal),
        })
        .await
        .map_err(|error| match error {
            RepositoryError::ActorNotAuthorized => ApiError::forbidden(),
            error => error.into(),
        })?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "run_id": run_id }))))
}

/// The signed-in user's own recent interactive runs.
pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    ApiQuery(query): ApiQuery<RunListQuery>,
) -> Result<Json<Vec<InteractiveRun>>, ApiError> {
    Ok(Json(
        repository
            .interactive_extension_runs(principal.0, query.extension_id.as_deref())
            .await?,
    ))
}

pub(super) async fn detail(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(run_id): ApiPath<Uuid>,
) -> Result<Json<RunDetailResponse>, ApiError> {
    let run = visible_run(&state, &repository, principal, workspace, run_id).await?;
    let artifacts = repository.interactive_run_artifacts(run_id).await?;
    Ok(Json(RunDetailResponse { run, artifacts }))
}

pub(super) async fn cancel(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(run_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let run = visible_run(&state, &repository, principal, workspace, run_id).await?;
    if !run.can_cancel {
        return Err(ApiError::conflict(
            "extension run can no longer be cancelled",
        ));
    }
    if repository.cancel_extension_operation(run_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::conflict(
            "extension run can no longer be cancelled",
        ))
    }
}

/// Keeps a safe, ASCII filename from an extension-chosen output name.
fn download_filename(name: Option<&str>, artifact_id: Uuid) -> String {
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect::<String>()
        .trim_start_matches('.')
        .chars()
        .take(128)
        .collect();
    if cleaned.is_empty() {
        format!("output-{artifact_id}")
    } else {
        cleaned
    }
}

pub(super) async fn download(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath((run_id, artifact_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    visible_run(&state, &repository, principal, workspace, run_id).await?;
    let name = repository
        .interactive_run_artifacts(run_id)
        .await?
        .into_iter()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| ApiError::not_found("completed extension output"))?
        .name;
    let artifact = repository
        .completed_extension_operation_artifact(run_id, artifact_id)
        .await?;
    let key = artifact
        .object_key
        .ok_or_else(|| ApiError::not_found("completed extension output"))?;
    let object = state
        .object_store
        .get_stream(&key)
        .await
        .map_err(|error| match error {
            ObjectStoreError::Unavailable | ObjectStoreError::TimedOut(_) => {
                ApiError::service_unavailable("extension output storage is unavailable")
            }
            ObjectStoreError::NotFound | ObjectStoreError::Operation(_) => {
                ApiError::internal("extension output could not be loaded")
            }
        })?;
    let mut response = Response::new(Body::from_stream(object.stream));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&artifact.media_type)
            .map_err(|_| ApiError::internal("stored output media type is invalid"))?,
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(artifact.content_length),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // Extension-controlled media must never render as active content.
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            download_filename(name.as_deref(), artifact_id)
        ))
        .map_err(|_| ApiError::internal("output filename is invalid"))?,
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox; default-src 'none'"),
    );
    Ok(response)
}

/// Operator inventory of an extension's annotation namespace.
pub(super) async fn annotation_namespace(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<Json<ExtensionAnnotationNamespace>, ApiError> {
    Ok(Json(
        repository
            .extension_annotation_namespace(&extension_id)
            .await?,
    ))
}

/// Explicitly adopts pre-existing annotations under an installed extension's
/// name. Data is preserved; nothing is renamed or deleted.
pub(super) async fn adopt_annotation_namespace(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(extension_id): ApiPath<String>,
) -> Result<Json<ExtensionAnnotationNamespace>, ApiError> {
    Ok(Json(
        repository
            .adopt_extension_annotation_namespace(&extension_id)
            .await?,
    ))
}

/// Privileged operator repair or cleanup of one extension namespace on an
/// entity. Requires `extensions.manage` plus `entities.write` on the entity.
pub(super) async fn repair_annotations(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath((extension_id, entity_id)): ApiPath<(String, Uuid)>,
    ApiJson(patch): ApiJson<ExtensionAnnotationPatch>,
) -> Result<Json<ExtensionAnnotations>, ApiError> {
    let may_write = state
        .repository
        .is_authorized(
            principal.0,
            workspace.0,
            "entities.write",
            Some(entity_id),
            None,
        )
        .await?
        && match principal.1 {
            Some(token_id) => {
                state
                    .repository
                    .personal_api_token_permits(token_id, "entities.write")
                    .await?
            }
            None => true,
        };
    if !may_write {
        return Err(ApiError::forbidden());
    }
    Ok(Json(
        repository
            .repair_extension_annotations(&extension_id, entity_id, patch)
            .await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_filenames_are_safe_ascii() {
        let id = Uuid::nil();
        assert_eq!(download_filename(Some("report.pdf"), id), "report.pdf");
        assert_eq!(download_filename(Some("../\"evil\r\n.pdf"), id), "evil.pdf");
        assert_eq!(download_filename(None, id), format!("output-{id}"));
    }
}
