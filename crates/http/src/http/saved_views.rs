use axum::{Json, http::StatusCode};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{
    auth::{AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
};
use crate::repository::SavedView;
use attricat_validation::saved_search;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewInput {
    kind: String,
    name: Option<String>,
    description: Option<String>,
    visibility: String,
    state: Value,
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    #[serde(default)]
    q: String,
}

const MAX_LIST_SEARCH_LENGTH: usize = 120;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LinkInput {
    kind: String,
    state: Value,
}

fn validate_state(kind: &str, state: &Value) -> Result<(), ApiError> {
    saved_search::validate_state(kind, state).map_err(ApiError::invalid_input)
}

fn normalized_state(state: &Value) -> Value {
    saved_search::normalize_state(state)
}

fn validate_named(input: &ViewInput) -> Result<(&str, Option<&str>), ApiError> {
    validate_state(&input.kind, &input.state)?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 120)
        .ok_or_else(|| ApiError::invalid_input("name must be 1–120 characters".to_owned()))?;
    if !matches!(input.visibility.as_str(), "private" | "workspace")
        || input.description.as_ref().is_some_and(|s| s.len() > 500)
    {
        return Err(ApiError::invalid_input(
            "invalid visibility or description".to_owned(),
        ));
    }
    Ok((name, input.description.as_deref()))
}

pub(super) async fn list(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiQuery(query): ApiQuery<ListQuery>,
) -> Result<Json<Vec<SavedView>>, ApiError> {
    let search = query.q.trim();
    if search.chars().count() > MAX_LIST_SEARCH_LENGTH || search.contains('\0') {
        return Err(ApiError::invalid_input("invalid saved view search".into()));
    }
    Ok(Json(repository.list_saved_views(actor, search).await?))
}
pub(super) async fn get(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<SavedView>, ApiError> {
    Ok(Json(
        repository
            .get_saved_view(actor, id, false)
            .await?
            .ok_or_else(|| ApiError::not_found("saved view"))?,
    ))
}
pub(super) async fn get_link(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<SavedView>, ApiError> {
    Ok(Json(
        repository
            .get_saved_view(actor, id, true)
            .await?
            .ok_or_else(|| ApiError::not_found("view state link"))?,
    ))
}
pub(super) async fn create(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiJson(input): ApiJson<ViewInput>,
) -> Result<(StatusCode, Json<SavedView>), ApiError> {
    let (name, description) = validate_named(&input)?;
    Ok((
        StatusCode::CREATED,
        Json(
            repository
                .create_saved_view(
                    actor,
                    Some(name),
                    description,
                    &input.visibility,
                    &normalized_state(&input.state),
                )
                .await?,
        ),
    ))
}
pub(super) async fn create_link(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiJson(input): ApiJson<LinkInput>,
) -> Result<(StatusCode, Json<SavedView>), ApiError> {
    validate_state(&input.kind, &input.state)?;
    Ok((
        StatusCode::CREATED,
        Json(
            repository
                .create_saved_view(actor, None, None, "link", &normalized_state(&input.state))
                .await?,
        ),
    ))
}
pub(super) async fn update(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<ViewInput>,
) -> Result<Json<SavedView>, ApiError> {
    let (name, description) = validate_named(&input)?;
    Ok(Json(
        repository
            .update_saved_view(
                actor,
                id,
                name,
                description,
                &input.visibility,
                &normalized_state(&input.state),
            )
            .await?
            .ok_or_else(|| ApiError::not_found("saved view"))?,
    ))
}
pub(super) async fn delete(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if !repository.delete_saved_view(actor, id).await? {
        return Err(ApiError::not_found("saved view"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saved_search_filter_validation_preserves_optional_fields_and_limits() {
        let state = json!({
            "blueprint": "product",
            "attributeFilters": [{"field": "name", "operator": "contains", "value": "shoe"}],
            "relationshipFacets": [{"field": "brand", "selectedIds": [Uuid::new_v4()]}],
        });
        assert!(validate_state("explorer_search", &state).is_ok());
        for facet in [
            json!({"field": "brand", "selectedIds": ["invalid"]}),
            json!({"field": "brand", "selectedIds": "invalid"}),
            json!({"field": "brand", "unexpected": true}),
        ] {
            assert!(
                validate_state(
                    "explorer_search",
                    &json!({"blueprint": "product", "relationshipFacets": [facet]})
                )
                .is_err()
            );
        }
        assert!(
            validate_state(
                "explorer_search",
                &json!({"blueprint": "product", "attributeFilters": [{"field": "name", "operator": "eq"}]})
            )
            .is_err()
        );
    }
}
