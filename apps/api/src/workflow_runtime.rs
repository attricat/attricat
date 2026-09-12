//! Workflow intake is an outbox handler; execution is a separate leased queue.
use crate::{
    domain_events::{ALL_EVENT_TYPES_V1, DomainEvent},
    event_dispatcher::{EventHandler, EventHandlerCommandContext},
    model::NewAttributeValue,
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
    {
        return Ok(());
    }
    let plan: catalog_workflow::CompiledWorkflow =
        serde_json::from_value(run.compiled_plan.clone())?;
    let worker = repository.for_event_handler(&event, &format!("workflow:{}", run.run.workflow_id));
    for (index, action) in plan.actions.iter().enumerate() {
        if repository
            .workflow_action_is_completed(run.run.id, index as i32)
            .await?
        {
            continue;
        }
        let action_worker = worker.for_workflow_run(
            run.run.workflow_id,
            run.run.workflow_version,
            run.run.id,
            index,
        );
        let entity = action_worker
            .get_entity(event.aggregate_id)
            .await?
            .ok_or("trigger entity no longer exists")?;
        match action {
            catalog_workflow::Action::SystemTagsAdd { tags } => {
                let mut result = entity.system_tags;
                for tag in tags {
                    if !result.contains(tag) {
                        result.push(tag.clone());
                    }
                }
                action_worker
                    .update_entity_with_values(
                        event.aggregate_id,
                        vec![],
                        vec![],
                        vec![],
                        Some(result),
                        None,
                    )
                    .await?;
            }
            catalog_workflow::Action::SystemTagsRemove { tags } => {
                let result = entity
                    .system_tags
                    .into_iter()
                    .filter(|tag| !tags.contains(tag))
                    .collect();
                action_worker
                    .update_entity_with_values(
                        event.aggregate_id,
                        vec![],
                        vec![],
                        vec![],
                        Some(result),
                        None,
                    )
                    .await?;
            }
            catalog_workflow::Action::SystemMetadataMerge { values } => {
                let mut metadata = entity
                    .system_metadata
                    .as_object()
                    .cloned()
                    .ok_or("entity metadata must be object")?;
                for (key, value) in values {
                    metadata.insert(key.clone(), value.clone());
                }
                action_worker
                    .update_entity_with_values(
                        event.aggregate_id,
                        vec![],
                        vec![],
                        vec![],
                        None,
                        Some(Value::Object(metadata)),
                    )
                    .await?;
            }
            catalog_workflow::Action::SystemMetadataDelete { keys } => {
                let mut metadata = entity
                    .system_metadata
                    .as_object()
                    .cloned()
                    .ok_or("entity metadata must be object")?;
                for key in keys {
                    metadata.remove(key);
                }
                action_worker
                    .update_entity_with_values(
                        event.aggregate_id,
                        vec![],
                        vec![],
                        vec![],
                        None,
                        Some(Value::Object(metadata)),
                    )
                    .await?;
            }
            catalog_workflow::Action::AttributeWrite {
                attribute_code,
                value,
            } => {
                let value = match value {
                    catalog_workflow::ScalarSource::Fixed { fixed } => fixed.clone(),
                    catalog_workflow::ScalarSource::Event { event_field } => {
                        event_path(&event.payload, event_field)
                            .cloned()
                            .ok_or("event field missing")?
                    }
                };
                action_worker
                    .update_entity_with_values(
                        event.aggregate_id,
                        vec![NewAttributeValue::Scalar {
                            attribute_id: None,
                            attribute_code: Some(attribute_code.clone()),
                            context_id: None,
                            value,
                        }],
                        vec![],
                        vec![],
                        None,
                        None,
                    )
                    .await?;
            }
        }
        repository
            .complete_workflow_action(run.run.id, index as i32)
            .await?;
    }
    Ok(())
}
fn event_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}
fn causal_depth(event: &DomainEvent) -> usize {
    event
        .metadata
        .get("workflow_causal_depth")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize
}
