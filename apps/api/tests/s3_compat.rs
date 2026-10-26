//! Opt-in compatibility coverage against the local RustFS S3 adapter.
//!
//! Run after `just dev` with `just test-s3-compat` so the test loads the same
//! configuration and secrets as the API and file worker.
use api::{
    config::Config,
    storage::{ObjectStore, ObjectStoreError, S3ObjectStore, StoredObject, StoredObjectStream},
};
use futures_util::TryStreamExt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Larger than the client's 8 MiB part size, so the upload is multipart.
const MULTIPART_FILE_BYTES: usize = 20 * 1024 * 1024;

#[tokio::test]
#[ignore = "requires the configured RustFS/S3-compatible endpoint and bucket"]
async fn rustfs_supports_attricat_s3_operations() {
    let config = Config::load().expect("valid S3 test configuration").storage;
    let store = S3ObjectStore::new(config).expect("S3 client");
    let key = format!("attricat-s3-compat-test/{}", Uuid::new_v4());

    store.readiness().await.expect("bucket is reachable");
    store
        .put(
            &key,
            StoredObject {
                bytes: "attricat-s3-compat".into(),
                content_type: Some("text/plain".into()),
            },
        )
        .await
        .expect("put object");

    let object = store.get(&key).await.expect("get object");
    assert_eq!(&object.bytes[..], b"attricat-s3-compat");
    assert_eq!(object.content_type.as_deref(), Some("text/plain"));

    let object = store
        .get_range(&key, Some("bytes=0-7"))
        .await
        .expect("get range");
    assert_eq!(&object.bytes[..], b"attricat");
    assert_eq!(object.content_type.as_deref(), Some("text/plain"));

    let streamed = collect(
        store
            .get_range_stream(&key, Some("bytes=9-10"))
            .await
            .expect("stream range"),
    )
    .await;
    assert_eq!(streamed, b"s3");

    store.delete(&key).await.expect("delete object");
    assert!(matches!(
        store.get(&key).await,
        Err(ObjectStoreError::NotFound)
    ));
    assert!(matches!(
        store.get_stream(&key).await,
        Err(ObjectStoreError::NotFound)
    ));
}

#[tokio::test]
#[ignore = "requires the configured RustFS/S3-compatible endpoint and bucket"]
async fn rustfs_round_trips_a_multipart_file_upload() {
    let config = Config::load().expect("valid S3 test configuration").storage;
    let store = S3ObjectStore::new(config).expect("S3 client");
    let key = format!("attricat-s3-compat-test/{}", Uuid::new_v4());
    let bytes: Vec<u8> = (0..MULTIPART_FILE_BYTES)
        .map(|index| (index % 251) as u8)
        .collect();
    let path = std::env::temp_dir().join(format!("attricat-s3-compat-{}", Uuid::new_v4()));
    tokio::fs::write(&path, &bytes).await.expect("write file");

    let uploaded = store
        .put_file(&key, &path, Some("application/octet-stream"))
        .await;
    tokio::fs::remove_file(&path).await.expect("remove file");
    uploaded.expect("multipart upload");

    let downloaded = collect(store.get_stream(&key).await.expect("stream object")).await;
    assert_eq!(downloaded.len(), bytes.len());
    assert_eq!(Sha256::digest(&downloaded), Sha256::digest(&bytes));

    store.delete(&key).await.expect("delete object");
}

async fn collect(object: StoredObjectStream) -> Vec<u8> {
    object
        .stream
        .try_fold(Vec::new(), |mut bytes, chunk| async move {
            bytes.extend_from_slice(&chunk);
            Ok(bytes)
        })
        .await
        .expect("read streamed object")
}
