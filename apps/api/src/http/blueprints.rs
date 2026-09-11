use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    catalog_service::CatalogMutationService,
    model::{Blueprint, BlueprintMigrationBatch, BlueprintWithAttributes, CreateBlueprint},
};
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BlueprintQuery {
    #[serde(default)]
    include_drafts: bool,
}
pub(super) async fn create_blueprint(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = CatalogMutationService::new(&repository)
        .create_blueprint(input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}
pub(super) async fn list_entity_blueprints(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(
        repository
            .list_entity_blueprints(query.include_drafts)
            .await?,
    ))
}
pub(super) async fn list_blueprints(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(repository.list_blueprints().await?))
}
pub(super) async fn create_blueprint_revision(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(blueprint_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = CatalogMutationService::new(&repository)
        .create_blueprint_revision(blueprint_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}
pub(super) async fn list_blueprint_revisions(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(
        repository.list_blueprint_revisions(blueprint_id).await?,
    ))
}
pub(super) async fn get_blueprint(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    repository
        .get_current_blueprint(blueprint_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}
pub(super) async fn get_blueprint_revision(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    repository
        .get_blueprint_revision(blueprint_id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}
pub(super) async fn start_safe_blueprint_migration_batch(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<(StatusCode, Json<BlueprintMigrationBatch>), ApiError> {
    let batch = repository
        .start_safe_blueprint_migration_batch(blueprint_id, version)
        .await?;
    let worker = repository.clone();
    let batch_id = batch.id;
    tokio::spawn(async move {
        if let Err(error) = worker.run_safe_blueprint_migration_batch(batch_id).await {
            tracing::error!(%batch_id, %error, "safe blueprint migration batch failed");
        }
    });
    invalidate_data_health(&state).await;
    Ok((StatusCode::ACCEPTED, Json(batch)))
}

pub(super) async fn publish_blueprint_revision(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = CatalogMutationService::new(&repository)
        .publish_blueprint_revision(blueprint_id, version)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(blueprint))
}
pub(super) async fn get_blueprint_by_code(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(code): ApiPath<String>,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = if query.include_drafts {
        repository
            .get_blueprint_by_code_including_drafts(&code)
            .await?
    } else {
        repository.get_blueprint_by_code(&code).await?
    };
    blueprint
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}
pub(super) async fn get_blueprint_by_code_and_version(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((code, version)): ApiPath<(String, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    repository
        .get_blueprint_by_code_and_version(&code, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}
