mod support;

use std::sync::Arc;

use api::storage::FakeObjectStore;
use reqwest::multipart::{Form, Part};
use sha2::{Digest, Sha256};
use support::*;

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
