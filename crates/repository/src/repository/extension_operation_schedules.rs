//! Interval-based operation producer. A locked occurrence is started with a
//! deterministic idempotency key before its schedule advances. A crash in
//! between replays the same key; competing coordinators skip the row lock.
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError, StartExtensionOperation};

const MAX_SNAPSHOT_BYTES: usize = 64 * 1024;
const MAX_DUE_PER_POLL: i64 = 16;

#[derive(Clone, Debug)]
pub struct CreateExtensionOperationSchedule {
    pub extension_id: String,
    pub release_id: Uuid,
    pub operation_id: String,
    pub input: Value,
    pub source_reference: Value,
    pub destination_reference: Value,
    pub interval_seconds: i32,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct ExtensionOperationSchedule {
    pub id: Uuid,
    pub extension_id: String,
    pub installed_release_id: Uuid,
    pub operation_id: String,
    pub interval_seconds: i32,
    pub next_at: DateTime<Utc>,
    pub enabled: bool,
    pub paused_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn invalid(message: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

fn safe_snapshot(
    input: &Value,
    source: &Value,
    destination: &Value,
) -> Result<(), RepositoryError> {
    for value in [input, source, destination] {
        if !value.is_object()
            || serde_json::to_vec(value).map_or(true, |v| v.len() > MAX_SNAPSHOT_BYTES)
        {
            return Err(invalid("schedule snapshots must be bounded objects"));
        }
    }
    // Schedules cannot persist arbitrary request bodies, credentials or mutable
    // URLs. A pinned ready workspace file is the only supported input source.
    if !destination.as_object().is_some_and(|map| map.is_empty())
        || !source.as_object().is_some_and(|map| {
            map.is_empty()
                || (map.len() == 1
                    && map
                        .get("input_file_id")
                        .and_then(Value::as_str)
                        .is_some_and(|v| v.parse::<Uuid>().is_ok()))
        })
    {
        return Err(invalid(
            "schedule references must use a ready workspace file, not a URL or credential",
        ));
    }
    fn contains_credentials(v: &Value) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, v)| {
                [
                    "secret",
                    "password",
                    "credential",
                    "token",
                    "authorization",
                    "api_key",
                ]
                .iter()
                .any(|needle| key.to_ascii_lowercase().contains(needle))
                    || contains_credentials(v)
            }),
            Value::Array(items) => items.iter().any(contains_credentials),
            _ => false,
        }
    }
    if contains_credentials(input) {
        return Err(invalid("schedule input cannot contain credential fields"));
    }
    Ok(())
}

const PROJECTION: &str = "id,extension_id,installed_release_id,operation_id,interval_seconds,next_at,enabled,paused_reason,created_at,updated_at";

impl CatalogRepository {
    pub async fn create_extension_operation_schedule(
        &self,
        input: CreateExtensionOperationSchedule,
    ) -> Result<ExtensionOperationSchedule, RepositoryError> {
        if !(60..=2_592_000).contains(&input.interval_seconds) {
            return Err(invalid(
                "schedule interval must be between one minute and 30 days",
            ));
        }
        safe_snapshot(
            &input.input,
            &input.source_reference,
            &input.destination_reference,
        )?;
        let installation = self
            .runtime_extension_installation(&input.extension_id, input.release_id)
            .await?
            .ok_or_else(|| invalid("schedule release is not enabled or authorized"))?;
        let operation = installation
            .manifest
            .server
            .as_ref()
            .and_then(|server| {
                server
                    .operations
                    .iter()
                    .find(|op| op.id == input.operation_id)
            })
            .ok_or_else(|| invalid("schedule operation is not declared"))?;
        if serde_json::to_vec(&input.input)
            .map_or(true, |v| v.len() > operation.max_request_bytes as usize)
        {
            return Err(invalid("schedule input exceeds operation limit"));
        }
        catalog_extension_manifest::validate_schema(&operation.request_schema, &input.input)
            .map_err(|_| invalid("schedule input does not match the operation schema"))?;
        safe_snapshot(
            &installation.configuration,
            &serde_json::json!({}),
            &serde_json::json!({}),
        )?;
        let id = Uuid::new_v4();
        // Validate the file now; occurrence creation revalidates its ready
        // state. The file stays locked until the schedule that references it
        // commits, so reconciliation cannot reclaim it in between.
        let mut transaction = self.pool.begin().await?;
        if !input
            .source_reference
            .as_object()
            .is_some_and(|v| v.is_empty())
        {
            self.validate_extension_operation_schedule_file(
                &mut transaction,
                &input.source_reference,
            )
            .await?;
        }
        let schedule = sqlx::query_as(&format!("INSERT INTO extension_operation_schedules(id,workspace_id,extension_id,installed_release_id,operation_id,input,configuration_snapshot,source_reference,destination_reference,interval_seconds,next_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,clock_timestamp()+($10*interval '1 second')) RETURNING {PROJECTION}"))
            .bind(id).bind(self.extension_workspace()).bind(&input.extension_id).bind(input.release_id)
            .bind(&input.operation_id).bind(input.input).bind(installation.configuration).bind(input.source_reference).bind(input.destination_reference)
            .bind(input.interval_seconds).fetch_one(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(schedule)
    }

    async fn validate_extension_operation_schedule_file(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        source: &Value,
    ) -> Result<(), RepositoryError> {
        let file: Uuid = source["input_file_id"]
            .as_str()
            .ok_or_else(|| invalid("invalid input file"))?
            .parse()
            .map_err(|_| invalid("invalid input file"))?;
        let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM files WHERE id=$1 AND workspace_id=$2 AND status='ready' AND deleted_at IS NULL FOR UPDATE")
            .bind(file).bind(self.extension_workspace()).fetch_optional(&mut **transaction).await?;
        if exists.is_none() {
            return Err(invalid("input file is not ready in this workspace"));
        }
        Ok(())
    }

    pub async fn list_extension_operation_schedules(
        &self,
    ) -> Result<Vec<ExtensionOperationSchedule>, RepositoryError> {
        sqlx::query_as(&format!("SELECT {PROJECTION} FROM extension_operation_schedules WHERE workspace_id=$1 ORDER BY created_at DESC LIMIT 500"))
            .bind(self.extension_workspace()).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn update_extension_operation_schedule(
        &self,
        id: Uuid,
        enabled: bool,
        interval_seconds: i32,
    ) -> Result<Option<ExtensionOperationSchedule>, RepositoryError> {
        if !(60..=2_592_000).contains(&interval_seconds) {
            return Err(invalid(
                "schedule interval must be between one minute and 30 days",
            ));
        }
        sqlx::query_as(&format!("UPDATE extension_operation_schedules SET enabled=$3,interval_seconds=$4,next_at=CASE WHEN $3 AND NOT enabled THEN clock_timestamp()+($4*interval '1 second') ELSE next_at END,paused_reason=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 RETURNING {PROJECTION}"))
            .bind(id).bind(self.extension_workspace()).bind(enabled).bind(interval_seconds)
            .fetch_optional(&self.pool).await.map_err(Into::into)
    }

    /// Missed ticks are skipped; only the latest due occurrence is considered.
    /// Overlapping occurrences are skipped (not queued for later). Disabled or
    /// upgraded releases are paused and checked again on the next interval.
    pub async fn produce_due_extension_operation_schedules(
        &self,
    ) -> Result<usize, RepositoryError> {
        let mut count = 0;
        for _ in 0..MAX_DUE_PER_POLL {
            let mut tx = self.pool.begin().await?;
            type DueSchedule = (
                Uuid,
                String,
                Uuid,
                String,
                Value,
                Value,
                Value,
                Value,
                i32,
                DateTime<Utc>,
            );
            let row: Option<DueSchedule> = sqlx::query_as(
                "SELECT id,extension_id,installed_release_id,operation_id,input,configuration_snapshot,source_reference,destination_reference,interval_seconds,next_at FROM extension_operation_schedules WHERE workspace_id=$1 AND enabled AND next_at<=clock_timestamp() ORDER BY next_at,id LIMIT 1 FOR NO KEY UPDATE SKIP LOCKED"
            ).bind(self.extension_workspace()).fetch_optional(&mut *tx).await?;
            let Some((
                id,
                extension_id,
                release_id,
                operation_id,
                input,
                configuration,
                source,
                destination,
                interval,
                due,
            )) = row
            else {
                break;
            };
            let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_runs WHERE workspace_id=$1 AND schedule_id=$2 AND status IN ('pending','leased'))")
                .bind(self.extension_workspace()).bind(id).fetch_one(&mut *tx).await?;
            let result = if active {
                Ok(None)
            } else if self
                .runtime_extension_installation(&extension_id, release_id)
                .await?
                .is_none()
            {
                Err(invalid("release is not currently authorized"))
            } else {
                self.start_extension_operation(StartExtensionOperation {
                    extension_id,
                    expected_release_id: release_id,
                    operation_id,
                    input,
                    source_reference: source,
                    destination_reference: destination,
                    idempotency_key: format!("schedule:{id}:{}", due.timestamp_micros()),
                    schedule_id: Some(id),
                    configuration_snapshot: Some(configuration),
                })
                .await
                .map(Some)
            };
            let reason = match result {
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(schedule = %id, %error, "extension operation occurrence paused");
                    Some("release_or_source_unavailable")
                }
            };
            // Anchor the next tick to the database clock. No backlog burst after
            // downtime or a long-running overlapping occurrence.
            let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
                .fetch_one(&mut *tx)
                .await?;
            let next = now + Duration::seconds(i64::from(interval));
            sqlx::query("UPDATE extension_operation_schedules SET next_at=$3,paused_reason=$4,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2")
                .bind(id).bind(self.extension_workspace()).bind(next).bind(reason).execute(&mut *tx).await?;
            tx.commit().await?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn schedule_metadata_rejects_credentials_and_urls() {
        assert!(
            safe_snapshot(&json!({"profile":{"token":"oops"}}), &json!({}), &json!({})).is_err()
        );
        assert!(
            safe_snapshot(
                &json!({}),
                &json!({"url":"https://example.org"}),
                &json!({})
            )
            .is_err()
        );
        assert!(safe_snapshot(&json!({}), &json!({}), &json!({})).is_ok());
    }
}
