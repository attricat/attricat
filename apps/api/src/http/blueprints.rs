use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::model::{Blueprint, BlueprintWithAttributes, CreateBlueprint};
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
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = state.repository.create_blueprint(input).await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}
pub(super) async fn list_entity_blueprints(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_entity_blueprints(query.include_drafts)
            .await?,
    ))
}
pub(super) async fn list_blueprints(
    State(state): State<AppState>,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(state.repository.list_blueprints().await?))
}
pub(super) async fn create_blueprint_revision(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    let blueprint = state
        .repository
        .create_blueprint_revision(blueprint_id, input)
        .await?;
    invalidate_data_health(&state).await;
    Ok((StatusCode::CREATED, Json(blueprint)))
}
pub(super) async fn list_blueprint_revisions(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<Vec<Blueprint>>, ApiError> {
    Ok(Json(
        state
            .repository
            .list_blueprint_revisions(blueprint_id)
            .await?,
    ))
}
pub(super) async fn get_blueprint(
    State(state): State<AppState>,
    ApiPath(blueprint_id): ApiPath<Uuid>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_current_blueprint(blueprint_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}
pub(super) async fn get_blueprint_revision(
    State(state): State<AppState>,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_revision(blueprint_id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}
pub(super) async fn publish_blueprint_revision(
    State(state): State<AppState>,
    ApiPath((blueprint_id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = state
        .repository
        .publish_blueprint_revision(blueprint_id, version)
        .await?;
    invalidate_data_health(&state).await;
    Ok(Json(blueprint))
}
pub(super) async fn get_blueprint_by_code(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
    ApiQuery(query): ApiQuery<BlueprintQuery>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    let blueprint = if query.include_drafts {
        state
            .repository
            .get_blueprint_by_code_including_drafts(&code)
            .await?
    } else {
        state.repository.get_blueprint_by_code(&code).await?
    };
    blueprint
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}
pub(super) async fn get_blueprint_by_code_and_version(
    State(state): State<AppState>,
    ApiPath((code, version)): ApiPath<(String, i64)>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_blueprint_by_code_and_version(&code, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint version"))
}
