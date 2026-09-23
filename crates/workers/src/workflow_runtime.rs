//! Workflow intake and schedule advancement are producers. Execution is owned
//! exclusively by the shared task worker after the workflow task cutover.
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    repository::{CatalogRepository, ClaimedTask},
    task_queue::TaskKind,
    task_worker::{TaskHandler, TaskHandlerError, TaskOutcome},
};
use async_trait::async_trait;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

const MAX_CAUSAL_DEPTH: usize = 8;

pub struct WorkflowIntakeHandler;
#[async_trait]
impl EventHandler for WorkflowIntakeHandler {
    fn name(&self) -> &'static str {
        "catalog.workflows"
    }
    fn event_types(&self) -> &'static [&'static str] {
        ALL_EVENT_TYPES_V1
    }
    async fn handle(
        &self,
        event: DomainEvent,
        context: EventHandlerCommandContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        context.repository().fan_out_workflow_runs(&event).await?;
        Ok(())
    }
}

pub fn add_to_registry(
    registry: crate::event_dispatcher::EventHandlerRegistry,
) -> crate::event_dispatcher::EventHandlerRegistry {
    registry
        .with_handler(Arc::new(WorkflowIntakeHandler))
        .expect("workflow handler name is unique")
}

pub struct WorkflowTaskHandler {
    repository: CatalogRepository,
}

pub fn task_handler(repository: CatalogRepository) -> Arc<dyn TaskHandler> {
    Arc::new(WorkflowTaskHandler { repository })
}

#[async_trait]
impl TaskHandler for WorkflowTaskHandler {
    fn kind(&self) -> TaskKind {
        TaskKind::WorkflowRunV1
    }

    async fn handle(&self, task: ClaimedTask) -> Result<TaskOutcome, TaskHandlerError> {
        let scoped = self
            .repository
            .for_workspace(task.workspace_id)
            .await
            .map_err(task_error)?;
        let run = match scoped.workflow_run_for_task(&task).await {
            Ok(run) => run,
            Err(error) => return fail_workflow_run(&scoped, &task, error.to_string()).await,
        };
        // A crash after the token-fenced domain completion but before generic
        // acknowledgement is replay-safe: no action is re-executed.
        if run.run.status == "dead_letter" {
            // Repair an older split terminal transition by terminalizing the
            // reclaimed envelope, never by incorrectly acknowledging it.
            scoped
                .dead_letter_task(
                    &task,
                    "workflow_run",
                    "workflow run is already dead-lettered",
                )
                .await
                .map_err(|error| task_error(error.into()))?;
            return Ok(TaskOutcome::DeadLettered);
        }
        if matches!(run.run.status.as_str(), "completed" | "cancelled") {
            return Ok(TaskOutcome::Complete);
        }
        match execute(&scoped, &run, &task).await {
            Ok(()) => match scoped.complete_workflow_run_task(&task).await {
                Ok(()) => Ok(TaskOutcome::Complete),
                Err(error) => fail_workflow_run(&scoped, &task, error.to_string()).await,
            },
            Err(error) => fail_workflow_run(&scoped, &task, error.to_string()).await,
        }
    }
}

async fn fail_workflow_run(
    scoped: &CatalogRepository,
    task: &ClaimedTask,
    message: String,
) -> Result<TaskOutcome, TaskHandlerError> {
    if task.failures + 1 >= task.max_failures {
        scoped
            .dead_letter_workflow_run_task(task, &bounded_error_message(&message))
            .await
            .map_err(task_error)?;
        Ok(TaskOutcome::DeadLettered)
    } else {
        // The generic task worker owns retries before the failure budget is
        // exhausted; the workflow run remains pending and replayable.
        Err(TaskHandlerError {
            code: "workflow_run",
            message,
        })
    }
}

fn bounded_error_message(message: &str) -> String {
    let mut end = message.len().min(1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}

fn task_error(error: crate::repository::RepositoryError) -> TaskHandlerError {
    TaskHandlerError {
        code: "workflow_run",
        message: error.to_string(),
    }
}

/// The only workflow schedule coordinator. It never claims workflow runs;
/// task-worker instances are the sole execution claimers.
pub fn start_schedule_coordinator(
    repository: CatalogRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let (workspaces, delay) = match repository.active_workspace_ids().await {
                Ok(workspaces) => {
                    metrics::gauge!("catalog_schedule_coordinator_healthy", "kind" => "workflow")
                        .set(1.0);
                    (workspaces, Duration::from_millis(250))
                }
                Err(error) => {
                    metrics::gauge!("catalog_schedule_coordinator_healthy", "kind" => "workflow")
                        .set(0.0);
                    tracing::warn!(%error, "workflow scheduler cannot discover workspaces");
                    (Vec::new(), Duration::from_secs(5))
                }
            };
            for workspace in workspaces {
                match repository.for_workspace(workspace).await {
                    Ok(scoped) => {
                        if let Err(error) = scoped.backfill_workflow_tasks().await {
                            tracing::error!(%error, "workflow task backfill failed");
                        }
                        if let Err(error) = scoped.schedule_workflow_runs().await {
                            tracing::error!(%error, "workflow schedule poll failed");
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "workflow scheduler workspace scope failed")
                    }
                }
            }
            tokio::select! { _ = tokio::time::sleep(delay) => {}, _ = shutdown.changed() => return }
        }
    })
}

async fn execute(
    repository: &CatalogRepository,
    run: &crate::repository::ClaimedWorkflowRun,
    task: &ClaimedTask,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event: DomainEvent = serde_json::from_value(run.trigger_event.clone())?;
    if event.aggregate_kind != "entity"
        || (run.run.source == "event" && event.source_name.starts_with("workflow:"))
        || (run.run.source == "event" && causal_depth(&event) >= MAX_CAUSAL_DEPTH)
        || run.run.causal_depth as usize >= MAX_CAUSAL_DEPTH
    {
        return Ok(());
    }
    let plan: catalog_workflow::CompiledWorkflow =
        serde_json::from_value(run.compiled_plan.clone())?;
    let worker = repository
        .for_event_handler(&event, &format!("workflow:{}", run.run.workflow_id))
        .for_workflow_task(task);
    for (index, action) in plan.actions.iter().enumerate() {
        let action_worker = worker.for_workflow_run(
            run.run.workflow_id,
            run.run.workflow_version,
            run.run.id,
            index,
        );
        match action_worker
            .execute_workflow_action(run, index as i32, action, &event)
            .await?
        {
            crate::repository::WorkflowActionResult::Executed
            | crate::repository::WorkflowActionResult::AlreadyCompleted => {}
            crate::repository::WorkflowActionResult::Cancelled => return Ok(()),
        }
    }
    Ok(())
}

fn causal_depth(event: &DomainEvent) -> usize {
    event
        .metadata
        .get("workflow_causal_depth")
        .and_then(Value::as_u64)
        .filter(|depth| *depth <= MAX_CAUSAL_DEPTH as u64)
        .unwrap_or(0) as usize
}
