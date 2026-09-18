use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    catalog_service::CatalogMutationService,
    model::{
        Blueprint, BlueprintEntityPublicationSummary, BlueprintMigrationBatch,
        BlueprintMigrationBatchStatus, BlueprintWithAttributes, CreateBlueprint,
        PublicationContextRequest,
    },
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
pub(super) async fn list_blueprint_migration_batches(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<Vec<BlueprintMigrationBatchStatus>>, ApiError> {
    Ok(Json(
        repository
            .list_blueprint_migration_batches(blueprint_id)
            .await?,
    ))
}

pub(super) async fn start_safe_blueprint_migration_batch(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<(StatusCode, Json<BlueprintMigrationBatch>), ApiError> {
    let batch = repository
        .start_safe_blueprint_migration_batch(blueprint_id, version)
        .await?;
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
pub(super) async fn publish_blueprint_entities(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
    ApiJson(input): ApiJson<PublicationContextRequest>,
) -> Result<Json<BlueprintEntityPublicationSummary>, ApiError> {
    let summary = CatalogMutationService::new(&repository)
        .publish_blueprint_entities(blueprint_id, version, Some(input.context_id))
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(summary))
}

pub(super) async fn publish_blueprint_entities_all_channels(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintEntityPublicationSummary>, ApiError> {
    let summary = CatalogMutationService::new(&repository)
        .publish_blueprint_entities(blueprint_id, version, None)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(summary))
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
