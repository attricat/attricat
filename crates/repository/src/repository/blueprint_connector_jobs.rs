use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError, StartExtensionOperation};
use catalog_blueprint::parse;
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
}

const FIELDS: &str = "id,blueprint_id,blueprint_version,code,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at,enabled";
fn invalid(message: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

impl CatalogRepository {
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
            if serde_json::to_vec(&input).map_or(true, |bytes| bytes.len() > 65_536) {
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
            catalog_extension_manifest::validate_schema(&operation.request_schema, &input)
                .map_err(|_| invalid("connector input does not match operation schema"))?;
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
        sqlx::query("UPDATE blueprint_connector_jobs SET enabled=false,updated_at=clock_timestamp() WHERE workspace_id=$1 AND blueprint_id=$2")
            .bind(ws).bind(blueprint_id).execute(&mut **tx).await?;
        for (job, input, context_id, file) in prepared {
            sqlx::query("INSERT INTO blueprint_connector_jobs(id,workspace_id,blueprint_id,blueprint_version,code,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at,enabled) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,CASE WHEN $12::integer IS NULL THEN NULL ELSE clock_timestamp()+($12*interval '1 second') END,$13) ON CONFLICT(workspace_id,blueprint_id,code) WHERE code IS NOT NULL DO UPDATE SET blueprint_version=$4,direction=$6,extension_id=$7,operation_id=$8,input=$9,context_id=$10,input_file_id=$11,interval_seconds=$12,next_at=CASE WHEN $12::integer IS NULL THEN NULL ELSE clock_timestamp()+($12*interval '1 second') END,enabled=$13,updated_at=clock_timestamp()")
                .bind(Uuid::new_v4()).bind(ws).bind(blueprint_id).bind(version).bind(&job.code)
                .bind(&job.direction).bind(&job.extension_id).bind(&job.operation_id).bind(input)
                .bind(context_id).bind(file).bind(job.interval_seconds).bind(job.enabled)
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

    /// One run per enabled publication channel. No channel or entity filter is
    /// supplied by the component: every catalog page is checked against this
    /// run's persisted scope by the host.
    pub async fn run_blueprint_connector_job(
        &self,
        id: Uuid,
        key: &str,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        if key.is_empty() || key.len() > 40 || !key.is_ascii() {
            return Err(invalid("invalid run key"));
        }
        let job: BlueprintConnectorJob = sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND id=$2 AND enabled AND code IS NOT NULL"))
            .bind(self.extension_workspace()).bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| RepositoryError::NotFound("blueprint connector job"))?;
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
        let version: i64 = sqlx::query_scalar("SELECT max(version) FROM blueprints WHERE workspace_id=$1 AND id=$2 AND kind='entity' AND status='published' AND deleted_at IS NULL")
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
            catalog_extension_manifest::validate_schema(&operation.request_schema, &input)
                .map_err(|_| invalid("scoped input does not match connector schema"))?;
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
                        idempotency_key: format!("job:{id}:{key}:{context}"),
                        schedule_id: None,
                        configuration_snapshot: None,
                    },
                    Some((
                        id,
                        job.blueprint_id,
                        context,
                        version,
                        (job.direction == "export").then_some(context),
                    )),
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
        Ok(due.len())
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
