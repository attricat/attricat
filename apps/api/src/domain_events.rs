use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

pub const ENTITY_CREATED_V1: &str = "entity.created.v1";
pub const ENTITY_UPDATED_V1: &str = "entity.updated.v1";
pub const ENTITY_DELETED_V1: &str = "entity.deleted.v1";
pub const ENTITY_MIGRATED_V1: &str = "entity.migrated.v1";
pub const ATTRIBUTE_VALUE_CHANGED_V1: &str = "attribute_value.changed.v1";
pub const ATTRIBUTE_VALUE_RESTORED_V1: &str = "attribute_value.restored.v1";
pub const RELATIONSHIP_CHANGED_V1: &str = "relationship.changed.v1";
pub const BLUEPRINT_CREATED_V1: &str = "blueprint.created.v1";
pub const BLUEPRINT_REVISION_CREATED_V1: &str = "blueprint.revision_created.v1";
pub const BLUEPRINT_PUBLISHED_V1: &str = "blueprint.published.v1";
pub const CONTEXT_CREATED_V1: &str = "context.created.v1";
pub const CONTEXT_UPDATED_V1: &str = "context.updated.v1";
pub const CONTEXT_DELETED_V1: &str = "context.deleted.v1";

const CORE_EVENT_TYPES: &[&str] = &[
    ENTITY_CREATED_V1,
    ENTITY_UPDATED_V1,
    ENTITY_DELETED_V1,
    ENTITY_MIGRATED_V1,
    ATTRIBUTE_VALUE_CHANGED_V1,
    ATTRIBUTE_VALUE_RESTORED_V1,
    RELATIONSHIP_CHANGED_V1,
    BLUEPRINT_CREATED_V1,
    BLUEPRINT_REVISION_CREATED_V1,
    BLUEPRINT_PUBLISHED_V1,
    CONTEXT_CREATED_V1,
    CONTEXT_UPDATED_V1,
    CONTEXT_DELETED_V1,
];
const CORE_EVENT_PREFIXES: &[&str] = &[
    "entity.",
    "attribute_value.",
    "relationship.",
    "blueprint.",
    "context.",
];
const MAX_JSON_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSourceKind {
    Api,
    Worker,
    Plugin,
    System,
}

impl EventSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Worker => "worker",
            Self::Plugin => "plugin",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventSource {
    pub kind: EventSourceKind,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NewDomainEvent {
    pub event_type: String,
    pub aggregate_kind: String,
    pub aggregate_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub source: EventSource,
    pub metadata: Value,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
pub struct DomainEvent {
    pub id: Uuid,
    pub sequence: i64,
    pub workspace_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub event_type: String,
    pub aggregate_kind: String,
    pub aggregate_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub source_kind: String,
    pub source_name: String,
    pub metadata: Value,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextCreatedV1 {
    pub context_id: Uuid,
    pub code: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EventContractError {
    #[error(
        "event type must be a known core type or a namespaced plugin type ending in .v<version>"
    )]
    InvalidEventType,
    #[error("plugin event types cannot use a reserved core namespace")]
    ReservedCoreNamespace,
    #[error(
        "aggregate kind and source name must be non-blank ASCII identifiers no longer than 128 bytes"
    )]
    InvalidRoutingField,
    #[error("event metadata and payload must be JSON objects no larger than 64 KiB")]
    InvalidJsonFacts,
}

impl NewDomainEvent {
    pub fn validate(&self) -> Result<(), EventContractError> {
        validate_event_type(&self.event_type)?;
        if !valid_identifier(&self.aggregate_kind) || !valid_identifier(&self.source.name) {
            return Err(EventContractError::InvalidRoutingField);
        }
        if !json_object_within_limit(&self.metadata) || !json_object_within_limit(&self.payload) {
            return Err(EventContractError::InvalidJsonFacts);
        }
        Ok(())
    }
}

fn validate_event_type(event_type: &str) -> Result<(), EventContractError> {
    if CORE_EVENT_TYPES.contains(&event_type) {
        return Ok(());
    }
    if CORE_EVENT_PREFIXES
        .iter()
        .any(|prefix| event_type.starts_with(prefix))
    {
        return Err(EventContractError::ReservedCoreNamespace);
    }
    let Some(version) = event_type.rsplit('.').next() else {
        return Err(EventContractError::InvalidEventType);
    };
    if !event_type.starts_with("plugin.")
        || !version.starts_with('v')
        || version.len() == 1
        || !version[1..].bytes().all(|byte| byte.is_ascii_digit())
        || event_type.split('.').count() < 4
        || !event_type.split('.').all(valid_identifier)
    {
        return Err(EventContractError::InvalidEventType);
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn json_object_within_limit(value: &Value) -> bool {
    value.is_object() && serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= MAX_JSON_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(event_type: &str) -> NewDomainEvent {
        NewDomainEvent {
            event_type: event_type.to_owned(),
            aggregate_kind: "context".to_owned(),
            aggregate_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            causation_id: None,
            source: EventSource {
                kind: EventSourceKind::Api,
                name: "catalog_api".to_owned(),
            },
            metadata: json!({}),
            payload: json!({}),
        }
    }

    #[test]
    fn validates_core_and_plugin_event_types() {
        assert!(event(CONTEXT_CREATED_V1).validate().is_ok());
        assert!(event("plugin.acme.score_recomputed.v1").validate().is_ok());
        assert_eq!(
            event("context.plugin_event.v1").validate(),
            Err(EventContractError::ReservedCoreNamespace)
        );
        assert_eq!(event("plugin.acme.score_recomputed.v0").validate(), Ok(()));
        assert_eq!(
            event("plugin.acme.score_recomputed").validate(),
            Err(EventContractError::InvalidEventType)
        );
    }

    #[test]
    fn envelope_serializes_with_a_stable_shape() {
        let encoded = serde_json::to_value(ContextCreatedV1 {
            context_id: Uuid::nil(),
            code: "regional".to_owned(),
            parent_id: Some(Uuid::nil()),
        })
        .unwrap();
        assert_eq!(
            encoded,
            json!({"context_id": Uuid::nil(), "code": "regional", "parent_id": Uuid::nil()})
        );
    }
}
