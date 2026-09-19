use super::{
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::model::{CreateManualRuleRun, CreateRule, Rule, RuleFinding, RuleRun};
use axum::{Json, extract::Query, http::StatusCode};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RuleQuery {
    pub blueprint_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
}
pub(super) async fn validate(
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        serde_json::to_value(catalog_rules::compile(&input.definition).map_err(|e| {
            ApiError::from(crate::repository::RepositoryError::InvalidRuleDefinition(
                e.to_string(),
            ))
        })?)
        .expect("rule serializes"),
    ))
}
pub(super) async fn list(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    Query(query): Query<RuleQuery>,
) -> Result<Json<Vec<Rule>>, ApiError> {
    Ok(Json(repo.list_rules(query.blueprint_id).await?))
}
pub(super) async fn create(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<(StatusCode, Json<Rule>), ApiError> {
    Ok((StatusCode::CREATED, Json(repo.create_rule(input).await?)))
}
pub(super) async fn create_revision(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<(StatusCode, Json<Rule>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(repo.create_rule_revision(id, input).await?),
    ))
}
pub(super) async fn get(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Rule>, ApiError> {
    repo.get_rule(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("rule"))
}
pub(super) async fn publish(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<Rule>, ApiError> {
    Ok(Json(repo.publish_rule(id, version).await?))
}
pub(super) async fn enable(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<Rule>, ApiError> {
    Ok(Json(repo.enable_rule(id, version).await?))
}
pub(super) async fn run_now(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateManualRuleRun>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let id = repo.create_manual_rule_run(id, input).await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "id": id }))))
}
pub(super) async fn list_runs(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
) -> Result<Json<Vec<RuleRun>>, ApiError> {
    Ok(Json(repo.list_rule_runs().await?))
}
pub(super) async fn replay_run(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    if repo.replay_rule_run(id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("dead-letter rule run"))
    }
}
pub(super) async fn disable(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Rule>, ApiError> {
    Ok(Json(repo.disable_rule(id).await?))
}
pub(super) async fn findings(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    Query(query): Query<RuleQuery>,
) -> Result<Json<Vec<RuleFinding>>, ApiError> {
    Ok(Json(repo.list_rule_findings(query.entity_id).await?))
}
pub(super) async fn acknowledge(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<RuleFinding>, ApiError> {
    Ok(Json(repo.acknowledge_rule_finding(id).await?))
}
