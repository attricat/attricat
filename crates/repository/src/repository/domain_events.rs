use async_trait::async_trait;
use std::time::Duration;

const EVENT_DELIVERY_MATERIALIZATION_BATCH_SIZE: i64 = 100;

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain_events::{DomainEvent, NewDomainEvent},
    task_queue::{TaskInsert, TaskKind},
};

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, sqlx::FromRow)]
pub struct EventConsumer {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub watermark: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventDelivery {
    pub consumer_id: Uuid,
    pub event_id: Uuid,
    pub attempts: i32,
    lease_owner: String,
    pub event: DomainEvent,
}

/// A delivery loaded through its token-fenced shared task.
#[derive(Debug)]
pub struct TaskEventDelivery {
    pub delivery_id: Uuid,
    pub event: DomainEvent,
}

#[derive(sqlx::FromRow)]
struct EventDeliveryRow {
    consumer_id: Uuid,
    event_id: Uuid,
    attempts: i32,
    id: Uuid,
    sequence: i64,
    workspace_id: Uuid,
    occurred_at: DateTime<Utc>,
    event_type: String,
    aggregate_kind: String,
    aggregate_id: Uuid,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    source_kind: String,
    source_name: String,
    metadata: serde_json::Value,
    payload: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct DeliveryBackfillRow {
    consumer_id: Uuid,
    event_id: Uuid,
    workspace_id: Uuid,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    status: String,
}

#[derive(sqlx::FromRow)]
struct EventTaskSeed {
    event_id: Uuid,
    workspace_id: Uuid,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct TaskEventDeliveryRow {
    delivery_id: Uuid,
    id: Uuid,
    sequence: i64,
    workspace_id: Uuid,
    occurred_at: DateTime<Utc>,
    event_type: String,
    aggregate_kind: String,
    aggregate_id: Uuid,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    source_kind: String,
    source_name: String,
    metadata: serde_json::Value,
    payload: serde_json::Value,
}

impl From<TaskEventDeliveryRow> for TaskEventDelivery {
    fn from(row: TaskEventDeliveryRow) -> Self {
        Self {
            delivery_id: row.delivery_id,
            event: DomainEvent {
                id: row.id,
                sequence: row.sequence,
                workspace_id: row.workspace_id,
                occurred_at: row.occurred_at,
                event_type: row.event_type,
                aggregate_kind: row.aggregate_kind,
                aggregate_id: row.aggregate_id,
                correlation_id: row.correlation_id,
                causation_id: row.causation_id,
                source_kind: row.source_kind,
                source_name: row.source_name,
                metadata: row.metadata,
                payload: row.payload,
            },
        }
    }
}

impl From<EventDeliveryRow> for EventDelivery {
    fn from(row: EventDeliveryRow) -> Self {
        Self {
            consumer_id: row.consumer_id,
            event_id: row.event_id,
            attempts: row.attempts,
            lease_owner: String::new(),
            event: DomainEvent {
                id: row.id,
                sequence: row.sequence,
                workspace_id: row.workspace_id,
                occurred_at: row.occurred_at,
                event_type: row.event_type,
                aggregate_kind: row.aggregate_kind,
                aggregate_id: row.aggregate_id,
                correlation_id: row.correlation_id,
                causation_id: row.causation_id,
                source_kind: row.source_kind,
                source_name: row.source_name,
                metadata: row.metadata,
                payload: row.payload,
            },
        }
    }
}

async fn lock_outbox_boundary(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
) -> Result<(), RepositoryError> {
    // A registration watermark must not pass an event with an allocated but
    // uncommitted sequence. Workflow activation uses this same boundary.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("workflow-activation-boundary:{workspace_id}"))
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

/// Transport-neutral producer boundary. Implementations must append the event
/// to the caller-owned transaction rather than publish it directly, preserving
/// outbox atomicity.
#[async_trait]
pub trait EventPublisher {
    async fn enqueue_event(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        event: NewDomainEvent,
    ) -> Result<DomainEvent, RepositoryError>;
}

#[async_trait]
impl EventPublisher for CatalogRepository {
    async fn enqueue_event(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        mut event: NewDomainEvent,
    ) -> Result<DomainEvent, RepositoryError> {
        super::add_initiating_actor_metadata(&mut event.metadata, self.audit_context.as_ref());
        event
            .validate()
            .map_err(RepositoryError::InvalidDomainEvent)?;
        // Coordinate every application outbox append with workflow activation's
        // high-water capture. This is a transaction-scoped lock, so an activation
        // boundary cannot overtake an event that has allocated a sequence but has
        // not committed yet.
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        lock_outbox_boundary(transaction, workspace_id).await?;
        Ok(sqlx::query_as::<_, DomainEvent>(
            r#"INSERT INTO domain_events (
                    id, workspace_id, event_type, aggregate_kind, aggregate_id,
                    correlation_id, causation_id, source_kind, source_name, metadata, payload
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                RETURNING id, sequence, workspace_id, occurred_at, event_type, aggregate_kind,
                    aggregate_id, correlation_id, causation_id, source_kind, source_name, metadata, payload"#,
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(event.event_type)
        .bind(event.aggregate_kind)
        .bind(event.aggregate_id)
        .bind(event.correlation_id)
        .bind(event.causation_id)
        .bind(event.source.kind.as_str())
        .bind(event.source.name)
        .bind(event.metadata)
        .bind(event.payload)
        .fetch_one(&mut **transaction)
        .await?)
    }
}

impl CatalogRepository {
    /// Registers a durable consumer after the current high-water mark, so a
    /// newly installed internal handler receives future events only.
    pub async fn active_workspace_ids(&self) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT id FROM workspaces WHERE deleted_at IS NULL")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// Idempotently registers a consumer. The INSERT's watermark is evaluated
    /// at registration time; replicas racing to register retain the same
    /// durable consumer rather than replaying historical events.
    pub async fn ensure_event_consumer(
        &self,
        name: &str,
        _event_types: &[&str],
    ) -> Result<EventConsumer, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        // Polling existing consumers should not acquire the boundary lock or
        // update their row on every dispatcher iteration.
        if let Some(consumer) = sqlx::query_as::<_, EventConsumer>(
            "SELECT id, workspace_id, name, watermark FROM event_consumers WHERE workspace_id=$1 AND name=$2",
        )
        .bind(workspace_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(consumer);
        }
        let mut transaction = self.pool.begin().await?;
        lock_outbox_boundary(&mut transaction, workspace_id).await?;
        let consumer = sqlx::query_as::<_, EventConsumer>(
            "INSERT INTO event_consumers (id, workspace_id, name, watermark) SELECT $1, $2, $3, COALESCE(max(sequence), 0) FROM domain_events WHERE workspace_id = $2 ON CONFLICT (workspace_id, name) DO UPDATE SET name = EXCLUDED.name RETURNING id, workspace_id, name, watermark",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(name)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(consumer)
    }

    pub async fn create_event_consumer(
        &self,
        name: &str,
    ) -> Result<EventConsumer, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        lock_outbox_boundary(&mut transaction, workspace_id).await?;
        let watermark = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(max(sequence), 0) FROM domain_events WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await?;
        let consumer = sqlx::query_as::<_, EventConsumer>(
            "INSERT INTO event_consumers (id, workspace_id, name, watermark) VALUES ($1, $2, $3, $4) RETURNING id, workspace_id, name, watermark",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(name)
        .bind(watermark)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(consumer)
    }

    /// Backfills pre-task deliveries and materializes each newly eligible
    /// delivery with its task before moving the consumer watermark. The
    /// coordinator is deliberately only a producer; shared-task handlers own
    /// execution and all receipt transitions.
    pub async fn materialize_event_delivery_tasks<T: AsRef<str>>(
        &self,
        consumer_name: &str,
        event_types: &[T],
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let event_types: Vec<String> = event_types
            .iter()
            .map(|value| value.as_ref().to_owned())
            .collect();
        if event_types.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool.begin().await?;

        // Migrations intentionally contain no data backfill. Assign every old
        // delivery a stable subject and a task in this transaction. A legacy
        // lease becomes queued because no legacy executor is started after the
        // extension cutover.
        let legacy: Vec<DeliveryBackfillRow> = sqlx::query_as(
            "SELECT d.consumer_id, d.event_id, c.workspace_id, e.correlation_id, e.causation_id, d.status FROM event_deliveries d JOIN event_consumers c ON c.id = d.consumer_id JOIN domain_events e ON e.id = d.event_id WHERE c.workspace_id = $1 AND c.name = $2 AND d.id IS NULL FOR UPDATE OF d",
        )
        .bind(workspace_id)
        .bind(consumer_name)
        .fetch_all(&mut *transaction)
        .await?;
        for row in legacy {
            let delivery_id = Uuid::new_v4();
            let task_id = self.enqueue_task(&mut transaction, TaskInsert {
                workspace_id: row.workspace_id,
                kind: TaskKind::EventDeliveryV1,
                subject_id: delivery_id,
                generation: 0,
                payload: serde_json::json!({"consumer_id": row.consumer_id.to_string(), "event_id": row.event_id.to_string()}),
                correlation_id: Some(row.correlation_id),
                causation_id: row.causation_id,
            }).await?.expect("a new delivery subject has no task conflict");
            sqlx::query("UPDATE event_deliveries SET id = $3, task_id = $4, status = CASE WHEN status = 'leased' THEN 'pending' ELSE status END, lease_owner = NULL, lease_until = NULL WHERE consumer_id = $1 AND event_id = $2")
                .bind(row.consumer_id).bind(row.event_id).bind(delivery_id).bind(task_id)
                .execute(&mut *transaction).await?;
            match row.status.as_str() {
                "completed" => {
                    sqlx::query(
                        "UPDATE tasks SET status = 'succeeded', completed_at = now() WHERE id = $1",
                    )
                    .bind(task_id)
                    .execute(&mut *transaction)
                    .await?;
                }
                "dead_letter" => {
                    sqlx::query("UPDATE tasks SET status = 'dead_letter', failures = max_failures, failed_at = now() WHERE id = $1")
                        .bind(task_id).execute(&mut *transaction).await?;
                }
                _ => {}
            }
        }

        let consumer: EventConsumer = sqlx::query_as(
            "SELECT id, workspace_id, name, watermark FROM event_consumers WHERE workspace_id = $1 AND name = $2 FOR UPDATE",
        )
        .bind(workspace_id).bind(consumer_name).fetch_one(&mut *transaction).await?;
        // A contract can become eligible after this shared consumer has already
        // passed its event. Drain that historical gap before advancing again;
        // otherwise a plugin event would be skipped permanently when a consumer
        // is enabled or re-granted after publication.
        let historical: Vec<EventTaskSeed> = sqlx::query_as(
            "SELECT e.id AS event_id, e.workspace_id, e.correlation_id, e.causation_id FROM domain_events e WHERE e.workspace_id = $1 AND e.sequence <= $2 AND e.event_type = ANY($3) AND NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.consumer_id = $4 AND d.event_id = e.id) ORDER BY e.sequence LIMIT $5",
        )
        .bind(workspace_id).bind(consumer.watermark).bind(&event_types).bind(consumer.id).bind(EVENT_DELIVERY_MATERIALIZATION_BATCH_SIZE)
        .fetch_all(&mut *transaction).await?;
        if !historical.is_empty() {
            self.enqueue_event_delivery_tasks(&mut transaction, consumer.id, historical)
                .await?;
            transaction.commit().await?;
            return Ok(());
        }

        // Bound both the scan and the inserts. Every event in this sequence
        // window has been considered before its final sequence becomes the
        // watermark; matching events get a durable receipt and task first.
        let sequences: Vec<i64> = sqlx::query_scalar(
            "SELECT sequence FROM domain_events WHERE workspace_id = $1 AND sequence > $2 ORDER BY sequence LIMIT $3",
        )
        .bind(workspace_id).bind(consumer.watermark).bind(EVENT_DELIVERY_MATERIALIZATION_BATCH_SIZE)
        .fetch_all(&mut *transaction).await?;
        let Some(max_sequence) = sequences.last().copied() else {
            transaction.commit().await?;
            return Ok(());
        };
        let seeds: Vec<EventTaskSeed> = sqlx::query_as(
            "SELECT e.id AS event_id, e.workspace_id, e.correlation_id, e.causation_id FROM domain_events e WHERE e.workspace_id = $1 AND e.sequence > $2 AND e.sequence <= $3 AND e.event_type = ANY($4) AND NOT EXISTS (SELECT 1 FROM event_deliveries d WHERE d.consumer_id = $5 AND d.event_id = e.id) ORDER BY e.sequence",
        )
        .bind(workspace_id).bind(consumer.watermark).bind(max_sequence).bind(&event_types).bind(consumer.id)
        .fetch_all(&mut *transaction).await?;
        self.enqueue_event_delivery_tasks(&mut transaction, consumer.id, seeds)
            .await?;
        sqlx::query("UPDATE event_consumers SET watermark = $2, updated_at = clock_timestamp() WHERE id = $1")
            .bind(consumer.id).bind(max_sequence).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn enqueue_event_delivery_tasks(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        consumer_id: Uuid,
        seeds: Vec<EventTaskSeed>,
    ) -> Result<(), RepositoryError> {
        for seed in seeds {
            let delivery_id = Uuid::new_v4();
            let task_id = self.enqueue_task(transaction, TaskInsert {
                workspace_id: seed.workspace_id,
                kind: TaskKind::EventDeliveryV1,
                subject_id: delivery_id,
                generation: 0,
                payload: serde_json::json!({"consumer_id": consumer_id.to_string(), "event_id": seed.event_id.to_string()}),
                correlation_id: Some(seed.correlation_id),
                causation_id: seed.causation_id,
            }).await?.expect("a new delivery subject has no task conflict");
            sqlx::query("INSERT INTO event_deliveries (consumer_id, event_id, id, task_id) VALUES ($1, $2, $3, $4)")
                .bind(consumer_id).bind(seed.event_id).bind(delivery_id).bind(task_id)
                .execute(&mut **transaction).await?;
        }
        Ok(())
    }

    /// Atomically advances the consumer watermark, creates deliveries for
    /// supported event versions, and leases one eligible delivery. This legacy
    /// claimer remains for non-extension intake handlers during their separate
    /// cutovers; task-linked deliveries are never leased here.
    pub async fn claim_event_delivery(
        &self,
        consumer_name: &str,
        event_types: &[&str],
        lease_owner: &str,
        lease_duration: Duration,
    ) -> Result<Option<EventDelivery>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let event_types: Vec<String> = event_types
            .iter()
            .map(|value| (*value).to_owned())
            .collect();
        let mut transaction = self.pool.begin().await?;
        let consumer: EventConsumer = sqlx::query_as(
            "SELECT id, workspace_id, name, watermark FROM event_consumers WHERE workspace_id = $1 AND name = $2 FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(consumer_name)
        .fetch_one(&mut *transaction)
        .await?;
        let max_sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(sequence), $2) FROM domain_events WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .bind(consumer.watermark)
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO event_deliveries (consumer_id, event_id) SELECT $1, id FROM domain_events WHERE workspace_id = $2 AND sequence > $3 AND sequence <= $4 AND event_type = ANY($5) ON CONFLICT DO NOTHING",
        )
        .bind(consumer.id)
        .bind(workspace_id)
        .bind(consumer.watermark)
        .bind(max_sequence)
        .bind(&event_types)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("UPDATE event_consumers SET watermark = $2, updated_at = clock_timestamp() WHERE id = $1")
            .bind(consumer.id)
            .bind(max_sequence)
            .execute(&mut *transaction)
            .await?;
        let row = sqlx::query_as::<_, EventDeliveryRow>(
            "WITH candidate AS (SELECT d.consumer_id, d.event_id FROM event_deliveries d WHERE d.consumer_id = $1 AND d.task_id IS NULL AND ((d.status = 'pending' AND d.next_attempt_at <= clock_timestamp()) OR (d.status = 'leased' AND d.lease_until <= clock_timestamp())) ORDER BY d.next_attempt_at, d.event_id FOR UPDATE SKIP LOCKED LIMIT 1), leased AS (UPDATE event_deliveries d SET status = 'leased', attempts = d.attempts + 1, lease_owner = $2, lease_until = clock_timestamp() + ($3 * interval '1 millisecond'), last_error = NULL FROM candidate c WHERE d.consumer_id = c.consumer_id AND d.event_id = c.event_id RETURNING d.consumer_id, d.event_id, d.attempts) SELECT l.consumer_id, l.event_id, l.attempts, e.id, e.sequence, e.workspace_id, e.occurred_at, e.event_type, e.aggregate_kind, e.aggregate_id, e.correlation_id, e.causation_id, e.source_kind, e.source_name, e.metadata, e.payload FROM leased l JOIN domain_events e ON e.id = l.event_id",
        )
        .bind(consumer.id)
        .bind(lease_owner)
        .bind(lease_duration.as_millis() as i64)
        .fetch_optional(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(row.map(|row| {
            let mut delivery: EventDelivery = row.into();
            delivery.lease_owner = lease_owner.to_owned();
            delivery
        }))
    }

    /// Claims the delivery receipt under the same token as the shared task.
    /// A stale task can neither invoke extensions nor mutate its receipt.
    pub async fn begin_task_event_delivery(
        &self,
        task: &super::ClaimedTask,
    ) -> Result<Option<TaskEventDelivery>, RepositoryError> {
        let row: Option<TaskEventDeliveryRow> = sqlx::query_as(
            "WITH receipt AS (UPDATE event_deliveries d SET status = 'leased', attempts = $4, lease_owner = $2, lease_until = $5 FROM tasks t WHERE d.id = $1 AND d.task_id = t.id AND t.id = $3 AND t.status = 'leased' AND t.lease_owner = $2 AND t.lease_token = $6 AND t.lease_until > now() AND d.status <> 'completed' RETURNING d.id, d.event_id) SELECT r.id AS delivery_id, e.id, e.sequence, e.workspace_id, e.occurred_at, e.event_type, e.aggregate_kind, e.aggregate_id, e.correlation_id, e.causation_id, e.source_kind, e.source_name, e.metadata, e.payload FROM receipt r JOIN domain_events e ON e.id = r.event_id",
        )
        .bind(task.subject_id).bind(&task.lease_owner).bind(task.id).bind(task.attempts).bind(task.lease_until).bind(task.lease_token)
        .fetch_optional(&self.pool).await?;
        Ok(row.map(Into::into))
    }

    /// Completes the delivery receipt only while the executing task's exact
    /// lease token remains valid. The worker then token-fences task completion.
    pub async fn complete_task_event_delivery(
        &self,
        task: &super::ClaimedTask,
    ) -> Result<(), RepositoryError> {
        let changed = sqlx::query("UPDATE event_deliveries d SET status = 'completed', completed_at = clock_timestamp(), lease_owner = NULL, lease_until = NULL FROM tasks t WHERE d.id = $1 AND d.task_id = t.id AND t.id = $2 AND t.status = 'leased' AND t.lease_owner = $3 AND t.lease_token = $4 AND t.lease_until > now()")
            .bind(task.subject_id).bind(task.id).bind(&task.lease_owner).bind(task.lease_token)
            .execute(&self.pool).await?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(super::RepositoryError::Task(super::TaskError::LeaseLost))
        }
    }

    pub async fn complete_event_delivery(
        &self,
        delivery: &EventDelivery,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE event_deliveries SET status = 'completed', completed_at = clock_timestamp(), lease_owner = NULL, lease_until = NULL WHERE consumer_id = $1 AND event_id = $2 AND status = 'leased' AND lease_owner = $3")
            .bind(delivery.consumer_id)
            .bind(delivery.event_id)
            .bind(&delivery.lease_owner)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn retry_event_delivery(
        &self,
        delivery: &EventDelivery,
        error: &str,
        delay: Duration,
        max_attempts: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE event_deliveries SET status = CASE WHEN attempts >= $6 THEN 'dead_letter' ELSE 'pending' END, next_attempt_at = CASE WHEN attempts >= $6 THEN next_attempt_at ELSE clock_timestamp() + ($3 * interval '1 millisecond') END, failed_at = clock_timestamp(), last_error = $4, lease_owner = NULL, lease_until = NULL WHERE consumer_id = $1 AND event_id = $2 AND status = 'leased' AND lease_owner = $5")
            .bind(delivery.consumer_id)
            .bind(delivery.event_id)
            .bind(delay.as_millis() as i64)
            .bind(error)
            .bind(&delivery.lease_owner)
            .bind(max_attempts)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct FailedEventDelivery {
    pub consumer_id: Uuid,
    pub event_id: Uuid,
    pub consumer_name: String,
    pub event_type: String,
    pub attempts: i32,
    pub failed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl CatalogRepository {
    pub async fn list_failed_event_deliveries(
        &self,
    ) -> Result<Vec<FailedEventDelivery>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as("SELECT d.consumer_id, d.event_id, c.name AS consumer_name, e.event_type, COALESCE(t.attempts, d.attempts) AS attempts, COALESCE(t.failed_at, d.failed_at) AS failed_at, COALESCE(t.last_error_message, d.last_error) AS last_error FROM event_deliveries d JOIN event_consumers c ON c.id = d.consumer_id JOIN domain_events e ON e.id = d.event_id LEFT JOIN tasks t ON t.id = d.task_id WHERE c.workspace_id = $1 AND (d.status = 'dead_letter' OR t.status = 'dead_letter') ORDER BY COALESCE(t.failed_at, d.failed_at) DESC NULLS LAST")
            .bind(workspace_id).fetch_all(&self.pool).await?)
    }

    /// Counts deliveries by state for dispatcher queue-health metrics.
    pub async fn event_delivery_health(&self) -> Result<Vec<(String, i64)>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        Ok(sqlx::query_as("SELECT CASE t.status WHEN 'queued' THEN 'pending' WHEN 'leased' THEN 'leased' WHEN 'succeeded' THEN 'completed' WHEN 'dead_letter' THEN 'dead_letter' ELSE d.status END AS status, count(*) FROM event_deliveries d JOIN event_consumers c ON c.id = d.consumer_id LEFT JOIN tasks t ON t.id = d.task_id WHERE c.workspace_id = $1 GROUP BY CASE t.status WHEN 'queued' THEN 'pending' WHEN 'leased' THEN 'leased' WHEN 'succeeded' THEN 'completed' WHEN 'dead_letter' THEN 'dead_letter' ELSE d.status END")
            .bind(workspace_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Reactivates only a terminal delivery; domain event facts are never modified.
    pub async fn replay_event_delivery(
        &self,
        consumer_id: Uuid,
        event_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        let delivery: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT d.task_id FROM event_deliveries d JOIN event_consumers c ON c.id = d.consumer_id LEFT JOIN tasks t ON t.id = d.task_id WHERE d.consumer_id = $1 AND d.event_id = $2 AND c.workspace_id = $3 AND (d.status = 'dead_letter' OR t.status = 'dead_letter') FOR UPDATE OF d")
            .bind(consumer_id).bind(event_id).bind(workspace_id).fetch_optional(&mut *transaction).await?;
        let Some((task_id,)) = delivery else {
            transaction.rollback().await?;
            return Ok(false);
        };
        let Some(task_id) = task_id else {
            sqlx::query("UPDATE event_deliveries SET status = 'pending', next_attempt_at = clock_timestamp(), lease_owner = NULL, lease_until = NULL, failed_at = NULL, last_error = NULL WHERE consumer_id = $1 AND event_id = $2 AND status = 'dead_letter'")
                .bind(consumer_id).bind(event_id).execute(&mut *transaction).await?;
            transaction.commit().await?;
            return Ok(true);
        };
        let Some(next_task_id) = self.replay_task(&mut transaction, task_id).await? else {
            transaction.rollback().await?;
            return Ok(false);
        };
        sqlx::query("UPDATE event_deliveries SET task_id = $3, status = 'pending', next_attempt_at = clock_timestamp(), lease_owner = NULL, lease_until = NULL, failed_at = NULL, last_error = NULL WHERE consumer_id = $1 AND event_id = $2")
            .bind(consumer_id).bind(event_id).bind(next_task_id).execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(true)
    }
}
