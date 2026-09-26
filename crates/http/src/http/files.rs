use super::{AppState, data_health::invalidate_data_health, error::ApiError, extractors::ApiPath};
use crate::{
    file_access::{FileAccessDecision, FileAccessOperation, authorize_file_read},
    repository::{CatalogRepository, FileObject, FilePolicy, NewUploadedFile},
    storage::ObjectStoreError,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};
use sha2::{Digest, Sha256};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};
use tokio::{fs, io::AsyncWriteExt};
use tracing::{Instrument, info_span};
use uuid::Uuid;

/// Maximum number of leading bytes retained while streaming an upload for
/// signature-based MIME detection. All supported signatures fit within this
/// prefix, so the remainder can be written directly to temporary storage.
const SIGNATURE_SNIFF_BYTES: usize = 512;
const MAX_CONTEXT_ID_BYTES: usize = 64;

/// MIME types accepted by every upload entry point. Attribute policies may
/// further restrict this set by MIME group, extension, size, or image-only.
const SUPPORTED_UPLOAD_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "application/pdf",
    "text/plain",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

/// Owns the staged path even if a multipart stream fails or the request is
/// cancelled before the handlers reach their explicit cleanup path.
struct TempUpload(PathBuf);

impl Deref for TempUpload {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TempUpload {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempUpload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct StagedFile {
    original_filename: String,
    display_filename: String,
    declared_mime: Option<String>,
    path: TempUpload,
    byte_size: u64,
    sha256: String,
    signature: Vec<u8>,
    valid_text: bool,
}

#[derive(Default)]
struct TextSniff {
    incomplete: Vec<u8>,
    invalid: bool,
}

impl TextSniff {
    fn feed(&mut self, chunk: &[u8]) {
        if self.invalid {
            return;
        }
        if chunk.contains(&0) {
            self.invalid = true;
            return;
        }
        if self.incomplete.is_empty() {
            self.validate(chunk);
        } else {
            // At most three bytes from a code point split between chunks.
            let mut joined = std::mem::take(&mut self.incomplete);
            joined.extend_from_slice(chunk);
            self.validate(&joined);
        }
    }

    fn validate(&mut self, bytes: &[u8]) {
        if let Err(error) = std::str::from_utf8(bytes) {
            if error.error_len().is_some() {
                self.invalid = true;
            } else {
                self.incomplete
                    .extend_from_slice(&bytes[error.valid_up_to()..]);
            }
        }
    }

    fn valid(&self) -> bool {
        !self.invalid && self.incomplete.is_empty()
    }
}

pub(super) async fn upload(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((entity_id, attribute_code)): ApiPath<(Uuid, String)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<crate::repository::FileUploadResult>), ApiError> {
    let span = info_span!("file.upload", %entity_id, attribute_code = %attribute_code);
    let result = async {
        authorize(&state, FileAccessOperation::Upload { entity_id }).await?;
        let mut context_id = None;
        let mut staged = Vec::new();
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|_| ApiError::invalid_file("multipart body is malformed"))?
        {
            let name = field.name().unwrap_or_default().to_owned();
            if name == "context_id" {
                if context_id.is_some() {
                    cleanup(&staged).await;
                    return Err(ApiError::invalid_file("context_id may appear only once"));
                }
                let mut field = field;
                let mut bytes = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|_| ApiError::invalid_file("context_id is invalid"))?
                {
                    if bytes.len().saturating_add(chunk.len()) > MAX_CONTEXT_ID_BYTES {
                        return Err(ApiError::invalid_file("context_id is invalid"));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                context_id = Some(
                    std::str::from_utf8(&bytes)
                        .map_err(|_| ApiError::invalid_file("context_id is invalid"))?
                        .parse()
                        .map_err(|_| ApiError::invalid_file("context_id is invalid"))?,
                );
                continue;
            }
            if name != "file" && name != "files" {
                cleanup(&staged).await;
                return Err(ApiError::invalid_file(
                    "multipart fields must be file or files",
                ));
            }
            if staged.len() >= state.max_upload_files {
                cleanup(&staged).await;
                return Err(ApiError::file_count_exceeded());
            }
            let original_filename = field
                .file_name()
                .ok_or_else(|| ApiError::invalid_file("file name is required"))?
                .to_owned();
            let display_filename = sanitize_filename(&original_filename)
                .ok_or_else(|| ApiError::invalid_file("file name is invalid"))?;
            let declared_mime = field.content_type().map(ToString::to_string);
            match stage_field(
                field,
                original_filename,
                display_filename,
                declared_mime,
                state.max_upload_file_bytes,
            )
            .await
            {
                Ok(file) => staged.push(file),
                Err(error) => {
                    cleanup(&staged).await;
                    return Err(error);
                }
            }
        }
        if staged.is_empty() {
            return Err(ApiError::invalid_file("at least one file is required"));
        }
        let policy = match repository
            .file_upload_policy(entity_id, &attribute_code, context_id)
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                cleanup(&staged).await;
                return Err(error.into());
            }
        };
        if policy.cardinality == "one" && staged.len() != 1 {
            cleanup(&staged).await;
            return Err(ApiError::invalid_file(
                "single-file attributes accept exactly one file",
            ));
        }
        let mut uploads = Vec::with_capacity(staged.len());
        for file in &staged {
            let Some(mime) =
                detected_mime(&file.signature, &file.display_filename, file.valid_text)
            else {
                cleanup(&staged).await;
                return Err(ApiError::unsupported_media_type());
            };
            if policy.max_bytes.is_some_and(|limit| file.byte_size > limit) {
                cleanup(&staged).await;
                return Err(ApiError::file_too_large());
            }
            if !declared_mime_matches(file.declared_mime.as_deref(), mime)
                || !allowed_by_policy(&policy, mime, &file.display_filename, file.byte_size)
            {
                cleanup(&staged).await;
                return Err(ApiError::unsupported_media_type());
            }
            uploads.push((format!("files/{}", Uuid::new_v4()), mime.to_owned()));
        }
        let mut written_keys = Vec::new();
        for (file, (key, mime)) in staged.iter().zip(&uploads) {
            // Multipart chunks are streamed to a private temporary file; handlers
            // never call Field::bytes or collect the multipart request.
            if let Err(error) = state
                .object_store
                .put_file(key, &file.path, Some(mime))
                .await
            {
                delete_objects(&state, &written_keys).await;
                cleanup(&staged).await;
                return Err(storage_error(error));
            }
            written_keys.push(key.clone());
        }
        let records = staged
            .iter()
            .zip(&uploads)
            .map(|(file, (key, mime))| NewUploadedFile {
                original_filename: file.original_filename.clone(),
                display_filename: file.display_filename.clone(),
                mime_type: mime.clone(),
                byte_size: file.byte_size,
                sha256: file.sha256.clone(),
                object_key: key.clone(),
            })
            .collect();
        let result = repository
            .persist_uploaded_files(entity_id, &attribute_code, context_id, records)
            .await;
        cleanup(&staged).await;
        match result {
            Ok(result) => {
                metrics::counter!("catalog_file_uploads_total", "outcome" => "success")
                    .increment(1);
                invalidate_data_health(&state).await;
                Ok((StatusCode::CREATED, Json(result)))
            }
            Err(error) => {
                delete_objects(&state, &written_keys).await;
                Err(error.into())
            }
        }
    }
    .instrument(span)
    .await;
    if result.is_err() {
        metrics::counter!("catalog_file_uploads_total", "outcome" => "rejected").increment(1);
    }
    result
}

/// Stores standalone files for a conversation. It deliberately shares the
/// streaming, signature validation, object-store, and processing pipeline used
/// by entity file attributes; only attribute-value persistence is omitted.
pub(super) async fn upload_conversation(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user_id, _): super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    authorize(
        &state,
        FileAccessOperation::ConversationUpload { conversation_id },
    )
    .await?;
    repository.get_conversation(conversation_id).await?;
    let mut staged = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::invalid_file("multipart body is malformed"))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if name != "file" && name != "files" {
            cleanup(&staged).await;
            return Err(ApiError::invalid_file(
                "multipart fields must be file or files",
            ));
        }
        if staged.len() >= state.max_upload_files {
            cleanup(&staged).await;
            return Err(ApiError::file_count_exceeded());
        }
        let original_filename = field
            .file_name()
            .ok_or_else(|| ApiError::invalid_file("file name is required"))?
            .to_owned();
        let display_filename = sanitize_filename(&original_filename)
            .ok_or_else(|| ApiError::invalid_file("file name is invalid"))?;
        let declared_mime = field.content_type().map(ToString::to_string);
        match stage_field(
            field,
            original_filename,
            display_filename,
            declared_mime,
            state.max_upload_file_bytes,
        )
        .await
        {
            Ok(file) => staged.push(file),
            Err(error) => {
                cleanup(&staged).await;
                return Err(error);
            }
        }
    }
    if staged.is_empty() {
        return Err(ApiError::invalid_file("at least one file is required"));
    }
    let mut uploads = Vec::with_capacity(staged.len());
    for file in &staged {
        let Some(mime) = detected_mime(&file.signature, &file.display_filename, file.valid_text)
        else {
            cleanup(&staged).await;
            return Err(ApiError::unsupported_media_type());
        };
        if !declared_mime_matches(file.declared_mime.as_deref(), mime)
            || !is_supported_upload_mime(mime)
        {
            cleanup(&staged).await;
            return Err(ApiError::unsupported_media_type());
        }
        uploads.push((format!("files/{}", Uuid::new_v4()), mime.to_owned()));
    }
    let mut written_keys = Vec::new();
    for (file, (key, mime)) in staged.iter().zip(&uploads) {
        if let Err(error) = state
            .object_store
            .put_file(key, &file.path, Some(mime))
            .await
        {
            delete_objects(&state, &written_keys).await;
            cleanup(&staged).await;
            return Err(storage_error(error));
        }
        written_keys.push(key.clone());
    }
    let records = staged
        .iter()
        .zip(&uploads)
        .map(|(file, (key, mime))| NewUploadedFile {
            original_filename: file.original_filename.clone(),
            display_filename: file.display_filename.clone(),
            mime_type: mime.clone(),
            byte_size: file.byte_size,
            sha256: file.sha256.clone(),
            object_key: key.clone(),
        })
        .collect();
    let result = repository
        .persist_conversation_uploads(conversation_id, user_id, records)
        .await;
    cleanup(&staged).await;
    match result {
        Ok(files) => {
            metrics::counter!("catalog_file_uploads_total", "outcome" => "success").increment(1);
            invalidate_data_health(&state).await;
            Ok((
                StatusCode::CREATED,
                Json(serde_json::json!({ "files": files })),
            ))
        }
        Err(error) => {
            delete_objects(&state, &written_keys).await;
            Err(error.into())
        }
    }
}

pub(super) async fn metadata(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(principal, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiPath(file_id): ApiPath<Uuid>,
) -> Result<Json<crate::repository::FileMetadata>, ApiError> {
    authorize_read(
        &state,
        &repository,
        principal,
        workspace,
        file_id,
        |file_id, entity_id, blueprint_id| FileAccessOperation::ReadMetadata {
            file_id,
            entity_id,
            blueprint_id,
        },
    )
    .await?;
    Ok(Json(repository.file_metadata(file_id).await?))
}

pub(super) async fn download_original(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(principal, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiPath(file_id): ApiPath<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    authorize_read(
        &state,
        &repository,
        principal,
        workspace,
        file_id,
        |file_id, entity_id, blueprint_id| FileAccessOperation::DownloadOriginal {
            file_id,
            entity_id,
            blueprint_id,
        },
    )
    .await?;
    download(
        &state,
        repository.file_object(file_id, None).await?,
        &headers,
    )
    .await
}

pub(super) async fn download_variant(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    super::auth::AuthenticatedPrincipal(principal, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    ApiPath((file_id, kind)): ApiPath<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    authorize_read(
        &state,
        &repository,
        principal,
        workspace,
        file_id,
        |file_id, entity_id, blueprint_id| FileAccessOperation::DownloadVariant {
            file_id,
            entity_id,
            blueprint_id,
        },
    )
    .await?;
    download(
        &state,
        repository.file_object(file_id, Some(&kind)).await?,
        &headers,
    )
    .await
}

async fn authorize(state: &AppState, operation: FileAccessOperation) -> Result<(), ApiError> {
    match state.file_access_policy.authorize(operation).await {
        FileAccessDecision::Allow => Ok(()),
        FileAccessDecision::Deny => Err(ApiError::forbidden()),
    }
}

async fn authorize_read<F>(
    state: &AppState,
    repository: &CatalogRepository,
    principal: Uuid,
    workspace: Uuid,
    file_id: Uuid,
    operation: F,
) -> Result<(), ApiError>
where
    F: Fn(Uuid, Uuid, Uuid) -> FileAccessOperation,
{
    authorize_file_read(
        repository,
        state.file_access_policy.as_ref(),
        principal,
        workspace,
        file_id,
        operation,
    )
    .await?
    .then_some(())
    .ok_or_else(ApiError::forbidden)
}

async fn download(
    state: &AppState,
    file: FileObject,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let span = info_span!("file.download");
    async {
        if file.status != "ready" {
            return Err(ApiError::file_processing());
        }
        // Metadata is database-owned: never trust storage-supplied content types.
        let total = file.byte_size as usize;
        let range = parse_range(
            headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok()),
            total,
        )?;
        let (status, start, end) = match range {
            Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
            None => (StatusCode::OK, 0, total - 1),
        };
        let provider_range =
            (status == StatusCode::PARTIAL_CONTENT).then(|| format!("bytes={start}-{end}"));
        let object = state
            .object_store
            .get_range_stream(&file.object_key, provider_range.as_deref())
            .await
            .map_err(storage_error)?;
        let mut response = Response::new(Body::from_stream(object.stream));
        *response.status_mut() = status;
        let response_headers = response.headers_mut();
        response_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&file.mime_type)
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        response_headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&(end - start + 1).to_string()).expect("length is valid"),
        );
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{}\"",
                safe_download_name(&file.display_filename)
            ))
            .expect("sanitized filename is valid"),
        );
        if status == StatusCode::PARTIAL_CONTENT {
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{total}"))
                    .expect("range is valid"),
            );
        }
        metrics::counter!("catalog_file_downloads_total", "outcome" => "success").increment(1);
        Ok(response)
    }
    .instrument(span)
    .await
}

fn parse_range(value: Option<&str>, total: usize) -> Result<Option<(usize, usize)>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(range) = value.strip_prefix("bytes=") else {
        return Err(ApiError::invalid_range());
    };
    if range.contains(',') {
        return Err(ApiError::invalid_range());
    }
    let Some((start, end)) = range.split_once('-') else {
        return Err(ApiError::invalid_range());
    };
    let (start, end) = match (start.parse::<usize>(), end) {
        (Ok(start), "") if start < total => (start, total - 1),
        (Ok(start), end) if start < total => match end.parse::<usize>() {
            Ok(end) if end >= start => (start, end.min(total - 1)),
            _ => return Err(ApiError::invalid_range()),
        },
        (Err(_), end) => match end.parse::<usize>() {
            Ok(length) if length > 0 => (total.saturating_sub(length), total - 1),
            _ => return Err(ApiError::invalid_range()),
        },
        _ => return Err(ApiError::invalid_range()),
    };
    Ok(Some((start, end)))
}

fn safe_download_name(filename: &str) -> String {
    let value: String = filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if !value.is_empty() {
        value
    } else {
        "download".to_owned()
    }
}

async fn stage_field(
    mut field: axum::extract::multipart::Field<'_>,
    original_filename: String,
    display_filename: String,
    declared_mime: Option<String>,
    max_bytes: u64,
) -> Result<StagedFile, ApiError> {
    let path = TempUpload(std::env::temp_dir().join(format!("catalog-upload-{}", Uuid::new_v4())));
    let mut output = fs::File::create(&path)
        .await
        .map_err(|_| ApiError::internal("temporary upload could not be created"))?;
    let mut size = 0u64;
    let mut digest = Sha256::new();
    let mut signature = Vec::with_capacity(SIGNATURE_SNIFF_BYTES);
    let mut text = TextSniff::default();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|_| ApiError::invalid_file("multipart file stream is malformed"))?
    {
        size = size.saturating_add(chunk.len() as u64);
        if size > max_bytes {
            let _ = fs::remove_file(&path).await;
            return Err(ApiError::file_too_large());
        }
        if signature.len() < SIGNATURE_SNIFF_BYTES {
            signature.extend_from_slice(
                &chunk[..chunk.len().min(SIGNATURE_SNIFF_BYTES - signature.len())],
            );
        }
        digest.update(&chunk);
        text.feed(&chunk);
        output
            .write_all(&chunk)
            .await
            .map_err(|_| ApiError::internal("temporary upload could not be written"))?;
    }
    output
        .flush()
        .await
        .map_err(|_| ApiError::internal("temporary upload could not be written"))?;
    if size == 0 {
        let _ = fs::remove_file(&path).await;
        return Err(ApiError::invalid_file("file must not be empty"));
    }
    Ok(StagedFile {
        original_filename,
        display_filename,
        declared_mime,
        path,
        byte_size: size,
        sha256: format!("{:x}", digest.finalize()),
        signature,
        valid_text: text.valid(),
    })
}
fn sanitize_filename(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let clean: String = base
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    (!clean.is_empty() && clean != "." && clean != ".." && clean.len() <= 255).then_some(clean)
}
fn extension(filename: &str) -> Option<String> {
    filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}
fn detected_mime(signature: &[u8], filename: &str, valid_text: bool) -> Option<&'static str> {
    if signature.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if signature.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if signature.starts_with(b"GIF87a") || signature.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if signature.len() >= 12 && &signature[..4] == b"RIFF" && &signature[8..12] == b"WEBP" {
        Some("image/webp")
    } else if signature.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if signature.starts_with(b"PK\x03\x04") {
        match extension(filename).as_deref() {
            Some("docx") => {
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            }
            Some("xlsx") => {
                Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
            }
            Some("pptx") => {
                Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
            }
            _ => None,
        }
    } else if valid_text {
        Some("text/plain")
    } else {
        None
    }
}
fn declared_mime_matches(declared: Option<&str>, detected: &str) -> bool {
    let declared = declared
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    matches!(declared, None | Some("application/octet-stream")) || declared == Some(detected)
}
fn is_supported_upload_mime(mime: &str) -> bool {
    SUPPORTED_UPLOAD_MIME_TYPES.contains(&mime)
}
fn allowed_by_policy(policy: &FilePolicy, mime: &str, filename: &str, size: u64) -> bool {
    if !is_supported_upload_mime(mime)
        || policy.max_bytes.is_some_and(|limit| size > limit)
        || (policy.image_only && !mime.starts_with("image/"))
    {
        return false;
    }
    if !policy.allowed_mime_groups.is_empty()
        && !policy.allowed_mime_groups.iter().any(|group| {
            group == mime
                || group.trim_end_matches("/*") == mime.split('/').next().unwrap_or_default()
                || group == mime.split('/').next().unwrap_or_default()
        })
    {
        return false;
    }
    policy.allowed_extensions.is_empty()
        || extension(filename).is_some_and(|extension| {
            policy.allowed_extensions.iter().any(|allowed| {
                allowed
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(&extension)
            })
        })
}
async fn cleanup(files: &[StagedFile]) {
    for file in files {
        let _ = fs::remove_file(&file.path).await;
    }
}
async fn delete_objects(state: &AppState, keys: &[String]) {
    for key in keys {
        let _ = state.object_store.delete(key).await;
    }
}
fn storage_error(error: ObjectStoreError) -> ApiError {
    tracing::error!(error = %error, "object storage operation failed");
    match error {
        ObjectStoreError::Unavailable | ObjectStoreError::TimedOut(_) => {
            ApiError::storage_unavailable()
        }
        ObjectStoreError::NotFound | ObjectStoreError::Operation(_) => {
            ApiError::internal("object storage upload failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> FilePolicy {
        FilePolicy {
            cardinality: "many".to_owned(),
            allowed_mime_groups: vec!["image/*".to_owned()],
            allowed_extensions: vec!["png".to_owned()],
            max_bytes: Some(1024),
            image_only: true,
        }
    }
    #[test]
    fn detects_signatures_without_trusting_file_name() {
        assert_eq!(
            detected_mime(b"\x89PNG\r\n\x1a\nrest", "not-image.txt", false),
            Some("image/png")
        );
        assert_eq!(
            detected_mime(b"not a known file\0", "image.png", false),
            None
        );
    }
    #[test]
    fn text_sniff_validates_the_whole_stream_across_chunk_boundaries() {
        let mut text = TextSniff::default();
        text.feed(&vec![b'a'; SIGNATURE_SNIFF_BYTES]);
        text.feed(b"\0binary");
        assert!(!text.valid());
        assert_eq!(
            detected_mime(&vec![b'a'; SIGNATURE_SNIFF_BYTES], "fake.txt", text.valid()),
            None
        );

        let mut unicode = TextSniff::default();
        unicode.feed(b"prefix\xf0\x9f");
        unicode.feed(b"\x98\x80");
        assert!(unicode.valid());
        unicode.feed(b"\xff");
        assert!(!unicode.valid());
    }

    #[test]
    fn staged_path_is_removed_when_request_is_dropped() {
        let path = std::env::temp_dir().join(format!("catalog-upload-{}", Uuid::new_v4()));
        std::fs::write(&path, b"partial upload").unwrap();
        drop(TempUpload(path.clone()));
        assert!(!path.exists());
    }

    #[test]
    fn policy_constrains_type_extension_and_size() {
        let policy = policy();
        assert!(allowed_by_policy(&policy, "image/png", "photo.PNG", 1024));
        assert!(!allowed_by_policy(&policy, "image/jpeg", "photo.jpg", 1));
        assert!(!allowed_by_policy(&policy, "image/png", "photo.png", 1025));
    }
    #[test]
    fn standalone_and_unrestricted_attribute_uploads_share_mime_boundaries() {
        let unrestricted = FilePolicy {
            cardinality: "many".to_owned(),
            allowed_mime_groups: vec![],
            allowed_extensions: vec![],
            max_bytes: None,
            image_only: false,
        };

        for mime in SUPPORTED_UPLOAD_MIME_TYPES {
            assert!(is_supported_upload_mime(mime), "{mime} should be accepted");
            assert!(
                allowed_by_policy(&unrestricted, mime, "upload.bin", 1),
                "{mime} should be accepted by an unrestricted attribute"
            );
        }
        for mime in [
            "application/zip",
            "image/svg+xml",
            "application/octet-stream",
        ] {
            assert!(!is_supported_upload_mime(mime), "{mime} should be rejected");
            assert!(
                !allowed_by_policy(&unrestricted, mime, "upload.bin", 1),
                "{mime} should be rejected by an unrestricted attribute"
            );
        }
    }
    #[test]
    fn sanitizes_path_like_file_names() {
        assert_eq!(
            sanitize_filename("../../report.pdf"),
            Some("report.pdf".to_owned())
        );
        assert_eq!(sanitize_filename(".."), None);
    }
}
