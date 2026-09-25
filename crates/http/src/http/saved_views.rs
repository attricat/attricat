use axum::{Json, extract::Path, http::StatusCode};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use super::{
    auth::{AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::ApiJson,
};
use crate::repository::SavedView;

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
#[serde(deny_unknown_fields)]
pub(super) struct LinkInput {
    kind: String,
    state: Value,
}

fn valid_attribute_filter(filter: &Value) -> bool {
    filter.as_object().is_some_and(|filter| {
        !filter
            .keys()
            .any(|key| !["field", "operator", "value"].contains(&key.as_str()))
            && filter
                .get("field")
                .and_then(Value::as_str)
                .is_some_and(|field| !field.is_empty())
            && matches!(
                filter.get("operator").and_then(Value::as_str),
                Some("eq" | "contains" | "starts_with" | "gt" | "gte" | "lt" | "lte")
            )
            && filter
                .get("value")
                .is_some_and(|value| value.is_string() || value.is_number() || value.is_boolean())
    })
}

fn valid_relationship_facet(facet: &Value) -> bool {
    facet.as_object().is_some_and(|facet| {
        !facet
            .keys()
            .any(|key| !["field", "selectedIds", "targetBlueprint"].contains(&key.as_str()))
            && facet
                .get("field")
                .and_then(Value::as_str)
                .is_some_and(|field| !field.is_empty())
            && facet.get("selectedIds").is_none_or(|ids| {
                ids.as_array().is_some_and(|ids| {
                    ids.len() <= 100
                        && ids
                            .iter()
                            .all(|id| id.as_str().is_some_and(|id| Uuid::parse_str(id).is_ok()))
                })
            })
            && facet.get("targetBlueprint").is_none_or(Value::is_string)
    })
}

fn validate_state(kind: &str, state: &Value) -> Result<(), ApiError> {
    if kind != "explorer_search" {
        return Err(ApiError::invalid_input(
            "unsupported saved view kind".to_owned(),
        ));
    }
    let bytes = serde_json::to_vec(state)
        .map_err(|_| ApiError::invalid_input("invalid state".to_owned()))?;
    if bytes.len() > 32_768 {
        return Err(ApiError::invalid_input(
            "search state exceeds 32 KiB".to_owned(),
        ));
    }
    let object = state
        .as_object()
        .ok_or_else(|| ApiError::invalid_input("search state must be an object".to_owned()))?;
    const KEYS: &[&str] = &[
        "blueprint",
        "version",
        "allVersions",
        "query",
        "context",
        "locked",
        "sort",
        "relationshipFacets",
        "attributeFilters",
    ];
    if object.keys().any(|key| !KEYS.contains(&key.as_str()))
        || !object
            .get("blueprint")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty() && s.len() <= 256)
    {
        return Err(ApiError::invalid_input(
            "invalid Explorer search state".to_owned(),
        ));
    }
    if let Some(filters) = object.get("attributeFilters") {
        let filters = filters.as_array().ok_or_else(|| {
            ApiError::invalid_input("attributeFilters must be an array".to_owned())
        })?;
        if filters.len() > 20 || !filters.iter().all(valid_attribute_filter) {
            return Err(ApiError::invalid_input(
                "invalid attributeFilters".to_owned(),
            ));
        }
    }
    if let Some(facets) = object.get("relationshipFacets") {
        let facets = facets.as_array().ok_or_else(|| {
            ApiError::invalid_input("relationshipFacets must be an array".to_owned())
        })?;
        if facets.len() > 20 || !facets.iter().all(valid_relationship_facet) {
            return Err(ApiError::invalid_input(
                "invalid relationshipFacets".to_owned(),
            ));
        }
    }
    if object
        .get("version")
        .is_some_and(|v| v.as_u64().is_none_or(|v| v == 0))
        || ["allVersions", "locked"]
            .iter()
            .any(|k| object.get(*k).is_some_and(|v| !v.is_boolean()))
        || ["query", "context"].iter().any(|k| {
            object
                .get(*k)
                .is_some_and(|v| v.as_str().is_none_or(str::is_empty))
        })
        || object.get("sort").is_some_and(|v| {
            v.as_object().is_none_or(|v| {
                v.len() != 2
                    || v.get("field")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || !matches!(
                        v.get("direction").and_then(Value::as_str),
                        Some("asc" | "desc")
                    )
            })
        })
    {
        return Err(ApiError::invalid_input(
            "invalid Explorer search state".to_owned(),
        ));
    }
    Ok(())
}

fn normalized_state(state: &Value) -> Value {
    let mut state = state.clone();
    let object = state
        .as_object_mut()
        .expect("validated search state is an object");
    for key in ["blueprint", "query", "context"] {
        if let Some(Value::String(value)) = object.get_mut(key) {
            *value = value.trim().to_owned();
        }
    }
    for key in ["allVersions", "locked"] {
        if object.get(key) == Some(&Value::Bool(false)) {
            object.remove(key);
        }
    }
    for key in ["attributeFilters", "relationshipFacets"] {
        if object
            .get(key)
            .is_some_and(|value| value.as_array().is_some_and(Vec::is_empty))
        {
            object.remove(key);
        }
    }
    if object.get("context").and_then(Value::as_str) == Some("default") {
        object.remove("context");
    }
    if object.get("query").and_then(Value::as_str) == Some("") {
        object.remove("query");
    }
    state
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
) -> Result<Json<Vec<SavedView>>, ApiError> {
    Ok(Json(repository.list_saved_views(actor).await?))
}
pub(super) async fn get(
    ScopedRepository(repository): ScopedRepository,
    AuthenticatedPrincipal(actor, _): AuthenticatedPrincipal,
    Path(id): Path<Uuid>,
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
    Path(id): Path<Uuid>,
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
    Path(id): Path<Uuid>,
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
    Path(id): Path<Uuid>,
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
        assert!(valid_relationship_facet(&json!({"field": "brand"})));
        assert!(!valid_attribute_filter(
            &json!({"field": "name", "operator": "eq"})
        ));
        assert!(!valid_relationship_facet(
            &json!({"field": "brand", "selectedIds": ["invalid"]})
        ));
        assert!(!valid_relationship_facet(
            &json!({"field": "brand", "selectedIds": "invalid"})
        ));
        assert!(!valid_relationship_facet(
            &json!({"field": "brand", "unexpected": true})
        ));
    }
}
