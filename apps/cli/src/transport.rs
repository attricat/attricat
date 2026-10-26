//! HTTP requests, bounded responses and file transfers.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::StreamExt;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::{Client, Method};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::error::CliError;

pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Large artifacts and file uploads are bounded independently from JSON API calls.
pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub(crate) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Media type of extension and solution-pack archives.
pub(crate) const ZSTD_MEDIA_TYPE: &str = "application/zstd";

pub(crate) async fn raw_upload(
    client: &Client,
    server: &Url,
    path: &str,
    file: &PathBuf,
    content_type: &str,
) -> Result<String, CliError> {
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::open(file).await.map_err(|error| {
            CliError::Input(format!("cannot read {}: {error}", file.display()))
        })?);
    raw_response(
        client
            .request(Method::POST, endpoint(server, path)?)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(reqwest::Body::wrap_stream(stream))
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
}

pub(crate) async fn multipart_upload(
    client: &Client,
    server: &Url,
    path: &str,
    files: Vec<PathBuf>,
    context_id: Option<Uuid>,
) -> Result<String, CliError> {
    let mut form = reqwest::multipart::Form::new();
    if let Some(context_id) = context_id {
        form = form.text("context_id", context_id.to_string());
    }
    for path in files {
        let length = fs::metadata(&path)
            .map_err(|error| CliError::Input(format!("cannot read {}: {error}", path.display())))?
            .len();
        let filename = path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| CliError::Input("file name must be UTF-8".to_owned()))?
            .to_owned();
        let stream =
            tokio_util::io::ReaderStream::new(tokio::fs::File::open(&path).await.map_err(
                |error| CliError::Input(format!("cannot read {}: {error}", path.display())),
            )?);
        let part = reqwest::multipart::Part::stream_with_length(
            reqwest::Body::wrap_stream(stream),
            length,
        )
        .file_name(filename)
        .mime_str(
            mime_guess::from_path(&path)
                .first_or_octet_stream()
                .as_ref(),
        )
        .map_err(|error| CliError::Input(format!("invalid file MIME type: {error}")))?;
        form = form.part("files", part);
    }
    raw_response(
        client
            .post(endpoint(server, path)?)
            .multipart(form)
            .timeout(TRANSFER_TIMEOUT)
            .send()
            .await
            .map_err(|error| CliError::Transport(error.to_string()))?,
    )
    .await
}

pub(crate) fn temporary_file(
    path: &Path,
    label: &str,
) -> Result<tempfile::NamedTempFile, CliError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        CliError::Input(format!(
            "cannot create temporary {label} for {}: {error}",
            path.display()
        ))
    })
}

pub(crate) fn persist_temporary_file(
    temporary: tempfile::NamedTempFile,
    path: &Path,
    label: &str,
) -> Result<(), CliError> {
    temporary.persist(path).map_err(|error| {
        CliError::Input(format!(
            "cannot replace {label} {}: {}",
            path.display(),
            error.error
        ))
    })?;
    Ok(())
}

pub(crate) async fn raw_download(
    client: &Client,
    server: &Url,
    path: &str,
    output: &Path,
    range: Option<&str>,
) -> Result<String, CliError> {
    let mut request = client.get(endpoint(server, path)?);
    if let Some(range) = range {
        request = request.header(reqwest::header::RANGE, range);
    }
    let response = request
        .timeout(TRANSFER_TIMEOUT)
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    if !response.status().is_success() {
        return raw_response(response).await;
    }
    let mut temporary = temporary_file(output, "download")?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
        std::io::Write::write_all(temporary.as_file_mut(), &chunk).map_err(|error| {
            CliError::Input(format!("cannot write {}: {error}", output.display()))
        })?;
    }
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| CliError::Input(format!("cannot sync {}: {error}", output.display())))?;
    persist_temporary_file(temporary, output, "download")?;
    Ok("null".to_owned())
}

pub(crate) async fn buffered_response_body(
    response: reqwest::Response,
) -> Result<(reqwest::StatusCode, String), CliError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(CliError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| CliError::Transport(error.to_string()))?;
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(CliError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        status,
        String::from_utf8(bytes).map_err(|_| CliError::InvalidResponse)?,
    ))
}

pub(crate) async fn raw_response(response: reqwest::Response) -> Result<String, CliError> {
    let (status, body) = buffered_response_body(response).await?;
    if status.is_success() {
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok("null".to_owned());
        }
        serde_json::from_str::<Value>(&body).map_err(|_| CliError::InvalidResponse)?;
        Ok(body)
    } else {
        api_error(status, &body)
    }
}

pub(crate) fn api_error(status: reqwest::StatusCode, body: &str) -> Result<String, CliError> {
    let error = serde_json::from_str::<Value>(body).ok();
    Err(CliError::Api {
        status: status.as_u16(),
        code: error
            .as_ref()
            .and_then(|body| body["error"]["code"].as_str())
            .unwrap_or("api_error")
            .to_owned(),
        details: error
            .as_ref()
            .map(|body| body["error"]["details"].clone())
            .filter(|details| !details.is_null()),
        message: error
            .as_ref()
            .and_then(|body| body["error"]["message"].as_str())
            .unwrap_or(body)
            .to_owned(),
    })
}

pub(crate) async fn request(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<String, CliError> {
    let url = endpoint(server, path)?;
    let mut request = client.request(method, url).timeout(REQUEST_TIMEOUT);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))?;
    raw_response(response).await
}

pub(crate) async fn request_value(
    client: &Client,
    server: &Url,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, CliError> {
    let response = request(client, server, method, path, body).await?;
    serde_json::from_str(&response).map_err(|_| CliError::InvalidResponse)
}

pub(crate) fn is_secure_credential_transport(server: &Url) -> bool {
    if server.scheme() == "https" {
        return true;
    }
    match server.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

pub(crate) fn endpoint(server: &Url, path: &str) -> Result<Url, CliError> {
    let mut base = server.clone();
    let mut base_path = base.path().to_owned();
    if !base_path.ends_with('/') {
        base_path.push('/');
        base.set_path(&base_path);
    }
    base.join(path.trim_start_matches('/'))
        .map_err(|error| CliError::Input(format!("invalid API path: {error}")))
}

/// Bytes percent-encoded in a path segment or query value: everything but
/// RFC 3986 unreserved characters.
pub(crate) const SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub(crate) fn segment(value: impl std::fmt::Display) -> String {
    utf8_percent_encode(&value.to_string(), SEGMENT_ENCODE_SET).to_string()
}
