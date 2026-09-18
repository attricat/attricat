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
    assert!(entity["values"].as_array().unwrap().iter().any(|value| {
        value["kind"] == "file"
            && value["attribute_code"] == "image"
            && value["files"]
                .as_array()
                .is_some_and(|files| files.len() == 1)
    }));

    let context_id: Uuid =
        sqlx::query_scalar("SELECT id FROM attribute_contexts WHERE code = 'default'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let preview: Value = client
        .get(format!(
            "{base_url}/entities/{entity_id}/resolved-preview?context_id={context_id}"
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
        .post(format!("{base_url}/v1/entities/search"))
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
async fn migration_retains_files_on_compatible_file_attributes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let blueprint = upload_blueprint(&client, &base_url).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let uploaded: Value = client
        .post(format!(
            "{base_url}/entities/{entity_id}/file-attributes/image/uploads"
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
           WHERE av.entity_id = $1"#,
    )
    .bind(entity_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&serde_json::json!({ "definition": r#"format_version = 1
code = "file_upload_product"
name = "Renamed file upload product"
kind = "entity"

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
            "{base_url}/v1/entities/{entity_id}/blueprint-migration/preview"
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
            "{base_url}/v1/entities/{entity_id}/blueprint-migration"
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
        .get(format!("{base_url}/v1/entities/{entity_id}"))
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
           WHERE av.entity_id = $1"#,
    )
    .bind(entity_id.parse::<uuid::Uuid>().unwrap())
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
        "attachment; filename=\"_.png\""
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
    assert!(policy.0.lock().await.iter().any(|operation| {
        matches!(
            operation,
            FileAccessOperation::ReadMetadata {
                entity_id,
                blueprint_id,
                ..
            } if *entity_id == entity["id"].as_str().unwrap().parse::<Uuid>().unwrap()
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
async fn file_reads_use_active_linked_entity_scopes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store).await;
    let owner = authenticated_client();
    let blueprint = upload_blueprint(&owner, &base_url).await;
    let permitted_entity = create_entity(&owner, &base_url, &blueprint).await;
    let denied_entity = create_entity(&owner, &base_url, &blueprint).await;
    let upload = |entity_id: String| {
        let base_url = base_url.clone();
        let owner = owner.clone();
        async move {
            owner
                .post(format!(
                    "{base_url}/entities/{entity_id}/file-attributes/image/uploads"
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
    let permitted_file = upload(permitted_entity["id"].as_str().unwrap().to_owned()).await;
    let denied_file = upload(denied_entity["id"].as_str().unwrap().to_owned()).await;
    let permitted_file_id = permitted_file["files"][0]["id"].as_str().unwrap();
    let denied_file_id = denied_file["files"][0]["id"].as_str().unwrap();
    // A file can be reused by several active values. The reader is scoped to
    // only the first entity, so this also verifies that one readable target is
    // sufficient even when another linked entity is out of scope.
    let denied_value_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM attribute_values WHERE entity_id = $1 AND active",
    )
    .bind(
        denied_entity["id"]
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
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'entity', $4)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(membership_id)
        .bind(permitted_entity["id"].as_str().unwrap().parse::<Uuid>().unwrap())
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
