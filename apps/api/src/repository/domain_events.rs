use async_trait::async_trait;
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
}
