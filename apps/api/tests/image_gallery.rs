mod support;

use api::storage::FakeObjectStore;
use reqwest::multipart::{Form, Part};
use std::sync::Arc;
use support::*;

async fn blueprint(client: &Client, base: &str, policy: &str) -> Value {
    create_blueprint(
        client,
        base,
        &format!(
            r#"format_version = 1
code = "gallery"
name = "Gallery"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["photos"]
[[attributes]]
code = "photos"
value_type = "file"
cardinality = "many"
image_only = true
{policy}
"#
        ),
    )
    .await
}

async fn upload(client: &Client, url: &str) -> reqwest::Response {
    client
        .post(format!("{url}/uploads"))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"\x89PNG\r\n\x1a\nimage".to_vec())
                    .file_name("photo.png")
                    .mime_str("image/png")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn gallery_references_are_ordered_conflict_checked_and_archived(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = true").await;
    let entity = create_entity(&client, &base, &bp).await;
    let entity_id = entity["id"].as_str().unwrap();
    let url = format!("{base}/entities/{entity_id}/file-attributes/photos");
    let first: Value = upload(&client, &url)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: Value = upload(&client, &url)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let a = first["files"][0]["id"].clone();
    let b = second["files"][0]["id"].clone();
    let context = first["context_id"].clone();
    let update = |expected: Value, ids: Value| {
        client
            .put(format!("{url}/references"))
            .json(&json!({"context_id":context,"expected_file_ids":expected,"file_ids":ids}))
    };
    for invalid in [json!([a, a]), json!([Uuid::new_v4()])] {
        assert_eq!(
            update(json!([a, b]), invalid)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        update(json!([a, b]), json!([b, a]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let form: Value = client
        .get(format!("{base}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(form["can_write"], true);
    assert_eq!(form["values"][0]["files"][0]["id"], b);
    assert_eq!(
        update(json!([a, b]), json!([a]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        update(json!([b, a]), json!([a]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        store.object_count().await,
        2,
        "detaching must not delete shared bytes"
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM attribute_file_reference_history")
            .fetch_one(&pool)
            .await
            .unwrap()
            >= 4
    );
    assert_eq!(
        update(json!([a]), json!([])).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    let form: Value = client
        .get(format!("{base}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        form["values"][0]["files"],
        json!([]),
        "an empty local value must survive reload"
    );
    let third: Value = upload(&client, &url)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        update(json!([]), json!([])).send().await.unwrap().status(),
        StatusCode::CONFLICT,
        "a concurrent upload must not be lost"
    );
    assert_ne!(third["files"][0]["id"], a);
    server.abort();
}

#[sqlx::test]
async fn unordered_gallery_allows_removal_but_not_reordering_or_foreign_references(pool: PgPool) {
    let (base, server) =
        start_server_with_object_store(pool, Arc::new(FakeObjectStore::available())).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = false").await;
    let entity = create_entity(&client, &base, &bp).await;
    let other = create_entity(&client, &base, &bp).await;
    let url = format!(
        "{base}/entities/{}/file-attributes/photos",
        entity["id"].as_str().unwrap()
    );
    let other_url = format!(
        "{base}/entities/{}/file-attributes/photos",
        other["id"].as_str().unwrap()
    );
    let a: Value = upload(&client, &url).await.json().await.unwrap();
    let b: Value = upload(&client, &url).await.json().await.unwrap();
    let foreign: Value = upload(&client, &other_url).await.json().await.unwrap();
    let a = a["files"][0]["id"].clone();
    let b = b["files"][0]["id"].clone();
    for ids in [json!([b, a]), json!([foreign["files"][0]["id"]])] {
        assert_eq!(
            client
                .put(format!("{url}/references"))
                .json(&json!({"expected_file_ids":[a,b],"file_ids":ids}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        client
            .put(format!("{url}/references"))
            .json(&json!({"expected_file_ids":[a,b],"file_ids":[b]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    server.abort();
}

#[sqlx::test]
async fn readonly_file_attributes_reject_uploads_and_reference_changes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "readonly = true").await;
    let entity = create_entity(&client, &base, &bp).await;
    let url = format!(
        "{base}/entities/{}/file-attributes/photos",
        entity["id"].as_str().unwrap()
    );
    let response = upload(&client, &url).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "file_attribute_readonly");
    let response = client
        .put(format!("{url}/references"))
        .json(&json!({"expected_file_ids":[],"file_ids":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(store.object_count().await, 0);
    server.abort();
}

#[sqlx::test]
async fn gallery_edits_are_local_and_empty_values_stop_inheritance(pool: PgPool) {
    let (base, server) =
        start_server_with_object_store(pool, Arc::new(FakeObjectStore::available())).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = true").await;
    let entity = create_entity(&client, &base, &bp).await;
    let id = entity["id"].as_str().unwrap();
    let url = format!("{base}/entities/{id}/file-attributes/photos");
    let original: Value = upload(&client, &url)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let child: Value = client
        .post(format!("{base}/contexts"))
        .json(&json!({"code":"gallery_child", "data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let context = child["id"].as_str().unwrap();
    let resolved_url = format!("{base}/entities/{id}/resolved-preview?context_id={context}");
    let inherited: Value = client
        .get(&resolved_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        inherited["values"]["photos"]["value"][0]["id"],
        original["files"][0]["id"]
    );
    assert_eq!(client.put(format!("{url}/references")).json(&json!({"context_id":context,"expected_file_ids":[original["files"][0]["id"]],"file_ids":[]})).send().await.unwrap().status(), StatusCode::CONFLICT, "inherited references cannot be mutated through a child context");
    let local: Value = client
        .post(format!("{url}/uploads"))
        .multipart(
            Form::new().text("context_id", context.to_owned()).part(
                "file",
                Part::bytes(b"\x89PNG\r\n\x1a\nlocal".to_vec())
                    .file_name("local.png")
                    .mime_str("image/png")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(client.put(format!("{url}/references")).json(&json!({"context_id":context,"expected_file_ids":[local["files"][0]["id"]],"file_ids":[]})).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    let cleared: Value = client
        .get(&resolved_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["values"]["photos"]["value"], json!([]));
    let default_id = original["context_id"].as_str().unwrap();
    let unchanged: Value = client
        .get(format!(
            "{base}/entities/{id}/resolved-preview?context_id={default_id}"
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
        unchanged["values"]["photos"]["value"][0]["id"],
        original["files"][0]["id"]
    );
    server.abort();
}

#[sqlx::test]
async fn default_context_policy_blocks_reference_changes_in_children(pool: PgPool) {
    let (base, server) =
        start_server_with_object_store(pool, Arc::new(FakeObjectStore::available())).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "context_editable = \"default\"").await;
    let entity = create_entity(&client, &base, &bp).await;
    let url = format!(
        "{base}/entities/{}/file-attributes/photos",
        entity["id"].as_str().unwrap()
    );
    let child: Value = client
        .post(format!("{base}/contexts"))
        .json(&json!({"code":"restricted_child", "data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let context = child["id"].as_str().unwrap();
    assert_eq!(
        client
            .put(format!("{url}/references"))
            .json(&json!({"context_id":context,"expected_file_ids":[],"file_ids":[]}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        client
            .post(format!("{url}/uploads"))
            .multipart(
                Form::new().text("context_id", context.to_owned()).part(
                    "file",
                    Part::bytes(b"\x89PNG\r\n\x1a\nlocal".to_vec())
                        .file_name("local.png")
                        .mime_str("image/png")
                        .unwrap()
                )
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    server.abort();
}
