//! Workflow intake is an outbox handler; execution is a separate leased queue.
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    repository::CatalogRepository,
};
use async_trait::async_trait;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use uuid::Uuid;

const MAX_CAUSAL_DEPTH: usize = 8;
const MAX_ATTEMPTS: i32 = 5;

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
        // The handler intentionally only creates durable rows; workers execute later.
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

pub fn start(
    repository: CatalogRepository,
    mut shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match run_once(&repository).await {
                Ok(false) => {}
                Ok(true) => {}
                Err(error) => tracing::error!(%error, "workflow run poll failed"),
            }
            tokio::select! { _ = tokio::time::sleep(Duration::from_millis(250)) => {}, _ = shutdown.changed() => return }
        }
    })
}
async fn run_once(
    repository: &CatalogRepository,
) -> Result<bool, crate::repository::RepositoryError> {
    for workspace in repository.active_workspace_ids().await? {
        let scoped = repository.for_workspace(workspace).await?;
        let owner = Uuid::new_v4().to_string();
        let Some(run) = scoped
            .claim_workflow_run(&owner, Duration::from_secs(30))
            .await?
        else {
            continue;
        };
        let result = execute(&scoped, &run).await;
        match result {
            Ok(()) => {
                scoped.complete_workflow_run(&run).await?;
                metrics::counter!("catalog_workflow_runs_total", "outcome" => "completed")
                    .increment(1);
            }
            Err(error) => {
                let delay = Duration::from_secs(
                    (1u64 << (run.run.attempts.saturating_sub(1) as u32).min(6)).min(60),
                );
                scoped
                    .retry_workflow_run(&run, &error.to_string(), delay, MAX_ATTEMPTS)
                    .await?;
                metrics::counter!("catalog_workflow_runs_total", "outcome" => if run.run.attempts >= MAX_ATTEMPTS { "dead_letter" } else { "retry" }).increment(1);
            }
        }
        return Ok(true);
    }
    Ok(false)
}
async fn execute(
    repository: &CatalogRepository,
    run: &crate::repository::ClaimedWorkflowRun,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let event: DomainEvent = serde_json::from_value(run.trigger_event.clone())?;
    // Entity scope and causal chain protection are rechecked at execution, not trusted from intake.
    if event.aggregate_kind != "entity"
        || event.source_name.starts_with("workflow:")
        || causal_depth(&event) >= MAX_CAUSAL_DEPTH
        || run.run.causal_depth as usize >= MAX_CAUSAL_DEPTH
    {
        return Ok(());
    }
    let plan: catalog_workflow::CompiledWorkflow =
        serde_json::from_value(run.compiled_plan.clone())?;
    let worker = repository.for_event_handler(&event, &format!("workflow:{}", run.run.workflow_id));
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
            // Disable/cancellation is terminal and intentionally has no further effect.
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
