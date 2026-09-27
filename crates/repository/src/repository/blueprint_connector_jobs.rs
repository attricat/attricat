use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError, StartExtensionOperation};

#[derive(Clone, Debug)]
pub struct CreateBlueprintConnectorJob {
    pub blueprint_id: Uuid,
    pub direction: String,
    pub extension_id: String,
    pub operation_id: String,
    pub input: Value,
    pub context_id: Option<Uuid>,
    pub input_file_id: Option<Uuid>,
    pub interval_seconds: Option<i32>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct BlueprintConnectorJob {
    pub id: Uuid,
    pub blueprint_id: Uuid,
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

const FIELDS: &str = "id,blueprint_id,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at,enabled";
fn invalid(message: &str) -> RepositoryError {
    RepositoryError::InvalidExtension(message.into())
}

impl CatalogRepository {
    pub async fn create_blueprint_connector_job(
        &self,
        job: CreateBlueprintConnectorJob,
    ) -> Result<BlueprintConnectorJob, RepositoryError> {
        if !matches!(job.direction.as_str(), "import" | "export")
            || (job.direction == "import") != job.context_id.is_some()
            || (job.direction == "export" && job.input_file_id.is_some())
            || job
                .interval_seconds
                .is_some_and(|seconds| !(60..=2_592_000).contains(&seconds))
            || !job.input.is_object()
            || serde_json::to_vec(&job.input).map_or(true, |bytes| bytes.len() > 65_536)
        {
            return Err(invalid("invalid connector job configuration"));
        }
        // Require the extension to be installed, enabled and to declare the operation.
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
        catalog_extension_manifest::validate_schema(&operation.request_schema, &job.input)
            .map_err(|_| invalid("connector input does not match operation schema"))?;
        // Scoped calls are supported only by the connector ABI; older operation
        // worlds cannot be constrained to host-selected entity pages.
        let range = semver::VersionReq::parse(&installation.manifest.catalog.host_api)
            .map_err(|_| invalid("invalid connector host API range"))?;
        if !range.matches(&semver::Version::new(1, 4, 0))
            || range.matches(&semver::Version::new(1, 3, 0))
        {
            return Err(invalid("blueprint jobs require the 1.4 connector ABI"));
        }
        let ws = self.extension_workspace();
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM blueprints WHERE workspace_id=$1 AND id=$2 AND kind='entity' AND status='published' AND deleted_at IS NULL)")
            .bind(ws).bind(job.blueprint_id).fetch_one(&self.pool).await?;
        if !exists {
            return Err(invalid("blueprint must be published"));
        }
        if let Some(context) = job.context_id {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM attribute_contexts WHERE workspace_id=$1 AND id=$2)",
            )
            .bind(ws)
            .bind(context)
            .fetch_one(&self.pool)
            .await?;
            if !exists {
                return Err(invalid("import context does not exist"));
            }
        }
        if let Some(file) = job.input_file_id {
            let ready: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files WHERE workspace_id=$1 AND id=$2 AND status='ready' AND deleted_at IS NULL)")
                .bind(ws).bind(file).fetch_one(&self.pool).await?;
            if !ready {
                return Err(invalid("import input file is not ready"));
            }
        }
        sqlx::query_as(&format!("INSERT INTO blueprint_connector_jobs(id,workspace_id,blueprint_id,direction,extension_id,operation_id,input,context_id,input_file_id,interval_seconds,next_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,CASE WHEN $10::integer IS NULL THEN NULL ELSE clock_timestamp()+($10*interval '1 second') END) RETURNING {FIELDS}"))
            .bind(Uuid::new_v4()).bind(ws).bind(job.blueprint_id).bind(job.direction)
            .bind(job.extension_id).bind(job.operation_id).bind(job.input).bind(job.context_id)
            .bind(job.input_file_id).bind(job.interval_seconds).fetch_one(&self.pool).await.map_err(Into::into)
    }

    pub async fn list_blueprint_connector_jobs(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<BlueprintConnectorJob>, RepositoryError> {
        sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND blueprint_id=$2 ORDER BY id"))
            .bind(self.extension_workspace()).bind(blueprint_id).fetch_all(&self.pool).await.map_err(Into::into)
    }

    pub async fn set_blueprint_connector_job_enabled(
        &self,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<BlueprintConnectorJob>, RepositoryError> {
        sqlx::query_as(&format!("UPDATE blueprint_connector_jobs SET enabled=$3,updated_at=clock_timestamp(),next_at=CASE WHEN $3 AND interval_seconds IS NOT NULL THEN clock_timestamp()+(interval_seconds*interval '1 second') ELSE next_at END WHERE id=$1 AND workspace_id=$2 RETURNING {FIELDS}"))
            .bind(id).bind(self.extension_workspace()).bind(enabled).fetch_optional(&self.pool).await.map_err(Into::into)
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
        let job: BlueprintConnectorJob = sqlx::query_as(&format!("SELECT {FIELDS} FROM blueprint_connector_jobs WHERE workspace_id=$1 AND id=$2 AND enabled"))
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
        let range = semver::VersionReq::parse(&installation.manifest.catalog.host_api)
            .map_err(|_| invalid("invalid connector host API range"))?;
        if !range.matches(&semver::Version::new(1, 4, 0))
            || range.matches(&semver::Version::new(1, 3, 0))
        {
            return Err(invalid("connector release no longer supports scoped jobs"));
        }
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
        let due: Vec<(Uuid, DateTime<Utc>, i32)> = sqlx::query_as("SELECT id,next_at,interval_seconds FROM blueprint_connector_jobs WHERE workspace_id=$1 AND enabled AND next_at<=clock_timestamp() ORDER BY next_at,id LIMIT 16")
            .bind(self.extension_workspace()).fetch_all(&self.pool).await?;
        for (id, tick, interval) in &due {
            // The run key is deterministic on retry. An interrupted producer
            // can enqueue missing channels without duplicating existing runs.
            let occurrence = tick.timestamp_micros().to_string();
            let other_active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_operation_runs WHERE workspace_id=$1 AND connector_job_id=$2 AND status IN ('pending','leased') AND idempotency_key NOT LIKE $3)")
                .bind(self.extension_workspace()).bind(id)
                .bind(format!("job:{id}:{occurrence}:%"))
                .fetch_one(&self.pool).await?;
            if !other_active {
                if let Err(error) = self.run_blueprint_connector_job(*id, &occurrence).await {
                    tracing::warn!(job=%id, %error, "connector job occurrence could not be queued");
                    continue;
                }
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
