//! Durable, release-pinned server extension operation runs.
//!
//! Operation input is kept only for execution and is deliberately omitted from
//! management projections and audit metadata. Snapshots and diagnostics are
//! recursively redacted before they become durable operator-visible data.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::task_queue::{TaskInsert, TaskKind};

use super::{CatalogRepository, ClaimedTask, RepositoryError};

const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_IDEMPOTENCY_BYTES: usize = 128;

#[derive(Clone, Debug)]
pub struct StartExtensionOperation {
    pub extension_id: String,
    /// The HTTP broker obtains this from an authorized runtime snapshot. The
    /// repository rejects a lifecycle race rather than starting on replacement
    /// code that was not schema-validated by that broker.
    pub expected_release_id: Uuid,
    pub operation_id: String,
    pub input: Value,
    pub source_reference: Value,
    pub destination_reference: Value,
    pub idempotency_key: String,
}

#[derive(Clone, Debug)]
pub struct ClaimedExtensionOperationRun {
    pub id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub operation_id: String,
    pub operation_handler: String,
    pub configuration: Value,
    pub input: Value,
    pub checkpoint: Value,
    pub batch_key: String,
    pub max_checkpoint_bytes: u64,
    /// Set only after a batch checkpoint commits; it is independent from the
    /// checkpoint's contents because `{}` is a valid extension checkpoint.
    pub lifecycle_started: bool,
    pub cancelling: bool,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ExtensionOperationRun {
    pub id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub abi_version: String,
    pub operation_id: String,
    pub status: String,
    pub progress: Value,
    pub checkpoint: Value,
    pub attempts: i32,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

fn bounded_object(value: &Value, name: &str) -> Result<(), RepositoryError> {
    if !value.is_object()
        || serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MAX_JSON_BYTES)
    {
        return Err(RepositoryError::InvalidExtension(format!(
            "operation {name} must be a bounded JSON object"
        )));
    }
    Ok(())
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(items) => Value::Object(
            items
                .iter()
                .map(|(key, value)| {
                    let sensitive = [
                        "secret",
                        "password",
                        "credential",
                        "token",
                        "authorization",
                        "key",
                    ]
                    .iter()
                    .any(|needle| key.to_ascii_lowercase().contains(needle));
                    (
                        key.clone(),
                        if sensitive {
                            Value::String("[redacted]".into())
                        } else {
                            redact(value)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        value => value.clone(),
    }
}

fn bounded_message(value: &str) -> String {
    // Runtime errors are supplied by untrusted components. Keep structured
    // diagnostics only after applying the same recursive redaction used by
    // management-visible snapshots; opaque trap text is not safe to persist.
    match serde_json::from_str::<Value>(value) {
        Ok(value) => serde_json::to_string(&redact(&value))
            .unwrap_or_else(|_| "operation failure (diagnostics redacted)".into())
            .chars()
            .take(1024)
            .collect(),
        Err(_) => "operation failure (diagnostics redacted)".into(),
    }
}

impl CatalogRepository {
    async fn enqueue_extension_operation_task(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        run_id: Uuid,
    ) -> Result<(), RepositoryError> {
        self.enqueue_task(
            transaction,
            TaskInsert {
                workspace_id: self.extension_workspace(),
                kind: TaskKind::ExtensionOperationRunV1,
                subject_id: run_id,
                generation: 0,
                payload: json!({}),
                correlation_id: self
                    .audit_context
                    .as_ref()
                    .map(|audit| audit.correlation_id),
                causation_id: None,
            },
        )
        .await?;
        Ok(())
    }

    /// Starts an operation against the installation snapshot that is enabled
    /// at this instant. Its unique key makes a client retry return the same run
    /// rather than run the operation twice.
    pub async fn start_extension_operation(
        &self,
        input: StartExtensionOperation,
    ) -> Result<Uuid, RepositoryError> {
        if input.idempotency_key.is_empty()
            || input.idempotency_key.len() > MAX_IDEMPOTENCY_BYTES
            || !input.idempotency_key.is_ascii()
        {
            return Err(RepositoryError::InvalidExtension(
                "operation idempotency key must be 1-128 ASCII bytes".into(),
            ));
        }
        bounded_object(&input.input, "input")?;
        bounded_object(&input.source_reference, "source reference")?;
        bounded_object(&input.destination_reference, "destination reference")?;

        let mut transaction = self.pool.begin().await?;
        let row: Option<(Uuid, Value, Value)> = sqlx::query_as(
            "SELECT i.installed_release_id, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id=i.installed_release_id JOIN workspaces w ON w.id=i.workspace_id WHERE i.workspace_id=$1 AND i.extension_id=$2 AND i.state='enabled' AND w.extensions_enabled FOR UPDATE OF i",
        )
        .bind(self.extension_workspace())
        .bind(&input.extension_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((release, configuration, manifest_value)) = row else {
            return Err(RepositoryError::NotFound("enabled extension installation"));
        };
        if release != input.expected_release_id {
            return Err(RepositoryError::InvalidExtension(
                "extension release changed; revalidate the operation request".into(),
            ));
        }
        let manifest: catalog_extension_manifest::Manifest = serde_json::from_value(manifest_value)
            .map_err(|_| {
                RepositoryError::InvalidExtension("stored extension manifest is invalid".into())
            })?;
        let operation = manifest
            .server
            .as_ref()
            .and_then(|server| {
                server
                    .operations
                    .iter()
                    .find(|operation| operation.id == input.operation_id)
            })
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "operation is not declared by the pinned release".into(),
                )
            })?;
        if serde_json::to_vec(&input.input).map_or(true, |bytes| {
            bytes.len() > operation.max_request_bytes as usize
        }) {
            return Err(RepositoryError::InvalidExtension(
                "operation input exceeds declared bound".into(),
            ));
        }

        let source_reference = input.source_reference.clone();
        let id = Uuid::new_v4();
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO extension_operation_runs(id,workspace_id,extension_id,installed_release_id,abi_version,operation_id,actor_user_id,actor_token_id,configuration_snapshot,input,source_reference,destination_reference,idempotency_key) VALUES($1,$2,$3,$4,'1.2.0',$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(workspace_id,extension_id,installed_release_id,operation_id,idempotency_key) DO NOTHING RETURNING id",
        )
        .bind(id)
        .bind(self.extension_workspace())
        .bind(&input.extension_id)
        .bind(release)
        .bind(&input.operation_id)
        .bind(self.audit_context.as_ref().and_then(|audit| audit.actor_user_id))
        .bind(self.audit_context.as_ref().and_then(|audit| audit.actor_token_id))
        .bind(configuration)
        .bind(input.input)
        .bind(redact(&input.source_reference))
        .bind(redact(&input.destination_reference))
        .bind(&input.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(run_id) = inserted {
            self.attach_extension_operation_input_file(
                &mut transaction,
                run_id,
                &input.extension_id,
                release,
                &source_reference,
            )
            .await?;
            self.enqueue_extension_operation_task(&mut transaction, run_id)
                .await?;
            transaction.commit().await?;
            Ok(run_id)
        } else {
            let existing = sqlx::query_scalar(
                "SELECT id FROM extension_operation_runs WHERE workspace_id=$1 AND extension_id=$2 AND installed_release_id=$3 AND operation_id=$4 AND idempotency_key=$5",
            )
            .bind(self.extension_workspace())
            .bind(&input.extension_id)
            .bind(release)
            .bind(&input.operation_id)
            .bind(&input.idempotency_key)
            .fetch_one(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok(existing)
        }
    }

    /// Attaches the claimed envelope to the run before invoking untrusted
    /// code. The task fence is the sole authority for a checkpoint commit.
    pub async fn begin_extension_operation_task(
        &self,
        task: &ClaimedTask,
    ) -> Result<Option<ClaimedExtensionOperationRun>, RepositoryError> {
        if task.kind != TaskKind::ExtensionOperationRunV1
            || task.workspace_id != self.extension_workspace()
        {
            return Err(RepositoryError::InvalidExtension(
                "operation task mismatch".into(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let row: Option<(String, bool, bool, String, Uuid, String, Value, Value, Value, String, i32)> = sqlx::query_as(
            "SELECT status,cancellation_requested,lifecycle_started,extension_id,installed_release_id,operation_id,configuration_snapshot,input,checkpoint,idempotency_key,batch_number FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((
            status,
            cancelling,
            lifecycle_started,
            extension_id,
            release,
            operation_id,
            configuration,
            input,
            checkpoint,
            idempotency_key,
            batch_number,
        )) = row
        else {
            return Err(RepositoryError::InvalidExtension(
                "operation task subject missing".into(),
            ));
        };
        if matches!(status.as_str(), "cancelled" | "completed" | "dead_letter") {
            transaction.commit().await?;
            return Ok(None);
        }
        let manifest_value: Value =
            sqlx::query_scalar("SELECT manifest FROM installed_extension_releases WHERE id=$1")
                .bind(release)
                .fetch_one(&mut *transaction)
                .await?;
        let manifest: catalog_extension_manifest::Manifest = serde_json::from_value(manifest_value)
            .map_err(|_| {
                RepositoryError::InvalidExtension("stored extension manifest is invalid".into())
            })?;
        let handler = manifest
            .server
            .as_ref()
            .and_then(|server| {
                server
                    .operations
                    .iter()
                    .find(|operation| operation.id == operation_id)
                    .map(|operation| (operation.handler.clone(), operation.max_checkpoint_bytes))
            })
            .ok_or_else(|| {
                RepositoryError::InvalidExtension(
                    "pinned operation is not declared by release".into(),
                )
            })?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET status='leased',attempts=attempts+1,lease_token=$3,lease_owner=$4,lease_until=$5,started_at=COALESCE(started_at,clock_timestamp()),updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status IN ('pending','leased')",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .bind(task.lease_token)
        .bind(&task.lease_owner)
        .bind(task.lease_until)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidExtension(
                "operation run is not runnable".into(),
            ));
        }
        transaction.commit().await?;
        Ok(Some(ClaimedExtensionOperationRun {
            id: task.subject_id,
            extension_id,
            installed_release_id: release,
            operation_id,
            operation_handler: handler.0,
            configuration,
            input,
            checkpoint,
            batch_key: format!("{}:{}", idempotency_key, batch_number),
            max_checkpoint_bytes: handler.1,
            lifecycle_started,
            cancelling,
        }))
    }

    /// Releases a run whose pinned release is currently disabled, quarantined,
    /// upgraded, or missing grants. The envelope remains queued for a future
    /// authorization change but never substitutes a newer release.
    pub async fn pause_extension_operation_task(
        &self,
        task: &ClaimedTask,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET status='pending',lease_token=NULL,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$3",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .bind(task.lease_token)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidExtension(
                "operation run lost its lease".into(),
            ));
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Commits exactly one batch while the task token is still live. A stale
    /// worker fails the task-fence check before it can overwrite a checkpoint.
    pub async fn checkpoint_extension_operation_task(
        &self,
        task: &ClaimedTask,
        run: &ClaimedExtensionOperationRun,
        checkpoint: Value,
        progress: Value,
        done: bool,
    ) -> Result<bool, RepositoryError> {
        bounded_object(&checkpoint, "checkpoint")?;
        bounded_object(&progress, "progress")?;
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let changed: Option<String> = sqlx::query_scalar(
            "UPDATE extension_operation_runs SET checkpoint=$3,progress=$4,lifecycle_started=true,batch_number=batch_number+1,status=CASE WHEN cancellation_requested AND cancellation_delivered THEN 'cancelled' WHEN cancellation_requested THEN 'pending' WHEN $5 THEN 'completed' ELSE 'pending' END,completed_at=CASE WHEN $5 AND NOT cancellation_requested THEN clock_timestamp() ELSE completed_at END,cancelled_at=CASE WHEN cancellation_requested AND cancellation_delivered THEN clock_timestamp() ELSE cancelled_at END,lease_token=NULL,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$6 RETURNING status",
        )
        .bind(run.id)
        .bind(self.extension_workspace())
        // Checkpoints are execution state and must round-trip unchanged; they
        // are never returned by management projections. Progress is operator
        // visible, so redact it before persisting it.
        .bind(checkpoint)
        .bind(redact(&progress))
        .bind(done)
        .bind(task.lease_token)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(status) = changed else {
            return Err(RepositoryError::InvalidExtension(
                "operation run lost its lease".into(),
            ));
        };
        transaction.commit().await?;
        Ok(matches!(status.as_str(), "completed" | "cancelled"))
    }

    /// Records that the cooperative cancellation export has returned while
    /// this worker's task lease remains valid. The checkpoint transition only
    /// makes a requested cancellation terminal after this durable delivery.
    pub async fn mark_extension_operation_cancellation_delivered(
        &self,
        task: &ClaimedTask,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET cancellation_delivered=true,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$3 AND cancellation_requested",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .bind(task.lease_token)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidExtension(
                "operation cancellation delivery lost its lease".into(),
            ));
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Reads the cancellation flag again while holding the task fence. This is
    /// called immediately before and after an untrusted component invocation.
    pub async fn extension_operation_cancellation_requested(
        &self,
        task: &ClaimedTask,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let requested: Option<bool> = sqlx::query_scalar(
            "SELECT cancellation_requested FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$3 FOR UPDATE",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .bind(task.lease_token)
        .fetch_optional(&mut *transaction)
        .await?;
        transaction.commit().await?;
        requested
            .ok_or_else(|| RepositoryError::InvalidExtension("operation run lost its lease".into()))
    }

    /// Atomically consumes the task failure budget and transitions the run so
    /// a process crash cannot leave a dead-lettered envelope looking runnable.
    pub async fn fail_extension_operation_task(
        &self,
        task: &ClaimedTask,
        error: &str,
    ) -> Result<bool, RepositoryError> {
        let terminal = task.failures + 1 >= task.max_failures;
        let error = bounded_message(error);
        let mut transaction = self.pool.begin().await?;
        self.for_extension_operation_task(task)
            .ensure_task_fence(&mut transaction)
            .await?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET status=CASE WHEN $3 THEN 'dead_letter' ELSE 'pending' END,last_error_code='operation',last_error_message=$4,lease_token=NULL,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$5",
        )
        .bind(task.subject_id)
        .bind(self.extension_workspace())
        .bind(terminal)
        .bind(&error)
        .bind(task.lease_token)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidExtension(
                "operation run lost its lease".into(),
            ));
        }
        let task_status = if terminal { "dead_letter" } else { "queued" };
        let changed = sqlx::query(
            "UPDATE tasks SET status=$4,failures=failures+1,available_at=CASE WHEN $4='queued' THEN clock_timestamp()+interval '1 second' ELSE available_at END,lease_owner=NULL,lease_token=NULL,lease_until=NULL,last_error_code='operation',last_error_message=$5,failed_at=CASE WHEN $4='dead_letter' THEN clock_timestamp() ELSE failed_at END,updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3 AND lease_until>clock_timestamp()",
        )
        .bind(task.id)
        .bind(&task.lease_owner)
        .bind(task.lease_token)
        .bind(task_status)
        .bind(&error)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::Task(super::TaskError::LeaseLost));
        }
        transaction.commit().await?;
        Ok(terminal)
    }

    pub async fn cancel_extension_operation(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(id)
        .bind(self.extension_workspace())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(status) = status else {
            transaction.commit().await?;
            return Ok(false);
        };
        let changed = match status.as_str() {
            "pending" => {
                let changed = sqlx::query("UPDATE extension_operation_runs SET status='cancelled',cancelled_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1")
                    .bind(id).execute(&mut *transaction).await?.rows_affected();
                let task_id: Uuid = sqlx::query_scalar("SELECT id FROM tasks WHERE workspace_id=$1 AND kind='extension_operation_run.v1' AND subject_id=$2 AND status='queued' ORDER BY generation DESC LIMIT 1 FOR UPDATE")
                    .bind(self.extension_workspace()).bind(id).fetch_one(&mut *transaction).await?;
                self.cancel_queued_task(&mut transaction, task_id).await?;
                changed
            }
            "leased" => sqlx::query("UPDATE extension_operation_runs SET cancellation_requested=true,updated_at=clock_timestamp() WHERE id=$1")
                .bind(id).execute(&mut *transaction).await?.rows_affected(),
            _ => 0,
        };
        transaction.commit().await?;
        Ok(changed == 1)
    }

    pub async fn replay_extension_operation(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET status='pending',cancellation_requested=false,cancellation_delivered=false,attempts=0,last_error_code=NULL,last_error_message=NULL,completed_at=NULL,cancelled_at=NULL,lease_token=NULL,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='dead_letter'",
        )
        .bind(id)
        .bind(self.extension_workspace())
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed == 0 {
            transaction.commit().await?;
            return Ok(false);
        }
        let task_id: Uuid = sqlx::query_scalar("SELECT id FROM tasks WHERE workspace_id=$1 AND kind='extension_operation_run.v1' AND subject_id=$2 AND status='dead_letter' ORDER BY generation DESC LIMIT 1 FOR UPDATE")
            .bind(self.extension_workspace()).bind(id).fetch_one(&mut *transaction).await?;
        self.replay_task(&mut transaction, task_id).await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn list_extension_operation_runs(
        &self,
    ) -> Result<Vec<ExtensionOperationRun>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT id,extension_id,installed_release_id,abi_version,operation_id,status,progress,checkpoint,attempts,last_error_code,created_at,completed_at FROM extension_operation_runs WHERE workspace_id=$1 ORDER BY created_at DESC",
        )
        .bind(self.extension_workspace())
        .fetch_all(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_removes_nested_credential_named_values() {
        let value = json!({"token": "top-secret", "nested": {"apiKey": "also-secret", "safe": 1}});
        assert_eq!(
            redact(&value),
            json!({"token": "[redacted]", "nested": {"apiKey": "[redacted]", "safe": 1}})
        );
    }

    #[test]
    fn bounded_objects_reject_scalars_and_oversized_values() {
        assert!(bounded_object(&json!("not an object"), "input").is_err());
        assert!(bounded_object(&json!({"payload": "x".repeat(MAX_JSON_BYTES)}), "input").is_err());
    }

    #[test]
    fn opaque_failure_diagnostics_are_not_persisted() {
        assert_eq!(
            bounded_message("wasm trap: token=top-secret"),
            "operation failure (diagnostics redacted)"
        );
        assert_eq!(
            bounded_message(r#"{"password":"top-secret","safe":"detail"}"#),
            r#"{"password":"[redacted]","safe":"detail"}"#
        );
    }
}
