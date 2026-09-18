//! Shared-task handler for safe blueprint migration batches.
//!
//! Delivery, retries, heartbeats, expiry recovery, and shutdown are owned by
//! `task_worker`; this module deliberately has no local channel or lease.

use crate::{
    repository::{CatalogRepository, ClaimedTask},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use async_trait::async_trait;
use uuid::Uuid;

pub struct BlueprintMigrationBatchTaskHandler {
    repository: CatalogRepository,
}

impl BlueprintMigrationBatchTaskHandler {
    pub fn new(repository: CatalogRepository) -> Self {
        Self { repository }
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
            .run_safe_blueprint_migration_batch_task(task.subject_id)
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
