//! Shared-task handler for safe blueprint migration batches.
//!
//! Delivery, retries, heartbeats, expiry recovery, and shutdown are owned by
//! `task_worker`; this module deliberately has no local channel or lease.

use crate::{
    repository::{ClaimedTask, SystemRepository},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use async_trait::async_trait;
use uuid::Uuid;

const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 1000;
const DEFAULT_CONCURRENCY: usize = 4;
const MAX_CONCURRENCY: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct BlueprintMigrationBatchConfig {
    pub page_size: usize,
    pub concurrency: usize,
}

impl Default for BlueprintMigrationBatchConfig {
    fn default() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            concurrency: DEFAULT_CONCURRENCY,
        }
    }
}

impl BlueprintMigrationBatchConfig {
    /// Parses the `BLUEPRINT_MIGRATION_*` settings, defaulting each unset one.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        Ok(Self {
            page_size: bounded_config_value(
                "BLUEPRINT_MIGRATION_PAGE_SIZE",
                get("BLUEPRINT_MIGRATION_PAGE_SIZE").as_deref(),
                DEFAULT_PAGE_SIZE,
                MAX_PAGE_SIZE,
            )?,
            concurrency: bounded_config_value(
                "BLUEPRINT_MIGRATION_CONCURRENCY",
                get("BLUEPRINT_MIGRATION_CONCURRENCY").as_deref(),
                DEFAULT_CONCURRENCY,
                MAX_CONCURRENCY,
            )?,
        })
    }
}

fn bounded_config_value(
    name: &str,
    value: Option<&str>,
    default: usize,
    maximum: usize,
) -> Result<usize, String> {
    match value {
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|value| (1..=maximum).contains(value))
            .ok_or_else(|| format!("{name} must be an integer between 1 and {maximum}")),
        None => Ok(default),
    }
}

pub struct BlueprintMigrationBatchTaskHandler {
    repository: SystemRepository,
    config: BlueprintMigrationBatchConfig,
}

impl BlueprintMigrationBatchTaskHandler {
    pub fn new(repository: impl Into<SystemRepository>) -> Self {
        Self::with_config(repository, BlueprintMigrationBatchConfig::default())
    }

    pub fn with_config(
        repository: impl Into<SystemRepository>,
        config: BlueprintMigrationBatchConfig,
    ) -> Self {
        Self {
            repository: repository.into(),
            config,
        }
    }
}

#[async_trait]
impl TaskHandler for BlueprintMigrationBatchTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::BlueprintMigrationBatchV1
    }

    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let payload_batch_id = task
            .payload
            .get("batch_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| value.parse::<Uuid>().ok())
            .ok_or_else(|| TaskHandlerError {
                code: "invalid_payload",
                message: "blueprint migration task payload has no valid batch_id".to_owned(),
            })?;
        if payload_batch_id != task.subject_id {
            return Err(TaskHandlerError {
                code: "subject_mismatch",
                message: "blueprint migration task subject does not match payload".to_owned(),
            });
        }
        let repository = self
            .repository
            .for_workspace(task.workspace_id)
            .for_task(&task);
        match repository
            .run_safe_blueprint_migration_batch_task(
                task.subject_id,
                self.config.page_size,
                self.config.concurrency,
            )
            .await
        {
            Ok(()) => Ok(TaskOutcome::Complete),
            Err(error) => Err(task_error(error)),
        }
    }

    /// The queue owns the failure budget and dead-letter record. This keeps
    /// the user-visible batch in step when that budget runs out.
    async fn on_exhausted(
        &self,
        task: &ClaimedTask,
        _message: &str,
    ) -> Result<bool, TaskHandlerError> {
        self.repository
            .for_workspace(task.workspace_id)
            .for_task(task)
            .dead_letter_safe_blueprint_migration_batch(task.subject_id)
            .await
            .map_err(task_error)?;
        Ok(false)
    }
}

fn task_error(error: crate::repository::RepositoryError) -> TaskHandlerError {
    TaskHandlerError {
        code: "blueprint_migration_batch",
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_config_value;

    #[test]
    fn batch_config_values_use_defaults_and_enforce_bounds() {
        assert_eq!(bounded_config_value("TEST", None, 4, 64), Ok(4));
        assert_eq!(bounded_config_value("TEST", Some("1"), 4, 64), Ok(1));
        assert_eq!(bounded_config_value("TEST", Some("64"), 4, 64), Ok(64));
        assert!(bounded_config_value("TEST", Some("0"), 4, 64).is_err());
        assert!(bounded_config_value("TEST", Some("65"), 4, 64).is_err());
        assert!(bounded_config_value("TEST", Some("invalid"), 4, 64).is_err());
    }
}
