use super::{
    AppState,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::{
    catalog_service::CatalogMutationService,
    model::{
        AttachReusableAttribute, CreateReusableAttribute, CreateReusableAttributeGroup,
        RecordReusableAttribute, ReusableAttribute, ReusableAttributeGroup,
    },
};
use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReusableAttributeQuery {
    #[serde(default)]
    include_drafts: bool,
}

pub(super) async fn list(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<ReusableAttributeQuery>,
) -> Result<Json<Vec<ReusableAttribute>>, ApiError> {
    Ok(Json(
        repository
            .list_reusable_attributes(query.include_drafts)
            .await?,
    ))
}

pub(super) async fn create(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateReusableAttribute>,
) -> Result<(StatusCode, Json<ReusableAttribute>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            CatalogMutationService::new(&repository)
                .create_reusable_attribute(input)
                .await?,
        ),
    ))
}

pub(super) async fn create_revision(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(definition_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateReusableAttribute>,
) -> Result<(StatusCode, Json<ReusableAttribute>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            CatalogMutationService::new(&repository)
                .create_reusable_attribute_revision(definition_id, input)
                .await?,
        ),
    ))
}

pub(super) async fn publish_revision(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(revision_id): ApiPath<Uuid>,
) -> Result<Json<ReusableAttribute>, ApiError> {
    Ok(Json(
        CatalogMutationService::new(&repository)
            .publish_reusable_attribute_revision(revision_id)
            .await?,
    ))
}

pub(super) async fn list_groups(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
) -> Result<Json<Vec<ReusableAttributeGroup>>, ApiError> {
    Ok(Json(repository.list_reusable_attribute_groups().await?))
}

pub(super) async fn create_group(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateReusableAttributeGroup>,
) -> Result<(StatusCode, Json<ReusableAttributeGroup>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            CatalogMutationService::new(&repository)
                .create_reusable_attribute_group(input)
                .await?,
        ),
    ))
}

pub(super) async fn attach(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<AttachReusableAttribute>,
) -> Result<(StatusCode, Json<RecordReusableAttribute>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            CatalogMutationService::new(&repository)
                .attach_reusable_attribute(record_id, input)
                .await?,
        ),
    ))
}

pub(super) async fn attach_group(
    State(_state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((record_id, group_id)): ApiPath<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<Vec<RecordReusableAttribute>>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            CatalogMutationService::new(&repository)
                .attach_reusable_attribute_group(record_id, group_id)
                .await?,
        ),
    ))
}
