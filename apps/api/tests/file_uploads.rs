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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["image"]

[views.table]
type = "table"
columns = [{ field = "image", renderer = { id = "catalog.table_image", version = 1 } }]

[[attributes]]
code = "image"
value_type = "file"
cardinality = "one"
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
async fn malformed_trailing_multipart_field_removes_staged_files(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let staged_paths = || -> std::collections::HashSet<_> {
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("catalog-upload-")
            })
            .collect()
    };
    let before = staged_paths();
    let boundary = "malformed-trailing-upload";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"ok.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
    body.extend_from_slice(PNG);
    body.extend_from_slice(
        format!("\r\n--{boundary}\r\ninvalid header\r\n\r\ncontent\r\n--{boundary}--\r\n")
            .as_bytes(),
    );
    let response = client
        .post(format!(
            "{base_url}/records/{}/file-attributes/image/uploads",
            record["id"].as_str().unwrap()
        ))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // Other upload tests share the process temp directory and may briefly
    // stage a file in parallel. Allow those requests to finish before checking
    // that this malformed request left no new persistent path behind.
    for _ in 0..20 {
        if staged_paths().difference(&before).next().is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        staged_paths().difference(&before).next().is_none(),
        "malformed multipart must not leak staged files"
    );
    server.abort();
}

#[sqlx::test]
async fn oversized_context_id_is_rejected_before_buffering_the_field(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let response = client
        .post(format!(
            "{base_url}/records/{}/file-attributes/image/uploads",
            record["id"].as_str().unwrap()
        ))
        .multipart(
            Form::new()
                .text("context_id", "x".repeat(128 * 1024))
                .part("file", png_part("image.png")),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}

#[sqlx::test]
async fn text_upload_with_binary_tail_is_rejected(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "text_upload_product"
name = "Text upload product"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["document"]
[[attributes]]
code = "document"
value_type = "file"
allowed_mime_groups = ["text/plain"]
allowed_extensions = ["txt"]
max_bytes = 1024
"#,
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let mut bytes = vec![b'a'; 512];
    bytes.extend_from_slice(b"\0binary tail");
    let response = client
        .post(format!(
            "{base_url}/records/{}/file-attributes/document/uploads",
            record["id"].as_str().unwrap()
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(bytes)
                    .file_name("bad.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(store.object_count().await, 0);
    server.abort();
}

#[sqlx::test]
async fn uploads_files_to_the_fake_store_and_persists_derived_metadata(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let record_before = create_record(&client, &base_url, &blueprint).await;
    let record_id = record_before["id"].as_str().unwrap();

    let response: Value = client
        .post(format!(
            "{base_url}/records/{record_id}/file-attributes/image/uploads"
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

    let record: Value = client
        .get(format!("{base_url}/v1/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(record["values"].as_array().unwrap().iter().any(|value| {
        value["kind"] == "file"
            && value["attribute_code"] == "image"
            && value["files"]
                .as_array()
                .is_some_and(|files| files.len() == 1)
    }));
    // An open edit form adopts this version so its next field save is not stale.
    assert!(record_before["updated_at"].is_string());
    assert_ne!(response["record_updated_at"], record_before["updated_at"]);
    assert_eq!(
        response["record_updated_at"],
        record["record"]["updated_at"]
    );

    let context_id: Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_contexts WHERE code = 'default'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let preview: Value = client
        .get(format!(
            "{base_url}/records/{record_id}/resolved-preview?context_id={context_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["values"]["image"]["value"][0]["filename"],
        "product.png"
    );

    let file_id = response["files"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let object_key =
        sqlx::query_scalar::<_, String>("SELECT original_key FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, byte_size, object_key, sha256) SELECT $1, workspace_id, id, 'thumbnail', 'image/webp', byte_size, $2, sha256 FROM files WHERE id = $3")
        .bind(Uuid::new_v4())
        .bind(object_key)
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();

    let search: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&serde_json::json!({
            "blueprint": { "code": "file_upload_product" },
            "filters": [],
            "page": { "size": 25, "cursor": null }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        search["items"][0]["table_values"]["image"][0]["filename"],
        "product.png"
    );
    assert_eq!(
        search["items"][0]["table_values"]["image"][0]["variants"][0]["kind"],
        "thumbnail"
    );

    client
        .put(format!("{base_url}/v1/records/{record_id}"))
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
async fn migration_retains_files_on_compatible_file_attributes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    let uploaded: Value = client
        .post(format!(
            "{base_url}/records/{record_id}/file-attributes/image/uploads"
        ))
        .multipart(Form::new().part("file", png_part("preserved.png")))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id = uploaded["files"][0]["id"].as_str().unwrap();
    let before = sqlx::query_as::<_, (uuid::Uuid, chrono::DateTime<chrono::Utc>, uuid::Uuid, i32)>(
        r#"SELECT av.id, av.created_at, r.file_id, r.position
           FROM attribute_values av
           JOIN attribute_file_references r ON r.attribute_value_id = av.id
           WHERE av.record_id = $1"#,
    )
    .bind(record_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&serde_json::json!({ "definition": r#"format_version = 1
code = "file_upload_product"
name = "Renamed file upload product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["image"]

[[attributes]]
code = "image"
value_type = "file"
cardinality = "one"
allowed_mime_groups = ["image"]
allowed_extensions = ["png"]
max_bytes = 1024
image_only = true"# }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let preview: Value = client
        .post(format!(
            "{base_url}/v1/records/{record_id}/blueprint-migration/preview"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base_url}/v1/records/{record_id}/blueprint-migration"
        ))
        .json(&serde_json::json!({
            "migration_id": preview["migration_id"],
            "expected_target_version": version,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let migrated: Value = client
        .get(format!("{base_url}/v1/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(migrated["values"].as_array().unwrap().iter().any(|value| {
        value["kind"] == "file"
            && value["attribute_code"] == "image"
            && value["files"][0]["id"] == file_id
    }));
    let after = sqlx::query_as::<_, (uuid::Uuid, chrono::DateTime<chrono::Utc>, uuid::Uuid, i32)>(
        r#"SELECT av.id, av.created_at, r.file_id, r.position
           FROM attribute_values av
           JOIN attribute_file_references r ON r.attribute_value_id = av.id
           WHERE av.record_id = $1"#,
    )
    .bind(record_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        after, before,
        "file parent and ordered reference stay in place"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_value_history WHERE id = $1",)
            .bind(before.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attribute_file_reference_history WHERE attribute_value_history_id = $1",
        )
        .bind(before.0)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
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
    let record = create_record(&client, &base_url, &blueprint).await;
    let upload: Value = client
        .post(format!(
            "{base_url}/records/{}/file-attributes/image/uploads",
            record["id"].as_str().unwrap()
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
    // Browsers revalidate every use, so revoked access or a purged file
    // takes effect at once.
    assert_eq!(response.headers()["cache-control"], "private, no-cache");
    assert_eq!(
        response.headers()["etag"],
        format!("\"{:x}\"", Sha256::digest(PNG))
    );
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"_.png\""
    );
    assert_eq!(response.bytes().await.unwrap(), &PNG[2..6]);
    let revalidated = client
        .get(format!("{base_url}/files/{file_id}/download"))
        .header("if-none-match", format!("\"{:x}\"", Sha256::digest(PNG)))
        .send()
        .await
        .unwrap();
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(revalidated.headers()["cache-control"], "private, no-cache");
    assert!(revalidated.bytes().await.unwrap().is_empty());
    // Revalidation is authorized like a download.
    let anonymous = reqwest::Client::new()
        .get(format!("{base_url}/files/{file_id}/download"))
        .header("if-none-match", format!("\"{:x}\"", Sha256::digest(PNG)))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
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
    assert!(policy.0.lock().await.iter().any(|operation| {
        matches!(
            operation,
            FileAccessOperation::ReadMetadata {
                record_id,
                blueprint_id,
                ..
            } if *record_id == record["id"].as_str().unwrap().parse::<Uuid>().unwrap()
                && *blueprint_id == blueprint["blueprint"]["id"].as_str().unwrap().parse::<Uuid>().unwrap()
        )
    }));
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
async fn file_reads_use_active_linked_record_scopes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let owner = authenticated_client();
    let blueprint = upload_blueprint(&owner, &base_url).await;
    let permitted_record = create_record(&owner, &base_url, &blueprint).await;
    let denied_record = create_record(&owner, &base_url, &blueprint).await;
    let upload = |record_id: String| {
        let base_url = base_url.clone();
        let owner = owner.clone();
        async move {
            owner
                .post(format!(
                    "{base_url}/records/{record_id}/file-attributes/image/uploads"
                ))
                .multipart(Form::new().part("file", png_part("product.png")))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let permitted_file = upload(permitted_record["id"].as_str().unwrap().to_owned()).await;
    let denied_file = upload(denied_record["id"].as_str().unwrap().to_owned()).await;
    let permitted_file_id = permitted_file["files"][0]["id"].as_str().unwrap();
    let denied_file_id = denied_file["files"][0]["id"].as_str().unwrap();
    // A file can be reused by several active values. The reader is scoped to
    // only the first record, so this also verifies that one readable target is
    // sufficient even when another linked record is out of scope.
    let denied_value_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM attribute_values WHERE record_id = $1 AND active",
    )
    .bind(
        denied_record["id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1, $2, $3, 1)")
        .bind(denied_value_id)
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind(permitted_file_id.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    for file_id in [permitted_file_id, denied_file_id] {
        sqlx::query("UPDATE files SET status = 'ready' WHERE id = $1")
            .bind(file_id.parse::<Uuid>().unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }
    let object_key =
        sqlx::query_scalar::<_, String>("SELECT original_key FROM files WHERE id = $1")
            .bind(permitted_file_id.parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO file_variants (id, workspace_id, file_id, kind, mime_type, byte_size, object_key, sha256) SELECT $1, workspace_id, id, 'thumbnail', mime_type, byte_size, $2, sha256 FROM files WHERE id = $3")
        .bind(Uuid::new_v4())
        .bind(object_key)
        .bind(permitted_file_id.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let reader_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'file-reader@example.test')")
        .bind(reader_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(workspace_id)
    .bind(reader_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'record', $4)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .bind(permitted_record["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-catalog-user-id",
        reqwest::header::HeaderValue::from_str(&reader_id.to_string()).unwrap(),
    );
    headers.insert(
        "x-catalog-workspace-id",
        reqwest::header::HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    let reader = Client::builder().default_headers(headers).build().unwrap();
    let permitted_record_id = permitted_record["id"].as_str().unwrap();
    let form: Value = reader
        .get(format!("{base_url}/v1/records/{permitted_record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(form["can_write"], false);
    assert_eq!(
        reader
            .put(format!(
                "{base_url}/records/{permitted_record_id}/file-attributes/image/references"
            ))
            .json(&json!({"expected_file_ids":[permitted_file_id],"file_ids":[]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        reader
            .post(format!(
                "{base_url}/records/{permitted_record_id}/file-attributes/image/uploads"
            ))
            .multipart(Form::new().part("file", png_part("denied.png")))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    for path in [
        format!("/files/{permitted_file_id}"),
        format!("/files/{permitted_file_id}/download"),
        format!("/files/{permitted_file_id}/variants/thumbnail/download"),
    ] {
        assert_eq!(
            reader
                .get(format!("{base_url}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    for path in [
        format!("/files/{denied_file_id}"),
        format!("/files/{denied_file_id}/download"),
        format!("/files/{denied_file_id}/variants/thumbnail/download"),
    ] {
        assert_eq!(
            reader
                .get(format!("{base_url}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    server.abort();
}

#[sqlx::test]
async fn rejects_policy_and_signature_mismatches_with_the_error_envelope(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();

    let response: Value = client
        .post(format!(
            "{base_url}/records/{record_id}/file-attributes/image/uploads"
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

#[sqlx::test]
async fn duplicating_an_record_is_atomic_with_its_file_links(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let workspace_id: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let repository = api::repository::CatalogRepository::new(pool.clone(), workspace_id);
    let file_id = Uuid::new_v4();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'p.png', 'p.png', 'image/png', 7, $3, 'files/p.png', 'ready')")
        .bind(file_id)
        .bind(workspace_id)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    repository
        .link_file_to_attribute(record_id, "image", None, file_id)
        .await
        .unwrap();

    let copy = repository.duplicate_record(record_id).await.unwrap();
    let references: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attribute_file_references r JOIN attribute_values v ON v.id = r.attribute_value_id WHERE v.record_id = $1 AND r.file_id = $2",
    )
    .bind(copy.id)
    .bind(file_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(references, 1);

    // The file can no longer be linked as an attachment, so linking it into
    // a second copy fails. That failure must roll back the whole copy.
    sqlx::query("UPDATE files SET purpose = 'avatar' WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .unwrap();
    let records = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM records WHERE blueprint_id = $1 AND deleted_at IS NULL",
        )
        .bind(copy.blueprint_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let before = records().await;
    assert!(repository.duplicate_record(record_id).await.is_err());
    assert_eq!(records().await, before);
    server.abort();
}
