use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::model::{AttributeContext, CreateAttributeContext, UpdateAttributeContext};
use axum::{Json, extract::State, http::StatusCode};
use uuid::Uuid;
pub(super) async fn create_context(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<CreateAttributeContext>,
) -> Result<(StatusCode, Json<AttributeContext>), ApiError> {
    let context = state.repository.create_context(input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(context)))
}
pub(super) async fn list_contexts(
    State(state): State<AppState>,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<Vec<AttributeContext>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_authorized_contexts(user_id, workspace_id)
            .await?,
    ))
}
pub(super) async fn get_context(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
) -> Result<Json<AttributeContext>, ApiError> {
    state
        .repository
        .get_context_by_code(&code)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("context"))
}
pub(super) async fn update_context(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateAttributeContext>,
) -> Result<Json<AttributeContext>, ApiError> {
    let context = state.repository.update_context(id, input).await?;
    invalidate_data_health(&state).await;
    Ok(Json(context))
}
pub(super) async fn delete_context(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.repository.delete_context(id).await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
