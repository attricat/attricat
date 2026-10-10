use super::{
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    pagination::{ArrayPage, array_response},
};
use crate::model::{CreateManualWorkflowRun, CreateWorkflow, Workflow};
use axum::{Json, http::StatusCode, response::Response};
use uuid::Uuid;

pub(super) async fn validate(
    ApiJson(input): ApiJson<CreateWorkflow>,
) -> Result<Json<attricat_workflow::CompiledWorkflow>, ApiError> {
    attricat_workflow::compile(&input.definition)
        .map(Json)
        .map_err(|e| {
            crate::repository::RepositoryError::InvalidWorkflowDefinition(e.to_string()).into()
        })
}
pub(super) async fn list(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
) -> Result<Json<Vec<Workflow>>, ApiError> {
    Ok(Json(repo.list_workflows().await?))
}
pub(super) async fn create(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateWorkflow>,
) -> Result<(StatusCode, Json<Workflow>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(repo.create_workflow(input).await?),
    ))
}
pub(super) async fn get(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Workflow>, ApiError> {
    repo.get_workflow(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("workflow"))
}
pub(super) async fn list_versions(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Vec<Workflow>>, ApiError> {
    Ok(Json(repo.list_workflow_revisions(id).await?))
}
pub(super) async fn create_revision(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateWorkflow>,
) -> Result<(StatusCode, Json<Workflow>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(repo.create_workflow_revision(id, input).await?),
    ))
}
pub(super) async fn get_version(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<Workflow>, ApiError> {
    repo.get_workflow_revision(id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("workflow revision"))
}
pub(super) async fn publish(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<Workflow>, ApiError> {
    Ok(Json(repo.publish_workflow_revision(id, version).await?))
}
pub(super) async fn enable(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<Workflow>, ApiError> {
    Ok(Json(repo.enable_workflow_revision(id, version).await?))
}
pub(super) async fn run_now(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateManualWorkflowRun>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let run_id = repo
        .create_manual_workflow_run(id, input.record_id, &input.idempotency_key)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "id": run_id })),
    ))
}
pub(super) async fn disable(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Workflow>, ApiError> {
    Ok(Json(repo.disable_workflow(id).await?))
}

/// Diagnostics intentionally expose run state and trigger references, never the
/// internal domain-event payload snapshot.
pub(super) async fn list_runs(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(page): ApiQuery<ArrayPage>,
) -> Result<Response, ApiError> {
    let (limit, offset) = page.bounds()?;
    let page_items = repo.workflow_runs_page(limit, offset).await?;
    Ok(array_response(page_items, (limit, offset)))
}
pub(super) async fn replay_run(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repo.replay_workflow_run(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("terminal workflow run"))
    }
}

/// Per-target outcomes of `referencing_records_update` actions in one run.
pub(super) async fn list_run_targets(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::WorkflowRunTarget>>, ApiError> {
    repo.workflow_run_targets(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("workflow run"))
}
