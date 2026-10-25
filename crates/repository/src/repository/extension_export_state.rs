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

use attricat_domain::states::ExportStatus;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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

/// The mappings an operator can still act on: of an installed extension,
/// and of the default destination or a connection that still exists.
/// Mappings outlive both, so a re-created connection keeps its IDs.
pub(crate) fn current_destination(mapping: &str) -> String {
    format!(
        "EXISTS(SELECT 1 FROM extension_installations i WHERE i.workspace_id={mapping}.workspace_id AND i.extension_id={mapping}.extension_id) \
         AND ({mapping}.connection IS NULL OR EXISTS(SELECT 1 FROM extension_connections c WHERE c.workspace_id={mapping}.workspace_id AND c.extension_id={mapping}.extension_id AND c.connection={mapping}.connection))"
    )
}

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

fn invalid(message: impl Into<String>) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

/// A connection key: the extension-local name of a destination account.
pub use attricat_validation::is_valid_connection_key as valid_connection_key;

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
    #[sqlx(try_from = "String")]
    pub status: ExportStatus,
    pub error: Option<Value>,
    pub attempts: i32,
    pub retry_requested_at: Option<DateTime<Utc>>,
    pub synced_at: Option<DateTime<Utc>>,
    /// When it last settled; leases and releases leave it alone.
    #[serde(skip)]
    pub settled_at: Option<DateTime<Utc>>,
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

/// Assignments that end a mapping's lease.
const CLEAR_LEASE: &str =
    "lease_token=NULL,lease_until=NULL,leased_at=NULL,lease_holder=NULL,leased_revision=NULL";
const MAPPING_FIELDS: &str = "extension_id,connection,record_id,external_id,external_ids,content_hash,record_revision,status,error,attempts,retry_requested_at,synced_at,settled_at,updated_at,lease_token,lease_until,lease_holder,leased_revision";

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

/// The kind and display name of a connection an export status names.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExportConnectionLabel {
    pub extension_id: String,
    pub connection: String,
    pub kind: String,
    pub display_name: String,
}

/// Counts of one connection's statuses.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExportStatusCount {
    pub extension_id: String,
    pub connection: Option<String>,
    #[sqlx(try_from = "String")]
    pub status: ExportStatus,
    pub count: i64,
}

/// A mapping locked for a change: whether its record is deleted, and
/// whether the lock created it.
struct LockedMapping {
    mapping: ExportMapping,
    deleted: bool,
    created: bool,
}

/// What [`AttricatRepository::record_export_rejection`] made of a rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionRecord {
    /// The record is now settled as rejected.
    Settled,
    /// It already was, at this revision or a newer one, or the record was
    /// deleted: nothing changed.
    Unchanged,
    /// Another worker holds the record's lease until `until`; its push
    /// settles it.
    Busy { until: DateTime<Utc> },
    /// It cannot be recorded, for example for an unknown connection.
    Refused,
}

/// Which mappings a retry selects.
#[derive(Clone, Debug)]
pub struct ExportRetrySelection {
    pub connection: Option<String>,
    pub record_ids: Option<Vec<Uuid>>,
    pub status: Option<String>,
    /// Names the request: one already recorded under it is not retried
    /// again, and its records are returned.
    pub request_key: Option<String>,
}

/// An operator's export retry, optionally resyncing the retried records.
#[derive(Clone, Debug)]
pub struct ExportRetryRequest {
    pub connection: Option<String>,
    pub record_ids: Option<Vec<Uuid>>,
    pub status: Option<String>,
    /// Also resync the retried records on every export job of the extension
    /// that targets the connection.
    pub run: bool,
    /// Makes the retry idempotent: sent again as it was, it retries nothing
    /// twice and finds the runs it started.
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportRetryOutcome {
    pub record_ids: Vec<Uuid>,
    pub run_ids: Vec<Uuid>,
    /// Jobs whose resync could not start; the retry itself still happened.
    pub job_errors: Vec<ExportRetryJobError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportRetryJobError {
    pub job_id: Uuid,
    pub message: String,
}

/// Record IDs in one order without repeats, as a selection names them.
pub fn sorted_record_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// How an idempotency key names a set of records, so a request sent again
/// maps to the same run key.
pub fn record_ids_selection(ids: &[Uuid]) -> String {
    sorted_record_ids(ids)
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// What a client is told about a job whose retry resync could not start.
fn resync_error_message(error: &RepositoryError) -> String {
    match error {
        RepositoryError::InvalidExtension(message) => message.clone(),
        RepositoryError::NotFound(what) => format!("{what} not found"),
        _ => "the job could not start a resync".to_owned(),
    }
}

/// Matches a mapping's `column` to the connection bound at `param`: equal
/// to it, or NULL for the default destination. Unlike `IS NOT DISTINCT FROM`
/// both forms can use the mapping indexes; `param` stays referenced either
/// way so the query binds the same parameters.
pub(crate) fn connection_is(column: &str, param: &str, connection: Option<&str>) -> String {
    match connection {
        Some(_) => format!("{column}={param}"),
        None => format!("{column} IS NULL AND {param}::text IS NULL"),
    }
}

impl AttricatRepository {}

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
            .bind(self.workspace_id()).bind(event_id)
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
            .bind(self.workspace_id()).bind(outcome.event_id).bind(outcome.extension_id)
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
            .bind(self.workspace_id()).bind(event_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Recent rejected and exhausted deliveries of an extension's handlers.
    pub async fn extension_handler_failures(
        &self,
        extension_id: &str,
        limit: i64,
    ) -> Result<Vec<HandlerFailure>, RepositoryError> {
        Ok(sqlx::query_as("SELECT a.event_id,e.event_type,a.handler_id,a.status,a.attempts,a.error_code,a.error_message,a.record_id,a.updated_at FROM extension_handler_attempts a JOIN domain_events e ON e.id=a.event_id WHERE a.workspace_id=$1 AND a.extension_id=$2 AND a.status IN ('rejected','exhausted') ORDER BY a.updated_at DESC LIMIT $3")
            .bind(self.workspace_id()).bind(extension_id).bind(limit.clamp(1, 200))
            .fetch_all(&self.pool).await?)
    }

    /// Whether a later event of `event_types` for `event`'s record exists
    /// that supersedes `event`'s debounced delivery to `extension_id`. A retry
    /// request concerns one connection only, so only a later retry request of
    /// the same extension and connection supersedes it, and no retry request
    /// supersedes any other event.
    pub async fn newer_record_event_exists(
        &self,
        extension_id: &str,
        event: &attricat_events::DomainEvent,
        event_types: &[String],
    ) -> Result<bool, RepositoryError> {
        let retry = event.event_type == crate::domain_events::EXPORT_RETRY_REQUESTED_V1;
        let retry_connection = event.payload.get("connection").and_then(Value::as_str);
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM domain_events WHERE workspace_id=$1 AND aggregate_kind='record' AND aggregate_id=$2 AND sequence>$3 AND event_type=ANY($4) AND CASE WHEN $7 THEN event_type=$6 AND payload->>'extension_id'=$5 AND payload->>'connection' IS NOT DISTINCT FROM $8 ELSE event_type<>$6 END)")
            .bind(self.workspace_id()).bind(event.aggregate_id).bind(event.sequence).bind(event_types)
            .bind(extension_id).bind(crate::domain_events::EXPORT_RETRY_REQUESTED_V1)
            .bind(retry).bind(retry_connection)
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
        let ws = self.workspace_id();
        Ok(match key {
            MappingKey::Record(record_id) => sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4", connection_is("connection", "$3", connection)))
                .bind(ws).bind(extension_id).bind(connection).bind(record_id)
                .fetch_optional(&self.pool).await?,
            MappingKey::ExternalId(external_id) => {
                if external_id.is_empty() || external_id.len() > MAX_EXTERNAL_ID_BYTES {
                    return Err(invalid("invalid external ID"));
                }
                // External IDs are unique per destination in practice; the
                // oldest mapping wins if an exporter reused one.
                sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND external_id=$4 ORDER BY created_at,id LIMIT 1", connection_is("connection", "$3", connection)))
                    .bind(ws).bind(extension_id).bind(connection).bind(external_id)
                    .fetch_optional(&self.pool).await?
            }
        })
    }

    /// A page of one connection's mappings in record ID order, optionally of
    /// one status. `after` is the last record ID of the previous page.
    /// `live_only` leaves out deleted records' mappings other than withdrawn
    /// ones, as the status counts do.
    pub async fn export_mapping_list(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        status: Option<&str>,
        after: Option<Uuid>,
        limit: u32,
        live_only: bool,
    ) -> Result<(Vec<ExportMapping>, Option<Uuid>), RepositoryError> {
        check_connection(connection)?;
        if limit == 0 || limit > MAX_EXPORT_MAPPING_PAGE {
            return Err(invalid(format!(
                "mapping page limit must be 1-{MAX_EXPORT_MAPPING_PAGE}"
            )));
        }
        if status.is_some_and(|status| ExportStatus::parse(status).is_none()) {
            return Err(invalid("unknown export status"));
        }
        let mut rows: Vec<ExportMapping> = sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND ($4::text IS NULL OR status=$4) AND ($5::uuid IS NULL OR record_id>$5) AND (NOT $7 OR ((status='withdrawn' OR EXISTS(SELECT 1 FROM records r WHERE r.workspace_id=extension_export_mappings.workspace_id AND r.id=extension_export_mappings.record_id AND r.deleted_at IS NULL)) AND {})) ORDER BY record_id LIMIT $6", connection_is("connection", "$3", connection), current_destination("extension_export_mappings")))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(status)
            .bind(after).bind(i64::from(limit) + 1).bind(live_only)
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
    ) -> Result<LockedMapping, RepositoryError> {
        let ws = self.workspace_id();
        // Whether the record is deleted: only its withdrawal is recorded then.
        let deleted: bool = sqlx::query_scalar(
            "SELECT deleted_at IS NOT NULL FROM records WHERE workspace_id=$1 AND id=$2",
        )
        .bind(ws)
        .bind(record_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("record"))?;
        if let Some(connection) = connection {
            let known = self
                .extension_connection_exists(&mut **transaction, extension_id, connection)
                .await?;
            if !known {
                return Err(RepositoryError::NotFound("extension connection"));
            }
        }
        let created = sqlx::query("INSERT INTO extension_export_mappings(id,workspace_id,extension_id,connection,record_id,status) VALUES($1,$2,$3,$4,$5,'pending') ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(ws).bind(extension_id).bind(connection).bind(record_id)
            .execute(&mut **transaction).await?.rows_affected() == 1;
        let mapping = sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 FOR UPDATE", connection_is("connection", "$3", connection)))
            .bind(ws).bind(extension_id).bind(connection).bind(record_id)
            .fetch_one(&mut **transaction).await?;
        Ok(LockedMapping {
            mapping,
            deleted,
            created,
        })
    }

    /// Ends `holder`'s or `token`'s live lease of a locked mapping, for a
    /// call that records nothing else.
    async fn end_own_lease(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(&format!("UPDATE extension_export_mappings SET {CLEAR_LEASE},updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4", connection_is("connection", "$3", connection)))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id)
            .execute(&mut **transaction).await?;
        Ok(())
    }

    /// Decides whether `record_revision` of a record should be pushed and, if
    /// so, leases the record for this destination. A revision older than the
    /// mapping's is always stale. The mapping's own revision is stale once
    /// settled (synced or rejected) unless `force` is set or an
    /// operator requested a retry. In a resync run started at
    /// `resync_since`, a mapping settled before then is pushed again; one the
    /// run settled itself, for example by rejecting it, stays stale so a
    /// replayed batch moves on. `holder` names the invocation taking the lease, so its own rejection of the
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
        resync_since: Option<DateTime<Utc>>,
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
        let LockedMapping {
            mapping,
            deleted,
            created,
        } = self
            .locked_mapping(&mut transaction, extension_id, connection, record_id)
            .await?;
        // A deleted record is never pushed again; its withdrawal is
        // committed without a lease. A mapping this call would have created
        // is not kept, and the caller's own earlier lease ends.
        if deleted {
            if created {
                transaction.rollback().await?;
                return Ok(ExportBegin::Stale { mapping });
            }
            let own = holder.is_some() && mapping.lease_holder.as_deref() == holder;
            if own {
                self.end_own_lease(&mut transaction, extension_id, connection, record_id)
                    .await?;
            }
            transaction.commit().await?;
            return Ok(ExportBegin::Stale { mapping });
        }
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
        // A revision older than the mapping's is stale whatever its status,
        // because its commit would be refused after the push. A withdrawal
        // does not settle its own revision for a push: a record published
        // again unchanged comes back at the revision it was withdrawn at.
        let settled = matches!(
            mapping.status,
            ExportStatus::Synced | ExportStatus::Rejected
        );
        if mapping.record_revision > record_revision
            || (!force
                && settled
                && mapping.retry_requested_at.is_none()
                && !resync_since
                    .is_some_and(|since| mapping.settled_at.is_none_or(|settled| settled < since))
                && mapping.record_revision == record_revision)
        {
            // A retried delivery or batch that finds its record stale ends
            // the lease it took before, so others are not held off.
            let own_lease = holder.is_some()
                && mapping.lease_holder.as_deref() == holder
                && mapping.lease_until.is_some_and(|until| until > now);
            let mapping = if own_lease {
                sqlx::query_as(&format!("UPDATE extension_export_mappings SET {CLEAR_LEASE},updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 RETURNING {MAPPING_FIELDS}", connection_is("connection", "$3", connection)))
                    .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id)
                    .fetch_one(&mut *transaction).await?
            } else {
                mapping
            };
            transaction.commit().await?;
            return Ok(ExportBegin::Stale { mapping });
        }
        // The same invocation taking its live lease back keeps its token, so
        // an earlier attempt still pushing can commit what it pushed, and the
        // lower leased revision, so that commit never claims a newer one.
        let (token, leased_revision) = match (mapping.lease_token, mapping.lease_until) {
            (Some(token), Some(until))
                if until > now && holder.is_some() && mapping.lease_holder.as_deref() == holder =>
            {
                (
                    token,
                    mapping
                        .leased_revision
                        .map_or(record_revision, |leased| leased.min(record_revision)),
                )
            }
            _ => (Uuid::new_v4(), record_revision),
        };
        let mapping: ExportMapping = sqlx::query_as(&format!("UPDATE extension_export_mappings SET lease_token=$5,lease_until=$6,leased_at=clock_timestamp(),lease_holder=$7,leased_revision=$8,updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 RETURNING {MAPPING_FIELDS}", connection_is("connection", "$3", connection)))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id)
            .bind(token).bind(now + lease).bind(holder).bind(leased_revision)
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
        .map(|(_, committed)| committed)
    }

    async fn commit_export_mapping(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        lease: Lease<'_>,
        update: ExportMappingUpdate,
    ) -> Result<(ExportMapping, ExportMapping), RepositoryError> {
        check_connection(connection)?;
        let status =
            ExportStatus::parse(&update.status).ok_or_else(|| invalid("unknown export status"))?;
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
        let LockedMapping {
            mapping,
            deleted,
            created,
        } = self
            .locked_mapping(&mut transaction, extension_id, connection, record_id)
            .await?;
        if deleted && status != ExportStatus::Withdrawn {
            // A refused commit still ends the caller's own lease; a mapping
            // the lock created is not kept.
            let own = match lease {
                Lease::Token(token) => token.is_some() && mapping.lease_token == token,
                Lease::Rejection(holder) => {
                    holder.is_some() && mapping.lease_holder.as_deref() == holder
                }
            };
            if own && !created {
                self.end_own_lease(&mut transaction, extension_id, connection, record_id)
                    .await?;
                transaction.commit().await?;
            }
            return Err(RepositoryError::ExportMappingConflict(
                "the record is deleted; only its withdrawal can be recorded".into(),
            ));
        }
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
                // The holder rejects the revision it was pushing, even when
                // its lease has lapsed meanwhile.
                let mine = holder.is_some() && mapping.lease_holder.as_deref() == holder;
                let revision = if own || mine {
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
                self.end_own_lease(&mut transaction, extension_id, connection, record_id)
                    .await?;
                transaction.commit().await?;
            }
            return Err(RepositoryError::ExportMappingConflict(
                "the mapping already records a newer record revision".into(),
            ));
        }
        let rejection = matches!(lease, Lease::Rejection(_));
        // A rejection the unleased mapping already records changes nothing:
        // a replayed delivery or batch neither counts another attempt nor
        // moves its settle time.
        if rejection
            && !holds_lease
            && mapping.status == ExportStatus::Rejected
            && mapping.lease_token.is_none()
            && mapping.record_revision >= revision
        {
            transaction.commit().await?;
            return Ok((mapping.clone(), mapping));
        }
        let failed = matches!(
            status,
            ExportStatus::Rejected | ExportStatus::Retrying | ExportStatus::Uncertain
        );
        let settled = matches!(
            status,
            ExportStatus::Synced | ExportStatus::Withdrawn | ExportStatus::Rejected
        );
        let committed: ExportMapping = sqlx::query_as(&format!(
            "UPDATE extension_export_mappings SET status=$5,record_revision=$6,\
             external_id=CASE WHEN $7 THEN $8 ELSE external_id END,\
             external_ids=COALESCE($9,external_ids),\
             content_hash=CASE WHEN $10 THEN $11 ELSE content_hash END,\
             error=$12,attempts=CASE WHEN $13 THEN attempts+1 ELSE 0 END,\
             synced_at=CASE WHEN $5='synced' THEN clock_timestamp() ELSE synced_at END,\
             settled_at=CASE WHEN $14 THEN clock_timestamp() ELSE settled_at END,\
             retry_requested_at=CASE WHEN $15 AND (leased_at IS NULL OR retry_requested_at <= leased_at) THEN NULL ELSE retry_requested_at END,\
             {CLEAR_LEASE},updated_at=clock_timestamp() \
             WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 \
             RETURNING {MAPPING_FIELDS}",
            connection_is("connection", "$3", connection)
        ))
        .bind(self.workspace_id())
        .bind(extension_id)
        .bind(connection)
        .bind(record_id)
        .bind(status.as_str())
        .bind(revision)
        .bind(update.external_id.is_some())
        .bind(update.external_id.clone().flatten())
        .bind(&update.external_ids)
        .bind(update.content_hash.is_some())
        .bind(update.content_hash.clone().flatten())
        .bind(if status == ExportStatus::Synced { None } else { update.error.clone() })
        .bind(failed)
        .bind(settled)
        // Only a leased result tells when its attempt began; an unleased
        // one may concern an attempt from before an operator's retry, so it
        // leaves that retry standing.
        .bind(settled && holds_lease)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        // The mapping as it was under the lock, and as committed.
        Ok((mapping, committed))
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
        Ok(sqlx::query(&format!("UPDATE extension_export_mappings SET {CLEAR_LEASE},updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 AND lease_token=$5", connection_is("connection", "$3", connection)))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id).bind(lease_token)
            .execute(&self.pool).await?.rows_affected() == 1)
    }

    /// Deletes a mapping. While another worker holds a live lease on it
    /// (one `lease_token` does not name), it is refused: that push's commit
    /// would otherwise lose the destination ID it creates.
    pub async fn export_mapping_delete(
        &self,
        extension_id: &str,
        connection: Option<&str>,
        record_id: Uuid,
        lease_token: Option<Uuid>,
    ) -> Result<bool, RepositoryError> {
        check_connection(connection)?;
        let deleted = sqlx::query(&format!("DELETE FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 AND (lease_until IS NULL OR lease_until<=clock_timestamp() OR lease_token=$5)", connection_is("connection", "$3", connection)))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id).bind(lease_token)
            .execute(&self.pool).await?.rows_affected() == 1;
        if !deleted {
            let leased: bool = sqlx::query_scalar(&format!("SELECT EXISTS(SELECT 1 FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4)", connection_is("connection", "$3", connection)))
                .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id)
                .fetch_one(&self.pool).await?;
            if leased {
                return Err(RepositoryError::ExportMappingConflict(
                    "export mapping is leased by another worker".into(),
                ));
            }
        }
        Ok(deleted)
    }

    /// The content revision `record_id` had at `at`: the oldest revision a
    /// run that started at `at` can have read, for a rejection that names no
    /// revision of its own. `channel` is the channel the run reads.
    pub async fn record_content_revision_at(
        &self,
        record_id: Uuid,
        at: DateTime<Utc>,
        channel: Option<Uuid>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar(&format!(
            "SELECT {} FROM records r WHERE r.workspace_id=$1 AND r.id=$2",
            super::extension_attricat_data::content_revision_sql("r", Some("$3"), Some("$4"))
        ))
        .bind(self.workspace_id())
        .bind(record_id)
        .bind(at)
        .bind(channel)
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
    /// invalid connection or a deleted record) never fails the delivery or
    /// run that reported it; the result says what became of it.
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
    ) -> Result<RejectionRecord, RepositoryError> {
        let result = async {
            let update = ExportMappingUpdate {
                status: ExportStatus::Rejected.as_str().into(),
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
            .map(|(before, _)| before)
        }
        .await;
        match result {
            // A rejection the mapping already recorded unleased is a repeat;
            // `before` is the mapping as the commit found it under its lock.
            Ok(before) => Ok(
                if before.status == ExportStatus::Rejected
                    && before.lease_token.is_none()
                    && record_revision.is_none_or(|revision| before.record_revision >= revision)
                {
                    RejectionRecord::Unchanged
                } else {
                    RejectionRecord::Settled
                },
            ),
            Err(RepositoryError::NotFound("record")) => Ok(RejectionRecord::Unchanged),
            // A refused rejection is busy when another worker's lease is
            // live now, by the database's clock, and outdated otherwise.
            Err(RepositoryError::ExportMappingConflict(_)) => {
                let until: Option<DateTime<Utc>> = sqlx::query_scalar(&format!("SELECT lease_until FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND record_id=$4 AND lease_until>clock_timestamp() AND lease_holder IS DISTINCT FROM $5", connection_is("connection", "$3", connection)))
                    .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(record_id).bind(holder)
                    .fetch_optional(&self.pool).await?;
                Ok(match until {
                    Some(until) => RejectionRecord::Busy { until },
                    None => RejectionRecord::Unchanged,
                })
            }
            Err(RepositoryError::NotFound(_) | RepositoryError::InvalidExtension(_)) => {
                Ok(RejectionRecord::Refused)
            }
            Err(error) => Err(error),
        }
    }

    /// Ends the leases `holder` took without changing their mappings, for
    /// an invocation whose work a newer one takes over.
    pub async fn release_export_leases(
        &self,
        extension_id: &str,
        holder: &str,
    ) -> Result<u64, RepositoryError> {
        Ok(sqlx::query(&format!("UPDATE extension_export_mappings SET {CLEAR_LEASE},updated_at=clock_timestamp() WHERE workspace_id=$1 AND extension_id=$2 AND lease_holder=$3"))
            .bind(self.workspace_id()).bind(extension_id).bind(holder)
            .execute(&self.pool).await?.rows_affected())
    }

    /// Marks the mappings `holder` leased as `retrying` with an error and
    /// ends those leases, for an invocation that gave up without a result.
    /// An expired lease counts too: the holder's name is the invocation's,
    /// and a newer lease by anyone else carries another. Their revision is kept, so a later push of the leased
    /// revision proceeds.
    pub async fn fail_export_leases(
        &self,
        extension_id: &str,
        holder: &str,
        code: &str,
        message: &str,
    ) -> Result<u64, RepositoryError> {
        // A deleted record's lease only ends: its mapping accepts nothing
        // but a withdrawal, so a retrying status would never clear.
        Ok(sqlx::query(&format!("UPDATE extension_export_mappings m SET status=CASE WHEN r.deleted_at IS NULL THEN 'retrying' ELSE m.status END,error=CASE WHEN r.deleted_at IS NULL THEN jsonb_build_object('code',$4::text,'message',$5::text) ELSE m.error END,attempts=CASE WHEN r.deleted_at IS NULL THEN m.attempts+1 ELSE m.attempts END,{CLEAR_LEASE},updated_at=clock_timestamp() FROM records r WHERE r.workspace_id=m.workspace_id AND r.id=m.record_id AND m.workspace_id=$1 AND m.extension_id=$2 AND m.lease_holder=$3"))
            .bind(self.workspace_id()).bind(extension_id).bind(holder).bind(code)
            .bind(message.chars().take(1024).collect::<String>())
            .execute(&self.pool).await?.rows_affected())
    }

    // ---- management -------------------------------------------------------

    /// Every installed extension's mapping of one record, for destinations
    /// that still exist.
    pub async fn export_mappings_for_record(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<ExportMapping>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {MAPPING_FIELDS} FROM extension_export_mappings WHERE workspace_id=$1 AND record_id=$2 AND {} ORDER BY extension_id,connection NULLS FIRST", current_destination("extension_export_mappings")))
            .bind(self.workspace_id()).bind(record_id)
            .fetch_all(&self.pool).await?)
    }

    /// The connections the record's export statuses name.
    pub async fn export_connection_labels_for_record(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<ExportConnectionLabel>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT c.extension_id,c.connection,c.kind,c.display_name FROM extension_export_mappings m JOIN extension_connections c ON c.workspace_id=m.workspace_id AND c.extension_id=m.extension_id AND c.connection=m.connection WHERE m.workspace_id=$1 AND m.record_id=$2 AND {} ORDER BY c.extension_id,c.connection", current_destination("m")))
            .bind(self.workspace_id()).bind(record_id)
            .fetch_all(&self.pool).await?)
    }

    /// Status counts per extension and connection, optionally of one extension.
    /// Deleted records count only as withdrawn, so every other count is of
    /// records an operator can still retry.
    pub async fn export_status_counts(
        &self,
        extension_id: Option<&str>,
    ) -> Result<Vec<ExportStatusCount>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT extension_id,connection,status,count(*)::bigint AS count FROM extension_export_mappings WHERE workspace_id=$1 AND ($2::text IS NULL OR extension_id=$2) AND {} AND (status='withdrawn' OR EXISTS(SELECT 1 FROM records r WHERE r.workspace_id=extension_export_mappings.workspace_id AND r.id=extension_export_mappings.record_id AND r.deleted_at IS NULL)) GROUP BY extension_id,connection,status ORDER BY extension_id,connection NULLS FIRST,status", current_destination("extension_export_mappings")))
            .bind(self.workspace_id()).bind(extension_id)
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
        if ExportStatus::parse(status).is_none() {
            return Err(invalid("unknown export status"));
        }
        let ids: Vec<Uuid> = sqlx::query_scalar(&format!("SELECT m.record_id FROM extension_export_mappings m JOIN records r ON r.workspace_id=m.workspace_id AND r.id=m.record_id WHERE m.workspace_id=$1 AND m.extension_id=$2 AND {} AND m.status=$4 AND r.blueprint_id=$5 AND r.deleted_at IS NULL AND {} ORDER BY m.record_id LIMIT $6", connection_is("m.connection", "$3", connection), current_destination("m")))
            .bind(self.workspace_id()).bind(extension_id).bind(connection).bind(status).bind(blueprint_id).bind(limit.saturating_add(1))
            .fetch_all(&self.pool).await?;
        Ok((ids.len() as i64 <= limit).then_some(ids))
    }

    /// Retries the selected mappings of `extension_id` and, when asked,
    /// resyncs the retried records on each matching export job.
    pub async fn retry_exports(
        &self,
        extension_id: &str,
        request: ExportRetryRequest,
    ) -> Result<ExportRetryOutcome, RepositoryError> {
        // With a key, the request as sent (not the records a status resolves
        // to now) names the retry. A request sent again finds the records the
        // first one retried instead of retrying anything twice, and resyncs
        // them under the same run keys, which start each job's run only once.
        let selection = request.idempotency_key.as_ref().map(|_| {
            format!(
                "retry:{}:{}:{}",
                request.connection.as_deref().unwrap_or(""),
                request.status.as_deref().unwrap_or(""),
                record_ids_selection(request.record_ids.as_deref().unwrap_or_default())
            )
        });
        let request_key = request
            .idempotency_key
            .as_ref()
            .zip(selection.as_ref())
            .map(|(key, selection)| {
                format!(
                    "{:x}",
                    Sha256::digest(format!("{extension_id}\0{key}\0{selection}").as_bytes())
                )
            });
        let record_ids = self
            .request_export_retry(
                extension_id,
                ExportRetrySelection {
                    connection: request.connection.clone(),
                    record_ids: request.record_ids,
                    status: request.status,
                    request_key,
                },
            )
            .await?;
        // The retry is recorded above. Each job resyncs only its blueprint's
        // records, and a job that cannot run is reported rather than failing
        // a retry that already happened.
        let mut run_ids = Vec::new();
        let mut job_errors = Vec::new();
        if request.run && !record_ids.is_empty() {
            let key = request
                .idempotency_key
                .unwrap_or_else(|| format!("retry-{}", Uuid::new_v4().simple()));
            for job in self
                .extension_export_connector_jobs(extension_id)
                .await?
                .into_iter()
                .filter(|job| job.connection == request.connection)
            {
                let ids = self
                    .records_of_blueprint(job.blueprint_id, &record_ids)
                    .await?;
                if ids.is_empty() {
                    continue;
                }
                match self
                    .resync_blueprint_connector_job(job.id, &key, selection.as_deref(), Some(ids))
                    .await
                {
                    Ok(runs) => run_ids.extend(runs),
                    Err(error) => {
                        tracing::warn!(job = %job.id, %error, "export retry resync could not start");
                        job_errors.push(ExportRetryJobError {
                            job_id: job.id,
                            message: resync_error_message(&error),
                        });
                    }
                }
            }
        }
        Ok(ExportRetryOutcome {
            record_ids,
            run_ids,
            job_errors,
        })
    }

    /// Marks the selected mappings for retry and emits
    /// `export.retry_requested.v1` per record, in one audited transaction.
    /// Records selected by ID without a status that were never exported get
    /// a pending mapping first. Deleted records are never selected, since no
    /// run would push them. Returns the selected record IDs. A status
    /// that matches more than
    /// [`MAX_EXPORT_RETRY_RECORDS`] records is refused rather than cut.
    pub async fn request_export_retry(
        &self,
        extension_id: &str,
        selection: ExportRetrySelection,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        check_connection(selection.connection.as_deref())?;
        if let Some(status) = &selection.status
            && ExportStatus::parse(status).is_none()
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
        // A withdrawal is closed out: publishing the record again exports it.
        if selection.status.as_deref() == Some("withdrawn") {
            return Err(invalid(
                "withdrawn records are not retried; publish them again",
            ));
        }
        let mut transaction = self.pool.begin().await?;
        // Only an installed extension granted export mappings has mappings
        // to retry; any other would get rows nothing ever settles.
        let exporter: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_installations i JOIN extension_grants g ON g.installation_id=i.id AND g.grant_kind='capability' AND g.grant_id='exports.mappings' WHERE i.workspace_id=$1 AND i.extension_id=$2)")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_one(&mut *transaction).await?;
        if !exporter {
            return Err(RepositoryError::NotFound("export extension"));
        }
        // A disabled or quarantined extension would receive none of the
        // retry requests, leaving the records pending with no error.
        let enabled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_installations WHERE workspace_id=$1 AND extension_id=$2 AND state='enabled')")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_one(&mut *transaction).await?;
        if !enabled {
            return Err(invalid("enable the extension before retrying its exports"));
        }
        // A keyed request claims its key in this transaction. One sent
        // before, even concurrently, holds it: the records it retried are
        // returned and nothing is retried twice.
        if let Some(key) = &selection.request_key {
            let claimed = sqlx::query("INSERT INTO extension_export_retry_requests(workspace_id,extension_id,request_key) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
                .bind(self.workspace_id()).bind(extension_id).bind(key)
                .execute(&mut *transaction).await?.rows_affected() == 1;
            if !claimed {
                let recorded: Vec<Uuid> = sqlx::query_scalar("SELECT record_ids FROM extension_export_retry_requests WHERE workspace_id=$1 AND extension_id=$2 AND request_key=$3")
                    .bind(self.workspace_id()).bind(extension_id).bind(key)
                    .fetch_one(&mut *transaction).await?;
                return Ok(recorded);
            }
        }
        // Records chosen by ID may never have been exported; give them a
        // pending mapping so the retry selects and pushes them too.
        if let (Some(record_ids), None) = (&selection.record_ids, &selection.status) {
            if let Some(connection) = &selection.connection {
                let known = self
                    .extension_connection_exists(&mut *transaction, extension_id, connection)
                    .await?;
                if !known {
                    return Err(RepositoryError::NotFound("extension connection"));
                }
            }
            // Only records something will push get one: those of a
            // blueprint the extension exports to this connection by a job,
            // or any record when the extension handles retry requests itself.
            let release = self
                .installed_extension(extension_id)
                .await?
                .installed_release_id;
            let handles_retries = self
                .installed_release_manifest(&mut transaction, release)
                .await?
                .server
                .is_some_and(|server| {
                    server.event_handlers.iter().any(|handler| {
                        handler
                            .event_types
                            .iter()
                            .any(|event_type| event_type == EXPORT_RETRY_REQUESTED_V1)
                    })
                });
            sqlx::query("INSERT INTO extension_export_mappings(id,workspace_id,extension_id,connection,record_id,status) SELECT gen_random_uuid(),$1,$2,$3,r.id,'pending' FROM records r WHERE r.workspace_id=$1 AND r.id=ANY($4) AND r.deleted_at IS NULL AND ($5 OR EXISTS(SELECT 1 FROM blueprint_connector_jobs j WHERE j.workspace_id=$1 AND j.extension_id=$2 AND j.direction='export' AND j.connection IS NOT DISTINCT FROM $3 AND j.blueprint_id=r.blueprint_id AND j.code IS NOT NULL AND j.enabled)) ON CONFLICT DO NOTHING")
                .bind(self.workspace_id()).bind(extension_id).bind(&selection.connection).bind(record_ids).bind(handles_retries)
                .execute(&mut *transaction).await?;
        }
        let ids: Vec<Uuid> = sqlx::query_scalar(&format!("WITH selected AS (SELECT id FROM extension_export_mappings WHERE workspace_id=$1 AND extension_id=$2 AND {} AND ($4::uuid[] IS NULL OR record_id=ANY($4)) AND ($5::text IS NULL OR status=$5) AND status<>'withdrawn' AND EXISTS(SELECT 1 FROM records r WHERE r.workspace_id=$1 AND r.id=record_id AND r.deleted_at IS NULL) AND {} ORDER BY record_id LIMIT $6 FOR UPDATE) UPDATE extension_export_mappings m SET status='pending',retry_requested_at=clock_timestamp(),attempts=0,updated_at=clock_timestamp() FROM selected s WHERE m.id=s.id RETURNING m.record_id", connection_is("connection", "$3", selection.connection.as_deref()), current_destination("extension_export_mappings")))
            .bind(self.workspace_id()).bind(extension_id).bind(&selection.connection)
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
        if let Some(key) = &selection.request_key {
            sqlx::query("UPDATE extension_export_retry_requests SET record_ids=$4 WHERE workspace_id=$1 AND extension_id=$2 AND request_key=$3")
                .bind(self.workspace_id()).bind(extension_id).bind(key).bind(&ids)
                .execute(&mut *transaction).await?;
        }
        self.write_audit_event(&mut transaction).await?;
        transaction.commit().await?;
        Ok(ids)
    }
}
