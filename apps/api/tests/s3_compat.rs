//! Opt-in compatibility coverage against the local RustFS S3 adapter.
//!
//! Run after `just dev` with `set -a; . ./.env; set +a;` so the test uses the
//! same S3 configuration as the API and file worker.
use api::storage::{ObjectStore, S3ObjectStore, StorageConfig, StoredObject};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the configured RustFS/S3-compatible endpoint and bucket"]
async fn rustfs_supports_catalog_s3_operations() {
    let config = StorageConfig::from_env().expect("valid S3 test configuration");
    let store = S3ObjectStore::new(config).await;
    let key = format!("catalog-s3-compat-test/{}", Uuid::new_v4());

    store.readiness().await.expect("bucket is reachable");
    store
        .put(
            &key,
            StoredObject {
                bytes: "catalog-s3-compat".into(),
                content_type: Some("text/plain".into()),
            },
        )
        .await
        .expect("put object");

    let object = store
        .get_range(&key, Some("bytes=0-6"))
        .await
        .expect("get range");
    assert_eq!(&object.bytes[..], b"catalog");
    assert_eq!(object.content_type.as_deref(), Some("text/plain"));

    store.delete(&key).await.expect("delete object");
}
