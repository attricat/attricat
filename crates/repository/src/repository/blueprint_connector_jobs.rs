use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError, StartExtensionOperation};
use attricat_blueprint::parse;
use sqlx::{Postgres, Transaction};

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct BlueprintConnectorJob {
    pub id: Uuid,
    pub blueprint_id: Uuid,
    pub blueprint_version: Option<i64>,
    pub code: Option<String>,
    pub direction: String,
    pub extension_id: String,
    pub operation_id: String,
    #[serde(skip_serializing)]
    pub input: Value,
    pub context_id: Option<Uuid>,
    pub input_file_id: Option<Uuid>,
    pub interval_seconds: Option<i32>,
    pub next_at: Option<DateTime<Utc>>,
    pub enabled: bool,
    pub connection: Option<String>,
    pub trigger_events: Option<Vec<String>>,
    pub trigger_debounce_seconds: Option<i32>,
}

const FIELDS: &str = "id,blueprint_id,blueprint_version,code,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at,enabled,connection,trigger_events,trigger_debounce_seconds";
/// How many change-triggered jobs one producer tick starts at most.
const TRIGGERED_JOBS_PER_TICK: i64 = 64;
/// How long a change trigger whose run could not be queued waits before it
/// tries again, so failing jobs cannot take every slot of a tick.
const TRIGGER_FAILURE_BACKOFF_SECONDS: i64 = 300;
/// A triggered job runs once its oldest pending change has waited this many
/// debounces, even while newer changes keep arriving.
const TRIGGER_MAX_WAIT_FACTOR: i32 = 10;
/// The most records a resync or a triggered run may select by ID.
pub const MAX_CONNECTOR_RECORD_FILTER: usize = 10_000;

/// How a connector run was started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorRunMode {
    /// A manual or interval run.
    Incremental,
    /// A backfill: re-export everything, or the selected records, skipping
    /// unchanged ones by their stored content hash.
    Resync,
    /// Started by record changes; selects the changed records.
    Triggered,
}

impl ConnectorRunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::Resync => "resync",
            Self::Triggered => "triggered",
        }
    }
}

/// What a connector run is confined to; the host enforces it on every call.
#[derive(Clone, Debug)]
pub(crate) struct ConnectorRunScope {
    pub job_id: Uuid,
    pub blueprint_id: Uuid,
    pub context_id: Uuid,
    pub blueprint_version: i64,
    pub channel_id: Option<Uuid>,
    pub connection: Option<String>,
    pub mode: ConnectorRunMode,
    pub record_ids: Option<Vec<Uuid>>,
}

/// A connector run's scope as its component sees it.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct ConnectorRunDetails {
    pub job_id: Uuid,
    pub job_code: Option<String>,
    pub direction: String,
    pub blueprint_id: Uuid,
    pub blueprint_version: i64,
    pub context_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub connection: Option<String>,
    pub mode: String,
    /// Present when the run is confined to selected records.
    pub record_count: Option<i64>,
    #[serde(skip)]
    pub record_ids: Option<Vec<Uuid>>,
}
const MAX_CONNECTOR_JOB_INPUT_BYTES: usize = 64 * 1024;
const MAX_RUN_KEY_BYTES: usize = 40;

fn invalid(message: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

impl AttricatRepository {
    /// Replace the live job declarations atomically with publication of the
    /// immutable blueprint revision. Stable codes retain run history/IDs;
    /// removed jobs are disabled, never deleted (runs reference them).
    pub(super) async fn sync_blueprint_connector_jobs(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        version: i64,
        definition: &str,
    ) -> Result<(), RepositoryError> {
        let blueprint = parse(definition).map_err(RepositoryError::invalid_blueprint_definition)?;
        let ws = self.extension_workspace();
        let mut prepared = Vec::new();
        for job in blueprint.connector_jobs {
            let input = serde_json::to_value(&job.input)
                .map_err(|_| invalid("invalid connector job input"))?;
            if serde_json::to_vec(&input)
                .map_or(true, |bytes| bytes.len() > MAX_CONNECTOR_JOB_INPUT_BYTES)
            {
                return Err(invalid("connector job input exceeds 64 KiB"));
            }
            let release = self
                .installed_extension(&job.extension_id)
                .await?
                .installed_release_id;
            let installation = self
                .runtime_extension_installation(&job.extension_id, release)
                .await?
                .ok_or_else(|| invalid("connector release is not enabled or authorized"))?;
            let operation = installation
                .manifest
                .server
                .as_ref()
                .and_then(|server| {
                    server
                        .operations
                        .iter()
                        .find(|op| op.id == job.operation_id)
                })
                .ok_or_else(|| invalid("connector operation is not declared"))?;
            attricat_extension_manifest::validate_schema(&operation.request_schema, &input)
                .map_err(|_| invalid("connector input does not match operation schema"))?;
            if job.connection.is_some()
                && installation
                    .manifest
                    .server
                    .as_ref()
                    .is_none_or(|server| server.connections.is_empty())
            {
                return Err(invalid("the connector extension declares no connections"));
            }
            let context_id: Option<Uuid> = match job.context.as_deref() {
                Some(code) => Some(
                    sqlx::query_scalar(
                        "SELECT id FROM attribute_contexts WHERE workspace_id=$1 AND code=$2",
                    )
                    .bind(ws)
                    .bind(code)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or_else(|| invalid("import context does not exist"))?,
                ),
                None => None,
            };
            let input_file_id = job
                .input_file_id
                .as_deref()
                .map(|value| {
                    value
                        .parse::<Uuid>()
                        .map_err(|_| invalid("invalid import input file ID"))
                })
                .transpose()?;
            if let Some(file) = input_file_id {
                // Lock the file so reconciliation cannot reclaim it before
                // the job that references it commits.
                let ready: Option<Uuid> = sqlx::query_scalar("SELECT id FROM files WHERE workspace_id=$1 AND id=$2 AND status='ready' AND deleted_at IS NULL FOR UPDATE")
                    .bind(ws).bind(file).fetch_optional(&mut **tx).await?;
                if ready.is_none() {
                    return Err(invalid("import input file is not ready"));
                }
            }
            prepared.push((job, input, context_id, input_file_id));
        }
        // Only a trigger that is watching now keeps its watermark; one that
        // was disabled or removed starts again at the end of the event log.
        let watching: Vec<String> = sqlx::query_scalar("SELECT code FROM blueprint_connector_jobs WHERE workspace_id=$1 AND blueprint_id=$2 AND enabled AND code IS NOT NULL AND trigger_watermark IS NOT NULL")
            .bind(ws).bind(blueprint_id).fetch_all(&mut **tx).await?;
        sqlx::query("UPDATE blueprint_connector_jobs SET enabled=false,updated_at=clock_timestamp() WHERE workspace_id=$1 AND blueprint_id=$2")
            .bind(ws).bind(blueprint_id).execute(&mut **tx).await?;
        // A change trigger starts watching at the current end of the event
        // log, so publishing a job never replays history. The outbox
        // boundary keeps an uncommitted event from landing below it.
        super::domain_events::lock_outbox_boundary(tx, ws).await?;
        let high_water: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(sequence),0) FROM domain_events WHERE workspace_id=$1",
        )
        .bind(ws)
        .fetch_one(&mut **tx)
        .await?;
        for (job, input, context_id, file) in prepared {
            let (trigger_events, trigger_debounce) = job
                .trigger
                .as_ref()
                .map(|trigger| (trigger.events.clone(), trigger.debounce_seconds))
                .unzip();
            sqlx::query("INSERT INTO blueprint_connector_jobs(id,workspace_id,blueprint_id,blueprint_version,code,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at,enabled,connection,trigger_events,trigger_debounce_seconds,trigger_watermark) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,CASE WHEN $12::integer IS NULL THEN NULL ELSE clock_timestamp()+($12*interval '1 second') END,$13,$14,$15,$16,CASE WHEN $15::text[] IS NULL THEN NULL ELSE $17::bigint END) ON CONFLICT(workspace_id,blueprint_id,code) WHERE code IS NOT NULL DO UPDATE SET blueprint_version=$4,direction=$6,extension_id=$7,operation_id=$8,input=$9,context_id=$10,input_file_id=$11,interval_seconds=$12,next_at=CASE WHEN $12::integer IS NULL THEN NULL ELSE clock_timestamp()+($12*interval '1 second') END,enabled=$13,connection=$14,trigger_events=$15,trigger_debounce_seconds=$16,trigger_watermark=CASE WHEN $15::text[] IS NULL THEN NULL WHEN $18 THEN COALESCE(blueprint_connector_jobs.trigger_watermark,$17::bigint) ELSE $17::bigint END,trigger_blocked_until=NULL,updated_at=clock_timestamp()")
                .bind(Uuid::new_v4()).bind(ws).bind(blueprint_id).bind(version).bind(&job.code)
                .bind(&job.direction).bind(&job.extension_id).bind(&job.operation_id).bind(input)
                .bind(context_id).bind(file).bind(job.interval_seconds).bind(job.enabled)
                .bind(&job.connection).bind(trigger_events).bind(trigger_debounce).bind(high_water)
                .bind(watching.contains(&job.code))
                .execute(&mut **tx).await?;
        }
        Ok(())
    }

    pub async fn list_blueprint_connector_jobs(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<BlueprintConnectorJob>, RepositoryError> {
        sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND blueprint_id=$2 AND code IS NOT NULL ORDER BY id"))
            .bind(self.extension_workspace()).bind(blueprint_id).fetch_all(&self.pool).await.map_err(Into::into)
    }

    /// The enabled export jobs of an extension.
    pub async fn extension_export_connector_jobs(
        &self,
        extension_id: &str,
    ) -> Result<Vec<BlueprintConnectorJob>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND extension_id=$2 AND direction='export' AND enabled AND code IS NOT NULL ORDER BY id"))
            .bind(self.extension_workspace()).bind(extension_id).fetch_all(&self.pool).await?)
    }

    pub async fn blueprint_connector_job(
        &self,
        id: Uuid,
    ) -> Result<Option<BlueprintConnectorJob>, RepositoryError> {
        Ok(sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND id=$2 AND code IS NOT NULL"))
            .bind(self.extension_workspace()).bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn blueprint_connector_jobs_page(
        &self,
        blueprint_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<BlueprintConnectorJob>, bool), RepositoryError> {
        let mut rows = sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND blueprint_id=$2 AND code IS NOT NULL ORDER BY id LIMIT $3 OFFSET $4"))
            .bind(self.extension_workspace()).bind(blueprint_id).bind(limit + 1).bind(offset)
            .fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        rows.truncate(limit as usize);
        Ok((rows, has_more))
    }

    /// One run per enabled publication channel. No channel or record filter is
    /// supplied by the component: every catalog page is checked against this
    /// run's persisted scope by the host.
    pub async fn run_blueprint_connector_job(
        &self,
        id: Uuid,
        key: &str,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        self.run_blueprint_connector_job_as(id, key, ConnectorRunMode::Incremental, None)
            .await
    }

    /// Starts a resync of an export job: every published record, or only
    /// `record_ids`. Its runs tell the component to skip records whose
    /// stored content hash is unchanged rather than records already synced.
    pub async fn resync_blueprint_connector_job(
        &self,
        id: Uuid,
        key: &str,
        record_ids: Option<Vec<Uuid>>,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        self.run_blueprint_connector_job_as(id, key, ConnectorRunMode::Resync, record_ids)
            .await
    }

    async fn run_blueprint_connector_job_as(
        &self,
        id: Uuid,
        key: &str,
        mode: ConnectorRunMode,
        record_ids: Option<Vec<Uuid>>,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        if key.is_empty() || key.len() > MAX_RUN_KEY_BYTES || !key.is_ascii() {
            return Err(invalid("invalid run key"));
        }
        if record_ids
            .as_ref()
            .is_some_and(|ids| ids.is_empty() || ids.len() > MAX_CONNECTOR_RECORD_FILTER)
        {
            return Err(invalid("a run selects 1-10000 records"));
        }
        let job: BlueprintConnectorJob = sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND id=$2 AND enabled AND code IS NOT NULL"))
            .bind(self.extension_workspace()).bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| RepositoryError::NotFound("blueprint connector job"))?;
        if mode != ConnectorRunMode::Incremental && job.direction != "export" {
            return Err(invalid("only export jobs can resync or be triggered"));
        }
        let release = self
            .installed_extension(&job.extension_id)
            .await?
            .installed_release_id;
        let installation = self
            .runtime_extension_installation(&job.extension_id, release)
            .await?
            .ok_or_else(|| invalid("connector release is not enabled"))?;
        let operation = installation
            .manifest
            .server
            .as_ref()
            .and_then(|server| {
                server
                    .operations
                    .iter()
                    .find(|op| op.id == job.operation_id)
            })
            .ok_or_else(|| invalid("connector operation is unavailable"))?;
        if let Some(connection) = &job.connection {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_connections WHERE workspace_id=$1 AND extension_id=$2 AND connection=$3)")
                .bind(self.extension_workspace()).bind(&job.extension_id).bind(connection)
                .fetch_one(&self.pool).await?;
            if !exists {
                return Err(invalid("the job's connection does not exist"));
            }
        }
        let version: i64 = sqlx::query_scalar("SELECT max(version) FROM blueprints WHERE workspace_id=$1 AND id=$2 AND kind='record' AND status='published' AND deleted_at IS NULL")
            .bind(self.extension_workspace()).bind(job.blueprint_id).fetch_optional(&self.pool).await?
            .flatten().ok_or_else(|| invalid("blueprint is no longer published"))?;
        let channels: Vec<Uuid> = if job.direction == "export" {
            sqlx::query_scalar("SELECT context_id FROM publication_channels WHERE workspace_id=$1 AND enabled ORDER BY context_id")
                .bind(self.extension_workspace()).fetch_all(&self.pool).await?
        } else {
            vec![job.context_id.expect("validated import context")]
        };
        let mut ids = Vec::new();
        for context in channels {
            let mut input = job.input.clone();
            // Compatible with the released CSV connector; other connectors may
            // read the same host-selected IDs from their operation input.
            if let Some(profile) = input.get_mut("profile").and_then(Value::as_object_mut) {
                profile.insert("blueprint_id".into(), json!(job.blueprint_id));
                profile.insert("blueprint_version".into(), json!(version));
                profile.insert("context_id".into(), json!(context));
            }
            attricat_extension_manifest::validate_schema(&operation.request_schema, &input)
                .map_err(|_| invalid("scoped input does not match connector schema"))?;
            let prefix = match mode {
                ConnectorRunMode::Incremental => "job",
                ConnectorRunMode::Resync => "resync",
                ConnectorRunMode::Triggered => "trigger",
            };
            let run = self
                .start_extension_operation_scoped(
                    StartExtensionOperation {
                        extension_id: job.extension_id.clone(),
                        expected_release_id: release,
                        operation_id: job.operation_id.clone(),
                        input,
                        source_reference: job
                            .input_file_id
                            .map_or_else(|| json!({}), |file| json!({"input_file_id":file})),
                        destination_reference: json!({}),
                        idempotency_key: format!("{prefix}:{id}:{key}:{context}"),
                        schedule_id: None,
                        configuration_snapshot: None,
                    },
                    Some(ConnectorRunScope {
                        job_id: id,
                        blueprint_id: job.blueprint_id,
                        context_id: context,
                        blueprint_version: version,
                        channel_id: (job.direction == "export").then_some(context),
                        connection: job.connection.clone(),
                        mode,
                        record_ids: record_ids.clone(),
                    }),
                )
                .await?;
            ids.push(run);
        }
        Ok(ids)
    }

    /// The existing extension-operation coordinator invokes this producer.
    pub async fn produce_due_blueprint_connector_jobs(&self) -> Result<usize, RepositoryError> {
        let due: Vec<(Uuid, DateTime<Utc>, i32)> = sqlx::query_as("SELECT id,next_at,interval_seconds FROM blueprint_connector_jobs WHERE workspace_id=$1 AND enabled AND code IS NOT NULL AND next_at<=clock_timestamp() ORDER BY next_at,id LIMIT 16")
            .bind(self.extension_workspace()).fetch_all(&self.pool).await?;
        for (id, tick, interval) in &due {
            // The run key is deterministic on retry. An interrupted producer
            // can enqueue missing channels without duplicating existing runs.
            let occurrence = tick.timestamp_micros().to_string();
            let other_active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_runs WHERE workspace_id=$1 AND connector_job_id=$2 AND status IN ('pending','leased') AND idempotency_key NOT LIKE $3)")
                .bind(self.extension_workspace()).bind(id)
                .bind(format!("job:{id}:{occurrence}:%"))
                .fetch_one(&self.pool).await?;
            if !other_active
                && let Err(error) = self.run_blueprint_connector_job(*id, &occurrence).await
            {
                tracing::warn!(job=%id, %error, "connector job occurrence could not be queued");
                continue;
            }
            let next = Utc::now() + Duration::seconds(i64::from(*interval));
            sqlx::query("UPDATE blueprint_connector_jobs SET next_at=$3,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND next_at=$4")
                .bind(id).bind(self.extension_workspace()).bind(next).bind(tick).execute(&self.pool).await?;
        }
        let triggered = self.produce_triggered_blueprint_connector_jobs().await?;
        Ok(due.len() + triggered)
    }

    /// Starts a run of every change-triggered export job whose blueprint's
    /// records changed after its watermark, once the newest change is older
    /// than the job's debounce or the oldest has waited
    /// [`TRIGGER_MAX_WAIT_FACTOR`] debounces. The run selects the changed
    /// records (or every record when too many changed), and the watermark
    /// moves past them.
    async fn produce_triggered_blueprint_connector_jobs(&self) -> Result<usize, RepositoryError> {
        // A job with no matching change up to the committed end of the log
        // moves its watermark there, so the next sweep scans only newer
        // events instead of all history since the job's last run.
        let high_water = self.committed_event_high_water().await?;
        sqlx::query(
            "UPDATE blueprint_connector_jobs j SET trigger_watermark=$2 \
             WHERE j.workspace_id=$1 AND j.enabled AND j.code IS NOT NULL AND j.trigger_events IS NOT NULL \
               AND j.trigger_watermark IS NOT NULL AND j.trigger_watermark<$2 \
               AND NOT EXISTS (SELECT 1 FROM domain_events e JOIN records r ON r.workspace_id=e.workspace_id AND r.id=e.aggregate_id \
                               WHERE e.workspace_id=j.workspace_id AND e.aggregate_kind='record' \
                                 AND e.sequence>j.trigger_watermark AND e.sequence<=$2 \
                                 AND e.event_type=ANY(j.trigger_events) AND r.blueprint_id=j.blueprint_id)",
        )
        .bind(self.extension_workspace())
        .bind(high_water)
        .execute(&self.pool)
        .await?;
        // Only jobs with pending changes are considered, the longest-waiting
        // first, so idle jobs cannot crowd out ones that have work.
        let jobs: Vec<(Uuid, Uuid, Vec<String>, i32, i64)> = sqlx::query_as(
            "SELECT j.id,j.blueprint_id,j.trigger_events,j.trigger_debounce_seconds,j.trigger_watermark FROM blueprint_connector_jobs j \
             WHERE j.workspace_id=$1 AND j.enabled AND j.code IS NOT NULL AND j.trigger_events IS NOT NULL AND j.trigger_watermark IS NOT NULL \
               AND (j.trigger_blocked_until IS NULL OR j.trigger_blocked_until<=clock_timestamp()) \
               AND EXISTS (SELECT 1 FROM domain_events e JOIN records r ON r.workspace_id=e.workspace_id AND r.id=e.aggregate_id \
                           WHERE e.workspace_id=j.workspace_id AND e.aggregate_kind='record' AND e.sequence>j.trigger_watermark \
                             AND e.event_type=ANY(j.trigger_events) AND r.blueprint_id=j.blueprint_id) \
             ORDER BY j.trigger_watermark,j.id LIMIT $2",
        )
        .bind(self.extension_workspace())
        .bind(TRIGGERED_JOBS_PER_TICK)
        .fetch_all(&self.pool)
        .await?;
        let mut started = 0;
        // The oldest and newest pending change decide whether a job is due;
        // each is one index-ordered row, so a job inside its debounce does
        // not read its whole change window on every tick.
        let pending_change = |newest: bool| {
            format!(
                "SELECT e.occurred_at FROM domain_events e \
                 WHERE e.workspace_id=$1 AND e.aggregate_kind='record' AND e.sequence>$2 AND e.event_type=ANY($3) \
                   AND EXISTS (SELECT 1 FROM records r WHERE r.workspace_id=e.workspace_id AND r.id=e.aggregate_id AND r.blueprint_id=$4) \
                 ORDER BY e.sequence {} LIMIT 1",
                if newest { "DESC" } else { "ASC" }
            )
        };
        let (oldest_sql, newest_sql) = (pending_change(false), pending_change(true));
        for (id, blueprint_id, events, debounce, watermark) in jobs {
            let mut bounds = Vec::with_capacity(2);
            for sql in [&oldest_sql, &newest_sql] {
                bounds.push(
                    sqlx::query_scalar::<_, DateTime<Utc>>(sql)
                        .bind(self.extension_workspace())
                        .bind(watermark)
                        .bind(&events)
                        .bind(blueprint_id)
                        .fetch_optional(&self.pool)
                        .await?,
                );
            }
            let [Some(oldest), Some(newest)] = bounds[..] else {
                continue;
            };
            let debounce = Duration::seconds(i64::from(debounce));
            let now = Utc::now();
            if now - newest < debounce && now - oldest < debounce * TRIGGER_MAX_WAIT_FACTOR {
                continue;
            }
            let changes: Vec<(i64, Uuid)> = sqlx::query_as(
                "SELECT e.sequence,e.aggregate_id FROM domain_events e \
                 WHERE e.workspace_id=$1 AND e.aggregate_kind='record' AND e.sequence>$2 AND e.event_type=ANY($3) \
                   AND EXISTS (SELECT 1 FROM records r WHERE r.workspace_id=e.workspace_id AND r.id=e.aggregate_id AND r.blueprint_id=$4) \
                 ORDER BY e.sequence LIMIT $5",
            )
            .bind(self.extension_workspace())
            .bind(watermark)
            .bind(&events)
            .bind(blueprint_id)
            .bind(MAX_CONNECTOR_RECORD_FILTER as i64 + 1)
            .fetch_all(&self.pool)
            .await?;
            let Some(&(high_water, _)) = changes.last() else {
                continue;
            };
            let mut record_ids: Vec<Uuid> = changes.iter().map(|change| change.1).collect();
            record_ids.sort_unstable();
            record_ids.dedup();
            // The run covers events up to `high_water` only, so the records
            // of this window are all that changed even when it is full.
            let filter = (record_ids.len() <= MAX_CONNECTOR_RECORD_FILTER).then_some(record_ids);
            match self
                .run_blueprint_connector_job_as(
                    id,
                    &high_water.to_string(),
                    ConnectorRunMode::Triggered,
                    filter,
                )
                .await
            {
                Ok(_) => started += 1,
                Err(error) => {
                    // Keep the watermark, like an interval occurrence keeps
                    // `next_at`, so the changes are exported once the job can
                    // run again instead of being skipped. Back off so the job
                    // leaves this tick's slots to others meanwhile.
                    tracing::warn!(job=%id, %error, "triggered connector run could not be queued");
                    sqlx::query("UPDATE blueprint_connector_jobs SET trigger_blocked_until=clock_timestamp()+make_interval(secs => $3) WHERE id=$1 AND workspace_id=$2")
                        .bind(id).bind(self.extension_workspace()).bind(TRIGGER_FAILURE_BACKOFF_SECONDS as f64)
                        .execute(&self.pool).await?;
                    continue;
                }
            }
            sqlx::query("UPDATE blueprint_connector_jobs SET trigger_watermark=$3,trigger_blocked_until=NULL,updated_at=clock_timestamp() WHERE id=$1 AND workspace_id=$2 AND trigger_watermark=$4")
                .bind(id).bind(self.extension_workspace()).bind(high_water).bind(watermark)
                .execute(&self.pool).await?;
        }
        Ok(started)
    }

    /// The connector scope of a run as its component sees it.
    pub async fn connector_run_details(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ConnectorRunDetails>, RepositoryError> {
        Ok(sqlx::query_as("SELECT j.id AS job_id,j.code AS job_code,j.direction,r.connector_blueprint_id AS blueprint_id,r.connector_blueprint_version AS blueprint_version,r.connector_context_id AS context_id,r.connector_channel_id AS channel_id,r.connector_connection AS connection,COALESCE(r.connector_mode,'incremental') AS mode,cardinality(r.connector_record_ids)::bigint AS record_count,r.connector_record_ids AS record_ids FROM extension_operation_runs r JOIN blueprint_connector_jobs j ON j.id=r.connector_job_id AND j.workspace_id=r.workspace_id WHERE r.id=$1 AND r.workspace_id=$2")
            .bind(run_id).bind(self.extension_workspace()).fetch_optional(&self.pool).await?)
    }

    /// The live records among `record_ids` of `blueprint_id`.
    pub async fn records_of_blueprint(
        &self,
        blueprint_id: Uuid,
        record_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT id FROM records WHERE workspace_id=$1 AND blueprint_id=$2 AND id=ANY($3) AND deleted_at IS NULL ORDER BY id")
            .bind(self.extension_workspace()).bind(blueprint_id).bind(record_ids)
            .fetch_all(&self.pool).await?)
    }

    /// Whether a connector run's catalog covers `record_id`: a live record of
    /// the run's blueprint version, among its selected records when it has a
    /// selection, and published in its channel when it exports to one.
    pub async fn connector_run_covers_record(
        &self,
        run_id: Uuid,
        record_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM extension_operation_runs r \
               JOIN records e ON e.workspace_id=r.workspace_id AND e.id=$3 AND e.deleted_at IS NULL \
                 AND e.blueprint_id=r.connector_blueprint_id AND e.blueprint_version=r.connector_blueprint_version \
             WHERE r.id=$1 AND r.workspace_id=$2 \
               AND (r.connector_record_ids IS NULL OR $3=ANY(r.connector_record_ids)) \
               AND (r.connector_channel_id IS NULL OR EXISTS (SELECT 1 FROM record_channel_publications p \
                 WHERE p.workspace_id=r.workspace_id AND p.record_id=e.id AND p.context_id=r.connector_channel_id AND p.published_at IS NOT NULL)))",
        )
        .bind(run_id)
        .bind(self.extension_workspace())
        .bind(record_id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Whether a connector run may write the export mapping of `record_id`:
    /// a record of the run's blueprint. Unlike
    /// [`Self::connector_run_covers_record`] this includes deleted,
    /// unpublished and older-version records and ones outside the run's
    /// selection, whose withdrawals the run's withdrawn feed reports.
    pub async fn connector_run_may_map_record(
        &self,
        run_id: Uuid,
        record_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM extension_operation_runs r \
               JOIN records e ON e.workspace_id=r.workspace_id AND e.id=$3 AND e.blueprint_id=r.connector_blueprint_id \
             WHERE r.id=$1 AND r.workspace_id=$2)",
        )
        .bind(run_id)
        .bind(self.extension_workspace())
        .bind(record_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn connector_run_scope(
        &self,
        run_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid, i64, Option<Uuid>, String)>, RepositoryError> {
        let scoped: Option<(Uuid, Uuid, i64, Option<Uuid>, String, bool)> = sqlx::query_as("SELECT r.connector_blueprint_id,r.connector_context_id,r.connector_blueprint_version,r.connector_channel_id,j.direction,j.enabled FROM extension_operation_runs r JOIN blueprint_connector_jobs j ON j.id=r.connector_job_id AND j.workspace_id=r.workspace_id WHERE r.id=$1 AND r.workspace_id=$2")
            .bind(run_id).bind(self.extension_workspace()).fetch_optional(&self.pool).await?;
        match scoped {
            Some((blueprint, context, version, channel, direction, true)) => {
                Ok(Some((blueprint, context, version, channel, direction)))
            }
            Some(_) => Err(invalid("connector job is disabled")),
            None => Ok(None),
        }
    }
}
