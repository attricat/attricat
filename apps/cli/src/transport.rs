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

pub(crate) const EVENT_STREAM_MEDIA_TYPE: &str = "text/event-stream";

const LAST_EVENT_ID_HEADER: &str = "last-event-id";

/// Media type of extension and solution-pack archives.
pub(crate) const ZSTD_MEDIA_TYPE: &str = "application/zstd";

/// The configured client and API base URL every command sends requests through.
pub(crate) struct Api {
    client: Client,
    server: Url,
}

impl Api {
    pub(crate) fn new(client: Client, server: Url) -> Self {
        Self { client, server }
    }

    pub(crate) fn url(&self, path: &str) -> Result<Url, CliError> {
        endpoint(&self.server, path)
    }

    /// Sends a JSON request and returns the unread response, for callers
    /// that need its headers.
    pub(crate) async fn send_raw(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<reqwest::Response, CliError> {
        let mut request = self
            .client
            .request(method, self.url(path)?)
            .timeout(REQUEST_TIMEOUT);
        if let Some(body) = body {
            request = request.json(&body);
        }
        send(request).await
    }

    /// Sends a JSON request and returns the response body unchanged.
    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<String, CliError> {
        raw_response(self.send_raw(method, path, body).await?).await
    }

    /// Sends a JSON request and parses the response.
    pub(crate) async fn send_value(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, CliError> {
        let response = self.send(method, path, body).await?;
        serde_json::from_str(&response).map_err(|_| CliError::InvalidResponse)
    }

    pub(crate) async fn get(&self, path: &str) -> Result<String, CliError> {
        self.send(Method::GET, path, None).await
    }

    pub(crate) async fn get_value(&self, path: &str) -> Result<Value, CliError> {
        self.send_value(Method::GET, path, None).await
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<String, CliError> {
        self.send(Method::DELETE, path, None).await
    }

    /// Posts without a body.
    pub(crate) async fn post(&self, path: &str) -> Result<String, CliError> {
        self.send(Method::POST, path, None).await
    }

    pub(crate) async fn post_json(&self, path: &str, body: Value) -> Result<String, CliError> {
        self.send(Method::POST, path, Some(body)).await
    }

    pub(crate) async fn put_json(&self, path: &str, body: Value) -> Result<String, CliError> {
        self.send(Method::PUT, path, Some(body)).await
    }

    pub(crate) async fn patch_json(&self, path: &str, body: Value) -> Result<String, CliError> {
        self.send(Method::PATCH, path, Some(body)).await
    }

    /// Opens a server-sent event stream, resuming after `last_event_id`.
    /// The stream has no overall timeout; the server bounds its lifetime.
    pub(crate) async fn open_stream(
        &self,
        path: &str,
        last_event_id: Option<&str>,
    ) -> Result<reqwest::Response, CliError> {
        let mut request = self
            .client
            .get(self.url(path)?)
            .header(reqwest::header::ACCEPT, EVENT_STREAM_MEDIA_TYPE);
        if let Some(id) = last_event_id {
            request = request.header(LAST_EVENT_ID_HEADER, id);
        }
        send(request).await
    }

    /// Streams a local file as the raw request body.
    pub(crate) async fn upload(
        &self,
        path: &str,
        file: &Path,
        content_type: &str,
    ) -> Result<String, CliError> {
        let stream = tokio_util::io::ReaderStream::new(tokio::fs::File::open(file).await.map_err(
            |error| CliError::Input(format!("cannot read {}: {error}", file.display())),
        )?);
        let request = self
            .client
            .post(self.url(path)?)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(reqwest::Body::wrap_stream(stream))
            .timeout(TRANSFER_TIMEOUT);
        raw_response(send(request).await?).await
    }

    /// Sends a multipart form with the transfer timeout.
    pub(crate) async fn send_form(
        &self,
        method: Method,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<String, CliError> {
        let request = self
            .client
            .request(method, self.url(path)?)
            .multipart(form)
            .timeout(TRANSFER_TIMEOUT);
        raw_response(send(request).await?).await
    }

    /// Uploads files as `files` parts, with an optional `context_id` part.
    pub(crate) async fn multipart(
        &self,
        path: &str,
        files: Vec<PathBuf>,
        context_id: Option<Uuid>,
    ) -> Result<String, CliError> {
        let mut form = reqwest::multipart::Form::new();
        if let Some(context_id) = context_id {
            form = form.text("context_id", context_id.to_string());
        }
        for file in files {
            form = form.part("files", file_part(&file).await?);
        }
        self.send_form(Method::POST, path, form).await
    }

    /// Streams a response body into `output`, replacing it only on success.
    pub(crate) async fn download(
        &self,
        path: &str,
        output: &Path,
        range: Option<&str>,
    ) -> Result<String, CliError> {
        let mut request = self.client.get(self.url(path)?);
        if let Some(range) = range {
            request = request.header(reqwest::header::RANGE, range);
        }
        let response = send(request.timeout(TRANSFER_TIMEOUT)).await?;
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
        temporary.as_file_mut().sync_all().map_err(|error| {
            CliError::Input(format!("cannot sync {}: {error}", output.display()))
        })?;
        persist_temporary_file(temporary, output, "download")?;
        Ok("null".to_owned())
    }
}

pub(crate) async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, CliError> {
    request
        .send()
        .await
        .map_err(|error| CliError::Transport(error.to_string()))
}

/// A streamed multipart file part named after the file, with a guessed type.
pub(crate) async fn file_part(path: &Path) -> Result<reqwest::multipart::Part, CliError> {
    let length = fs::metadata(path)
        .map_err(|error| CliError::Input(format!("cannot read {}: {error}", path.display())))?
        .len();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::Input("file name must be UTF-8".to_owned()))?
        .to_owned();
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::open(path).await.map_err(|error| {
            CliError::Input(format!("cannot read {}: {error}", path.display()))
        })?);
    reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), length)
        .file_name(filename)
        .mime_str(mime_guess::from_path(path).first_or_octet_stream().as_ref())
        .map_err(|error| CliError::Input(format!("invalid file MIME type: {error}")))
}

/// A `true` query value for a set boolean flag, or none.
pub(crate) fn flag(set: bool) -> Option<String> {
    set.then(|| "true".to_owned())
}

/// Appends the present query parameters to `path`.
pub(crate) fn with_query<'a>(
    path: &str,
    parameters: impl IntoIterator<Item = (&'a str, Option<String>)>,
) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    let mut empty = true;
    for (name, value) in parameters {
        if let Some(value) = value {
            query.append_pair(name, &value);
            empty = false;
        }
    }
    if empty {
        path.to_owned()
    } else {
        format!("{path}?{}", query.finish())
    }
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

/// Path segments URL resolution removes or resolves against their parent.
/// [`segment`] leaves `.` unencoded, so a value that is exactly one of these
/// would otherwise retarget the request (`secret delete ..` would become
/// `DELETE /workspace/`).
const DOT_SEGMENTS: [&str; 2] = [".", ".."];

pub(crate) fn endpoint(server: &Url, path: &str) -> Result<Url, CliError> {
    let route = path.split_once('?').map_or(path, |(route, _)| route);
    if route.split('/').any(|part| DOT_SEGMENTS.contains(&part)) {
        return Err(CliError::Input(
            "a path argument must not be `.` or `..`".to_owned(),
        ));
    }
    let mut base = server.clone();
    let mut base_path = base.path().to_owned();
    if !base_path.ends_with('/') {
        base_path.push('/');
        base.set_path(&base_path);
    }
    base.join(path.trim_start_matches('/'))
        .map_err(|error| CliError::Input(format!("invalid API path: {error}")))
}

/// Bytes percent-encoded in a path segment: everything but RFC 3986
/// unreserved characters. Query values go through [`with_query`] instead.
pub(crate) const SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub(crate) fn segment(value: impl std::fmt::Display) -> String {
    utf8_percent_encode(&value.to_string(), SEGMENT_ENCODE_SET).to_string()
}
