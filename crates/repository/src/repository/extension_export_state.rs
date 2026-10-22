//! Host-managed export state of extensions (#424): per-handler outcomes of
//! event deliveries and the record -> destination mapping store with its
//! per-record export status.
//!
//! The mapping store is keyed by installation (extension ID), connection
//! (`None` is the extension's default destination) and record. Exporters
//! guard against reordered or duplicated work by comparing record content
//! revisions: [`AttricatRepository::export_mapping_begin`] refuses a revision
//! that is not newer than the one already pushed, and leases the row so an
//! event handler and a connector job never push the same record at once.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// Whose lease a mapping commit honours.
enum Lease<'a> {
    /// A push's own result: refused while another worker holds the lease.
    Token(Option<Uuid>),
    /// An invocation's rejection of the record. It settles the revision the
    /// invocation leased and ends that lease; another holder's live lease
    /// leaves the record to that holder's push.
    Rejection(Option<&'a str>),
}

/// The per-record export statuses, in the order the UI lists them.
pub const EXPORT_STATUSES: &[&str] = &[
    "pending",
    "retrying",
    "uncertain",
    "rejected",
    "synced",
    "withdrawn",
];
/// The most mapping rows one page returns.
pub const MAX_EXPORT_MAPPING_PAGE: u32 = 100;
/// The most records one retry request may select.
pub const MAX_EXPORT_RETRY_RECORDS: i64 = 1_000;
/// Records a retry was requested for; delivered to the target extension only.
pub const EXPORT_RETRY_REQUESTED_V1: &str = crate::domain_events::EXPORT_RETRY_REQUESTED_V1;
/// The longest a mapping lease may be held before another worker may take it.
pub const MAX_EXPORT_LEASE: Duration = Duration::minutes(15);
const DEFAULT_EXPORT_LEASE: Duration = Duration::minutes(2);
const MAX_EXTERNAL_ID_BYTES: usize = 512;
const MAX_EXTERNAL_IDS_BYTES: usize = 8 * 1024;
const MAX_CONTENT_HASH_BYTES: usize = 128;
const MAX_EXPORT_ERROR_BYTES: usize = 8 * 1024;
/// How long rejected and exhausted handler outcomes stay visible.
const HANDLER_OUTCOME_RETENTION: Duration = Duration::days(30);

fn invalid(message: impl Into<String>) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

/// A connection key: the extension-local name of a destination account.
pub fn valid_connection_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn check_connection(connection: Option<&str>) -> Result<(), RepositoryError> {
    match connection {
        Some(key) if !valid_connection_key(key) => Err(invalid("invalid connection key")),
        _ => Ok(()),
    }
}

/// The stored outcome of one handler for one event.
#[derive(Clone, Debug, FromRow)]
pub struct HandlerAttempt {
    pub status: String,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
}

/// An outcome to record for one handler and event.
#[derive(Clone, Debug)]
pub struct HandlerAttemptOutcome<'a> {
    pub event_id: Uuid,
    pub extension_id: &'a str,
    pub handler_id: &'a str,
    pub installed_release_id: Uuid,
    pub status: &'a str,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub error_code: Option<&'a str>,
    pub error_message: Option<&'a str>,
    pub record_id: Option<Uuid>,
}

/// A failed (rejected or exhausted) handler delivery, for operators.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct HandlerFailure {
    pub event_id: Uuid,
    pub event_type: String,
    pub handler_id: String,
    pub status: String,
    pub attempts: i32,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub record_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// One record's mapping to a destination.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExportMapping {
    pub extension_id: String,
    pub connection: Option<String>,
    pub record_id: Uuid,
    pub external_id: Option<String>,
    pub external_ids: Value,
    pub content_hash: Option<String>,
    pub record_revision: i64,
    pub status: String,
    pub error: Option<Value>,
    pub attempts: i32,
    pub retry_requested_at: Option<DateTime<Utc>>,
    pub synced_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub lease_token: Option<Uuid>,
    #[serde(skip)]
    pub lease_until: Option<DateTime<Utc>>,
    /// The invocation holding the lease (a run, or one handler's delivery).
    #[serde(skip)]
    pub lease_holder: Option<String>,
    /// The record revision the lease holder is pushing.
    #[serde(skip)]
    pub leased_revision: Option<i64>,
}

const MAPPING_FIELDS: &str = "extension_id,connection,record_id,external_id,external_ids,content_hash,record_revision,status,error,attempts,retry_requested_at,synced_at,updated_at,lease_token,lease_until,lease_holder,leased_revision";

/// How a mapping is selected.
#[derive(Clone, Copy, Debug)]
pub enum MappingKey<'a> {
    Record(Uuid),
    ExternalId(&'a str),
}

/// What [`AttricatRepository::export_mapping_begin`] decided.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExportBegin {
    /// Push this revision; commit with the lease token.
    Proceed {
        lease_token: Uuid,
        mapping: ExportMapping,
    },
    /// The destination already has this revision or a newer one.
    Stale { mapping: ExportMapping },
    /// Another worker holds the record's lease until `lease_until`.
    Busy { lease_until: DateTime<Utc> },
}

/// The result of a push to record with [`AttricatRepository::export_mapping_commit`].
#[derive(Clone, Debug, Default)]
pub struct ExportMappingUpdate {
    pub status: String,
    pub record_revision: Option<i64>,
    pub external_id: Option<Option<String>>,
    pub external_ids: Option<Value>,
    pub content_hash: Option<Option<String>>,
    pub error: Option<Value>,
}

/// Counts of one connection's statuses.
/// The kind and display name of a connection an export status names.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExportConnectionLabel {
    pub extension_id: String,
    pub connection: String,
    pub kind: String,
    pub display_name: String,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExportStatusCount {
    pub extension_id: String,
    pub connection: Option<String>,
    pub status: String,
    pub count: i64,
}

/// Which mappings a retry selects.
#[derive(Clone, Debug)]
pub struct ExportRetrySelection {
    pub connection: Option<String>,
    pub record_ids: Option<Vec<Uuid>>,
    pub status: Option<String>,
}

fn bounded_json(value: &Value, max: usize, label: &str) -> Result<(), RepositoryError> {
    if serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > max) {
        return Err(invalid(format!("{label} exceeds {max} bytes")));
    }
    Ok(())
}

impl AttricatRepository {
    // ---- handler outcomes -------------------------------------------------

    /// Every stored handler outcome of an event, by extension and handler,
    /// read once per delivery rather than once per handler.
    pub async fn extension_handler_attempts(
        &self,
        event_id: Uuid,
    ) -> Result<std::collections::HashMap<(String, String), HandlerAttempt>, RepositoryError> {
        #[derive(FromRow)]
        struct StoredAttempt {
            extension_id: String,
            handler_id: String,
            #[sqlx(flatten)]
            attempt: HandlerAttempt,
        }
        let rows: Vec<StoredAttempt> = sqlx::query_as("SELECT extension_id,handler_id,status,attempts,next_attempt_at FROM extension_handler_attempts WHERE workspace_id=$1 AND event_id=$2")
            .bind(self.extension_workspace()).bind(event_id)
            .fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| ((row.extension_id, row.handler_id), row.attempt))
            .collect())
    }

    pub async fn record_extension_handler_attempt(
        &self,
        outcome: HandlerAttemptOutcome<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO extension_handler_attempts(workspace_id,event_id,extension_id,handler_id,installed_release_id,status,attempts,next_attempt_at,error_code,error_message,record_id,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,clock_timestamp()) ON CONFLICT(workspace_id,event_id,extension_id,handler_id) DO UPDATE SET installed_release_id=$5,status=$6,attempts=$7,next_attempt_at=$8,error_code=$9,error_message=$10,record_id=$11,updated_at=clock_timestamp()")
            .bind(self.extension_workspace()).bind(outcome.event_id).bind(outcome.extension_id)
            .bind(outcome.handler_id).bind(outcome.installed_release_id).bind(outcome.status)
            .bind(outcome.attempts).bind(outcome.next_attempt_at).bind(outcome.error_code)
            .bind(outcome.error_message).bind(outcome.record_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Forgets the transient outcomes of a delivery that has completed; only
    /// rejected and exhausted outcomes stay for operators.
    pub async fn clear_extension_handler_attempts(
        &self,
        event_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM extension_handler_attempts WHERE workspace_id=$1 AND event_id=$2 AND status NOT IN ('rejected','exhausted')")
            .bind(self.extension_workspace()).bind(event_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Drops rejected and exhausted outcomes past their retention. Other
    /// outcomes belong to a delivery still in progress: they go when it
    /// settles, so a late retry never runs a completed handler again.
    pub async fn sweep_extension_handler_attempts(&self) -> Result<u64, RepositoryError> {
        Ok(sqlx::query("DELETE FROM extension_handler_attempts WHERE workspace_id=$1 AND status IN ('rejected','exhausted') AND updated_at < clock_timestamp() - make_interval(secs => $2)")
            .bind(self.extension_workspace())
            .bind(HANDLER_OUTCOME_RETENTION.num_seconds() as f64)
            .execute(&self.pool).await?.rows_affected())
    }

    /// Recent rejected and exhausted deliveries of an extension's handlers.
    pub async fn extension_handler_failures(
        &self,
        extension_id: &str,
        limit: i64,
    ) -> Result<Vec<HandlerFailure>, RepositoryError> {
        Ok(sqlx::query_as("SELECT a.event_id,e.event_type,a.handler_id,a.status,a.attempts,a.error_code,a.error_message,a.record_id,a.updated_at FROM extension_handler_attempts a JOIN domain_events e ON e.id=a.event_id WHERE a.workspace_id=$1 AND a.extension_id=$2 AND a.status IN ('rejected','exhausted') ORDER BY a.updated_at DESC LIMIT $3")
            .bind(self.extension_workspace()).bind(extension_id).bind(limit.clamp(1, 200))
            .fetch_all(&self.pool).await?)
    }

    /// Whether a later event of `event_types` exists for the same record,
    /// which supersedes a debounced delivery.
    pub async fn newer_record_event_exists(
        &self,
        record_id: Uuid,
        after_sequence: i64,
        event_types: &[String],
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM domain_events WHERE workspace_id=$1 AND aggregate_kind='record' AND aggregate_id=$2 AND sequence>$3 AND event_type=ANY($4))")
            .bind(self.extension_workspace()).bind(record_id).bind(after_sequence).bind(event_types)
            .fetch_one(&self.pool).await?)
    }

    // ---- mapping store ----------------------------------------------------

    pub async fn export_mapping_get(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        key: MappingKey<'_>,
    ) -> Result<Option<ExportMapping>, RepositoryError> {
        check_connection(connection)?;
        let ws = self.extension_workspace();
        Ok(match key {
            MappingKey::Record(record_id) => sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4"))
                .bind(ws).bind(extension_id).bind(connection).bind(record_id)
                .fetch_optional(&self.pool).await?,
            MappingKey::ExternalId(external_id) => {
                if external_id.is_empty() || external_id.len() > MAX_EXTERNAL_ID_BYTES {
                    return Err(invalid("invalid external ID"));
                }
                // External IDs are unique per destination in practice; the
                // oldest mapping wins if an exporter reused one.
                sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND external_id=$4 ORDER BY created_at,id LIMIT 1"))
                    .bind(ws).bind(extension_id).bind(connection).bind(external_id)
                    .fetch_optional(&self.pool).await?
            }
        })
    }

    /// A page of one connection's mappings in record ID order, optionally of
    /// one status. `after` is the last record ID of the previous page.
    pub async fn export_mapping_list(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        status: Option<&str>,
        after: Option<Uuid>,
        limit: u32,
    ) -> Result<(Vec<ExportMapping>, Option<Uuid>), RepositoryError> {
        check_connection(connection)?;
        if limit == 0 || limit > MAX_EXPORT_MAPPING_PAGE {
            return Err(invalid(format!(
                "mapping page limit must be 1-{MAX_EXPORT_MAPPING_PAGE}"
            )));
        }
        if status.is_some_and(|status| !EXPORT_STATUSES.contains(&status)) {
            return Err(invalid("unknown export status"));
        }
        let mut rows: Vec<ExportMapping> = sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND ($4::text IS NULL OR status=$4) AND ($5::uuid IS NULL OR record_id>$5) ORDER BY record_id LIMIT $6"))
            .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(status)
            .bind(after).bind(i64::from(limit) + 1)
            .fetch_all(&self.pool).await?;
        let next = if rows.len() > limit as usize {
            rows.truncate(limit as usize);
            rows.last().map(|row| row.record_id)
        } else {
            None
        };
        Ok((rows, next))
    }

    async fn locked_mapping(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
    ) -> Result<ExportMapping, RepositoryError> {
        let ws = self.extension_workspace();
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM records WHERE workspace_id=$1 AND id=$2)",
        )
        .bind(ws)
        .bind(record_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !exists {
            return Err(RepositoryError::NotFound("record"));
        }
        if let Some(connection) = connection {
            let known: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3)")
                .bind(ws).bind(extension_id).bind(connection)
                .fetch_one(&mut **transaction).await?;
            if !known {
                return Err(RepositoryError::NotFound("extension connection"));
            }
        }
        sqlx::query("INSERT INTO extension_export_mappings(id,workspace_id,extension_id,connection,record_id,status) VALUES($1,$2,$3,$4,$5,'pending') ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(ws).bind(extension_id).bind(connection).bind(record_id)
            .execute(&mut **transaction).await?;
        Ok(sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4 FOR UPDATE"))
            .bind(ws).bind(extension_id).bind(connection).bind(record_id)
            .fetch_one(&mut **transaction).await?)
    }

    /// Decides whether `record_revision` of a record should be pushed and, if
    /// so, leases the record for this destination. A revision that is not
    /// newer than one already pushed (synced, withdrawn or rejected) is stale
    /// unless `force` is set or an operator requested a retry. `holder`
    /// names the invocation taking the lease, so its own rejection of the
    /// record settles at the revision it was pushing.
    #[allow(clippy::too_many_arguments)]
    pub async fn export_mapping_begin(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        record_revision: i64,
        lease: Option<Duration>,
        force: bool,
        holder: Option<&str>,
    ) -> Result<ExportBegin, RepositoryError> {
        check_connection(connection)?;
        if record_revision < 0 {
            return Err(invalid("record revision must not be negative"));
        }
        let lease = lease.unwrap_or(DEFAULT_EXPORT_LEASE);
        if lease <= Duration::zero() || lease > MAX_EXPORT_LEASE {
            return Err(invalid("lease must be 1 ms to 15 minutes"));
        }
        let mut transaction = self.pool.begin().await?;
        let mapping = self
            .locked_mapping(&mut transaction, extension_id, connection, record_id)
            .await?;
        let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *transaction)
            .await?;
        // A live lease of another invocation keeps the record busy. The same
        // invocation (a retried delivery or batch) takes its own lease back.
        if let (Some(_), Some(until)) = (mapping.lease_token, mapping.lease_until)
            && until > now
            && (holder.is_none() || mapping.lease_holder.as_deref() != holder)
        {
            transaction.commit().await?;
            return Ok(ExportBegin::Busy { lease_until: until });
        }
        let settled = matches!(mapping.status.as_str(), "synced" | "withdrawn" | "rejected");
        if !force
            && settled
            && mapping.retry_requested_at.is_none()
            && mapping.record_revision >= record_revision
        {
            transaction.commit().await?;
            return Ok(ExportBegin::Stale { mapping });
        }
        let token = Uuid::new_v4();
        let mapping: ExportMapping = sqlx::query_as(&format!("UPDATE extension_export_mappings SET lease_token=$5,lease_until=$6,leased_at=clock_timestamp(),lease_holder=$7,leased_revision=$8,updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4 RETURNING {MAPPING_FIELDS}"))
            .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(record_id)
            .bind(token).bind(now + lease).bind(holder).bind(record_revision)
            .fetch_one(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(ExportBegin::Proceed {
            lease_token: token,
            mapping,
        })
    }

    /// Records the result of a push and releases its lease. Without a lease
    /// token the write fails while another worker holds the record. A result
    /// for an older revision than the mapping's is late and refused, so it
    /// can neither replace a newer outcome nor lower the revision.
    pub async fn export_mapping_commit(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        lease_token: Option<Uuid>,
        update: ExportMappingUpdate,
    ) -> Result<ExportMapping, RepositoryError> {
        self.commit_export_mapping(
            extension_id,
            connection,
            record_id,
            Lease::Token(lease_token),
            update,
        )
        .await
    }

    async fn commit_export_mapping(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        lease: Lease<'_>,
        update: ExportMappingUpdate,
    ) -> Result<ExportMapping, RepositoryError> {
        check_connection(connection)?;
        if !EXPORT_STATUSES.contains(&update.status.as_str()) {
            return Err(invalid("unknown export status"));
        }
        if let Some(Some(external_id)) = &update.external_id
            && (external_id.is_empty() || external_id.len() > MAX_EXTERNAL_ID_BYTES)
        {
            return Err(invalid("external ID must be 1-512 bytes"));
        }
        if let Some(external_ids) = &update.external_ids {
            if !external_ids.is_object() {
                return Err(invalid("external IDs must be an object"));
            }
            bounded_json(external_ids, MAX_EXTERNAL_IDS_BYTES, "external IDs")?;
        }
        if let Some(Some(hash)) = &update.content_hash
            && (hash.is_empty() || hash.len() > MAX_CONTENT_HASH_BYTES)
        {
            return Err(invalid("content hash must be 1-128 bytes"));
        }
        if let Some(error) = &update.error {
            if !error.is_object() {
                return Err(invalid("export error must be an object"));
            }
            bounded_json(error, MAX_EXPORT_ERROR_BYTES, "export error")?;
        }
        if update.record_revision.is_some_and(|revision| revision < 0) {
            return Err(invalid("record revision must not be negative"));
        }
        let mut transaction = self.pool.begin().await?;
        let mapping = self
            .locked_mapping(&mut transaction, extension_id, connection, record_id)
            .await?;
        let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *transaction)
            .await?;
        let leased =
            mapping.lease_token.is_some() && mapping.lease_until.is_some_and(|until| until > now);
        let (holds_lease, revision) = match lease {
            Lease::Token(lease_token) => {
                let leased_by_other = leased && mapping.lease_token != lease_token;
                if leased_by_other || (lease_token.is_some() && mapping.lease_token != lease_token)
                {
                    return Err(RepositoryError::ExportMappingConflict(
                        "export mapping is leased by another worker".into(),
                    ));
                }
                // A leased push without a revision is for the revision it
                // leased.
                let revision = update.record_revision.or(if lease_token.is_some() {
                    mapping.leased_revision
                } else {
                    None
                });
                (
                    lease_token.is_some(),
                    revision.unwrap_or(mapping.record_revision),
                )
            }
            Lease::Rejection(holder) => {
                let own = leased && holder.is_some() && mapping.lease_holder.as_deref() == holder;
                // Another worker's push under way settles the record itself.
                if leased && !own {
                    return Err(RepositoryError::ExportMappingConflict(
                        "export mapping is leased by another worker".into(),
                    ));
                }
                // The holder rejects the revision it was pushing.
                let revision = if own {
                    mapping.leased_revision.or(update.record_revision)
                } else {
                    update.record_revision
                };
                (own, revision.unwrap_or(mapping.record_revision))
            }
        };
        if mapping.record_revision > revision {
            // A refused result still ends the lease of the worker that sent
            // it, so the record is not held until the lease expires.
            if holds_lease {
                sqlx::query("UPDATE extension_export_mappings SET lease_token=NULL,lease_until=NULL,leased_at=NULL,lease_holder=NULL,leased_revision=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4")
                    .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(record_id)
                    .execute(&mut *transaction).await?;
                transaction.commit().await?;
            }
            return Err(RepositoryError::ExportMappingConflict(
                "the mapping already records a newer record revision".into(),
            ));
        }
        let failed = matches!(
            update.status.as_str(),
            "rejected" | "retrying" | "uncertain"
        );
        let settled = matches!(update.status.as_str(), "synced" | "withdrawn" | "rejected");
        let mapping: ExportMapping = sqlx::query_as(&format!(
            "UPDATE extension_export_mappings SET status=$5,record_revision=$6,\
             external_id=CASE WHEN $7 THEN $8 ELSE external_id END,\
             external_ids=COALESCE($9,external_ids),\
             content_hash=CASE WHEN $10 THEN $11 ELSE content_hash END,\
             error=$12,attempts=CASE WHEN $13 THEN attempts+1 ELSE 0 END,\
             synced_at=CASE WHEN $5='synced' THEN clock_timestamp() ELSE synced_at END,\
             retry_requested_at=CASE WHEN $14 AND (leased_at IS NULL OR retry_requested_at <= leased_at) THEN NULL ELSE retry_requested_at END,\
             lease_token=NULL,lease_until=NULL,leased_at=NULL,lease_holder=NULL,leased_revision=NULL,updated_at=clock_timestamp() \
             WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4 \
             RETURNING {MAPPING_FIELDS}"
        ))
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(connection)
        .bind(record_id)
        .bind(&update.status)
        .bind(revision)
        .bind(update.external_id.is_some())
        .bind(update.external_id.clone().flatten())
        .bind(&update.external_ids)
        .bind(update.content_hash.is_some())
        .bind(update.content_hash.clone().flatten())
        .bind(if update.status == "synced" { None } else { update.error.clone() })
        .bind(failed)
        .bind(settled)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(mapping)
    }

    /// Releases a lease without recording a result, for example after a
    /// push was skipped.
    pub async fn export_mapping_release(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        lease_token: Uuid,
    ) -> Result<bool, RepositoryError> {
        check_connection(connection)?;
        Ok(sqlx::query("UPDATE extension_export_mappings SET lease_token=NULL,lease_until=NULL,leased_at=NULL,lease_holder=NULL,leased_revision=NULL,updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4 AND lease_token=$5")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(record_id).bind(lease_token)
            .execute(&self.pool).await?.rows_affected() == 1)
    }

    pub async fn export_mapping_delete(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        check_connection(connection)?;
        Ok(sqlx::query("DELETE FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND record_id=$4")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(record_id)
            .execute(&self.pool).await?.rows_affected() == 1)
    }

    /// The content revision `record_id` had at `at`: the oldest revision a
    /// run that started at `at` can have read, for a rejection that names no
    /// revision of its own.
    pub async fn record_content_revision_at(
        &self,
        record_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar(&format!(
            "SELECT {} FROM records r WHERE r.workspace_id=$1 AND r.id=$2",
            super::extension_attricat_data::content_revision_sql("r", Some("$3"))
        ))
        .bind(self.extension_workspace())
        .bind(record_id)
        .bind(at)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(0))
    }

    /// Records an invocation's rejection of a record as that record's status.
    /// `holder` is the rejecting invocation: when it holds the record's lease
    /// the rejection settles the revision it leased and ends the lease, and
    /// while another invocation holds it the rejection is left to that push.
    /// Otherwise `record_revision` is the revision the rejection concerns. A
    /// rejection of a revision older than the mapping's is outdated and kept
    /// out of the status. A rejection that cannot be recorded (an unknown or
    /// invalid connection or a deleted record) is ignored, so it never fails
    /// the delivery or run that reported it.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_export_rejection(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        record_revision: Option<i64>,
        holder: Option<&str>,
        code: &str,
        message: &str,
    ) -> Result<(), RepositoryError> {
        let result = async {
            let update = ExportMappingUpdate {
                status: "rejected".into(),
                record_revision: record_revision.filter(|revision| *revision >= 0),
                error: Some(json!({"code": code, "message": message})),
                ..Default::default()
            };
            self.commit_export_mapping(
                extension_id,
                connection,
                record_id,
                Lease::Rejection(holder),
                update,
            )
            .await
            .map(|_| ())
        }
        .await;
        match result {
            Ok(())
            | Err(
                RepositoryError::NotFound(_)
                | RepositoryError::ExportMappingConflict(_)
                | RepositoryError::InvalidExtension(_),
            ) => Ok(()),
            Err(error) => Err(error),
        }
    }

    // ---- management -------------------------------------------------------

    /// Every extension's mapping of one record.
    pub async fn export_mappings_for_record(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<ExportMapping>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND record_id=$2 ORDER BY extension_id,connection NULLS FIRST"))
            .bind(self.extension_workspace()).bind(record_id)
            .fetch_all(&self.pool).await?)
    }

    /// The connections the record's export statuses name.
    pub async fn export_connection_labels_for_record(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<ExportConnectionLabel>, RepositoryError> {
        Ok(sqlx::query_as("SELECT c.extension_id,c.connection,c.kind,c.display_name FROM extension_export_mappings m JOIN extension_connections c ON c.workspace_id=m.workspace_id AND c.extension_id=m.extension_id AND c.connection=m.connection WHERE m.workspace_id=$1 AND m.record_id=$2 ORDER BY c.extension_id,c.connection")
            .bind(self.extension_workspace()).bind(record_id)
            .fetch_all(&self.pool).await?)
    }

    /// Status counts per extension and connection, optionally of one extension.
    pub async fn export_status_counts(
        &self,
        extension_id: Option<&str>,
    ) -> Result<Vec<ExportStatusCount>, RepositoryError> {
        Ok(sqlx::query_as("SELECT extension_id,connection,status,count(*)::bigint AS count FROM extension_export_mappings WHERE workspace_id=$1 AND ($2::text IS NULL OR extension_id=$2) GROUP BY extension_id,connection,status ORDER BY extension_id,connection NULLS FIRST,status")
            .bind(self.extension_workspace()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    /// Records of `blueprint_id` with a given export status, for the
    /// Explorer filter and status resyncs. Returns `None` when more than
    /// `limit` records match, so a caller never acts on a silently cut list.
    pub async fn export_status_record_ids(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        status: &str,
        blueprint_id: Uuid,
        limit: i64,
    ) -> Result<Option<Vec<Uuid>>, RepositoryError> {
        check_connection(connection)?;
        if !EXPORT_STATUSES.contains(&status) {
            return Err(invalid("unknown export status"));
        }
        let ids: Vec<Uuid> = sqlx::query_scalar("SELECT m.record_id FROM extension_export_mappings m JOIN records r ON r.workspace_id=m.workspace_id AND r.id=m.record_id WHERE m.workspace_id=$1 AND m.extension_id=$2 AND m.connection IS NOT DISTINCT FROM $3 AND m.status=$4 AND r.blueprint_id=$5 AND r.deleted_at IS NULL ORDER BY m.record_id LIMIT $6")
            .bind(self.extension_workspace()).bind(extension_id).bind(connection).bind(status).bind(blueprint_id).bind(limit.saturating_add(1))
            .fetch_all(&self.pool).await?;
        Ok((ids.len() as i64 <= limit).then_some(ids))
    }

    /// Marks the selected mappings for retry and emits
    /// `export.retry_requested.v1` per record, in one audited transaction.
    /// Returns the selected record IDs. A status that matches more than
    /// [`MAX_EXPORT_RETRY_RECORDS`] records is refused rather than cut.
    pub async fn request_export_retry(
        &self,
        extension_id: &str,
        selection: ExportRetrySelection,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        check_connection(selection.connection.as_deref())?;
        if let Some(status) = &selection.status
            && !EXPORT_STATUSES.contains(&status.as_str())
        {
            return Err(invalid("unknown export status"));
        }
        if selection
            .record_ids
            .as_ref()
            .is_some_and(|ids| ids.is_empty() || ids.len() as i64 > MAX_EXPORT_RETRY_RECORDS)
        {
            return Err(invalid(format!(
                "a retry selects 1-{MAX_EXPORT_RETRY_RECORDS} records"
            )));
        }
        if selection.record_ids.is_none() && selection.status.is_none() {
            return Err(invalid("a retry needs record IDs or a status"));
        }
        let mut transaction = self.pool.begin().await?;
        let ids: Vec<Uuid> = sqlx::query_scalar("WITH selected AS (SELECT id FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND connection IS NOT DISTINCT FROM $3 AND ($4::uuid[] IS NULL OR record_id=ANY($4)) AND ($5::text IS NULL OR status=$5) ORDER BY record_id LIMIT $6 FOR UPDATE) UPDATE extension_export_mappings m SET status='pending',retry_requested_at=clock_timestamp(),attempts=0,updated_at=clock_timestamp() FROM selected s WHERE m.id=s.id RETURNING m.record_id")
            .bind(self.extension_workspace()).bind(extension_id).bind(&selection.connection)
            .bind(&selection.record_ids).bind(&selection.status).bind(MAX_EXPORT_RETRY_RECORDS + 1)
            .fetch_all(&mut *transaction).await?;
        if ids.len() as i64 > MAX_EXPORT_RETRY_RECORDS {
            // Dropping the transaction rolls the update back.
            return Err(invalid(format!(
                "more than {MAX_EXPORT_RETRY_RECORDS} records match; retry them by record ID in batches"
            )));
        }
        let events = ids
            .iter()
            .map(|record_id| {
                self.core_event(
                    EXPORT_RETRY_REQUESTED_V1,
                    "record",
                    *record_id,
                    json!({
                        "record_id": record_id,
                        "extension_id": extension_id,
                        "connection": selection.connection,
                    }),
                )
            })
            .collect();
        self.enqueue_events(&mut transaction, events).await?;
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(ids)
    }
}
