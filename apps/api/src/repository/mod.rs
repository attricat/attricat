use std::collections::HashSet;

use catalog_validation::is_valid_code;
use chrono::{DateTime, NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    blueprint_resolver::compile_definition,
    model::{
        AppendAttributeValues, Attribute, AttributeContext, AttributeValue, AttributeValueHistory,
        AttributeValueSelector, Blueprint, BlueprintWithAttributes, CreateBlueprint, Entity,
        EntityHierarchyItem, EntityHierarchyResponse, EntityIdentity, EntityMigrationPreview,
        EntityPreview, EntityPreviewPage, FormAttributeValue, IncomingRelationshipItem,
        IncomingRelationshipSelector, IncomingRelationshipsPage, MigrateEntityRequest,
        MigrationIssue, NewAttributeValue, RelationshipMutation, RelationshipTargets,
        RelationshipTreeFacetChildItem, RelationshipTreeFacetChildrenResponse,
        RelationshipTreeFacetItem, RelationshipTreeFacetResponse, ResolvedEntityPreviewResponse,
    },
};

mod blueprints;
mod contexts;
mod entity_commands;
mod entity_migration;
mod entity_projection;
mod entity_search;
mod health;
mod values;

pub(crate) use entity_search::decode_search_cursor;

#[derive(Clone)]
/// The stable catalog persistence facade. Feature modules add inherent methods
/// here so HTTP handlers and other callers do not depend on storage internals.
pub struct CatalogRepository {
    pub(in crate::repository) pool: PgPool,
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0} was not found")]
    NotFound(&'static str),
    #[error("the context code 'default' is reserved")]
    ReservedContextCode,
    #[error("code must contain only ASCII letters, numbers, hyphens, and underscores")]
    InvalidCode,
    #[error("context data must be a JSON object")]
    InvalidContextData,
    #[error("context was not found")]
    InvalidContext,
    #[error("the default context cannot be changed or deleted")]
    DefaultContextProtected,
    #[error("a context cannot be its own descendant")]
    ContextCycle,
    #[error("a context with descendants or active values cannot be deleted")]
    ContextInUse,
    #[error("attribute can only be edited in the default context")]
    DefaultContextOnly,
    #[error("attribute does not belong to the entity blueprint version")]
    AttributeNotApplicable,
    #[error("provide exactly one of attribute_id or attribute_code")]
    InvalidAttributeSelector,
    #[error("attribute kind does not match the supplied value")]
    AttributeKindMismatch,
    #[error("value does not match the attribute type")]
    AttributeValueTypeMismatch,
    #[error(
        "value for attribute '{attribute}' does not match its schema at '{instance_path}': {message}"
    )]
    AttributeValueSchemaMismatch {
        attribute: String,
        instance_path: String,
        message: String,
    },
    #[error(
        "resolved entity values for context '{context}' do not match the entity schema at '{instance_path}': {message}"
    )]
    EntitySchemaMismatch {
        context: String,
        instance_path: String,
        message: String,
    },
    #[error("stored attribute value does not match its attribute type")]
    InvalidStoredAttributeValue,
    #[error("relationship target does not match the attribute target blueprint")]
    RelationshipTargetTypeMismatch,
    #[error("entity preview must be a JSON object organized by context")]
    InvalidPreview,
    #[error("hierarchy field must be a self-targeting relationship")]
    InvalidHierarchyRelationship,
    #[error("invalid blueprint definition: {0}")]
    InvalidBlueprintDefinition(String),
    #[error("blueprint code is already owned by another blueprint")]
    BlueprintCodeTaken,
    #[error("blueprint revision is not published")]
    BlueprintNotPublished,
    #[error("entity is already on the latest blueprint revision")]
    EntityBlueprintCurrent,
    #[error("the latest blueprint revision changed; refresh the migration preview")]
    MigrationTargetChanged,
    #[error("migration does not apply to this entity")]
    MigrationNotApplicable,
    #[error("migration needs resolutions for: {}", .0.join(", "))]
    MigrationNeedsResolution(Vec<String>),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    pub(crate) fn invalid_blueprint_definition(error: impl std::fmt::Display) -> Self {
        Self::InvalidBlueprintDefinition(error.to_string())
    }
}

impl CatalogRepository {
    const DEFAULT_CONTEXT_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);

    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn purge_value_history(&self, retention_days: i64) -> Result<(), RepositoryError> {
        sqlx::query("SELECT purge_attribute_value_history($1 * interval '1 day')")
            .bind(retention_days)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_audit_event(
        &self,
        workspace_id: Uuid,
        actor_user_id: Option<Uuid>,
        request_id: Uuid,
        correlation_id: Uuid,
        action: &str,
        authorization_scope: Value,
        target: Value,
        outcome: &str,
        metadata: Value,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO audit_events (id, workspace_id, actor_user_id, request_id, correlation_id, action, authorization_scope, target, outcome, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(request_id)
        .bind(correlation_id)
        .bind(action)
        .bind(authorization_scope)
        .bind(target)
        .bind(outcome)
        .bind(metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn is_active_principal(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_request_principal_active($1, $2)")
                .bind(user_id)
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Checks the durable membership/grant graph in one database operation so
    /// callers cannot learn whether an out-of-scope target exists.
    pub async fn is_authorized(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT catalog_authorize_request($1, $2, $3, $4, $5)")
                .bind(user_id)
                .bind(workspace_id)
                .bind(permission)
                .bind(target_id)
                .bind(target_code)
                .fetch_one(&self.pool)
                .await?,
        )
    }
}

pub(crate) fn validate_code(value: &str) -> Result<(), RepositoryError> {
    if is_valid_code(value) {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCode)
    }
}

pub(crate) fn missing_required_fields(message: &str, target_codes: &HashSet<&str>) -> Vec<String> {
    let mut fields = HashSet::new();
    for delimiter in ['"', '\''] {
        for (index, value) in message.split(delimiter).enumerate() {
            if index % 2 == 1 && target_codes.contains(value) {
                fields.insert(value.to_owned());
            }
        }
    }
    fields.into_iter().collect()
}
