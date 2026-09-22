use axum::{
    Json,
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{AppState, auth::ScopedRepository, error::ApiError, extractors::ApiPath};
use crate::{
    repository::{MAX_PRESENTATION_ASSET_PAGE_SIZE, PresentationAsset},
    storage::{ObjectStoreError, get_object_for_integrity},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    25
}

fn private_cache_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    Query(query): Query<ListQuery>,
) -> Result<(HeaderMap, Json<Vec<PresentationAsset>>), ApiError> {
    if !(1..=MAX_PRESENTATION_ASSET_PAGE_SIZE).contains(&query.limit)
        || !(0..=10_000).contains(&query.offset)
    {
        return Err(ApiError::invalid_input(
            "limit must be 1-100 and offset must be 0-10000".into(),
        ));
    }
    Ok((
        private_cache_headers(),
        Json(
            repository
                .list_presentation_assets(query.limit, query.offset)
                .await?,
        ),
    ))
}

pub(super) async fn get(
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<(HeaderMap, Json<PresentationAsset>), ApiError> {
    Ok((
        private_cache_headers(),
        Json(repository.get_presentation_asset(id).await?),
    ))
}

pub(super) async fn content(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Response, ApiError> {
    let asset = repository.get_presentation_asset(id).await?;
    let expected_size = usize::try_from(asset.byte_size)
        .map_err(|_| ApiError::internal("presentation asset size is invalid"))?;
    let object = get_object_for_integrity(
        state.object_store.as_ref(),
        &asset.object_key,
        expected_size,
    )
    .await
    .map_err(|error| match error {
        ObjectStoreError::Unavailable
        | ObjectStoreError::TimedOut(_)
        | ObjectStoreError::Operation(_) => ApiError::storage_unavailable(),
        ObjectStoreError::NotFound => {
            ApiError::internal("presentation asset object integrity failed")
        }
    })?;
    if object.bytes.len() as i64 != asset.byte_size || hex_sha256(&object.bytes) != asset.sha256 {
        return Err(ApiError::internal(
            "presentation asset object integrity failed",
        ));
    }
    let mut response = Response::new(Body::from(object.bytes));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.media_type).expect("stored media type"),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&asset.byte_size.to_string()).expect("stored size"),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{}\"", asset.sha256)).expect("stored digest"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'"),
    );
    Ok(response)
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
