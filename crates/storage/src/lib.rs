//! S3-compatible object storage boundary.
//!
//! Object keys are intentionally opaque to this module's callers. HTTP and
//! domain code must never expose them to clients.

use std::{
    collections::BTreeMap,
    env,
    path::Path,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{Client, config::Builder as S3ConfigBuilder, primitives::ByteStream};
use bytes::Bytes;
use futures_util::Stream;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tokio::{sync::Mutex, time::timeout};
use url::Url;

const UPLOAD_OPERATION: &str = "upload";
const DOWNLOAD_OPERATION: &str = "download";
const DELETE_OPERATION: &str = "delete";
const READINESS_OPERATION: &str = "readiness";

/// Configuration required to connect to an S3-compatible object store.
///
/// All values are deployment configuration. In particular, credentials are not
/// inferred from the ambient AWS environment so a missing deployment setting
/// cannot silently select a different account or bucket.
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
    pub fn from_env() -> Result<Self, StorageConfigError> {
        Self::from_values(|name| env::var(name).ok())
    }

    fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, StorageConfigError> {
        let endpoint = required(&get, "S3_ENDPOINT")?;
        let endpoint = Url::parse(&endpoint).map_err(|_| StorageConfigError::InvalidEndpoint)?;
        if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
            return Err(StorageConfigError::InvalidEndpoint);
        }

        let region = required(&get, "S3_REGION")?;
        let bucket = required(&get, "S3_BUCKET")?;
        let access_key_id = required(&get, "S3_ACCESS_KEY_ID")?;
        let secret_access_key = SecretString::from(required(&get, "S3_SECRET_ACCESS_KEY")?);
        let force_path_style = required(&get, "S3_FORCE_PATH_STYLE")?
            .parse()
            .map_err(|_| StorageConfigError::InvalidBoolean("S3_FORCE_PATH_STYLE"))?;
        let upload_timeout = timeout_from_env(&get, "S3_UPLOAD_TIMEOUT_SECONDS")?;
        let download_timeout = timeout_from_env(&get, "S3_DOWNLOAD_TIMEOUT_SECONDS")?;

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

fn timeout_from_env(
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

pub type ObjectByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, ObjectStoreError>> + Send>>;

/// A provider response whose bytes remain streamed until the HTTP consumer
/// writes them to the client.
pub struct StoredObjectStream {
    pub stream: ObjectByteStream,
    pub content_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum ObjectStoreError {
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

/// AWS SDK-backed implementation for AWS S3 and compatible endpoints.
pub struct S3ObjectStore {
    client: Client,
    bucket: String,
    upload_timeout: Duration,
    download_timeout: Duration,
}

impl S3ObjectStore {
    pub async fn new(config: StorageConfig) -> Self {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region))
            .credentials_provider(Credentials::new(
                config.access_key_id,
                config.secret_access_key.expose_secret(),
                None,
                None,
                "catalog-config",
            ))
            .load()
            .await;
        let client = Client::from_conf(
            S3ConfigBuilder::from(&sdk_config)
                .endpoint_url(config.endpoint.as_str())
                .force_path_style(config.force_path_style)
                .build(),
        );
        Self {
            client,
            bucket: config.bucket,
            upload_timeout: config.upload_timeout,
            download_timeout: config.download_timeout,
        }
    }

    async fn send_upload<T>(
        &self,
        operation: &'static str,
        future: impl Future<
            Output = Result<
                T,
                aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>,
            >,
        >,
    ) -> Result<T, ObjectStoreError> {
        timeout(self.upload_timeout, future)
            .await
            .map_err(|_| ObjectStoreError::TimedOut(operation))?
            .map_err(|_| ObjectStoreError::Operation(operation))
    }
}

#[async_trait]
impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &str, object: StoredObject) -> Result<(), ObjectStoreError> {
        let mut request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(object.bytes));
        if let Some(content_type) = object.content_type {
            request = request.content_type(content_type);
        }
        let result = self.send_upload(UPLOAD_OPERATION, request.send()).await;
        if result.is_err() {
            record_operation(UPLOAD_OPERATION, "failure");
        }
        result?;
        record_operation(UPLOAD_OPERATION, "success");
        Ok(())
    }

    async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        let body = ByteStream::from_path(path)
            .await
            .map_err(|_| ObjectStoreError::Operation(UPLOAD_OPERATION))?;
        let mut request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body);
        if let Some(content_type) = content_type {
            request = request.content_type(content_type);
        }
        let result = self.send_upload(UPLOAD_OPERATION, request.send()).await;
        if result.is_err() {
            record_operation(UPLOAD_OPERATION, "failure");
        }
        result?;
        record_operation(UPLOAD_OPERATION, "success");
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<StoredObject, ObjectStoreError> {
        let result = timeout(
            self.download_timeout,
            self.client
                .get_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        )
        .await
        .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
        .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION)));
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let result = result?;
        let content_type = result.content_type().map(str::to_owned);
        let bytes = timeout(self.download_timeout, result.body.collect())
            .await
            .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION)));
        if bytes.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let bytes = bytes?.into_bytes();
        record_operation(DOWNLOAD_OPERATION, "success");
        Ok(StoredObject {
            bytes,
            content_type,
        })
    }

    async fn get_stream(&self, key: &str) -> Result<StoredObjectStream, ObjectStoreError> {
        self.get_range_stream(key, None).await
    }

    async fn get_range(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObject, ObjectStoreError> {
        let mut request = self.client.get_object().bucket(&self.bucket).key(key);
        if let Some(range) = range {
            request = request.range(range);
        }
        let result = timeout(self.download_timeout, request.send())
            .await
            .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION)));
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let result = result?;
        let content_type = result.content_type().map(str::to_owned);
        let bytes = timeout(self.download_timeout, result.body.collect())
            .await
            .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION)));
        if bytes.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let bytes = bytes?.into_bytes();
        record_operation(DOWNLOAD_OPERATION, "success");
        Ok(StoredObject {
            bytes,
            content_type,
        })
    }

    async fn get_range_stream(
        &self,
        key: &str,
        range: Option<&str>,
    ) -> Result<StoredObjectStream, ObjectStoreError> {
        let mut request = self.client.get_object().bucket(&self.bucket).key(key);
        if let Some(range) = range {
            request = request.range(range);
        }
        let result = timeout(self.download_timeout, request.send())
            .await
            .map_err(|_| ObjectStoreError::TimedOut(DOWNLOAD_OPERATION))
            .and_then(|result| result.map_err(|_| ObjectStoreError::Operation(DOWNLOAD_OPERATION)));
        if result.is_err() {
            record_operation(DOWNLOAD_OPERATION, "failure");
        }
        let result = result?;
        let content_type = result.content_type().map(str::to_owned);
        let download_timeout = self.download_timeout;
        let stream = async_stream::stream! {
            let mut body = result.body;
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
            content_type,
        })
    }

    async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        let result = timeout(
            self.upload_timeout,
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send(),
        )
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

    async fn readiness(&self) -> Result<(), ObjectStoreError> {
        let result = timeout(
            self.download_timeout,
            self.client.head_bucket().bucket(&self.bucket).send(),
        )
        .await
        .map_err(|_| ObjectStoreError::TimedOut(READINESS_OPERATION))?
        .map_err(|_| ObjectStoreError::Operation(READINESS_OPERATION));
        record_readiness(result.is_ok());
        result.map(|_| ())
    }
}

fn record_operation(operation: &'static str, outcome: &'static str) {
    metrics::counter!("catalog_object_store_operations_total", "operation" => operation, "outcome" => outcome)
        .increment(1);
}

fn record_readiness(ready: bool) {
    record_operation(
        READINESS_OPERATION,
        if ready { "success" } else { "failure" },
    );
    metrics::gauge!("catalog_object_store_ready").set(if ready { 1.0 } else { 0.0 });
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
            .ok_or(ObjectStoreError::Operation(DOWNLOAD_OPERATION));
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
    use futures_util::TryStreamExt;

    use super::*;

    fn values(name: &str) -> Option<String> {
        [
            ("S3_ENDPOINT", "http://127.0.0.1:9000"),
            ("S3_REGION", "us-east-1"),
            ("S3_BUCKET", "catalog-files"),
            ("S3_ACCESS_KEY_ID", "catalog-dev"),
            ("S3_SECRET_ACCESS_KEY", "catalog-dev-secret"),
            ("S3_FORCE_PATH_STYLE", "true"),
            ("S3_UPLOAD_TIMEOUT_SECONDS", "30"),
            ("S3_DOWNLOAD_TIMEOUT_SECONDS", "30"),
        ]
        .into_iter()
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
    }

    #[test]
    fn storage_config_requires_each_setting() {
        let error = StorageConfig::from_values(|name| {
            (name != "S3_BUCKET").then(|| values(name)).flatten()
        });
        assert!(matches!(
            error,
            Err(StorageConfigError::Missing("S3_BUCKET"))
        ));
    }

    #[test]
    fn storage_config_rejects_invalid_path_style_and_timeouts() {
        let error = StorageConfig::from_values(|name| {
            if name == "S3_FORCE_PATH_STYLE" {
                Some("yes".to_owned())
            } else {
                values(name)
            }
        });
        assert!(matches!(
            error,
            Err(StorageConfigError::InvalidBoolean("S3_FORCE_PATH_STYLE"))
        ));

        let error = StorageConfig::from_values(|name| {
            if name == "S3_UPLOAD_TIMEOUT_SECONDS" {
                Some("0".to_owned())
            } else {
                values(name)
            }
        });
        assert!(matches!(
            error,
            Err(StorageConfigError::InvalidTimeout(
                "S3_UPLOAD_TIMEOUT_SECONDS"
            ))
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
