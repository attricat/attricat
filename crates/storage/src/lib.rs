//! S3-compatible object storage boundary.
//!
//! Object keys are intentionally opaque to this module's callers. HTTP and
//! domain code must never expose them to clients.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::Path,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, StreamExt, TryStreamExt};
use http::{HeaderValue, header};
use s3::{Bucket, Region, creds::Credentials, error::S3Error};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tokio::{sync::Mutex, time::timeout};
use url::Url;

const UPLOAD_OPERATION: &str = "upload";
const DOWNLOAD_OPERATION: &str = "download";
const DELETE_OPERATION: &str = "delete";
const READINESS_OPERATION: &str = "readiness";

/// What S3 assumes for an object stored without a content type.
const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

/// Configuration required to connect to an S3-compatible object store.
///
/// All values are deployment configuration. In particular, credentials are not
/// inferred from the ambient AWS environment so a missing deployment setting
/// cannot silently select a different account or bucket. The secret access
/// key arrives separately because it is read from a secret file, not a
/// setting.
#[derive(Clone)]
pub struct StorageConfig {
    endpoint: Url,
    region: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: SecretString,
    force_path_style: bool,
    upload_timeout: Duration,
    download_timeout: Duration,
}

impl StorageConfig {
    pub fn from_values(
        get: impl Fn(&str) -> Option<String>,
        secret_access_key: Option<SecretString>,
    ) -> Result<Self, StorageConfigError> {
        let endpoint = required(&get, "S3_ENDPOINT")?;
        let endpoint = Url::parse(&endpoint).map_err(|_| StorageConfigError::InvalidEndpoint)?;
        if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
            return Err(StorageConfigError::InvalidEndpoint);
        }

        let region = required(&get, "S3_REGION")?;
        let bucket = required(&get, "S3_BUCKET")?;
        let access_key_id = required(&get, "S3_ACCESS_KEY_ID")?;
        let secret_access_key =
            secret_access_key.ok_or(StorageConfigError::MissingSecret("s3_secret_access_key"))?;
        let force_path_style = required(&get, "S3_FORCE_PATH_STYLE")?
            .parse()
            .map_err(|_| StorageConfigError::InvalidBoolean("S3_FORCE_PATH_STYLE"))?;
        let upload_timeout = timeout_setting(&get, "S3_UPLOAD_TIMEOUT_SECONDS")?;
        let download_timeout = timeout_setting(&get, "S3_DOWNLOAD_TIMEOUT_SECONDS")?;

        Ok(Self {
            endpoint,
            region,
            bucket,
            access_key_id,
            secret_access_key,
            force_path_style,
            upload_timeout,
            download_timeout,
        })
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }
}

fn required(
    get: &impl Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<String, StorageConfigError> {
    match get(name).map(|value| value.trim().to_owned()) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(StorageConfigError::Missing(name)),
    }
}

fn timeout_setting(
    get: &impl Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<Duration, StorageConfigError> {
    let seconds = required(get, name)?
        .parse::<u64>()
        .map_err(|_| StorageConfigError::InvalidTimeout(name))?;
    if seconds == 0 {
        return Err(StorageConfigError::InvalidTimeout(name));
    }
    Ok(Duration::from_secs(seconds))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageConfigError {
    #[error("{0} must be set")]
    Missing(&'static str),
    #[error("the {0} secret must be provided")]
    MissingSecret(&'static str),
    #[error("S3_ENDPOINT must be an absolute HTTP or HTTPS URL")]
    InvalidEndpoint,
    #[error("{0} must be `true` or `false`")]
    InvalidBoolean(&'static str),
    #[error("{0} must be a positive number of seconds")]
    InvalidTimeout(&'static str),
}

/// An object returned by the storage boundary. Object metadata is deliberately
/// small; file metadata belongs to the catalog database, not provider headers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredObject {
    pub bytes: Bytes,
    pub content_type: Option<String>,
}

pub(crate) type ObjectByteStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, ObjectStoreError>> + Send>>;

/// A provider response whose bytes remain streamed until the HTTP consumer
/// writes them to the client.
pub struct StoredObjectStream {
    pub stream: ObjectByteStream,
    pub content_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("object was not found in storage")]
    NotFound,
    #[error("object storage is unavailable")]
    Unavailable,
    #[error("object storage {0} timed out")]
    TimedOut(&'static str),
    #[error("object storage {0} failed")]
    Operation(&'static str),
}

/// Provider-neutral object storage operations used by file services.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put(&self, key: &str, object: StoredObject) -> Result<(), ObjectStoreError>;
    /// Upload a staged file without collecting it in the HTTP process.
    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError>;
    async fn get(&self, key: &str) -> Result<StoredObject, ObjectStoreError>;
    /// Fetch an object without materializing it in the API process.
    async fn get_stream(&self, key: &str) -> Result<StoredObjectStream, ObjectStoreError>;
    /// Fetch a requested range through the provider; callers never receive an object URL.
    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObject, ObjectStoreError>;
    /// Fetch a requested range without materializing it in the API process.
    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObjectStream, ObjectStoreError>;
    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError>;
    async fn readiness(&self) -> Result<(), ObjectStoreError>;
}

/// `rust-s3`-backed implementation for AWS S3 and compatible endpoints.
pub struct S3ObjectStore {
    bucket: Box<Bucket>,
    upload_timeout: Duration,
    download_timeout: Duration,
}

impl S3ObjectStore {
    pub fn new(config: StorageConfig) -> Result<Self, ObjectStoreError> {
        let region = Region::Custom {
            region: config.region,
            endpoint: config.endpoint.as_str().trim_end_matches('/').to_owned(),
        };
        let credentials = Credentials::new(
            Some(&config.access_key_id),
            Some(config.secret_access_key.expose_secret()),
            None,
            None,
            None,
        )
        .map_err(|_| ObjectStoreError::Unavailable)?;
        let mut bucket = Bucket::new(&config.bucket, region, credentials)
            .map_err(|_| ObjectStoreError::Unavailable)?;
        if config.force_path_style {
            bucket = bucket.with_path_style();
        }
        // The operations below bound each request and each streamed chunk
        // themselves; a whole-request deadline would cut off long downloads.
        bucket.set_request_timeout(None);
        Ok(Self {
            bucket,
            upload_timeout: config.upload_timeout,
            download_timeout: config.download_timeout,
        })
    }

    async fn send_upload<T>(
        &self,
        future: impl Future<Output = Result<T, S3Error>>,
    ) -> Result<T, ObjectStoreError> {
        let result = timeout(self.upload_timeout, future)
            .await
            .map_err(|_| ObjectStoreError::TimedOut(UPLOAD_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(UPLOAD_OPERATION)));
        record_operation(
            UPLOAD_OPERATION,
            if result.is_ok() { "success" } else { "failure" },
        );
        result
    }

    /// The bucket handle for one download, carrying the `Range` header when a
    /// range is requested. The header is signed with the request.
    fn download_bucket(&self, range: Option<&str>) -> Result<Cow<'_, Bucket>, ObjectStoreError> {
        let Some(range) = range else {
            return Ok(Cow::Borrowed(&self.bucket));
        };
        let range = HeaderValue::from_str(range)
            .map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION))?;
        let mut headers = self.bucket.extra_headers.clone();
        headers.insert(header::RANGE, range);
        self.bucket
            .with_extra_headers(headers)
            .map(Cow::Owned)
            .map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION))
    }
}

/// HTTP 404, which S3 answers for a missing key.
const HTTP_NOT_FOUND: u16 = 404;

fn classify_download_error(error: S3Error) -> ObjectStoreError {
    match error {
        S3Error::HttpFailWithBody(HTTP_NOT_FOUND, _) => ObjectStoreError::NotFound,
        _ => ObjectStoreError::Operation(DOWNLOAD_OPERATION),
    }
}

/// Read at most `expected_size + 1` bytes. Callers compare the returned exact size and
/// digest; the extra byte makes oversized replacement objects observable without an
/// unbounded provider body collection.
pub async fn get_object_for_integrity(
    store: &dyn ObjectStore,
    key: &str,
    expected_size: usize,
) -> Result<StoredObject, ObjectStoreError> {
    let local_limit = expected_size
        .checked_add(1)
        .ok_or(ObjectStoreError::Operation(DOWNLOAD_OPERATION))?;
    let range = format!("bytes=0-{expected_size}");
    let StoredObjectStream {
        mut stream,
        content_type,
    } = store.get_range_stream(key, Some(&range)).await?;
    let mut bytes = Vec::with_capacity(local_limit);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let remaining = local_limit - bytes.len();
        if chunk.len() >= remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(StoredObject {
        bytes: Bytes::from(bytes),
        content_type,
    })
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, object: StoredObject) -> Result<(), ObjectStoreError> {
        let content_type = object
            .content_type
            .as_deref()
            .unwrap_or(DEFAULT_CONTENT_TYPE);
        self.send_upload(
            self.bucket
                .put_object_with_content_type(key, &object.bytes, content_type),
        )
        .await
        .map(|_| ())
    }

    /// Objects above the library's 8 MiB part size upload as a multipart
    /// upload, which is aborted when a part fails.
    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        let mut file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(_) => {
                record_operation(UPLOAD_OPERATION, "failure");
                return Err(ObjectStoreError::Operation(UPLOAD_OPERATION));
            }
        };
        self.send_upload(self.bucket.put_object_stream_with_content_type(
            &mut file,
            key,
            content_type.unwrap_or(DEFAULT_CONTENT_TYPE),
        ))
        .await
        .map(|_| ())
    }

    async fn get(&self, key: &str) -> Result<StoredObject, ObjectStoreError> {
        self.get_range(key, None).await
    }

    async fn get_stream(&self, key: &str) -> Result<StoredObjectStream, ObjectStoreError> {
        self.get_range_stream(key, None).await
    }

    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObject, ObjectStoreError> {
        // The response body is read before the request completes, so one
        // deadline covers both.
        let result = match self.download_bucket(range) {
            Ok(bucket) => timeout(self.download_timeout, bucket.get_object(key))
                .await
                .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
                .and_then(|result| result.map_err(classify_download_error)),
            Err(error) => Err(error),
        };
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let response = result?;
        let content_type = response.headers().remove(header::CONTENT_TYPE.as_str());
        record_operation(DOWNLOAD_OPERATION, "success");
        Ok(StoredObject {
            bytes: response.into_bytes(),
            content_type,
        })
    }

    /// Streamed objects carry no content type; the catalog owns file metadata.
    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObjectStream, ObjectStoreError> {
        let result = match self.download_bucket(range) {
            Ok(bucket) => timeout(self.download_timeout, bucket.get_object_stream(key))
                .await
                .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
                .and_then(|result| result.map_err(classify_download_error)),
            Err(error) => Err(error),
        };
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let response = result?;
        let download_timeout = self.download_timeout;
        let stream = async_stream::stream! {
            let mut body = response.bytes;
            loop {
                match timeout(download_timeout, body.try_next()).await {
                    Ok(Ok(Some(bytes))) => yield Ok(bytes),
                    Ok(Ok(None)) => {
                        record_operation(DOWNLOAD_OPERATION, "success");
                        break;
                    }
                    Ok(Err(_)) => {
                        record_operation(DOWNLOAD_OPERATION, "failure");
                        yield Err(ObjectStoreError::Operation(DOWNLOAD_OPERATION));
                        break;
                    }
                    Err(_) => {
                        record_operation(DOWNLOAD_OPERATION, "failure");
                        yield Err(ObjectStoreError::TimedOut(DOWNLOAD_OPERATION));
                        break;
                    }
                }
            }
        };
        Ok(StoredObjectStream {
            stream: Box::pin(stream),
            content_type: None,
        })
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        let result = timeout(self.upload_timeout, self.bucket.delete_object(key))
            .await
            .map_err(|_| ObjectStoreError::TimedOut(DELETE_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DELETE_OPERATION)));
        if result.is_err() {
            record_operation(DELETE_OPERATION, "failure");
        }
        result?;
        record_operation(DELETE_OPERATION, "success");
        Ok(())
    }

    /// Lists at most one key. `rust-s3` has no HEAD bucket call; listing
    /// needs the same `s3:ListBucket` permission.
    async fn readiness(&self) -> Result<(), ObjectStoreError> {
        let result = timeout(
            self.download_timeout,
            self.bucket
                .list_page(String::new(), None, None, None, Some(1)),
        )
        .await
        .map_err(|_| ObjectStoreError::TimedOut(READINESS_OPERATION))?
        .map_err(|_| ObjectStoreError::Operation(READINESS_OPERATION));
        record_readiness(result.is_ok());
        result.map(|_| ())
    }
}

fn record_operation(operation: &'static str, outcome: &'static str) {
    metrics::counter!("attricat_object_store_operations_total", "operation" => operation, "outcome" => outcome)
        .increment(1);
}

fn record_readiness(ready: bool) {
    record_operation(
        READINESS_OPERATION,
        if ready { "success" } else { "failure" },
    );
    metrics::gauge!("attricat_object_store_ready").set(if ready { 1.0 } else { 0.0 });
}

/// Deterministic in-memory storage for tests. It is intentionally not enabled
/// by configuration: production startup always validates the configured bucket.
#[derive(Default)]
pub struct FakeObjectStore {
    objects: Mutex<BTreeMap<String, StoredObject>>,
    available: AtomicBool,
}

impl FakeObjectStore {
    pub fn available() -> Self {
        Self {
            objects: Mutex::new(BTreeMap::new()),
            available: AtomicBool::new(true),
        }
    }

    pub fn set_available(&self, available: bool) {
        self.available.store(available, Ordering::Relaxed);
    }

    pub async fn object_count(&self) -> usize {
        self.objects.lock().await.len()
    }

    fn ensure_available(&self) -> Result<(), ObjectStoreError> {
        self.available
            .load(Ordering::Relaxed)
            .then_some(())
            .ok_or(ObjectStoreError::Unavailable)
    }
}

#[async_trait]
impl ObjectStore for FakeObjectStore {
    async fn put(&self, key: &str, object: StoredObject) -> Result<(), ObjectStoreError> {
        if let Err(error) = self.ensure_available() {
            record_operation(UPLOAD_OPERATION, "failure");
            return Err(error);
        }
        self.objects.lock().await.insert(key.to_owned(), object);
        record_operation(UPLOAD_OPERATION, "success");
        Ok(())
    }

    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|_| ObjectStoreError::Operation(UPLOAD_OPERATION))?;
        self.put(
            key,
            StoredObject {
                bytes: Bytes::from(bytes),
                content_type: content_type.map(str::to_owned),
            },
        )
        .await
    }

    async fn get(&self, key: &str) -> Result<StoredObject, ObjectStoreError> {
        if let Err(error) = self.ensure_available() {
            record_operation(DOWNLOAD_OPERATION, "failure");
            return Err(error);
        }
        let result = self
            .objects
            .lock()
            .await
            .get(key)
            .cloned()
            .ok_or(ObjectStoreError::NotFound);
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        } else {
            record_operation(DOWNLOAD_OPERATION, "success");
        }
        result
    }

    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObject, ObjectStoreError> {
        let object = self.get(key).await?;
        let Some(range) = range else {
            return Ok(object);
        };
        let Some(range) = range.strip_prefix("bytes=") else {
            return Err(ObjectStoreError::Operation(DOWNLOAD_OPERATION));
        };
        let Some((start, end)) = range.split_once('-') else {
            return Err(ObjectStoreError::Operation(DOWNLOAD_OPERATION));
        };
        let start = start
            .parse::<usize>()
            .map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION))?;
        let end = if end.is_empty() {
            object.bytes.len().saturating_sub(1)
        } else {
            end.parse::<usize>()
                .map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION))?
                .min(object.bytes.len().saturating_sub(1))
        };
        if start > end {
            return Err(ObjectStoreError::Operation(DOWNLOAD_OPERATION));
        }
        Ok(StoredObject {
            bytes: object.bytes.slice(start..=end),
            content_type: object.content_type,
        })
    }

    async fn get_stream(&self, key: &str) -> Result<StoredObjectStream, ObjectStoreError> {
        self.get_range_stream(key, None).await
    }

    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObjectStream, ObjectStoreError> {
        let object = self.get_range(key, range).await?;
        Ok(StoredObjectStream {
            stream: Box::pin(futures_util::stream::once(async move { Ok(object.bytes) })),
            content_type: object.content_type,
        })
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        if let Err(error) = self.ensure_available() {
            record_operation(DELETE_OPERATION, "failure");
            return Err(error);
        }
        self.objects.lock().await.remove(key);
        record_operation(DELETE_OPERATION, "success");
        Ok(())
    }

    async fn readiness(&self) -> Result<(), ObjectStoreError> {
        let result = self.ensure_available();
        record_readiness(result.is_ok());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(name: &str) -> Option<String> {
        [
            ("S3_ENDPOINT", "http://127.0.0.1:9000"),
            ("S3_REGION", "us-east-1"),
            ("S3_BUCKET", "attricat-files"),
            ("S3_ACCESS_KEY_ID", "attricat-dev"),
            ("S3_FORCE_PATH_STYLE", "true"),
            ("S3_UPLOAD_TIMEOUT_SECONDS", "30"),
            ("S3_DOWNLOAD_TIMEOUT_SECONDS", "30"),
        ]
        .into_iter()
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
    }

    fn secret() -> SecretString {
        SecretString::from("attricat-dev-secret")
    }

    #[test]
    fn storage_config_requires_each_setting() {
        let error = StorageConfig::from_values(
            |name| (name != "S3_BUCKET").then(|| values(name)).flatten(),
            Some(secret()),
        );
        assert!(matches!(
            error,
            Err(StorageConfigError::Missing("S3_BUCKET"))
        ));
        let error = StorageConfig::from_values(values, None);
        assert!(matches!(
            error,
            Err(StorageConfigError::MissingSecret("s3_secret_access_key"))
        ));
    }

    #[test]
    fn storage_config_rejects_invalid_path_style_and_timeouts() {
        let error = StorageConfig::from_values(
            |name| {
                if name == "S3_FORCE_PATH_STYLE" {
                    Some("yes".to_owned())
                } else {
                    values(name)
                }
            },
            Some(secret()),
        );
        assert!(matches!(
            error,
            Err(StorageConfigError::InvalidBoolean("S3_FORCE_PATH_STYLE"))
        ));

        let error = StorageConfig::from_values(
            |name| {
                if name == "S3_UPLOAD_TIMEOUT_SECONDS" {
                    Some("0".to_owned())
                } else {
                    values(name)
                }
            },
            Some(secret()),
        );
        assert!(matches!(
            error,
            Err(StorageConfigError::InvalidTimeout(
                "S3_UPLOAD_TIMEOUT_SECONDS"
            ))
        ));
    }

    #[test]
    fn download_errors_classify_missing_keys_and_other_failures() {
        let missing = S3Error::HttpFailWithBody(HTTP_NOT_FOUND, "<Code>NoSuchKey</Code>".into());
        assert!(matches!(
            classify_download_error(missing),
            ObjectStoreError::NotFound
        ));

        let provider_failure = S3Error::HttpFailWithBody(500, "<Code>InternalError</Code>".into());
        assert!(matches!(
            classify_download_error(provider_failure),
            ObjectStoreError::Operation(DOWNLOAD_OPERATION)
        ));

        assert!(matches!(
            classify_download_error(S3Error::HttpFail),
            ObjectStoreError::Operation(DOWNLOAD_OPERATION)
        ));
    }

    #[test]
    fn ranged_downloads_carry_the_range_header_and_others_do_not() {
        let store = S3ObjectStore::new(
            StorageConfig::from_values(values, Some(secret())).expect("valid test config"),
        )
        .expect("bucket handle");
        assert!(
            store
                .download_bucket(None)
                .unwrap()
                .extra_headers
                .get(header::RANGE)
                .is_none()
        );
        let ranged = store.download_bucket(Some("bytes=0-6")).unwrap();
        assert_eq!(
            ranged.extra_headers.get(header::RANGE).unwrap(),
            "bytes=0-6"
        );
        assert!(matches!(
            store.download_bucket(Some("bytes=0-\n6")),
            Err(ObjectStoreError::Operation(DOWNLOAD_OPERATION))
        ));
    }

    #[tokio::test]
    async fn fake_store_round_trips_and_reports_unavailability() {
        let store = FakeObjectStore::available();
        store.readiness().await.unwrap();
        let object = StoredObject {
            bytes: Bytes::from_static(b"file contents"),
            content_type: Some("text/plain".to_owned()),
        };
        store.put("opaque-key", object.clone()).await.unwrap();
        assert_eq!(store.get("opaque-key").await.unwrap(), object);
        let streamed = store
            .get_range_stream("opaque-key", Some("bytes=0-3"))
            .await
            .unwrap();
        let bytes = streamed
            .stream
            .try_fold(Vec::new(), |mut collected, chunk| async move {
                collected.extend_from_slice(&chunk);
                Ok(collected)
            })
            .await
            .unwrap();
        assert_eq!(bytes, b"file");
        store.delete("opaque-key").await.unwrap();
        assert!(store.get("opaque-key").await.is_err());
        store.set_available(false);
        assert!(matches!(
            store.readiness().await,
            Err(ObjectStoreError::Unavailable)
        ));
    }
}
