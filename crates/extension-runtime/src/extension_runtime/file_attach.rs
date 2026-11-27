//! `files.attach.v1`: stores a completed input artifact of an operation run,
//! such as a document fetched with `transfer.fetch-input`, as a file of a
//! record's file attribute. It is an `api.call` operation, so the WIT is
//! unchanged, but it works only during a run, whose artifacts it reads.

use attricat_domain::permissions::Permission;
use attricat_storage::StoredObject;
use attricat_validation::files::{detect_mime, sanitize_supplied_filename};
use bytes::Bytes;

use super::*;
use crate::extensions::InteractiveScope;
use crate::repository::NewUploadedFile;

/// The largest artifact a run may attach, the most `open-record-file` copies
/// into a run.
const MAX_ATTACHED_FILE_BYTES: i64 = 16 * 1024 * 1024;
/// How long an object stored for an attachment that never commits is kept
/// before cleanup deletes it.
const ATTACH_CLEANUP_HOURS: i64 = 1;
/// Where uploaded record files are stored.
const FILE_OBJECT_PREFIX: &str = "files/";
/// The capabilities an attachment needs: it writes a value and reads an
/// artifact of the run.
const ATTACH_GRANTS: [&str; 2] = ["attricat.write", "artifacts.read"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileAttach {
    record_id: Uuid,
    attribute_code: String,
    #[serde(default)]
    context_id: Option<Uuid>,
    artifact_id: Uuid,
    filename: String,
}

impl OperationState {
    /// Attaches a completed input artifact of this run to a record's file
    /// attribute. A file of the same content and name already there is
    /// returned as `replayed`, so a retried batch attaches it once.
    pub(super) async fn files_attach(&mut self, request: String) -> Result<String, String> {
        let input: FileAttach = parse_host_request(&request, "file attach request")?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.run_id = Some(self.run_id);
        host.require_all_active(ATTACH_GRANTS).await?;
        let repository = attach_repository(
            &self.repository,
            self.run_id,
            &self.installation.extension_id,
            input.record_id,
        )
        .await?;
        let filename = sanitize_supplied_filename(&input.filename)
            .ok_or_else(|| "the file name is not usable".to_owned())?;
        let artifact = self
            .repository
            .extension_operation_input_artifact(
                self.run_id,
                &self.installation.extension_id,
                self.installation.installed_release_id,
                &input.artifact_id.to_string(),
            )
            .await
            .map_err(|error| error.to_string())?;
        let length = artifact.content_length;
        let (Some(object_key), Some(sha256)) = (
            artifact.object_key.as_deref(),
            artifact.checksum_sha256.as_deref(),
        ) else {
            return Err("the input artifact is not complete".into());
        };
        if length <= 0 || length > MAX_ATTACHED_FILE_BYTES {
            return Err(format!(
                "input artifacts up to {MAX_ATTACHED_FILE_BYTES} bytes can be attached"
            ));
        }
        let (policy, current) = repository
            .file_upload_target_files(input.record_id, &input.attribute_code, input.context_id)
            .await
            .map_err(|error| error.to_string())?;
        if let Some(file) = current
            .iter()
            .find(|file| file.sha256 == sha256 && file.filename == filename)
        {
            return bounded_serialize(&json!({"file": file, "replayed": true}));
        }
        let object = self
            .object_store
            .get(object_key)
            .await
            .map_err(|_| "file storage unavailable")?;
        let bytes: Bytes = object.bytes;
        let valid_text = std::str::from_utf8(&bytes).is_ok_and(|text| !text.contains('\0'));
        let mime = detect_mime(&bytes, &filename, valid_text)
            .ok_or_else(|| "the file type is not supported".to_owned())?;
        if !policy.allows(mime, &filename, bytes.len() as u64) {
            return Err(format!(
                "attribute {} does not accept this file",
                input.attribute_code
            ));
        }
        let key = format!("{FILE_OBJECT_PREFIX}{}", Uuid::new_v4());
        // The intent is committed before the object is written, so an
        // attachment that never commits still has its object cleaned up.
        let cleanup_after = chrono::Utc::now() + chrono::Duration::hours(ATTACH_CLEANUP_HOURS);
        self.repository
            .begin_file_uploads(std::slice::from_ref(&key), cleanup_after)
            .await
            .map_err(|error| error.to_string())?;
        let byte_size = bytes.len() as u64;
        self.object_store
            .put(
                &key,
                StoredObject {
                    bytes,
                    content_type: Some(mime.to_owned()),
                },
            )
            .await
            .map_err(|_| "file storage unavailable")?;
        let result = repository
            .persist_uploaded_files(
                input.record_id,
                &input.attribute_code,
                input.context_id,
                vec![NewUploadedFile {
                    original_filename: filename.clone(),
                    display_filename: filename,
                    mime_type: mime.to_owned(),
                    byte_size,
                    sha256: sha256.to_owned(),
                    object_key: key,
                }],
            )
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&json!({"file": result.files.first(), "replayed": false}))
    }
}

/// The repository an attachment writes through, after checking the run
/// may write `record_id`: a selection run writes only its selected
/// records, as their initiator; connector and blueprint runs write no
/// files; other runs write as the extension. It borrows only the
/// repository, which is Sync.
async fn attach_repository(
    repository: &AttricatRepository,
    run_id: Uuid,
    extension_id: &str,
    record_id: Uuid,
) -> Result<AttricatRepository, String> {
    if repository
        .connector_run_scope(run_id)
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("connector runs cannot write record files".into());
    }
    let Some(scope) = repository
        .interactive_run_scope(run_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(repository.for_extension(extension_id));
    };
    if scope.scope != InteractiveScope::Selection {
        return Err("blueprint runs cannot write record files".into());
    }
    if !scope.record_ids.contains(&record_id) {
        return Err("the record is outside the run selection".into());
    }
    let writable = repository
        .principal_record_ids(
            scope.actor,
            repository.workspace_id(),
            Permission::RecordsWrite,
            &[record_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    if !writable.contains(&record_id) {
        return Err("the run's initiator cannot change the record".into());
    }
    Ok(repository.for_interactive_run(extension_id, run_id, &scope))
}
