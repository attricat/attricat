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
use std::env;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct BlueprintMigrationBatchConfig {
    pub page_size: usize,
    pub concurrency: usize,
}

impl Default for BlueprintMigrationBatchConfig {
    fn default() -> Self {
        Self {
            page_size: 100,
            concurrency: 4,
        }
    }
}

impl BlueprintMigrationBatchConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            page_size: bounded_config_value(
                "BLUEPRINT_MIGRATION_PAGE_SIZE",
                env::var("BLUEPRINT_MIGRATION_PAGE_SIZE").ok().as_deref(),
                100,
                1000,
            )?,
            concurrency: bounded_config_value(
                "BLUEPRINT_MIGRATION_CONCURRENCY",
                env::var("BLUEPRINT_MIGRATION_CONCURRENCY").ok().as_deref(),
                4,
                64,
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
            .await
            .map_err(task_error)?
            .for_blueprint_migration_task(&task);
        match repository
            .run_safe_blueprint_migration_batch_task(
                task.subject_id,
                self.config.page_size,
                self.config.concurrency,
            )
            .await
        {
            Ok(()) => Ok(TaskOutcome::Complete),
            Err(error) => {
                // The queue owns the five-attempt budget and dead-letter
                // record. Keep the user-visible batch in step on the final
                // failed attempt, fenced by this same task token.
                if task.failures + 1 >= task.max_failures {
                    repository
                        .dead_letter_safe_blueprint_migration_batch(task.subject_id)
                        .await
                        .map_err(task_error)?;
                }
                Err(task_error(error))
            }
        }
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
