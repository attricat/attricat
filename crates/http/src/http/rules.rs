use super::{
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    pagination::{ArrayPage, array_response},
};
use crate::model::{CreateManualRuleRun, CreateRule, EnableRule, Rule, RuleFinding};
use axum::{Json, http::StatusCode, response::Response};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RuleQuery {
    pub blueprint_id: Option<Uuid>,
}
pub(super) async fn validate(
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<Json<attricat_rules::CompiledRule>, ApiError> {
    attricat_rules::compile(&input.definition)
        .map(Json)
        .map_err(|e| {
            crate::repository::RepositoryError::InvalidRuleDefinition(e.to_string()).into()
        })
}
pub(super) async fn list(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<RuleQuery>,
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
    input: Option<ApiJson<EnableRule>>,
) -> Result<Json<Rule>, ApiError> {
    let options = input.map(|ApiJson(input)| input).unwrap_or_default();
    Ok(Json(repo.enable_rule_with(id, version, options).await?))
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
    ApiQuery(page): ApiQuery<ArrayPage>,
) -> Result<Response, ApiError> {
    let (limit, offset) = page.bounds()?;
    let page_items = repo.rule_runs_page(None, limit, offset).await?;
    Ok(array_response(page_items, (limit, offset)))
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
#[derive(Deserialize)]
pub struct FindingsQuery {
    record_id: Option<Uuid>,
    limit: Option<i64>,
    offset: Option<i64>,
}
pub(super) async fn findings(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<FindingsQuery>,
) -> Result<Response, ApiError> {
    let (limit, offset) = ArrayPage::new(query.limit, query.offset).bounds()?;
    let page_items = repo
        .rule_findings_page(query.record_id, limit, offset)
        .await?;
    Ok(array_response(page_items, (limit, offset)))
}
pub(super) async fn acknowledge(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<RuleFinding>, ApiError> {
    Ok(Json(repo.acknowledge_rule_finding(id).await?))
}
