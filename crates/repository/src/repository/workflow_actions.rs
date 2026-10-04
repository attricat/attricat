//! Workflow action execution. Every workflow write, whether to the trigger
//! entity or to an entity that references it, goes through
//! [`CatalogRepository::apply_workflow_actions`], which uses the ordinary
//! entity update seam, so it is validated, audited and published exactly like
//! any other entity write.
use super::system_annotations::{TagMetadataPatch, apply_tag_metadata_patch};
use super::*;
use crate::model::UpdateEntityFormRequest;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;

/// Bound for an operator-visible per-target error message.
const MAX_TARGET_ERROR_BYTES: usize = 1024;

fn workflow_event_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| match value {
        Value::Object(_) => value.get(key),
        Value::Array(values) => key
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index)),
        _ => None,
    })
}

fn bounded_error(message: &str) -> String {
    message[..message.floor_char_boundary(MAX_TARGET_ERROR_BYTES)].to_owned()
}

/// The accumulated effect of one or more local actions on one locked entity.
#[derive(Default)]
struct WorkflowEntityChange {
    values: Vec<NewAttributeValue>,
    system_tags: Option<Vec<String>>,
    system_metadata: Option<Value>,
}

impl WorkflowEntityChange {
    /// Derives desired tags/metadata from current state: event payloads
    /// intentionally contain only immutable facts and are unordered.
    fn stage(
        &mut self,
        entity: &Entity,
        action: &catalog_workflow::Action,
        event: &crate::domain_events::DomainEvent,
    ) -> Result<(), RepositoryError> {
        use catalog_workflow::Action;
        match action {
            Action::SystemTagsAdd { tags } => self.patch_tags(
                entity,
                TagMetadataPatch {
                    add_tags: tags.clone(),
                    ..TagMetadataPatch::default()
                },
            ),
            Action::SystemTagsRemove { tags } => self.patch_tags(
                entity,
                TagMetadataPatch {
                    remove_tags: tags.clone(),
                    ..TagMetadataPatch::default()
                },
            ),
            Action::SystemMetadataMerge { values } => self.patch_metadata(
                entity,
                TagMetadataPatch {
                    set_metadata: values.clone(),
                    ..TagMetadataPatch::default()
                },
            )?,
            Action::SystemMetadataDelete { keys } => self.patch_metadata(
                entity,
                TagMetadataPatch {
                    remove_metadata: keys.clone(),
                    ..TagMetadataPatch::default()
                },
            )?,
            Action::AttributeWrite {
                attribute_code,
                value,
            } => self.values.push(NewAttributeValue::Scalar {
                attribute_id: None,
                attribute_code: Some(attribute_code.clone()),
                context_id: None,
                value: scalar_source_value(value, event)?,
            }),
            Action::ReferencingEntitiesUpdate { .. } => {
                return Err(RepositoryError::InvalidWorkflowDefinition(
                    "referencing_entities_update is not a local entity action".into(),
                ));
            }
        }
        Ok(())
    }

    fn patch_tags(&mut self, entity: &Entity, patch: TagMetadataPatch) {
        let tags = self
            .system_tags
            .get_or_insert_with(|| entity.system_tags.clone());
        apply_tag_metadata_patch(tags, &mut Map::new(), &patch);
    }

    fn patch_metadata(
        &mut self,
        entity: &Entity,
        patch: TagMetadataPatch,
    ) -> Result<(), RepositoryError> {
        let mut metadata = self
            .system_metadata
            .as_ref()
            .unwrap_or(&entity.system_metadata)
            .as_object()
            .cloned()
            .ok_or(RepositoryError::InvalidSystemMetadata)?;
        apply_tag_metadata_patch(&mut Vec::new(), &mut metadata, &patch);
        self.system_metadata = Some(Value::Object(metadata));
        Ok(())
    }
}

/// Resolves an attribute write's value, which must be a JSON scalar.
fn scalar_source_value(
    source: &catalog_workflow::ScalarSource,
    event: &crate::domain_events::DomainEvent,
) -> Result<Value, RepositoryError> {
    let value = match source {
        catalog_workflow::ScalarSource::Fixed { fixed } => fixed.clone(),
        catalog_workflow::ScalarSource::Event { event_field } => {
            workflow_event_path(&event.payload, event_field)
                .cloned()
                .ok_or_else(|| {
                    RepositoryError::InvalidWorkflowDefinition("event field missing".into())
                })?
        }
    };
    if value.is_array() || value.is_object() {
        return Err(RepositoryError::InvalidWorkflowDefinition(
            "event field must resolve to a scalar".into(),
        ));
    }
    Ok(value)
}

/// One `referencing_entities_update` action of a claimed run, applied to
/// each entity that references the trigger entity.
struct ReferencingAction<'a> {
    run: &'a super::ClaimedWorkflowRun,
    action_index: i32,
    relationship_attribute: &'a str,
    actions: &'a [catalog_workflow::Action],
    event: &'a crate::domain_events::DomainEvent,
}

impl CatalogRepository {
    /// Executes one workflow action. A local action records its idempotency key
    /// in the same transaction as the locked entity mutation, audit, and outbox
    /// event, so a reclaimed run observes the key and cannot repeat effects.
    pub async fn execute_workflow_action(
        &self,
        run: &super::ClaimedWorkflowRun,
        action_index: i32,
        action: &catalog_workflow::Action,
        event: &crate::domain_events::DomainEvent,
    ) -> Result<super::WorkflowActionResult, RepositoryError> {
        if let catalog_workflow::Action::ReferencingEntitiesUpdate {
            relationship_attribute,
            max_targets,
            actions,
        } = action
        {
            return self
                .execute_workflow_referencing_action(
                    &ReferencingAction {
                        run,
                        action_index,
                        relationship_attribute,
                        actions,
                        event,
                    },
                    *max_targets,
                )
                .await;
        }
        let mut transaction = self.pool.begin().await?;
        if !self.fence_workflow_action(&mut transaction, run).await? {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::Cancelled);
        }
        let marker = sqlx::query("INSERT INTO workflow_run_actions(run_id,action_index) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(run.run.id).bind(action_index).execute(&mut *transaction).await?;
        if marker.rows_affected() == 0 {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::AlreadyCompleted);
        }
        let entity = self
            .lock_entity(&mut transaction, event.aggregate_id)
            .await?;
        self.apply_workflow_actions(
            &mut transaction,
            entity,
            std::slice::from_ref(action),
            event,
        )
        .await?;
        transaction.commit().await?;
        Ok(super::WorkflowActionResult::Executed)
    }

    /// Takes the lifecycle lock, then the run lock, and validates the task
    /// fence. Returns `false` when the run must stop; the caller then commits
    /// (persisting any cancellation) and reports the run as cancelled.
    async fn fence_workflow_action(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        run: &super::ClaimedWorkflowRun,
    ) -> Result<bool, RepositoryError> {
        let ws = self.workspace_id.0;
        // Every path that changes workflow state takes this lock before a run
        // lock. Keeping that order aligned with disable avoids a run/lifecycle
        // deadlock while making disable a durable execution fence.
        let enabled_version: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT enabled_version FROM workflow_lifecycles WHERE workflow_id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.run.workflow_id)
        .bind(ws)
        .fetch_optional(&mut **transaction)
        .await?
        .flatten();
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM workflow_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(run.run.id)
        .bind(ws)
        .fetch_optional(&mut **transaction)
        .await?;
        match status.as_deref() {
            Some("cancelled") => {
                self.ensure_task_fence(transaction).await?;
                return Ok(false);
            }
            Some("pending") => {}
            _ => {
                return Err(RepositoryError::InvalidWorkflowDefinition(
                    "workflow run is not executable".into(),
                ));
            }
        }
        // Fence the marker before it can become visible. The same transaction
        // later fences the catalog effect and outbox/audit writes as well.
        self.ensure_task_fence(transaction).await?;
        if enabled_version != Some(run.run.workflow_version) {
            sqlx::query("UPDATE workflow_runs SET status='cancelled',cancelled_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='pending'")
                .bind(run.run.id)
                .execute(&mut **transaction)
                .await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Applies local actions to one locked entity as a single ordinary entity
    /// update through [`Self::update_entity_in_transaction`]: schema, status
    /// transition, check and annotation validation, preview rebuild, audit
    /// evidence, publication reconciliation and one `entity.updated.v1`
    /// outbox event. Like every server-side write, it does not enforce an
    /// attribute's `readonly` flag, which only marks the field read-only in
    /// the editing UI.
    async fn apply_workflow_actions(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: Entity,
        actions: &[catalog_workflow::Action],
        event: &crate::domain_events::DomainEvent,
    ) -> Result<(), RepositoryError> {
        let mut change = WorkflowEntityChange::default();
        for action in actions {
            change.stage(&entity, action, event)?;
        }
        let WorkflowEntityChange {
            values,
            system_tags,
            system_metadata,
        } = change;
        // The caller holds the row lock, so the entity cannot have changed.
        let (_, changes, event) = self
            .update_entity_in_transaction(
                transaction,
                entity.id,
                UpdateEntityFormRequest {
                    expected_updated_at: Some(entity.updated_at),
                    values,
                    relationships: Vec::new(),
                    remove_values: Vec::new(),
                    system_tags,
                    system_metadata,
                },
            )
            .await?;
        // Revalidates the task fence immediately before the durable effect's
        // audit/outbox completion boundary.
        self.stage_entity_mutation(transaction, changes, event)
            .await
    }

    /// Live entities whose `relationship_attribute` (of their current blueprint
    /// revision or their own additional attributes) actively targets `target`
    /// in any context, in stable ID order. `only` narrows the check to one entity.
    async fn referencing_entities(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        target: Uuid,
        relationship_attribute: &str,
        only: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        super::references::referencing_entity_ids(
            transaction,
            self.workspace_id.0,
            super::references::ReferenceQuery {
                target,
                attribute_code: relationship_attribute,
                referrers: super::references::Referrers::Any,
                only,
                exclude_target: true,
                limit,
            },
        )
        .await
    }

    /// Updates every entity that references the trigger entity. Each target is
    /// its own transaction with its own `(run, action, entity)` key, so a retry
    /// re-attempts only targets that failed or were not reached. The action's
    /// own marker is written only after every current target has settled.
    async fn execute_workflow_referencing_action(
        &self,
        action: &ReferencingAction<'_>,
        max_targets: u32,
    ) -> Result<super::WorkflowActionResult, RepositoryError> {
        let ReferencingAction {
            run,
            action_index,
            relationship_attribute,
            ..
        } = *action;
        let mut transaction = self.pool.begin().await?;
        if !self.fence_workflow_action(&mut transaction, run).await? {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::Cancelled);
        }
        let completed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workflow_run_actions WHERE run_id=$1 AND action_index=$2)",
        )
        .bind(run.run.id)
        .bind(action_index)
        .fetch_one(&mut *transaction)
        .await?;
        if completed {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::AlreadyCompleted);
        }
        let targets = self
            .referencing_entities(
                &mut transaction,
                action.event.aggregate_id,
                relationship_attribute,
                None,
                i64::from(max_targets) + 1,
            )
            .await?;
        let recorded: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT entity_id,status FROM workflow_run_action_targets WHERE run_id=$1 AND action_index=$2 ORDER BY entity_id",
        )
        .bind(run.run.id)
        .bind(action_index)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        if targets.len() > max_targets as usize {
            return Err(RepositoryError::InvalidWorkflowDefinition(format!(
                "more than {max_targets} entities reference the trigger entity through '{relationship_attribute}'; raise max_targets or narrow the relationship"
            )));
        }
        let settled: HashSet<Uuid> = recorded
            .iter()
            .filter(|(_, status)| status != "failed")
            .map(|(entity_id, _)| *entity_id)
            .collect();
        // Previously failed targets are always revisited, even if they stopped
        // referencing the trigger entity; they then settle as `skipped`.
        let mut pending: Vec<Uuid> = targets
            .into_iter()
            .chain(
                recorded
                    .into_iter()
                    .filter(|(_, status)| status == "failed")
                    .map(|(entity_id, _)| entity_id),
            )
            .filter(|target| !settled.contains(target))
            .collect();
        pending.sort_unstable();
        pending.dedup();
        let mut failures = Vec::new();
        for target in &pending {
            match self.update_referencing_entity(action, *target).await {
                Ok(true) => {}
                Ok(false) => return Ok(super::WorkflowActionResult::Cancelled),
                // A lost task lease is not a target failure: stop immediately.
                Err(error @ RepositoryError::Task(_)) => return Err(error),
                Err(error) => {
                    let message = bounded_error(&error.to_string());
                    self.record_referencing_target_failure(action, *target, &message)
                        .await?;
                    failures.push((*target, message));
                }
            }
        }
        if let Some((entity_id, message)) = failures.first() {
            return Err(RepositoryError::InvalidWorkflowDefinition(format!(
                "referencing_entities_update action {action_index} failed for {} of {} pending target entities; first failure {entity_id}: {message}",
                failures.len(),
                pending.len()
            )));
        }
        let mut transaction = self.pool.begin().await?;
        if !self.fence_workflow_action(&mut transaction, run).await? {
            transaction.commit().await?;
            return Ok(super::WorkflowActionResult::Cancelled);
        }
        sqlx::query("INSERT INTO workflow_run_actions(run_id,action_index) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(run.run.id)
            .bind(action_index)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(super::WorkflowActionResult::Executed)
    }

    /// Returns `false` when the run was cancelled or disabled. A target that no
    /// longer exists or no longer references the trigger entity is recorded as
    /// `skipped`; it is never written.
    async fn update_referencing_entity(
        &self,
        action: &ReferencingAction<'_>,
        entity_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        if !self
            .fence_workflow_action(&mut transaction, action.run)
            .await?
        {
            transaction.commit().await?;
            return Ok(false);
        }
        // Lock before rechecking the reference: relationship writes lock the
        // same entity row, so the reference cannot change underneath us.
        let entity = match self.lock_entity(&mut transaction, entity_id).await {
            Ok(entity) => Some(entity),
            Err(RepositoryError::NotFound(_)) => None,
            Err(error) => return Err(error),
        };
        let references = match &entity {
            Some(_) => !self
                .referencing_entities(
                    &mut transaction,
                    action.event.aggregate_id,
                    action.relationship_attribute,
                    Some(entity_id),
                    1,
                )
                .await?
                .is_empty(),
            None => false,
        };
        let status = if references { "completed" } else { "skipped" };
        let marker = sqlx::query(
            r#"INSERT INTO workflow_run_action_targets AS t (run_id, action_index, entity_id, workspace_id, status)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (run_id, action_index, entity_id) DO UPDATE
               SET status = EXCLUDED.status, attempts = t.attempts + 1, last_error = NULL, updated_at = clock_timestamp()
               WHERE t.status = 'failed'"#,
        )
        .bind(action.run.run.id)
        .bind(action.action_index)
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(status)
        .execute(&mut *transaction)
        .await?;
        if marker.rows_affected() == 1
            && let Some(entity) = entity.filter(|_| references)
        {
            self.apply_workflow_actions(&mut transaction, entity, action.actions, action.event)
                .await?;
        } else {
            self.ensure_task_fence(&mut transaction).await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// Records a failed target attempt outside the rolled-back target write,
    /// so partial failures stay visible while the run retries.
    async fn record_referencing_target_failure(
        &self,
        action: &ReferencingAction<'_>,
        entity_id: Uuid,
        message: &str,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.ensure_task_fence(&mut transaction).await?;
        sqlx::query(
            r#"INSERT INTO workflow_run_action_targets AS t (run_id, action_index, entity_id, workspace_id, status, last_error)
               VALUES ($1, $2, $3, $4, 'failed', $5)
               ON CONFLICT (run_id, action_index, entity_id) DO UPDATE
               SET attempts = t.attempts + 1, last_error = EXCLUDED.last_error, updated_at = clock_timestamp()
               WHERE t.status = 'failed'"#,
        )
        .bind(action.run.run.id)
        .bind(action.action_index)
        .bind(entity_id)
        .bind(self.workspace_id.0)
        .bind(message)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{bounded_error, workflow_event_path};
    use serde_json::json;

    #[test]
    fn resolves_the_compiled_facts_array_path_exactly() {
        let event = json!({"facts": [{"attribute_code": "title"}]});
        assert_eq!(
            workflow_event_path(&event, "facts.0.attribute_code"),
            Some(&json!("title"))
        );
        assert_eq!(workflow_event_path(&event, "facts.title"), None);
        assert_eq!(workflow_event_path(&event, "facts.1.attribute_code"), None);
    }

    #[test]
    fn target_errors_are_bounded_on_a_char_boundary() {
        let message = "é".repeat(600);
        let bounded = bounded_error(&message);
        assert!(bounded.len() <= 1024);
        assert!(message.starts_with(&bounded));
    }
}
