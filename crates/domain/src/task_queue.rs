//! Closed task-envelope contract shared by API-owned background work.
//!
//! This module deliberately does not include file processing: that queue has a
//! separate operational boundary and remains owned by `file_worker`.

use std::{fmt, str::FromStr, time::Duration};

use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_TASK_PAYLOAD_BYTES: usize = 4096;
pub const MAX_TASK_PAYLOAD_KEYS: usize = 16;
pub const MAX_TASK_PAYLOAD_KEY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskKind {
    AgentRunV1,
    EventDeliveryV1,
    WorkflowRunV1,
    RuleRunV1,
    BlueprintMigrationBatchV1,
    ExtensionOperationRunV1,
}

impl TaskKind {
    pub const ALL: [Self; 6] = [
        Self::AgentRunV1,
        Self::EventDeliveryV1,
        Self::WorkflowRunV1,
        Self::RuleRunV1,
        Self::BlueprintMigrationBatchV1,
        Self::ExtensionOperationRunV1,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentRunV1 => "agent_run.v1",
            Self::EventDeliveryV1 => "event_delivery.v1",
            Self::WorkflowRunV1 => "workflow_run.v1",
            Self::RuleRunV1 => "rule_run.v1",
            Self::BlueprintMigrationBatchV1 => "blueprint_migration_batch.v1",
            Self::ExtensionOperationRunV1 => "extension_operation_run.v1",
        }
    }

    /// Policy defaults are deliberately bounded and are not a public API.
    /// Individual handlers may only use their registered policy.
    pub const fn policy(self) -> TaskPolicy {
        match self {
            Self::AgentRunV1 => TaskPolicy::new(300, 3, false, false),
            Self::EventDeliveryV1 => TaskPolicy::new(30, 5, true, true),
            Self::WorkflowRunV1 => TaskPolicy::new(30, 5, true, true),
            Self::RuleRunV1 => TaskPolicy::new(30, 5, true, true),
            // Batch checkpoints are token-fenced and retry-safe. A short
            // lease plus heartbeats makes shutdown/crash recovery prompt.
            Self::BlueprintMigrationBatchV1 => TaskPolicy::new(30, 5, false, false),
            Self::ExtensionOperationRunV1 => TaskPolicy::new(30, 5, true, true),
        }
    }
}

impl fmt::Display for TaskKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unknown task kind '{0}'")]
pub struct ParseTaskKindError(pub String);

impl FromStr for TaskKind {
    type Err = ParseTaskKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "agent_run.v1" => Ok(Self::AgentRunV1),
            "event_delivery.v1" => Ok(Self::EventDeliveryV1),
            "workflow_run.v1" => Ok(Self::WorkflowRunV1),
            "rule_run.v1" => Ok(Self::RuleRunV1),
            "blueprint_migration_batch.v1" => Ok(Self::BlueprintMigrationBatchV1),
            "extension_operation_run.v1" => Ok(Self::ExtensionOperationRunV1),
            _ => Err(ParseTaskKindError(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskPolicy {
    pub lease_duration: Duration,
    pub max_failures: i32,
    pub replay_allowed: bool,
    pub cancel_allowed: bool,
}

impl TaskPolicy {
    const fn new(
        lease_seconds: u64,
        max_failures: i32,
        replay_allowed: bool,
        cancel_allowed: bool,
    ) -> Self {
        Self {
            lease_duration: Duration::from_secs(lease_seconds),
            max_failures,
            replay_allowed,
            cancel_allowed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskInsert {
    pub workspace_id: Uuid,
    pub kind: TaskKind,
    pub subject_id: Uuid,
    pub generation: i32,
    pub payload: Value,
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
}

impl TaskInsert {
    pub fn validate(&self) -> Result<(), TaskValidationError> {
        if self.generation < 0 {
            return Err(TaskValidationError::InvalidGeneration);
        }
        let Value::Object(values) = &self.payload else {
            return Err(TaskValidationError::PayloadNotObject);
        };
        if values.len() > MAX_TASK_PAYLOAD_KEYS {
            return Err(TaskValidationError::TooManyPayloadKeys);
        }
        for (key, value) in values {
            if key.is_empty() || key.len() > MAX_TASK_PAYLOAD_KEY_BYTES {
                return Err(TaskValidationError::InvalidPayloadKey);
            }
            if !matches!(
                value,
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
            ) {
                return Err(TaskValidationError::PayloadValueNotScalar);
            }
        }
        if serde_json::to_vec(&self.payload)
            .map_err(|_| TaskValidationError::PayloadTooLarge)?
            .len()
            > MAX_TASK_PAYLOAD_BYTES
        {
            return Err(TaskValidationError::PayloadTooLarge);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskValidationError {
    #[error("task generation must not be negative")]
    InvalidGeneration,
    #[error("task payload must be a JSON object")]
    PayloadNotObject,
    #[error("task payload has too many keys")]
    TooManyPayloadKeys,
    #[error("task payload keys must be non-empty and at most {MAX_TASK_PAYLOAD_KEY_BYTES} bytes")]
    InvalidPayloadKey,
    #[error("task payload values must be scalar identifiers or versions")]
    PayloadValueNotScalar,
    #[error("task payload exceeds {MAX_TASK_PAYLOAD_BYTES} bytes")]
    PayloadTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Leased,
    Succeeded,
    DeadLetter,
    Cancelled,
}

impl TaskStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::Succeeded => "succeeded",
            Self::DeadLetter => "dead_letter",
            Self::Cancelled => "cancelled",
        }
    }
}
