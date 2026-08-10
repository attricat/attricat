use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    model::{
        AppendAttributeValues, BlueprintWithAttributes, CreateAttributeContext, CreateBlueprint,
        CreateEntity, Entity,
    },
    repository::{CatalogRepository, RepositoryError},
};

#[derive(Clone)]
pub struct AppState {
    pub repository: CatalogRepository,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/blueprints", post(create_blueprint))
        .route(
            "/blueprints/{blueprint_id}/versions",
            post(create_blueprint_revision),
        )
        .route("/blueprints/{blueprint_id}", get(get_blueprint))
        .route("/contexts", post(create_context))
        .route("/entities", post(create_entity))
        .route("/entities/{entity_id}", get(get_entity))
        .route(
            "/entities/{entity_id}/projections/preview",
            get(get_preview),
        )
        .route("/entities/{entity_id}/values", post(append_values))
        .with_state(state)
}

async fn create_blueprint(
    State(state): State<AppState>,
    Json(input): Json<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_blueprint(input).await?),
    ))
}

async fn create_blueprint_revision(
    State(state): State<AppState>,
    Path(blueprint_id): Path<Uuid>,
    Json(input): Json<CreateBlueprint>,
) -> Result<(StatusCode, Json<BlueprintWithAttributes>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .repository
                .create_blueprint_revision(blueprint_id, input)
                .await?,
        ),
    ))
}

async fn get_blueprint(
    State(state): State<AppState>,
    Path(blueprint_id): Path<Uuid>,
) -> Result<Json<BlueprintWithAttributes>, ApiError> {
    state
        .repository
        .get_current_blueprint(blueprint_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("blueprint"))
}

async fn create_context(
    State(state): State<AppState>,
    Json(input): Json<CreateAttributeContext>,
) -> Result<(StatusCode, Json<crate::model::AttributeContext>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_context(input).await?),
    ))
}

async fn create_entity(
    State(state): State<AppState>,
    Json(input): Json<CreateEntity>,
) -> Result<(StatusCode, Json<Entity>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.create_entity(input).await?),
    ))
}

async fn get_entity(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
) -> Result<Json<Entity>, ApiError> {
    state
        .repository
        .get_entity(entity_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("entity"))
}

async fn get_preview(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let entity = state
        .repository
        .get_entity(entity_id)
        .await?
        .ok_or_else(|| ApiError::not_found("entity"))?;
    entity
        .projections
        .get("preview")
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::internal("entity preview is missing"))
}

async fn append_values(
    State(state): State<AppState>,
    Path(entity_id): Path<Uuid>,
    Json(input): Json<AppendAttributeValues>,
) -> Result<(StatusCode, Json<Vec<crate::model::AttributeValue>>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(state.repository.append_values(entity_id, input).await?),
    ))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn not_found(resource: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: format!("{resource} was not found"),
        }
    }

    fn internal(message: &'static str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.to_owned(),
        }
    }
}

impl From<RepositoryError> for ApiError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::NotFound(resource) => Self::not_found(resource),
            RepositoryError::AttributeNotApplicable => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_not_applicable",
                message: error.to_string(),
            },
            RepositoryError::AttributeKindMismatch => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "attribute_kind_mismatch",
                message: error.to_string(),
            },
            RepositoryError::InvalidProjections | RepositoryError::ReservedContextCode => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_input",
                message: error.to_string(),
            },
            RepositoryError::InvalidBlueprintDefinition(_) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: "invalid_blueprint_definition",
                message: error.to_string(),
            },
            RepositoryError::BlueprintCodeTaken => Self {
                status: StatusCode::CONFLICT,
                code: "conflict",
                message: error.to_string(),
            },
            RepositoryError::Database(sqlx::Error::Database(database_error))
                if database_error.is_unique_violation() =>
            {
                Self {
                    status: StatusCode::CONFLICT,
                    code: "conflict",
                    message: "a record with the same unique value already exists".to_owned(),
                }
            }
            RepositoryError::Database(_) => Self::internal("database operation failed"),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: &self.message,
                },
            })),
        )
            .into_response()
    }
}
