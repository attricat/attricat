use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath, ApiQuery},
    pagination::{next_offset, page_bounds},
};
use crate::{
    attricat_read_service::AttricatReadService,
    attricat_service::AttricatMutationService,
    constants::DEFAULT_PAGE_SIZE,
    model::{
        AppendAttributeValues, AttributeValue, CreateRecordFormRequest, IncomingRelationshipsPage,
        IncomingRelationshipsRequest, MigrateRecordRequest, PublicationContextRequest, Record,
        RecordBatchRequest, RecordBatchResponse, RecordFormResponse, RelationshipMutation,
        UpdateRecordFormRequest,
    },
    repository::decode_search_cursor,
};
use axum::{Json, extract::State, http::StatusCode};
use chrono::DateTime;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use attricat_agent_runtime::smart_fill::{
    SmartFillError, SmartFillProposal, SmartFillRequest, propose_record_form_values,
};

pub(super) async fn smart_fill_record_form(
    State(state): State<AppState>,
    principal @ super::auth::AuthenticatedPrincipal(user, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace_id): super::auth::ActiveWorkspace,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<SmartFillRequest>,
) -> Result<Json<SmartFillProposal>, ApiError> {
    if !repository
        .principal_may(
            principal.actor(),
            workspace_id,
            "records.read",
            Some(input.record_id),
            None,
        )
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let config = state
        .agent_provider
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("agents are not configured"))?;
    propose_record_form_values(
        &repository,
        state.object_store.as_ref(),
        config,
        user,
        input,
    )
    .await
    .map(Json)
    .map_err(|error| match error {
        SmartFillError::Invalid(message) => ApiError::invalid_input(message),
        SmartFillError::NotFound(resource) => ApiError::not_found(resource),
        SmartFillError::Unavailable(message) => ApiError::service_unavailable(message),
        SmartFillError::BadGateway(message) => ApiError::bad_gateway(message),
        SmartFillError::Repository(error) => error.into(),
    })
}

const DEFAULT_HISTORY_PAGE_SIZE: i64 = 25;
const MAX_HISTORY_PAGE_SIZE: i64 = 50;

pub(super) async fn delete_record(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    AttricatMutationService::new(&repository)
        .delete_record(record_id)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_record_form(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user_id, _): super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<CreateRecordFormRequest>,
) -> Result<(StatusCode, Json<Record>), ApiError> {
    let record = AttricatMutationService::new(&repository)
        .create_record(input, Some(user_id))
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(record)))
}
/// Applies creates, updates (including status transitions) and deletes to
/// several records in one transaction. Every operation is authorized against
/// its own record before anything runs.
pub(super) async fn apply_record_batch(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user, token): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace_id): super::auth::ActiveWorkspace,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiJson(input): ApiJson<RecordBatchRequest>,
) -> Result<Json<RecordBatchResponse>, ApiError> {
    if !repository
        .is_authorized_for_record_batch(user, workspace_id, token, &input)
        .await?
    {
        return Err(ApiError::forbidden());
    }
    let response = AttricatMutationService::new(&repository)
        .apply_record_batch(input)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(response))
}
pub(super) async fn duplicate_record(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<(StatusCode, Json<Record>), ApiError> {
    let record = AttricatMutationService::new(&repository)
        .duplicate_record(record_id)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(record)))
}

pub(super) async fn get_record_form(
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace_id): super::auth::ActiveWorkspace,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<RecordFormResponse>, ApiError> {
    let (record, values) = AttricatReadService::new(&repository)
        .record_with_values(record_id)
        .await?;
    // The remaining reads are independent of each other.
    let (blueprint, reusable_attributes, reusable_values, can_write) = tokio::try_join!(
        async {
            repository
                .get_blueprint_revision(record.blueprint_id, record.blueprint_version)
                .await?
                .ok_or_else(|| ApiError::not_found("blueprint version"))
        },
        async { Ok(repository.record_reusable_attributes(record_id).await?) },
        async { Ok(repository.reusable_form_values(record_id).await?) },
        async {
            Ok(repository
                .principal_may(
                    principal.actor(),
                    workspace_id,
                    "records.write",
                    Some(record_id),
                    None,
                )
                .await?)
        },
    )?;
    Ok(Json(RecordFormResponse {
        can_write,
        context: record
            .projections
            .get("preview")
            .cloned()
            .ok_or(ApiError::internal(
                "record is missing its preview projection",
            ))?,
        record,
        blueprint,
        values,
        reusable_attributes,
        reusable_values,
    }))
}
pub(super) async fn update_record_form(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<UpdateRecordFormRequest>,
) -> Result<Json<Record>, ApiError> {
    let record = AttricatMutationService::new(&repository)
        .update_record(record_id, input)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(record))
}
pub(super) async fn list_incoming_relationships(
    State(state): State<AppState>,
    principal: super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<IncomingRelationshipsRequest>,
) -> Result<Json<IncomingRelationshipsPage>, ApiError> {
    if input.relationships.is_empty() {
        return Err(ApiError::invalid_input(
            "relationships must not be empty".to_owned(),
        ));
    }
    let requested = input.page.size.unwrap_or(DEFAULT_PAGE_SIZE);
    if requested == 0 {
        return Err(ApiError::invalid_input(
            "page.size must be greater than zero".to_owned(),
        ));
    }
    let limit = requested.min(state.max_incoming_relationship_page_size);
    let cursor = match input.page.cursor.as_deref() {
        Some(v) => Some(
            decode_search_cursor(v)
                .ok_or_else(|| ApiError::invalid_input("page.cursor is invalid".to_owned()))?,
        ),
        None => None,
    };
    Ok(Json(
        repository
            .with_authorization_actor(principal.actor())
            .incoming_relationships(record_id, input.relationships, limit.into(), cursor)
            .await?,
    ))
}
pub(super) async fn list_record_publications(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::RecordPublicationStatus>>, ApiError> {
    Ok(Json(repository.publication_statuses(record_id).await?))
}
/// Readiness of the record for each enabled channel's required checks.
pub(super) async fn record_publication_readiness(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::repository::PublicationReadiness>>, ApiError> {
    Ok(Json(repository.publication_readiness(record_id).await?))
}
pub(super) async fn publish_record(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PublicationContextRequest>,
) -> Result<Json<crate::model::RecordPublicationStatus>, ApiError> {
    let status = AttricatMutationService::new(&repository)
        .publish_record(record_id, input.context_id)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(status))
}
pub(super) async fn publish_record_all_channels(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Vec<crate::model::RecordPublicationStatus>>, ApiError> {
    let status = AttricatMutationService::new(&repository)
        .publish_record_all_channels(record_id)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(status))
}
pub(super) async fn unpublish_record(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<PublicationContextRequest>,
) -> Result<StatusCode, ApiError> {
    AttricatMutationService::new(&repository)
        .unpublish_record(record_id, input.context_id)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn preview_record_migration(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<crate::model::RecordMigrationPreview>, ApiError> {
    Ok(Json(repository.preview_record_migration(record_id).await?))
}
pub(super) async fn migrate_record_to_latest(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(version): ApiQuery<RecordVersionQuery>,
    ApiJson(input): ApiJson<MigrateRecordRequest>,
) -> Result<Json<Record>, ApiError> {
    let record = AttricatMutationService::new(&repository)
        .migrate_record_checked(record_id, input, version.expected_updated_at)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(record))
}
pub(super) async fn append_values(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = AttricatMutationService::new(&repository)
        .append_values(record_id, input)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn get_current_values(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
) -> Result<Json<Vec<AttributeValue>>, ApiError> {
    if !repository.record_exists(record_id).await? {
        return Err(ApiError::not_found("record"));
    }
    Ok(Json(repository.current_values(record_id).await?))
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct HistoryPageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

fn checked_history_page(query: &HistoryPageQuery) -> Result<(i64, i64), ApiError> {
    page_bounds(
        query.limit,
        query.offset,
        DEFAULT_HISTORY_PAGE_SIZE,
        MAX_HISTORY_PAGE_SIZE,
    )
}

pub(super) async fn get_record_changes(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HistoryPageQuery>,
) -> Result<Json<Value>, ApiError> {
    if !repository.record_exists(record_id).await? {
        return Err(ApiError::not_found("record"));
    }
    if query.limit.is_none() && query.offset.is_none() {
        return Ok(Json(json!(
            repository.record_audit_changes(record_id).await?
        )));
    }
    let (limit, offset) = checked_history_page(&query)?;
    let (items, has_more) = repository
        .record_audit_changes_page(record_id, limit, offset)
        .await?;
    Ok(Json(
        json!({"items": items, "next_offset": next_offset(has_more, (limit, offset))}),
    ))
}
pub(super) async fn get_value_history(
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiQuery(query): ApiQuery<HistoryPageQuery>,
) -> Result<Json<Value>, ApiError> {
    if !repository.record_exists(record_id).await? {
        return Err(ApiError::not_found("record"));
    }
    if query.limit.is_none() && query.offset.is_none() {
        return Ok(Json(json!(repository.value_history(record_id).await?)));
    }
    let (limit, offset) = checked_history_page(&query)?;
    let (items, has_more) = repository
        .value_history_page(record_id, limit, offset)
        .await?;
    Ok(Json(
        json!({"items": items, "next_offset": next_offset(has_more, (limit, offset))}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordVersionQuery {
    expected_updated_at: Option<DateTime<chrono::Utc>>,
}

pub(super) async fn restore_value(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((record_id, history_id)): ApiPath<(Uuid, Uuid)>,
    ApiQuery(input): ApiQuery<RecordVersionQuery>,
) -> Result<(StatusCode, Json<AttributeValue>), ApiError> {
    let value = AttricatMutationService::new(&repository)
        .restore_value_checked(record_id, history_id, input.expected_updated_at)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(value)))
}
pub(super) async fn replace_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = AttricatMutationService::new(&repository)
        .replace_relationships(record_id, input)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(values)))
}
pub(super) async fn remove_relationships(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(record_id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RelationshipMutation>,
) -> Result<(StatusCode, Json<Vec<AttributeValue>>), ApiError> {
    let values = AttricatMutationService::new(&repository)
        .remove_relationships(record_id, input)
        .await?;
    invalidate_data_health(&state, &repository);
    Ok((StatusCode::CREATED, Json(values)))
}
