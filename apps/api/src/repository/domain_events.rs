use async_trait::async_trait;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::domain_events::{DomainEvent, NewDomainEvent};

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
        event: NewDomainEvent,
    ) -> Result<DomainEvent, RepositoryError> {
        event
            .validate()
            .map_err(RepositoryError::InvalidDomainEvent)?;
        Ok(sqlx::query_as::<_, DomainEvent>(
            r#"INSERT INTO domain_events (
                    id, workspace_id, event_type, aggregate_kind, aggregate_id,
                    correlation_id, causation_id, source_kind, source_name, metadata, payload
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                RETURNING id, sequence, workspace_id, occurred_at, event_type, aggregate_kind,
                    aggregate_id, correlation_id, causation_id, source_kind, source_name, metadata, payload"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
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
        Ok(sqlx::query_as::<_, EventConsumer>(
            "INSERT INTO event_consumers (id, workspace_id, name, watermark) SELECT $1, $2, $3, COALESCE(max(sequence), 0) FROM domain_events WHERE workspace_id = $2 ON CONFLICT (workspace_id, name) DO UPDATE SET name = EXCLUDED.name RETURNING id, workspace_id, name, watermark",
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn create_event_consumer(
        &self,
        name: &str,
    ) -> Result<EventConsumer, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
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

    /// Atomically advances the consumer watermark, creates deliveries for
    /// supported event versions, and leases one eligible delivery. Locked rows
    /// are skipped so API replicas never execute the same active lease.
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
            "WITH candidate AS (SELECT d.consumer_id, d.event_id FROM event_deliveries d WHERE d.consumer_id = $1 AND ((d.status = 'pending' AND d.next_attempt_at <= clock_timestamp()) OR (d.status = 'leased' AND d.lease_until <= clock_timestamp())) ORDER BY d.next_attempt_at, d.event_id FOR UPDATE SKIP LOCKED LIMIT 1), leased AS (UPDATE event_deliveries d SET status = 'leased', attempts = d.attempts + 1, lease_owner = $2, lease_until = clock_timestamp() + ($3 * interval '1 millisecond'), last_error = NULL FROM candidate c WHERE d.consumer_id = c.consumer_id AND d.event_id = c.event_id RETURNING d.consumer_id, d.event_id, d.attempts) SELECT l.consumer_id, l.event_id, l.attempts, e.id, e.sequence, e.workspace_id, e.occurred_at, e.event_type, e.aggregate_kind, e.aggregate_id, e.correlation_id, e.causation_id, e.source_kind, e.source_name, e.metadata, e.payload FROM leased l JOIN domain_events e ON e.id = l.event_id",
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
    ) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE event_deliveries SET status = 'pending', next_attempt_at = clock_timestamp() + ($3 * interval '1 millisecond'), failed_at = clock_timestamp(), last_error = $4, lease_owner = NULL, lease_until = NULL WHERE consumer_id = $1 AND event_id = $2 AND status = 'leased' AND lease_owner = $5")
            .bind(delivery.consumer_id)
            .bind(delivery.event_id)
            .bind(delay.as_millis() as i64)
            .bind(error)
            .bind(&delivery.lease_owner)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
