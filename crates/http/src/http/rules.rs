use super::{
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    pagination::{ArrayPage, array_response},
};
use crate::model::{
    CreateManualRuleRun, CreateRule, EnableRule, Rule, RuleDetails, RuleFinding, RuleRevision,
    UpdateRule,
};
use crate::repository::RuleFindingFilter;
use attricat_domain::states::{RuleFindingSeverity, RuleFindingState};
use axum::{Json, http::StatusCode, response::Response};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RuleQuery {
    pub blueprint_id: Option<Uuid>,
}
pub(super) async fn validate(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<Json<attricat_rules::CompiledRule>, ApiError> {
    Ok(Json(repo.validate_rule(&input).await?))
}
pub(super) async fn list(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<RuleQuery>,
) -> Result<Json<Vec<RuleDetails>>, ApiError> {
    Ok(Json(repo.list_rules(query.blueprint_id).await?))
}
pub(super) async fn create(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateRule>,
) -> Result<(StatusCode, Json<Rule>), ApiError> {
    Ok((StatusCode::CREATED, Json(repo.create_rule(input).await?)))
}
/// Saves a changed definition as the rule's new current revision.
pub(super) async fn update(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateRule>,
) -> Result<Json<Rule>, ApiError> {
    Ok(Json(repo.update_rule(id, input).await?))
}
pub(super) async fn get(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<RuleDetails>, ApiError> {
    repo.get_rule_details(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("rule"))
}
/// The rule's changelog, newest first.
pub(super) async fn revisions(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Vec<RuleRevision>>, ApiError> {
    Ok(Json(repo.list_rule_revisions(id).await?))
}
pub(super) async fn revision(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath((id, version)): ApiPath<(Uuid, i64)>,
) -> Result<Json<RuleRevision>, ApiError> {
    repo.get_rule_revision(id, version)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("rule revision"))
}
pub(super) async fn enable(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    input: Option<ApiJson<EnableRule>>,
) -> Result<Json<Rule>, ApiError> {
    let options = input.map(|ApiJson(input)| input).unwrap_or_default();
    Ok(Json(repo.enable_rule(id, options).await?))
}
pub(super) async fn run_now(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<CreateManualRuleRun>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let id = repo.create_manual_rule_run(id, input).await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "id": id }))))
}
#[derive(Deserialize)]
pub struct RunsQuery {
    record_id: Option<Uuid>,
    limit: Option<i64>,
    offset: Option<i64>,
}
pub(super) async fn list_runs(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<RunsQuery>,
) -> Result<Response, ApiError> {
    let (limit, offset) = ArrayPage::new(query.limit, query.offset).bounds()?;
    let page_items = repo
        .rule_runs_page(None, query.record_id, limit, offset)
        .await?;
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
#[serde(deny_unknown_fields)]
pub struct FindingsQuery {
    record_id: Option<Uuid>,
    rule_id: Option<Uuid>,
    blueprint_id: Option<Uuid>,
    context_id: Option<Uuid>,
    severity: Option<RuleFindingSeverity>,
    state: Option<RuleFindingState>,
    /// Leaves out resolved findings.
    #[serde(default)]
    unresolved: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}
pub(super) async fn findings(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<FindingsQuery>,
) -> Result<Response, ApiError> {
    let (limit, offset) = ArrayPage::new(query.limit, query.offset).bounds()?;
    let filter = RuleFindingFilter {
        record_id: query.record_id,
        rule_id: query.rule_id,
        blueprint_id: query.blueprint_id,
        context_id: query.context_id,
        severity: query.severity,
        state: query.state,
        unresolved: query.unresolved,
    };
    let page_items = repo.rule_findings_page(&filter, limit, offset).await?;
    Ok(array_response(page_items, (limit, offset)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingCountQuery {
    record_id: Option<Uuid>,
}
/// Counts unresolved findings, optionally of one record.
pub(super) async fn finding_count(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiQuery(query): ApiQuery<FindingCountQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let count = repo.unresolved_rule_finding_count(query.record_id).await?;
    Ok(Json(serde_json::json!({ "count": count })))
}
pub(super) async fn acknowledge(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<RuleFinding>, ApiError> {
    Ok(Json(repo.acknowledge_rule_finding(id).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordPageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}
/// One page of a record's open and acknowledged findings, for anyone who
/// may read the record.
pub(super) async fn record_findings(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<RecordPageQuery>,
) -> Result<Response, ApiError> {
    let (limit, offset) = ArrayPage::new(query.limit, query.offset).bounds()?;
    let filter = RuleFindingFilter {
        record_id: Some(record_id),
        unresolved: true,
        ..Default::default()
    };
    let page_items = repo.rule_findings_page(&filter, limit, offset).await?;
    Ok(array_response(page_items, (limit, offset)))
}
/// Counts a record's unresolved findings, for anyone who may read the record.
pub(super) async fn record_finding_count(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let count = repo.unresolved_rule_finding_count(Some(record_id)).await?;
    Ok(Json(serde_json::json!({ "count": count })))
}
/// The runs that checked a record, newest first, for anyone who may read it.
pub(super) async fn record_runs(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<RecordPageQuery>,
) -> Result<Response, ApiError> {
    let (limit, offset) = ArrayPage::new(query.limit, query.offset).bounds()?;
    let page_items = repo
        .rule_runs_page(None, Some(record_id), limit, offset)
        .await?;
    Ok(array_response(page_items, (limit, offset)))
}
/// The rules of a record's blueprint, for anyone who may read the record.
pub(super) async fn record_rules(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Vec<RuleDetails>>, ApiError> {
    Ok(Json(repo.list_record_rules(record_id).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecheckRecord {
    record_id: Uuid,
    idempotency_key: String,
}
/// Re-checks one record against every enabled rule of its blueprint revision.
pub(super) async fn recheck_record(
    super::auth::ScopedRepository(repo): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<RecheckRecord>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let ids = repo
        .create_record_rule_runs(input.record_id, &input.idempotency_key)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "run_ids": ids })),
    ))
}
