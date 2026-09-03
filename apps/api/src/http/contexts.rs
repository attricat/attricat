use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal},
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    catalog_service::CatalogMutationService,
    model::{AttributeContext, CreateAttributeContext, UpdateAttributeContext},
};
use axum::{Json, extract::State, http::StatusCode};
use uuid::Uuid;
pub(super) async fn create_context(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateAttributeContext>,
) -> Result<(StatusCode, Json<AttributeContext>), ApiError> {
    let context = CatalogMutationService::new(&repository)
        .create_context(input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(context)))
}
pub(super) async fn list_contexts(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    AuthenticatedPrincipal(user_id, _): AuthenticatedPrincipal,
    ActiveWorkspace(workspace_id): ActiveWorkspace,
) -> Result<Json<Vec<AttributeContext>>, ApiError> {
    Ok(Json(
        repository
            .list_authorized_contexts(user_id, workspace_id)
            .await?,
    ))
}
pub(super) async fn get_context(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(code): ApiPath<String>,
) -> Result<Json<AttributeContext>, ApiError> {
    repository
        .get_context_by_code(&code)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("context"))
}
pub(super) async fn update_context(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateAttributeContext>,
) -> Result<Json<AttributeContext>, ApiError> {
    let context = CatalogMutationService::new(&repository)
        .update_context(id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(context))
}
pub(super) async fn delete_context(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    CatalogMutationService::new(&repository)
        .delete_context(id)
        .await?;
    invalidate_data_health(&state).await;
    Ok(StatusCode::NO_CONTENT)
}
