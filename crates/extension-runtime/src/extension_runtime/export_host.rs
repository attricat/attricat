//! Export host calls (attricat:host 1.1): the mapping store
//! (`mappings.*.v1`), connection descriptions (`connections.*.v1`) and
//! public file links (`files.link.v1`). They are `api.call` operations, so
//! they work from event handlers and from operation runs alike.

use chrono::{TimeDelta, Utc};
use ring::rand::{SecureRandom, SystemRandom};

use super::*;
use crate::repository::{ExportBegin, ExportMappingUpdate, MAX_EXPORT_MAPPING_PAGE, MappingKey};

/// The shortest and longest lifetime of a public file link. Marketplaces
/// fetch images asynchronously, sometimes hours after a listing is submitted.
const MIN_FILE_LINK_SECONDS: u64 = 60;
const MAX_FILE_LINK_SECONDS: u64 = 7 * 24 * 60 * 60;

static PUBLIC_FILE_LINK_BASE: OnceLock<Url> = OnceLock::new();

/// Installs the externally reachable base URL of public file links, such as
/// `https://attricat.example.com/api/public/extension-files/`. Without it,
/// `files.link.v1` is unavailable. Later calls are ignored.
pub fn use_public_file_link_base(base: Url) {
    let _ = PUBLIC_FILE_LINK_BASE.set(base);
}

/// Deserializes a field that distinguishes absent (`None`) from `null`
/// (`Some(None)`).
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingGet {
    #[serde(default)]
    connection: Option<String>,
    #[serde(default)]
    record_id: Option<Uuid>,
    #[serde(default)]
    external_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingList {
    #[serde(default)]
    connection: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    after: Option<Uuid>,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingBegin {
    #[serde(default)]
    connection: Option<String>,
    record_id: Uuid,
    record_revision: i64,
    #[serde(default)]
    lease_ms: Option<u64>,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingCommit {
    #[serde(default)]
    connection: Option<String>,
    record_id: Uuid,
    #[serde(default)]
    lease_token: Option<Uuid>,
    status: String,
    #[serde(default)]
    record_revision: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    external_id: Option<Option<String>>,
    #[serde(default)]
    external_ids: Option<Value>,
    #[serde(default, deserialize_with = "double_option")]
    content_hash: Option<Option<String>>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingRecord {
    #[serde(default)]
    connection: Option<String>,
    record_id: Uuid,
    #[serde(default)]
    lease_token: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionGet {
    connection: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLink {
    file_id: Uuid,
    record_id: Uuid,
    expires_in_seconds: u64,
}

impl HostState {
    pub(super) async fn mappings_call(
        &self,
        operation: &str,
        request: &str,
    ) -> Result<String, String> {
        let extension_id = &self.installation.extension_id;
        let repository = &self.repository;
        let error = |error: RepositoryError| error.to_string();
        match operation {
            "mappings.get.v1" => {
                let input: MappingGet = parse_host_request(request, "mapping request")?;
                let key = match (input.record_id, input.external_id.as_deref()) {
                    (Some(record_id), None) => MappingKey::Record(record_id),
                    (None, Some(external_id)) => MappingKey::ExternalId(external_id),
                    _ => return Err("name exactly one of record_id and external_id".into()),
                };
                let mapping = repository
                    .export_mapping_get(extension_id, input.connection.as_deref(), key)
                    .await
                    .map_err(error)?;
                bounded_serialize(&json!({"mapping": mapping}))
            }
            "mappings.list.v1" => {
                let input: MappingList = parse_host_request(request, "mapping request")?;
                let (mappings, next) = repository
                    .export_mapping_list(
                        extension_id,
                        input.connection.as_deref(),
                        input.status.as_deref(),
                        input.after,
                        input.limit.unwrap_or(MAX_EXPORT_MAPPING_PAGE),
                    )
                    .await
                    .map_err(error)?;
                bounded_serialize(&json!({"mappings": mappings, "next": next}))
            }
            "mappings.begin.v1" => {
                let input: MappingBegin = parse_host_request(request, "mapping request")?;
                self.ensure_may_map(input.record_id).await?;
                let lease = input.lease_ms.map(|milliseconds| {
                    TimeDelta::try_milliseconds(milliseconds.min(i64::MAX as u64) as i64)
                        .unwrap_or(TimeDelta::MAX)
                });
                let begin = repository
                    .export_mapping_begin(
                        extension_id,
                        input.connection.as_deref(),
                        input.record_id,
                        input.record_revision,
                        lease,
                        input.force,
                        self.lease_holder.as_deref(),
                    )
                    .await
                    .map_err(error)?;
                if matches!(begin, ExportBegin::Stale { .. }) {
                    counter!("attricat_extension_export_pushes_total", "outcome" => "stale")
                        .increment(1);
                }
                bounded_serialize(&begin)
            }
            "mappings.commit.v1" => {
                let input: MappingCommit = parse_host_request(request, "mapping request")?;
                self.ensure_may_map(input.record_id).await?;
                let mapping = repository
                    .export_mapping_commit(
                        extension_id,
                        input.connection.as_deref(),
                        input.record_id,
                        input.lease_token,
                        ExportMappingUpdate {
                            status: input.status,
                            record_revision: input.record_revision,
                            external_id: input.external_id,
                            external_ids: input.external_ids,
                            content_hash: input.content_hash,
                            error: input.error,
                        },
                    )
                    .await
                    .map_err(error)?;
                counter!("attricat_extension_export_pushes_total", "outcome" => mapping.status.clone())
                    .increment(1);
                bounded_serialize(&json!({"mapping": mapping}))
            }
            "mappings.release.v1" => {
                let input: MappingRecord = parse_host_request(request, "mapping request")?;
                self.ensure_may_map(input.record_id).await?;
                let lease_token = input
                    .lease_token
                    .ok_or_else(|| "lease_token is required".to_owned())?;
                let released = repository
                    .export_mapping_release(
                        extension_id,
                        input.connection.as_deref(),
                        input.record_id,
                        lease_token,
                    )
                    .await
                    .map_err(error)?;
                bounded_serialize(&json!({"released": released}))
            }
            "mappings.delete.v1" => {
                let input: MappingRecord = parse_host_request(request, "mapping request")?;
                self.ensure_may_map(input.record_id).await?;
                let deleted = repository
                    .export_mapping_delete(
                        extension_id,
                        input.connection.as_deref(),
                        input.record_id,
                    )
                    .await
                    .map_err(error)?;
                bounded_serialize(&json!({"deleted": deleted}))
            }
            _ => Err("unknown host operation".into()),
        }
    }

    /// Inside an operation run, refuses a mapping write for a record outside
    /// the run's scope.
    async fn ensure_may_map(&self, record_id: Uuid) -> Result<(), String> {
        match self.run_id {
            Some(run_id) => {
                super::transfer::ensure_run_may_map_record(&self.repository, run_id, record_id)
                    .await
            }
            None => Ok(()),
        }
    }

    /// Describes connections without their credentials.
    pub(super) async fn connections_call(
        &self,
        operation: &str,
        request: &str,
    ) -> Result<String, String> {
        let describe = |connection: crate::repository::ExtensionConnection| {
            json!({
                "connection": connection.connection,
                "kind": connection.kind,
                "display_name": connection.display_name,
                "parameters": connection.parameters,
                "status": connection.status,
            })
        };
        match operation {
            "connections.get.v1" => {
                let input: ConnectionGet = parse_host_request(request, "connection request")?;
                let connection = self
                    .repository
                    .extension_connection(&self.installation.extension_id, &input.connection)
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&describe(connection))
            }
            "connections.list.v1" => {
                let connections = self
                    .repository
                    .list_extension_connections(&self.installation.extension_id)
                    .await
                    .map_err(|error| error.to_string())?;
                bounded_serialize(&json!({
                    "connections": connections.into_iter().map(describe).collect::<Vec<_>>()
                }))
            }
            _ => Err("unknown host operation".into()),
        }
    }

    /// Issues a short-lived public link to a file a live record references,
    /// so a marketplace can fetch it. A run links only records in its scope.
    pub(super) async fn file_link_call(&self, request: &str) -> Result<String, String> {
        // A link exposes the file's bytes, so it needs the same grant as
        // reading them directly.
        self.require("attricat.read")
            .and_then(|()| self.require("attricat.files.read"))
            .map_err(|error| error.to_string())?;
        let input: FileLink = parse_host_request(request, "file link request")?;
        let base = PUBLIC_FILE_LINK_BASE
            .get()
            .ok_or_else(|| "public file links are not configured on this deployment".to_owned())?;
        if !(MIN_FILE_LINK_SECONDS..=MAX_FILE_LINK_SECONDS).contains(&input.expires_in_seconds) {
            return Err(format!(
                "file links live {MIN_FILE_LINK_SECONDS}-{MAX_FILE_LINK_SECONDS} seconds"
            ));
        }
        let targets = self
            .repository
            .file_read_targets(input.file_id)
            .await
            .map_err(|error| error.to_string())?;
        if !targets
            .iter()
            .any(|target| target.record_id == input.record_id)
        {
            return Err("the record does not reference that file".into());
        }
        if let Some(run_id) = self.run_id {
            super::transfer::ensure_run_covers_record(&self.repository, run_id, input.record_id)
                .await?;
        }
        let mut token = [0u8; 32];
        SystemRandom::new()
            .fill(&mut token)
            .map_err(|_| "random source unavailable")?;
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
        let expires_at = Utc::now() + TimeDelta::seconds(input.expires_in_seconds as i64);
        self.repository
            .create_extension_file_link(
                &self.installation.extension_id,
                input.file_id,
                input.record_id,
                &format!("{:x}", Sha256::digest(token.as_bytes())),
                expires_at,
            )
            .await
            .map_err(|error| error.to_string())?;
        let url = base
            .join(&token)
            .map_err(|_| "public file link base is invalid")?;
        bounded_serialize(&json!({"url": url.as_str(), "expires_at": expires_at}))
    }
}
