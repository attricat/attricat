mod support;

use std::sync::Arc;

use api::{
    file_access::{FileAccessDecision, FileAccessOperation, FileAccessPolicy},
    storage::FakeObjectStore,
};
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use sha2::{Digest, Sha256};
use support::*;
use tokio::sync::Mutex;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\ncontents";

async fn upload_blueprint(client: &Client, base_url: &str) -> Value {
    create_blueprint(
        client,
        base_url,
        r#"format_version = 1
code = "file_upload_product"
name = "File upload product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["image"]

[[attributes]]
code = "image"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["png"]
max_bytes = 1024
image_only = true"#,
    )
    .await
}

struct RecordingFilePolicy(Mutex<Vec<FileAccessOperation>>);

#[async_trait]
impl FileAccessPolicy for RecordingFilePolicy {
    async fn authorize(&self, operation: FileAccessOperation) -> FileAccessDecision {
        self.0.lock().await.push(operation);
        FileAccessDecision::Allow
    }
}

fn png_part(name: &str) -> Part {
    Part::bytes(PNG.to_vec())
        .file_name(name.to_owned())
        .mime_str("image/png")
        .unwrap()
}

#[sqlx::test]
async fn uploads_files_to_the_fake_store_and_persists_derived_metadata(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();

    let response: Value = client
        .post(format!(
            "{base_url}/entities/{entity_id}/file-attributes/image/uploads"
        ))
        .multipart(Form::new().part("file", png_part("product.png")))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(response["attribute_code"], "image");
    assert_eq!(response["files"][0]["mime_type"], "image/png");
    assert_eq!(response["files"][0]["status"], "queued");
    assert_eq!(
        response["files"][0]["sha256"],
        format!("{:x}", Sha256::digest(PNG))
    );
    assert_eq!(store.object_count().await, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_processing_jobs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM attribute_file_references")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let entity: Value = client
        .get(format!("{base_url}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        entity["values"]
            .as_array()
            .unwrap()
            .iter()
            .all(|value| value["attribute_code"] != "image")
    );

    client
        .put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&serde_json::json!({
            "values": [],
            "relationships": [],
            "remove_values": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    server.abort();
}

#[sqlx::test]
async fn reads_file_metadata_and_downloads_with_safe_range_headers(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let policy = Arc::new(RecordingFilePolicy(Mutex::new(Vec::new())));
    let (base_url, server) =
        start_server_with_file_access_policy(pool.clone(), store, policy.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let upload: Value = client
        .post(format!(
            "{base_url}/entities/{}/file-attributes/image/uploads",
            entity["id"].as_str().unwrap()
        ))
        .multipart(Form::new().part("file", png_part("unsafe name \\\".png")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id = upload["files"][0]["id"].as_str().unwrap();

    let metadata: Value = client
        .get(format!("{base_url}/files/{file_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(metadata["status"], "queued");
    assert!(metadata.get("original_key").is_none());
    assert!(metadata.get("original_filename").is_none());
    assert_eq!(
        client
            .get(format!("{base_url}/files/{file_id}/download"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    sqlx::query("UPDATE files SET status = 'ready' WHERE id = $1")
        .bind(file_id.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let response = client
        .get(format!("{base_url}/files/{file_id}/download"))
        .header("Range", "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(
        response.headers()["content-range"],
        format!("bytes 2-5/{}", PNG.len())
    );
    assert_eq!(response.headers()["accept-ranges"], "bytes");
    assert_eq!(response.headers()["cache-control"], "private, no-store");
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"unsafe_name___.png\""
    );
    assert_eq!(response.bytes().await.unwrap(), &PNG[2..6]);
    let object_key =
        sqlx::query_scalar::<_, String>("SELECT original_key FROM files WHERE id = $1")
            .bind(file_id.parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, byte_size, object_key, sha256) SELECT $1, workspace_id, id, 'thumbnail', mime_type, byte_size, $2, sha256 FROM files WHERE id = $3")
        .bind(Uuid::new_v4())
        .bind(object_key)
        .bind(file_id.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!(
                "{base_url}/files/{file_id}/variants/thumbnail/download"
            ))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let invalid: Value = client
        .get(format!("{base_url}/files/{file_id}/download"))
        .header("Range", "bytes=999-")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(invalid["error"]["code"], "invalid_range");
    assert!(
        policy
            .0
            .lock()
            .await
            .iter()
            .any(|operation| matches!(operation, FileAccessOperation::Upload { .. }))
    );
    assert!(
        policy
            .0
            .lock()
            .await
            .iter()
            .any(|operation| matches!(operation, FileAccessOperation::ReadMetadata { .. }))
    );
    assert!(
        policy
            .0
            .lock()
            .await
            .iter()
            .any(|operation| matches!(operation, FileAccessOperation::DownloadOriginal { .. }))
    );
    server.abort();
}

#[sqlx::test]
async fn rejects_policy_and_signature_mismatches_with_the_error_envelope(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();

    let response: Value = client
        .post(format!(
            "{base_url}/entities/{entity_id}/file-attributes/image/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"not an image\0".to_vec())
                    .file_name("pretend.png")
                    .mime_str("image/png")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(response["error"]["code"], "unsupported_media_type");
    assert_eq!(store.object_count().await, 0);
    server.abort();
}
