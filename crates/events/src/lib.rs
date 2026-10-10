use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use thiserror::Error;
use uuid::Uuid;

pub const RECORD_CREATED_V1: &str = "record.created.v1";
pub const RECORD_UPDATED_V1: &str = "record.updated.v1";
pub const RECORD_DELETED_V1: &str = "record.deleted.v1";
pub const RECORD_MIGRATED_V1: &str = "record.migrated.v1";
pub const RECORD_PUBLISHED_V1: &str = "record.published.v1";
pub const RECORD_UNPUBLISHED_V1: &str = "record.unpublished.v1";
/// An extension changed its own namespace of record system tags/metadata.
pub const RECORD_ANNOTATIONS_CHANGED_V1: &str = "record.annotations_changed.v1";
pub const ATTRIBUTE_VALUE_CHANGED_V1: &str = "attribute_value.changed.v1";
pub const ATTRIBUTE_VALUE_RESTORED_V1: &str = "attribute_value.restored.v1";
pub const RELATIONSHIP_CHANGED_V1: &str = "relationship.changed.v1";
pub const BLUEPRINT_CREATED_V1: &str = "blueprint.created.v1";
pub const BLUEPRINT_REVISION_CREATED_V1: &str = "blueprint.revision_created.v1";
pub const BLUEPRINT_PUBLISHED_V1: &str = "blueprint.published.v1";
pub const CONTEXT_CREATED_V1: &str = "context.created.v1";
pub const CONTEXT_UPDATED_V1: &str = "context.updated.v1";
pub const CONTEXT_DELETED_V1: &str = "context.deleted.v1";

pub const ALL_EVENT_TYPES_V1: &[&str] = &[
    RECORD_CREATED_V1,
    RECORD_UPDATED_V1,
    RECORD_DELETED_V1,
    RECORD_MIGRATED_V1,
    RECORD_PUBLISHED_V1,
    RECORD_UNPUBLISHED_V1,
    RECORD_ANNOTATIONS_CHANGED_V1,
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
    "record.",
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
pub struct BlueprintRevisionV1 {
    pub blueprint_id: Uuid,
    pub code: String,
    pub kind: String,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordPublicationV1 {
    pub record_id: Uuid,
    pub context_id: Uuid,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by_user_id: Option<Uuid>,
    /// `manual` identifies an explicit unpublish; other values identify the
    /// mutation that automatically withdrew publication approval.
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordMigratedV1 {
    pub record_id: Uuid,
    pub blueprint_id: Uuid,
    pub source_version: i64,
    pub target_version: i64,
    pub migration_id: Uuid,
    /// Relationship targets the migration stopped referencing, because it
    /// dropped or re-pointed a relationship. The event carries no `facts`, so
    /// this lets consumers such as rule fan-out reach the released records.
    /// Absent when the migration released none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub released_relationships: Vec<ReleasedRelationshipV1>,
}

/// The targets one relationship attribute released in a migration, at most
/// [`MAX_RELEASED_RELATIONSHIP_TARGETS`] in ascending order. Across all
/// attributes an event lists at most [`MAX_RELEASED_TARGETS_PER_EVENT`], so
/// the payload stays within its size limit however many relationships the
/// migration released.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReleasedRelationshipV1 {
    pub attribute_code: String,
    pub target_record_ids: Vec<Uuid>,
}

/// The most released targets `record.migrated.v1` lists per relationship.
pub const MAX_RELEASED_RELATIONSHIP_TARGETS: usize = 100;

/// The most released targets `record.migrated.v1` lists across all
/// relationships; about 40 KB of the 64 KiB payload limit.
pub const MAX_RELEASED_TARGETS_PER_EVENT: usize = 1_000;

/// The most bytes the serialized `released_relationships` may use, so many
/// relationships with long codes also stay within the payload limit.
pub const MAX_RELEASED_RELATIONSHIPS_BYTES: usize = 48 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextCreatedV1 {
    pub context_id: Uuid,
    pub code: String,
    pub parent_id: Option<Uuid>,
}

pub type ContextUpdatedV1 = ContextCreatedV1;
pub type ContextDeletedV1 = ContextCreatedV1;

/// A normalized catalog fact affected by a record mutation. This deliberately
/// excludes record projections and other snapshots so consumers can update
/// their own state from the smallest useful before/after representation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AffectedFactV1 {
    pub attribute_id: Uuid,
    pub attribute_code: String,
    pub context_id: Option<Uuid>,
    pub context_code: Option<String>,
    pub relationship_target_record_id: Option<Uuid>,
    pub change_kind: String,
    pub before_value: Option<Value>,
    pub after_value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordMutationV1 {
    pub record_id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub facts: Vec<AffectedFactV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttributeValueMutationV1 {
    pub record_id: Uuid,
    pub facts: Vec<AffectedFactV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationshipMutationV1 {
    pub record_id: Uuid,
    pub facts: Vec<AffectedFactV1>,
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
        if !valid_identifier(&self.aggregate_kind) || !valid_source_name(&self.source.name) {
            return Err(EventContractError::InvalidRoutingField);
        }
        if !json_object_within_limit(&self.metadata) || !json_object_within_limit(&self.payload) {
            return Err(EventContractError::InvalidJsonFacts);
        }
        Ok(())
    }
}

fn validate_event_type(event_type: &str) -> Result<(), EventContractError> {
    if ALL_EVENT_TYPES_V1.contains(&event_type) {
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

fn valid_source_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_alphabetic()
            } else {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':')
            }
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
    fn accepts_extension_source_names() {
        let mut event = event(CONTEXT_CREATED_V1);
        event.source.name = "extension:attricat-extension-example".to_owned();
        assert!(event.validate().is_ok());
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
