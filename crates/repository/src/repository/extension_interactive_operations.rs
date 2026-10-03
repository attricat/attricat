//! User-initiated extension operations bound to an explicit entity selection.
//!
//! The initiating principal, release, operation, validated input, context and
//! ordered membership are frozen in one transaction. Membership is not an
//! authorization grant: the initiator's current access is checked again by
//! every selection read, catalog write and artifact download.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use super::{
    AuthorizationActor, CatalogRepository, ClaimedTask, RepositoryError,
    extension_annotations::own_annotations,
};
use crate::task_queue::{TaskInsert, TaskKind};

/// Interactive runs use the additive 1.5 operation world.
pub const INTERACTIVE_OPERATION_ABI: &str = "1.5.0";
/// Completed output remains downloadable for this long.
pub const OPERATION_OUTPUT_RETENTION_DAYS: i64 = 30;
const MAX_CLIENT_IDEMPOTENCY_BYTES: usize = 64;
const MAX_SELECTION_PAGE: u32 = 10;
/// Leaves headroom below the 64 KiB host JSON bound for the page envelope.
const SELECTION_PAGE_BUDGET_BYTES: usize = 60 * 1024;
const MAX_USER_RUNS: i64 = 50;
/// Safe failure code for an initiator who lost access to the workspace.
pub const INITIATOR_ACCESS_REVOKED: &str = "initiator_access_revoked";

#[derive(Clone, Debug)]
pub struct StartInteractiveOperation {
    pub extension_id: String,
    pub contribution_id: String,
    /// Resolved by the HTTP broker from the authorized contribution.
    pub expected_release_id: Uuid,
    pub operation_id: String,
    pub input: Value,
    pub idempotency_key: String,
    pub entity_ids: Vec<Uuid>,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
    pub actor: AuthorizationActor,
}

/// The run scope enforced by host calls for an interactive run.
#[derive(Clone, Debug)]
pub struct InteractiveRunScope {
    pub actor: AuthorizationActor,
    pub entity_ids: Vec<Uuid>,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Option<Uuid>,
}

#[derive(Clone, Debug, FromRow)]
struct InteractiveRunRow {
    id: Uuid,
    extension_id: String,
    contribution_id: Option<String>,
    operation_id: String,
    actor_user_id: Option<Uuid>,
    status: String,
    cancellation_requested: bool,
    lifecycle_started: bool,
    attempts: i32,
    progress: Value,
    last_error_code: Option<String>,
    outputs_expired: bool,
    selection_blueprint_id: Option<Uuid>,
    selection_blueprint_version: Option<i64>,
    selection_context_id: Option<Uuid>,
    selection_count: i64,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    cancelled_at: Option<DateTime<Utc>>,
}

/// End-user projection of an interactive run. It omits input, configuration,
/// checkpoints, storage keys and component diagnostics.
#[derive(Clone, Debug, Serialize)]
pub struct InteractiveRun {
    pub id: Uuid,
    pub extension_id: String,
    pub contribution_id: Option<String>,
    pub operation_id: String,
    #[serde(skip)]
    pub actor_user_id: Option<Uuid>,
    /// Execution status: `queued`, `running`, `cancelling`, `cancelled`,
    /// `completed` or `failed`. The extension's domain outcome is in progress.
    pub status: &'static str,
    pub progress: Value,
    pub failure: Option<&'static str>,
    pub can_cancel: bool,
    pub selection_count: i64,
    pub blueprint_id: Option<Uuid>,
    pub blueprint_version: Option<i64>,
    pub context_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub outputs_expire_at: Option<DateTime<Utc>>,
    pub outputs_expired: bool,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct InteractiveRunArtifact {
    pub id: Uuid,
    pub name: Option<String>,
    pub media_type: String,
    pub content_length: i64,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<InteractiveRunRow> for InteractiveRun {
    fn from(row: InteractiveRunRow) -> Self {
        let started = row.lifecycle_started || row.attempts > 0;
        let status = match row.status.as_str() {
            "completed" => "completed",
            "cancelled" => "cancelled",
            "dead_letter" => "failed",
            _ if row.cancellation_requested => "cancelling",
            "leased" => "running",
            _ if started => "running",
            _ => "queued",
        };
        let failure = (status == "failed").then_some(match row.last_error_code.as_deref() {
            Some(INITIATOR_ACCESS_REVOKED) => "access_revoked",
            _ => "extension_failed",
        });
        Self {
            id: row.id,
            extension_id: row.extension_id,
            contribution_id: row.contribution_id,
            operation_id: row.operation_id,
            actor_user_id: row.actor_user_id,
            can_cancel: matches!(status, "queued" | "running"),
            status,
            progress: row.progress,
            failure,
            selection_count: row.selection_count,
            blueprint_id: row.selection_blueprint_id,
            blueprint_version: row.selection_blueprint_version,
            context_id: row.selection_context_id,
            created_at: row.created_at,
            outputs_expire_at: (row.status == "completed")
                .then_some(row.completed_at)
                .flatten()
                .map(|at| at + Duration::days(OPERATION_OUTPUT_RETENTION_DAYS)),
            completed_at: row.completed_at,
            cancelled_at: row.cancelled_at,
            outputs_expired: row.outputs_expired,
        }
    }
}

type RunScopeRow = (
    String,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<i64>,
    Option<Uuid>,
);

const RUN_COLUMNS: &str = "r.id,r.extension_id,r.contribution_id,r.operation_id,r.actor_user_id,r.status,r.cancellation_requested,r.lifecycle_started,r.attempts,r.progress,r.last_error_code,r.outputs_expired,r.selection_blueprint_id,r.selection_blueprint_version,r.selection_context_id,(SELECT COUNT(*) FROM extension_operation_run_entities m WHERE m.operation_run_id=r.id) AS selection_count,r.created_at,r.completed_at,r.cancelled_at";

fn request_digest(input: &StartInteractiveOperation) -> String {
    let canonical = json!({
        "contribution_id": input.contribution_id,
        "operation_id": input.operation_id,
        "input": input.input,
        "entity_ids": input.entity_ids,
        "blueprint_id": input.blueprint_id,
        "blueprint_version": input.blueprint_version,
        "context_id": input.context_id,
    });
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical).expect("request digest serializes"))
    )
}

impl CatalogRepository {
    /// A repository for one interactive run's host calls: catalog effects are
    /// bounded by the initiator's live grants and attributed to that user.
    pub fn for_interactive_run(
        &self,
        extension_id: &str,
        run_id: Uuid,
        scope: &InteractiveRunScope,
    ) -> Self {
        let mut repository = self.clone().with_authorization_actor(scope.actor);
        repository.audit_context = Some(super::AuditContext {
            actor_user_id: Some(scope.actor.user_id),
            actor_token_id: scope.actor.token_id,
            request_id: Uuid::new_v4(),
            correlation_id: run_id,
            action: "catalog.extensions.operations.write".to_owned(),
            authorization_scope: json!({
                "extension_id": extension_id,
                "operation_run_id": run_id,
                "invocation": "interactive",
            }),
            target: json!({"type": "extension_operation_run", "id": run_id}),
            metadata: json!({"extension_id": extension_id, "operation_run_id": run_id}),
            agent: None,
        });
        repository.for_extension(extension_id)
    }

    /// Starts, or returns the identical retry of, an interactive run. The whole
    /// selection is rejected if the actor cannot read any one entity.
    pub async fn start_interactive_extension_operation(
        &self,
        input: StartInteractiveOperation,
    ) -> Result<Uuid, RepositoryError> {
        self.start_interactive_operation_attempt(input, false).await
    }

    async fn start_interactive_operation_attempt(
        &self,
        input: StartInteractiveOperation,
        retried: bool,
    ) -> Result<Uuid, RepositoryError> {
        if input.idempotency_key.is_empty()
            || input.idempotency_key.len() > MAX_CLIENT_IDEMPOTENCY_BYTES
            || !input.idempotency_key.bytes().all(|c| c.is_ascii_graphic())
        {
            return Err(RepositoryError::InvalidExtension(format!(
                "idempotency key must be 1-{MAX_CLIENT_IDEMPOTENCY_BYTES} visible ASCII bytes"
            )));
        }
        let digest = request_digest(&input);
        // Keys are scoped to the initiator: two users never share a run.
        let stored_key = format!(
            "interactive:{}:{}",
            input.actor.user_id, input.idempotency_key
        );
        let mut transaction = self.pool.begin().await?;
        let row: Option<(Uuid, Value, Value)> = sqlx::query_as(
            "SELECT i.installed_release_id, i.configuration, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id=i.installed_release_id JOIN workspaces w ON w.id=i.workspace_id WHERE i.workspace_id=$1 AND i.extension_id=$2 AND i.state='enabled' AND w.extensions_enabled FOR SHARE OF i",
        )
        .bind(self.workspace_id.0)
        .bind(&input.extension_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((release, configuration, manifest_value)) = row else {
            return Err(RepositoryError::NotFound("enabled extension installation"));
        };
        if release != input.expected_release_id {
            return Err(RepositoryError::InvalidExtension(
                "extension release changed; reopen the action".into(),
            ));
        }
        // The key names one request whichever operation it targets; the
        // digest (which covers the operation) detects a changed request.
        let existing: Option<(Uuid, Option<String>)> = sqlx::query_as(
            "SELECT id, request_digest FROM extension_operation_runs WHERE workspace_id=$1 AND extension_id=$2 AND installed_release_id=$3 AND idempotency_key=$4 AND invocation='interactive'",
        )
        .bind(self.workspace_id.0)
        .bind(&input.extension_id)
        .bind(release)
        .bind(&stored_key)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some((id, stored_digest)) = existing {
            transaction.commit().await?;
            return if stored_digest.as_deref() == Some(digest.as_str()) {
                Ok(id)
            } else {
                Err(RepositoryError::IdempotencyKeyReused)
            };
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
            .ok_or(RepositoryError::NotFound("extension operation"))?;
        let interactive = operation.interactive.as_ref().ok_or_else(|| {
            RepositoryError::InvalidExtension("operation is not exposed for interactive use".into())
        })?;
        let unique = input
            .entity_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        if input.entity_ids.is_empty()
            || input.entity_ids.len() > interactive.max_selection as usize
            || unique != input.entity_ids.len()
        {
            return Err(RepositoryError::InvalidExtension(format!(
                "selection must contain 1-{} unique entities",
                interactive.max_selection
            )));
        }
        if serde_json::to_vec(&input.input).map_or(true, |bytes| {
            bytes.len() > operation.max_request_bytes as usize
        }) || !input.input.is_object()
        {
            return Err(RepositoryError::InvalidExtension(
                "operation input exceeds its declared bound".into(),
            ));
        }
        if let Some(context_id) = input.context_id {
            // The share lock serializes with `delete_context`, which refuses
            // to delete a context that an active run reads from.
            let exists: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM attribute_contexts WHERE id=$1 AND workspace_id=$2 FOR SHARE",
            )
            .bind(context_id)
            .bind(self.workspace_id.0)
            .fetch_optional(&mut *transaction)
            .await?;
            if exists.is_none() {
                return Err(RepositoryError::InvalidContext);
            }
        }
        let members: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
            "SELECT id, blueprint_id, blueprint_version FROM entities WHERE workspace_id=$1 AND id = ANY($2) AND deleted_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .bind(&input.entity_ids)
        .fetch_all(&mut *transaction)
        .await?;
        if members.len() != input.entity_ids.len()
            || members.iter().any(|(_, blueprint_id, version)| {
                *blueprint_id != input.blueprint_id || *version != input.blueprint_version
            })
        {
            return Err(RepositoryError::InvalidExtension(
                "selection must contain saved entities from one blueprint revision".into(),
            ));
        }
        for entity_id in &input.entity_ids {
            self.ensure_principal_may(&mut transaction, input.actor, "entities.read", *entity_id)
                .await?;
        }
        let id = Uuid::new_v4();
        let inserted: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO extension_operation_runs(id,workspace_id,extension_id,installed_release_id,abi_version,operation_id,actor_user_id,actor_token_id,configuration_snapshot,input,idempotency_key,invocation,contribution_id,selection_blueprint_id,selection_blueprint_version,selection_context_id,request_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'interactive',$12,$13,$14,$15,$16) ON CONFLICT DO NOTHING RETURNING id",
        )
        .bind(id)
        .bind(self.workspace_id.0)
        .bind(&input.extension_id)
        .bind(release)
        .bind(INTERACTIVE_OPERATION_ABI)
        .bind(&input.operation_id)
        .bind(input.actor.user_id)
        .bind(input.actor.token_id)
        .bind(configuration)
        .bind(&input.input)
        .bind(&stored_key)
        .bind(&input.contribution_id)
        .bind(input.blueprint_id)
        .bind(input.blueprint_version)
        .bind(input.context_id)
        .bind(&digest)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(run_id) = inserted else {
            // A concurrent submission with the same key won a unique index;
            // one retry returns its run or reports the key as reused. If the
            // key is still taken, it collides with a non-interactive run
            // (administrative keys are free-form), which no lookup here sees.
            transaction.rollback().await?;
            if retried {
                return Err(RepositoryError::IdempotencyKeyReused);
            }
            return Box::pin(self.start_interactive_operation_attempt(input, true)).await;
        };
        for (position, entity_id) in input.entity_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO extension_operation_run_entities(operation_run_id,workspace_id,position,entity_id) VALUES($1,$2,$3,$4)",
            )
            .bind(run_id)
            .bind(self.workspace_id.0)
            .bind(position as i32)
            .bind(entity_id)
            .execute(&mut *transaction)
            .await?;
        }
        self.enqueue_task(
            &mut transaction,
            TaskInsert {
                workspace_id: self.workspace_id.0,
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
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(run_id)
    }

    /// The initiator's recent interactive runs, newest first.
    /// The actor's recent runs whose whole selection they can still read;
    /// like the detail view, a run with any unreadable member is hidden.
    pub async fn interactive_extension_runs(
        &self,
        actor: AuthorizationActor,
        extension_id: Option<&str>,
    ) -> Result<Vec<InteractiveRun>, RepositoryError> {
        let rows: Vec<InteractiveRunRow> = sqlx::query_as(&format!(
            "SELECT {RUN_COLUMNS} FROM extension_operation_runs r WHERE r.workspace_id=$1 AND r.invocation='interactive' AND r.actor_user_id=$2 AND ($3::text IS NULL OR r.extension_id=$3) ORDER BY r.created_at DESC, r.id DESC LIMIT $4"
        ))
        .bind(self.workspace_id.0)
        .bind(actor.user_id)
        .bind(extension_id)
        .bind(MAX_USER_RUNS)
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(token_id) = actor.token_id
            && !self
                .personal_api_token_permits(token_id, "entities.read")
                .await?
        {
            return Ok(Vec::new());
        }
        let run_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let members: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT operation_run_id, entity_id FROM extension_operation_run_entities WHERE workspace_id=$1 AND operation_run_id = ANY($2)",
        )
        .bind(self.workspace_id.0)
        .bind(&run_ids)
        .fetch_all(&self.pool)
        .await?;
        let entity_ids: Vec<Uuid> = members
            .iter()
            .map(|(_, entity_id)| *entity_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let readable = self
            .authorized_entity_ids(
                actor.user_id,
                self.workspace_id.0,
                "entities.read",
                &entity_ids,
            )
            .await?;
        let hidden: HashSet<Uuid> = members
            .iter()
            .filter(|(_, entity_id)| !readable.contains(entity_id))
            .map(|(run_id, _)| *run_id)
            .collect();
        Ok(rows
            .into_iter()
            .filter(|row| !hidden.contains(&row.id))
            .map(Into::into)
            .collect())
    }

    pub async fn interactive_extension_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<InteractiveRun>, RepositoryError> {
        let row: Option<InteractiveRunRow> = sqlx::query_as(&format!(
            "SELECT {RUN_COLUMNS} FROM extension_operation_runs r WHERE r.id=$1 AND r.workspace_id=$2 AND r.invocation='interactive'"
        ))
        .bind(run_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    /// Completed, unexpired outputs of a completed run. Finalized output of a
    /// running, cancelled or failed run is never offered for download.
    pub async fn interactive_run_artifacts(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<InteractiveRunArtifact>, RepositoryError> {
        Ok(sqlx::query_as(
            "SELECT a.id,a.output_name AS name,a.media_type,a.content_length,a.completed_at FROM extension_operation_artifacts a JOIN extension_operation_runs r ON r.id=a.operation_run_id WHERE a.operation_run_id=$1 AND a.workspace_id=$2 AND a.direction='output' AND a.state='completed' AND r.status='completed' AND NOT r.outputs_expired ORDER BY a.created_at,a.id LIMIT 100",
        )
        .bind(run_id)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Verifies that the actor can still read every member of a run. A
    /// combined artifact may contain any member, so loss of any one denies all.
    pub async fn ensure_principal_may_read_run_selection(
        &self,
        actor: AuthorizationActor,
        run_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        let members: Vec<Uuid> = sqlx::query_scalar(
            "SELECT entity_id FROM extension_operation_run_entities WHERE operation_run_id=$1 AND workspace_id=$2 ORDER BY position",
        )
        .bind(run_id)
        .bind(self.workspace_id.0)
        .fetch_all(&mut *connection)
        .await?;
        for entity_id in members {
            self.ensure_principal_may(&mut connection, actor, "entities.read", entity_id)
                .await?;
        }
        Ok(())
    }

    /// Host-call scope for a run. `None` means the run is administrative.
    pub async fn interactive_run_scope(
        &self,
        run_id: Uuid,
    ) -> Result<Option<InteractiveRunScope>, RepositoryError> {
        let row: Option<RunScopeRow> = sqlx::query_as(
                "SELECT invocation,actor_user_id,actor_token_id,selection_blueprint_id,selection_blueprint_version,selection_context_id FROM extension_operation_runs WHERE id=$1 AND workspace_id=$2",
            )
            .bind(run_id)
            .bind(self.workspace_id.0)
            .fetch_optional(&self.pool)
            .await?;
        let Some((
            invocation,
            Some(user_id),
            token_id,
            Some(blueprint_id),
            Some(blueprint_version),
            context_id,
        )) = row
        else {
            return Ok(None);
        };
        if invocation != "interactive" {
            return Ok(None);
        }
        let entity_ids = sqlx::query_scalar(
            "SELECT entity_id FROM extension_operation_run_entities WHERE operation_run_id=$1 AND workspace_id=$2 ORDER BY position",
        )
        .bind(run_id)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(InteractiveRunScope {
            actor: AuthorizationActor { user_id, token_id },
            entity_ids,
            blueprint_id,
            blueprint_version,
            context_id,
        }))
    }

    /// Whether the initiator is still an active workspace member and, for a
    /// token-initiated run, the token remains live.
    pub async fn interactive_actor_active(
        &self,
        actor: AuthorizationActor,
    ) -> Result<bool, RepositoryError> {
        if !self
            .is_active_principal(actor.user_id, self.workspace_id.0)
            .await?
        {
            return Ok(false);
        }
        let Some(token_id) = actor.token_id else {
            return Ok(true);
        };
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM personal_api_tokens WHERE id=$1 AND user_id=$2 AND workspace_id=$3 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > clock_timestamp()))",
        )
        .bind(token_id)
        .bind(actor.user_id)
        .bind(self.workspace_id.0)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Ends a run whose initiator lost workspace access. It fails closed with a
    /// safe reason instead of continuing under the installer's grants. An
    /// operator can replay the dead-lettered run after access is restored.
    pub async fn fail_extension_operation_for_revoked_initiator(
        &self,
        task: &ClaimedTask,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        self.ensure_task_fence(&mut transaction).await?;
        let changed = sqlx::query(
            "UPDATE extension_operation_runs SET status='dead_letter',last_error_code=$3,last_error_message='the initiating user no longer has access',lease_token=NULL,lease_owner=NULL,lease_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND status='leased' AND lease_token=$4",
        )
        .bind(task.subject_id)
        .bind(self.workspace_id.0)
        .bind(INITIATOR_ACCESS_REVOKED)
        .bind(task.lease_token)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::InvalidExtension(
                "operation run lost its lease".into(),
            ));
        }
        sqlx::query(
            "UPDATE tasks SET status='dead_letter',failures=failures+1,lease_owner=NULL,lease_token=NULL,lease_until=NULL,last_error_code=$4,last_error_message='the initiating user no longer has access',failed_at=clock_timestamp(),updated_at=clock_timestamp() WHERE id=$1 AND status='leased' AND lease_owner=$2 AND lease_token=$3",
        )
        .bind(task.id)
        .bind(&task.lease_owner)
        .bind(task.lease_token)
        .bind(INITIATOR_ACCESS_REVOKED)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Reads one bounded page of a run's frozen selection, resolving saved
    /// values in the pinned context. Members the initiator can no longer read
    /// are returned without data as `unavailable`.
    pub async fn interactive_selection_page(
        &self,
        extension_id: &str,
        scope: &InteractiveRunScope,
        cursor: &str,
        limit: u32,
    ) -> Result<Value, RepositoryError> {
        if limit == 0 || limit > MAX_SELECTION_PAGE {
            return Err(RepositoryError::InvalidExtension(format!(
                "selection page limit must be 1-{MAX_SELECTION_PAGE}"
            )));
        }
        let start: usize = if cursor.is_empty() {
            0
        } else {
            cursor
                .parse()
                .ok()
                .filter(|start| *start < scope.entity_ids.len())
                .ok_or_else(|| {
                    RepositoryError::InvalidExtension("selection cursor is invalid".into())
                })?
        };
        let context_id =
            match scope.context_id {
                Some(context_id) => context_id,
                None => sqlx::query_scalar(
                    "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND code='default'",
                )
                .bind(self.workspace_id.0)
                .fetch_one(&self.pool)
                .await?,
            };
        let read_at: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&self.pool)
            .await?;
        let mut entities = Vec::new();
        let mut bytes = 0;
        let mut next = None;
        for (position, entity_id) in scope
            .entity_ids
            .iter()
            .enumerate()
            .skip(start)
            .take(limit as usize)
        {
            // The connection is released before the entity reads below, which
            // acquire their own: a run never holds two pool connections.
            let readable = {
                let mut connection = self.pool.acquire().await?;
                match self
                    .ensure_principal_may(&mut connection, scope.actor, "entities.read", *entity_id)
                    .await
                {
                    Ok(()) => true,
                    Err(RepositoryError::ActorNotAuthorized) => false,
                    Err(error) => return Err(error),
                }
            };
            let item = if !readable {
                json!({"position": position, "entity_id": entity_id, "status": "unavailable"})
            } else {
                match (
                    self.get_entity(*entity_id).await?,
                    self.resolved_preview(*entity_id, context_id, 0).await?,
                ) {
                    (Some(entity), Some(resolved)) if entity.deleted_at.is_none() => {
                        let revision: i64 = sqlx::query_scalar(
                            "SELECT revision FROM entity_extension_annotation_revisions WHERE workspace_id=$1 AND entity_id=$2 AND extension_id=$3",
                        )
                        .bind(self.workspace_id.0)
                        .bind(entity_id)
                        .bind(extension_id)
                        .fetch_optional(&self.pool)
                        .await?
                        .unwrap_or(0);
                        json!({
                            "position": position,
                            "entity_id": entity_id,
                            "status": "available",
                            "blueprint_id": entity.blueprint_id,
                            "blueprint_version": entity.blueprint_version,
                            "updated_at": entity.updated_at,
                            "values": resolved.values,
                            "annotations": own_annotations(extension_id, &entity.system_tags, &entity.system_metadata, revision),
                        })
                    }
                    _ => json!({"position": position, "entity_id": entity_id, "status": "deleted"}),
                }
            };
            let mut size = serde_json::to_vec(&item).map_or(usize::MAX, |value| value.len());
            let item = if size > SELECTION_PAGE_BUDGET_BYTES {
                let truncated =
                    json!({"position": position, "entity_id": entity_id, "status": "too_large"});
                size = serde_json::to_vec(&truncated).map_or(0, |value| value.len());
                truncated
            } else {
                item
            };
            if !entities.is_empty() && bytes + size > SELECTION_PAGE_BUDGET_BYTES {
                next = Some(position);
                break;
            }
            bytes += size;
            entities.push(item);
        }
        let consumed = start + entities.len();
        let next = next.or((consumed < scope.entity_ids.len()).then_some(consumed));
        Ok(json!({
            "entities": entities,
            "context_id": context_id,
            "read_at": read_at,
            "next_cursor": next.map(|position| position.to_string()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: &str) -> InteractiveRunRow {
        InteractiveRunRow {
            id: Uuid::nil(),
            extension_id: "acme".into(),
            contribution_id: Some("bulk".into()),
            operation_id: "generate".into(),
            actor_user_id: None,
            status: status.into(),
            cancellation_requested: false,
            lifecycle_started: false,
            attempts: 0,
            progress: json!({}),
            last_error_code: None,
            outputs_expired: false,
            selection_blueprint_id: None,
            selection_blueprint_version: None,
            selection_context_id: None,
            selection_count: 1,
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
            cancelled_at: None,
        }
    }

    #[test]
    fn execution_status_is_separate_from_domain_outcome() {
        assert_eq!(InteractiveRun::from(row("pending")).status, "queued");
        let mut started = row("pending");
        started.lifecycle_started = true;
        assert_eq!(InteractiveRun::from(started).status, "running");
        let mut cancelling = row("leased");
        cancelling.cancellation_requested = true;
        let cancelling = InteractiveRun::from(cancelling);
        assert_eq!(cancelling.status, "cancelling");
        assert!(!cancelling.can_cancel);
        let completed = InteractiveRun::from(row("completed"));
        assert_eq!(completed.status, "completed");
        assert!(completed.outputs_expire_at.is_some());
        let mut revoked = row("dead_letter");
        revoked.last_error_code = Some(INITIATOR_ACCESS_REVOKED.into());
        assert_eq!(
            InteractiveRun::from(revoked).failure,
            Some("access_revoked")
        );
        assert_eq!(
            InteractiveRun::from(row("dead_letter")).failure,
            Some("extension_failed")
        );
    }
}
