use super::{
    AppState,
    data_health::invalidate_data_health,
    error::ApiError,
    extractors::{ApiJson, ApiPath},
};
use crate::{
    file_access::{FileAccessDecision, FileAccessOperation, authorize_file_read},
    repository::{AVATAR_VARIANT_KIND, CatalogRepository, FileObject, FilePolicy, NewUploadedFile},
    storage::ObjectStoreError,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
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

use catalog_validation::files::{detect_mime as detected_mime, is_supported_upload_mime};

/// Avatars accept only these formats; the file worker re-encodes them.
const AVATAR_MIME_TYPES: &[&str] = &["image/png", "image/jpeg"];
/// Avatars are small once processed, so originals are capped well below the
/// general upload limit.
const AVATAR_MAX_BYTES: u64 = 10 * 1024 * 1024;

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
            .map_err(|error| multipart_error(error, "multipart body is malformed"))?
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
                    .map_err(|error| multipart_error(error, "context_id is invalid"))?
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
        reserve_upload_keys(
            &state,
            &repository,
            uploads.iter().map(|(key, _)| key.clone()).collect(),
        )
        .await?;
        for (file, (key, mime)) in staged.iter().zip(&uploads) {
            // Stream from disk; the durable intent owns cleanup on cancellation.
            if let Err(error) = state
                .object_store
                .put_file(key, &file.path, Some(mime))
                .await
            {
                cleanup(&staged).await;
                return Err(storage_error(error));
            }
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
                invalidate_data_health(&state, &repository);
                Ok((StatusCode::CREATED, Json(result)))
            }
            Err(error) => Err(error.into()),
        }
    }
    .instrument(span)
    .await;
    if result.is_err() {
        metrics::counter!("catalog_file_uploads_total", "outcome" => "rejected").increment(1);
    }
    result
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateFileReferences {
    context_id: Option<Uuid>,
    expected_file_ids: Vec<Uuid>,
    file_ids: Vec<Uuid>,
}

pub(super) async fn update_references(
    State(state): State<AppState>,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath((entity_id, attribute_code)): ApiPath<(Uuid, String)>,
    ApiJson(input): ApiJson<UpdateFileReferences>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, FileAccessOperation::UpdateReferences { entity_id }).await?;
    let entity_updated_at = repository
        .update_file_references(
            entity_id,
            &attribute_code,
            input.context_id,
            &input.expected_file_ids,
            &input.file_ids,
        )
        .await?;
    invalidate_data_health(&state, &repository);
    Ok(Json(
        serde_json::json!({ "entity_updated_at": entity_updated_at }),
    ))
}

/// Stores standalone files for a conversation. It deliberately shares the
/// streaming, signature validation, object-store, and processing pipeline used
/// by entity file attributes; only attribute-value persistence is omitted.
pub(super) async fn upload_conversation(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user_id, _): super::auth::AuthenticatedPrincipal,
    super::auth::ActiveWorkspace(workspace): super::auth::ActiveWorkspace,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    ApiPath(conversation_id): ApiPath<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    authorize(
        &state,
        FileAccessOperation::ConversationUpload { conversation_id },
    )
    .await?;
    super::agents::readable_conversation(&repository, user_id, workspace, conversation_id).await?;
    let mut staged = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| multipart_error(error, "multipart body is malformed"))?
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
    reserve_upload_keys(
        &state,
        &repository,
        uploads.iter().map(|(key, _)| key.clone()).collect(),
    )
    .await?;
    for (file, (key, mime)) in staged.iter().zip(&uploads) {
        if let Err(error) = state
            .object_store
            .put_file(key, &file.path, Some(mime))
            .await
        {
            cleanup(&staged).await;
            return Err(storage_error(error));
        }
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
            invalidate_data_health(&state, &repository);
            Ok((
                StatusCode::CREATED,
                Json(serde_json::json!({ "files": files })),
            ))
        }
        Err(error) => Err(error.into()),
    }
}

/// Replaces the caller's avatar in the active workspace. The upload shares the
/// streaming, signature validation, object-store and processing pipeline used
/// by other uploads; the file worker produces the square `avatar` variant.
pub(super) async fn upload_avatar(
    State(state): State<AppState>,
    super::auth::AuthenticatedPrincipal(user_id, _): super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<crate::repository::OwnAvatar>), ApiError> {
    authorize(&state, FileAccessOperation::AvatarUpload { user_id }).await?;
    let mut staged: Option<StagedFile> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| multipart_error(error, "multipart body is malformed"))?
    {
        if field.name() != Some("file") {
            return Err(ApiError::invalid_file("multipart field must be file"));
        }
        if staged.is_some() {
            return Err(ApiError::invalid_file("an avatar is exactly one file"));
        }
        let original_filename = field
            .file_name()
            .ok_or_else(|| ApiError::invalid_file("file name is required"))?
            .to_owned();
        let display_filename = sanitize_filename(&original_filename)
            .ok_or_else(|| ApiError::invalid_file("file name is invalid"))?;
        let declared_mime = field.content_type().map(ToString::to_string);
        staged = Some(
            stage_field(
                field,
                original_filename,
                display_filename,
                declared_mime,
                state.max_upload_file_bytes.min(AVATAR_MAX_BYTES),
            )
            .await?,
        );
    }
    // Dropping `staged` removes its temporary file on every return path.
    let file = staged.ok_or_else(|| ApiError::invalid_file("a file is required"))?;
    let mime = detected_mime(&file.signature, &file.display_filename, file.valid_text)
        .filter(|mime| AVATAR_MIME_TYPES.contains(mime))
        .filter(|mime| declared_mime_matches(file.declared_mime.as_deref(), mime))
        .ok_or_else(ApiError::unsupported_media_type)?;
    let key = format!("files/{}", Uuid::new_v4());
    reserve_upload_keys(&state, &repository, vec![key.clone()]).await?;
    state
        .object_store
        .put_file(&key, &file.path, Some(mime))
        .await
        .map_err(storage_error)?;
    let result = repository
        .persist_avatar_upload(
            user_id,
            NewUploadedFile {
                original_filename: file.original_filename.clone(),
                display_filename: file.display_filename.clone(),
                mime_type: mime.to_owned(),
                byte_size: file.byte_size,
                sha256: file.sha256.clone(),
                object_key: key.clone(),
            },
        )
        .await;
    match result {
        Ok(avatar) => {
            metrics::counter!("catalog_file_uploads_total", "outcome" => "success").increment(1);
            invalidate_data_health(&state, &repository);
            Ok((StatusCode::CREATED, Json(avatar)))
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) async fn delete_avatar(
    super::auth::AuthenticatedPrincipal(user_id, _): super::auth::AuthenticatedPrincipal,
    super::auth::ScopedRepository(repository): super::auth::ScopedRepository,
) -> Result<StatusCode, ApiError> {
    repository.clear_avatar(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
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
    // Members may see each other's processed avatars. The original upload is
    // never served this way: it may carry EXIF metadata such as location.
    if kind == AVATAR_VARIANT_KIND && repository.is_member_avatar(file_id).await? {
        authorize(&state, FileAccessOperation::AvatarRead { file_id }).await?;
        return download(
            &state,
            repository.file_object(file_id, Some(&kind)).await?,
            &headers,
        )
        .await;
    }
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
        let range = match parse_range(
            headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok()),
            total,
        ) {
            Ok(range) => range,
            Err(error) => {
                let mut response = error.into_response();
                response
                    .headers_mut()
                    .insert(header::CONTENT_RANGE, unsatisfied_range(total));
                return Ok(response);
            }
        };
        let (status, content_length) = match range {
            Some((start, end)) => (StatusCode::PARTIAL_CONTENT, end - start + 1),
            None => (StatusCode::OK, total),
        };
        let provider_range = range.map(|(start, end)| format!("bytes={start}-{end}"));
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
        response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(content_length));
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
        if let Some((start, end)) = range {
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

/// Parses a single RFC 9110 byte range. `Ok(None)` means the full
/// representation is served: no header, a non-`bytes` unit, a multi-range
/// request (unsupported, so ignored as the RFC permits), or an empty object.
fn parse_range(value: Option<&str>, total: usize) -> Result<Option<(usize, usize)>, ApiError> {
    let Some(range) = value.and_then(|value| value.strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if range.contains(',') || total == 0 {
        return Ok(None);
    }
    let Some((start, end)) = range.trim().split_once('-') else {
        return Err(ApiError::invalid_range());
    };
    let range = match (start, end) {
        ("", suffix) => match suffix.parse::<usize>() {
            Ok(length) if length > 0 => (total.saturating_sub(length), total - 1),
            _ => return Err(ApiError::invalid_range()),
        },
        (start, end) => {
            let start = start
                .parse::<usize>()
                .ok()
                .filter(|start| *start < total)
                .ok_or_else(ApiError::invalid_range)?;
            if end.is_empty() {
                (start, total - 1)
            } else {
                match end.parse::<usize>() {
                    Ok(end) if end >= start => (start, end.min(total - 1)),
                    _ => return Err(ApiError::invalid_range()),
                }
            }
        }
    };
    Ok(Some(range))
}

/// Body-limit breaches surface as multipart stream errors; report them as 413.
fn multipart_error(
    error: axum::extract::multipart::MultipartError,
    message: &'static str,
) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::payload_too_large()
    } else {
        ApiError::invalid_file(message)
    }
}

fn unsatisfied_range(total: usize) -> HeaderValue {
    HeaderValue::from_str(&format!("bytes */{total}")).expect("range is valid")
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
        .map_err(|error| multipart_error(error, "multipart file stream is malformed"))?
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
fn declared_mime_matches(declared: Option<&str>, detected: &str) -> bool {
    let declared = declared
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    matches!(declared, None | Some("application/octet-stream")) || declared == Some(detected)
}
fn allowed_by_policy(policy: &FilePolicy, mime: &str, filename: &str, size: u64) -> bool {
    policy.allows(mime, filename, size)
}
async fn cleanup(files: &[StagedFile]) {
    for file in files {
        let _ = fs::remove_file(&file.path).await;
    }
}
async fn reserve_upload_keys(
    state: &AppState,
    repository: &CatalogRepository,
    keys: Vec<String>,
) -> Result<(), ApiError> {
    // A full request deadline plus an hour permits late completion at the S3
    // boundary without racing cleanup. Cancellation needs no async destructor.
    let grace = chrono::Duration::from_std(state.request_timeout)
        .ok()
        .and_then(|duration| duration.checked_add(&chrono::Duration::hours(1)))
        .and_then(|duration| chrono::Utc::now().checked_add_signed(duration))
        .ok_or_else(|| ApiError::internal("upload deadline is invalid"))?;
    repository.begin_file_uploads(&keys, grace).await?;
    Ok(())
}
fn storage_error(error: ObjectStoreError) -> ApiError {
    tracing::error!(error = %error, "object storage operation failed");
    match error {
        ObjectStoreError::Unavailable | ObjectStoreError::TimedOut(_) => {
            ApiError::storage_unavailable()
        }
        ObjectStoreError::NotFound | ObjectStoreError::Operation(_) => {
            ApiError::internal("object storage operation failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalog_validation::files::SUPPORTED_UPLOAD_MIME_TYPES;

    #[test]
    fn byte_ranges_follow_rfc_9110() {
        let range = |value| parse_range(Some(value), 10).map_err(|error| error.status());
        assert_eq!(parse_range(None, 10).unwrap(), None);
        assert_eq!(range("bytes=2-4"), Ok(Some((2, 4))));
        assert_eq!(range("bytes=2-"), Ok(Some((2, 9))));
        assert_eq!(range("bytes=5-99"), Ok(Some((5, 9))));
        assert_eq!(range("bytes=-3"), Ok(Some((7, 9))));
        assert_eq!(range("bytes=-99"), Ok(Some((0, 9))));
        // Unsupported forms are ignored and the full representation served.
        assert_eq!(range("items=0-1"), Ok(None));
        assert_eq!(range("bytes=0-1,4-5"), Ok(None));
        for unsatisfiable in [
            "bytes=10-",
            "bytes=4-2",
            "bytes=-0",
            "bytes=abc-5",
            "bytes=5",
        ] {
            assert_eq!(
                range(unsatisfiable),
                Err(StatusCode::RANGE_NOT_SATISFIABLE),
                "{unsatisfiable}"
            );
        }
        assert_eq!(parse_range(Some("bytes=0-"), 0).unwrap(), None);
    }

    fn policy() -> FilePolicy {
        FilePolicy {
            cardinality: "many".to_owned(),
            ordered: true,
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
            ordered: true,
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
