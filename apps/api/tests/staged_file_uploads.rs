//! Files uploaded before their record exists and claimed when it is created.

mod support;

use reqwest::multipart::{Form, Part};
use support::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\ncontents";

/// A blueprint whose record schema requires a single PNG `image` and that
/// also has an optional many-valued `gallery`.
async fn required_image_blueprint(client: &Client, base_url: &str, code: &str) -> Value {
    create_blueprint(
        client,
        base_url,
        &format!(
            r#"format_version = 1
code = "{code}"
name = "Required image"
kind = "record"
record_schema = '''{{"required": ["image"]}}'''

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
image_only = true

[[attributes]]
code = "gallery"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["png"]
image_only = true"#
        ),
    )
    .await
}

fn blueprint_id(blueprint: &Value) -> &str {
    blueprint["blueprint"]["id"].as_str().unwrap()
}

fn png(name: &str) -> Part {
    Part::bytes(PNG.to_vec())
        .file_name(name.to_owned())
        .mime_str("image/png")
        .unwrap()
}

async fn stage(
    client: &Client,
    base_url: &str,
    blueprint: &Value,
    attribute_code: &str,
    form: Form,
) -> reqwest::Response {
    client
        .post(format!(
            "{base_url}/blueprints/{}/file-attributes/{attribute_code}/staged-uploads",
            blueprint_id(blueprint)
        ))
        .multipart(form)
        .send()
        .await
        .unwrap()
}

/// Stages one PNG for `attribute_code` and returns its file ID.
async fn stage_png(
    client: &Client,
    base_url: &str,
    blueprint: &Value,
    attribute_code: &str,
) -> String {
    let body = expect_status(
        stage(
            client,
            base_url,
            blueprint,
            attribute_code,
            Form::new().part("file", png("photo.png")),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(body["attribute_code"], attribute_code);
    assert!(body["expires_at"].is_string(), "{body}");
    body["files"][0]["id"].as_str().unwrap().to_owned()
}

async fn create_with_files(
    client: &Client,
    base_url: &str,
    blueprint: &Value,
    files: Value,
) -> reqwest::Response {
    client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({
            "blueprint": {"code": blueprint["blueprint"]["code"]},
            "values": [],
            "files": files,
        }))
        .send()
        .await
        .unwrap()
}

async fn linked_files(pool: &PgPool, record_id: &str) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT r.file_id FROM attribute_file_references r JOIN attribute_values v ON v.id = r.attribute_value_id WHERE v.record_id = $1 AND v.active ORDER BY r.position",
    )
    .bind(record_id.parse::<Uuid>().unwrap())
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn a_staged_file_satisfies_a_required_file_attribute(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "required_image").await;

    expect_error(
        create_with_files(&client, &base_url, &blueprint, json!([])).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "record_schema_mismatch",
    )
    .await;

    let file_id = stage_png(&client, &base_url, &blueprint, "image").await;
    // Until a create claims it, the upload stays bound and protected.
    assert_eq!(
        count(
            &pool,
            &format!("SELECT count(*) FROM files WHERE id = '{file_id}' AND staged_upload_user_id IS NOT NULL AND attachment_expires_at > now()"),
        )
        .await,
        1
    );
    let record = expect_status(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "image", "file_ids": [file_id]}]),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    let record_id = record["id"].as_str().unwrap();
    assert_eq!(
        linked_files(&pool, record_id).await,
        vec![file_id.parse::<Uuid>().unwrap()]
    );
    // Claiming ends the staging window and clears the binding.
    assert_eq!(
        count(
            &pool,
            &format!("SELECT count(*) FROM files WHERE id = '{file_id}' AND staged_upload_user_id IS NULL AND attachment_expires_at IS NULL"),
        )
        .await,
        1
    );
    // The creation audit records the file value like any other value.
    let audited: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT c.after_value FROM audit_event_changes c WHERE c.record_id = $1 AND c.attribute_code = 'image'",
    )
    .bind(record_id.parse::<Uuid>().unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audited, vec![json!([file_id])]);
    server.abort();
}

#[sqlx::test]
async fn a_staged_file_is_claimed_only_once_by_its_uploader_for_its_binding(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "claim_once").await;
    let other_blueprint = required_image_blueprint(&client, &base_url, "claim_other").await;
    let writer = create_role(&pool, "staged_writer", &["records.read", "records.write"]).await;
    let other_user = client_for(member_with_role(&pool, writer).await);
    let image = |file_id: &str| json!([{"attribute_code": "image", "file_ids": [file_id]}]);
    let rejected = |response| async move {
        expect_error(
            response,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_file_references",
        )
        .await;
    };

    // Another user cannot claim the upload.
    let file_id = stage_png(&client, &base_url, &blueprint, "image").await;
    rejected(create_with_files(&other_user, &base_url, &blueprint, image(&file_id)).await).await;
    // Nor can it be claimed for another blueprint or another attribute.
    rejected(create_with_files(&client, &base_url, &other_blueprint, image(&file_id)).await).await;
    rejected(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "gallery", "file_ids": [file_id]}, {"attribute_code": "image", "file_ids": [stage_png(&client, &base_url, &blueprint, "image").await]}]),
        )
        .await,
    )
    .await;
    // A failed create leaves the upload claimable; a successful one claims it.
    expect_status(
        create_with_files(&client, &base_url, &blueprint, image(&file_id)).await,
        StatusCode::CREATED,
    )
    .await;
    rejected(create_with_files(&client, &base_url, &blueprint, image(&file_id)).await).await;

    // An expired upload cannot be claimed.
    let expired = stage_png(&client, &base_url, &blueprint, "image").await;
    sqlx::query(
        "UPDATE files SET attachment_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(expired.parse::<Uuid>().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    rejected(create_with_files(&client, &base_url, &blueprint, image(&expired)).await).await;

    // An unknown file is rejected the same way, without revealing more.
    rejected(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            image(&Uuid::new_v4().to_string()),
        )
        .await,
    )
    .await;
    server.abort();
}

#[sqlx::test]
async fn a_staged_file_cannot_be_linked_to_an_existing_record(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "staged_link").await;
    let first = stage_png(&client, &base_url, &blueprint, "image").await;
    let record = expect_status(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "image", "file_ids": [first]}]),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let staged: Uuid = stage_png(&client, &base_url, &blueprint, "gallery")
        .await
        .parse()
        .unwrap();
    let repository =
        api::repository::CatalogRepository::new(pool.clone(), bootstrap_workspace_id());
    assert!(
        repository
            .link_file_to_attribute(record_id, "gallery", None, staged)
            .await
            .is_err()
    );
    server.abort();
}

#[sqlx::test]
async fn staged_uploads_follow_the_attribute_cardinality_and_policy(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "staged_policy").await;

    // The upload itself enforces cardinality, type and size.
    expect_error(
        stage(
            &client,
            &base_url,
            &blueprint,
            "image",
            Form::new()
                .part("files", png("a.png"))
                .part("files", png("b.png")),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_file",
    )
    .await;
    expect_error(
        stage(
            &client,
            &base_url,
            &blueprint,
            "image",
            Form::new().part(
                "file",
                Part::bytes(b"plain text".to_vec())
                    .file_name("notes.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
    )
    .await;
    let mut large = PNG.to_vec();
    large.resize(2048, 0);
    expect_error(
        stage(
            &client,
            &base_url,
            &blueprint,
            "image",
            Form::new().part(
                "file",
                Part::bytes(large)
                    .file_name("large.png")
                    .mime_str("image/png")
                    .unwrap(),
            ),
        )
        .await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "file_too_large",
    )
    .await;
    // Only file attributes of a published record blueprint accept uploads.
    expect_error(
        stage(
            &client,
            &base_url,
            &blueprint,
            "missing",
            Form::new().part("file", png("a.png")),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "attribute_not_applicable",
    )
    .await;
    let unknown = client
        .post(format!(
            "{base_url}/blueprints/{}/file-attributes/image/staged-uploads",
            Uuid::new_v4()
        ))
        .multipart(Form::new().part("file", png("a.png")))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

    // A single-file attribute takes one file on create too.
    let first = stage_png(&client, &base_url, &blueprint, "image").await;
    let second = stage_png(&client, &base_url, &blueprint, "image").await;
    expect_error(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "image", "file_ids": [first, second]}]),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "file_cardinality_exceeded",
    )
    .await;
    // A many-valued attribute keeps the given order.
    let photos = [
        stage_png(&client, &base_url, &blueprint, "gallery").await,
        stage_png(&client, &base_url, &blueprint, "gallery").await,
    ];
    let record = expect_status(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([
                {"attribute_code": "image", "file_ids": [first]},
                {"attribute_code": "gallery", "file_ids": [photos[1], photos[0]]},
            ]),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    let linked = linked_files(&pool, record["id"].as_str().unwrap()).await;
    assert_eq!(linked.len(), 3);
    assert!(linked.contains(&first.parse().unwrap()));
    server.abort();
}

#[sqlx::test]
async fn staged_uploads_require_permission_to_create_records(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "staged_permission").await;
    let reader = create_role(&pool, "staged_reader", &["records.read"]).await;
    let reader = client_for(member_with_role(&pool, reader).await);
    let response = stage(
        &reader,
        &base_url,
        &blueprint,
        "image",
        Form::new().part("file", png("a.png")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    server.abort();
}

#[sqlx::test]
async fn duplicating_a_record_whose_schema_requires_a_file_succeeds(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "duplicate_required").await;
    let file_id = stage_png(&client, &base_url, &blueprint, "image").await;
    let record = expect_status(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "image", "file_ids": [file_id]}]),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    let copy = expect_status(
        client
            .post(format!(
                "{base_url}/v1/records/{}/duplicate",
                record["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(
        linked_files(&pool, copy["id"].as_str().unwrap()).await,
        vec![file_id.parse::<Uuid>().unwrap()]
    );
    server.abort();
}

#[sqlx::test]
async fn expired_unclaimed_staged_files_are_reclaimed(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = required_image_blueprint(&client, &base_url, "staged_reclaim").await;
    let pending: Uuid = stage_png(&client, &base_url, &blueprint, "image")
        .await
        .parse()
        .unwrap();
    let expired: Uuid = stage_png(&client, &base_url, &blueprint, "image")
        .await
        .parse()
        .unwrap();
    let claimed = stage_png(&client, &base_url, &blueprint, "image").await;
    expect_status(
        create_with_files(
            &client,
            &base_url,
            &blueprint,
            json!([{"attribute_code": "image", "file_ids": [claimed]}]),
        )
        .await,
        StatusCode::CREATED,
    )
    .await;
    sqlx::query(
        "UPDATE files SET attachment_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(expired)
    .execute(&pool)
    .await
    .unwrap();

    let system = api::repository::CatalogRepository::system(pool.clone());
    while system.mark_unreferenced_files_deleted(0).await.unwrap() > 0 {}
    let deleted = |id: String| {
        let pool = pool.clone();
        async move {
            count(
                &pool,
                &format!("SELECT count(*) FROM files WHERE id = '{id}' AND deleted_at IS NOT NULL"),
            )
            .await
        }
    };
    assert_eq!(deleted(expired.to_string()).await, 1);
    assert_eq!(deleted(pending.to_string()).await, 0);
    assert_eq!(deleted(claimed).await, 0);
    server.abort();
}
