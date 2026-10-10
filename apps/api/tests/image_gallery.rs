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
kind = "record"
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
    let record = create_record(&client, &base, &bp).await;
    let record_id = record["id"].as_str().unwrap();
    let url = format!("{base}/records/{record_id}/file-attributes/photos");
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
        StatusCode::OK
    );
    let form: Value = client
        .get(format!("{base}/v1/records/{record_id}"))
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
        StatusCode::OK
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
        StatusCode::OK
    );
    let form: Value = client
        .get(format!("{base}/v1/records/{record_id}"))
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
    let record = create_record(&client, &base, &bp).await;
    let other = create_record(&client, &base, &bp).await;
    let url = format!(
        "{base}/records/{}/file-attributes/photos",
        record["id"].as_str().unwrap()
    );
    let other_url = format!(
        "{base}/records/{}/file-attributes/photos",
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
        StatusCode::OK
    );
    server.abort();
}

#[sqlx::test]
async fn readonly_file_attributes_reject_uploads_and_reference_changes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool, store.clone()).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "readonly = true").await;
    let record = create_record(&client, &base, &bp).await;
    let url = format!(
        "{base}/records/{}/file-attributes/photos",
        record["id"].as_str().unwrap()
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
    let record = create_record(&client, &base, &bp).await;
    let id = record["id"].as_str().unwrap();
    let url = format!("{base}/records/{id}/file-attributes/photos");
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
    let resolved_url = format!("{base}/records/{id}/resolved-preview?context_id={context}");
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
    assert_eq!(client.put(format!("{url}/references")).json(&json!({"context_id":context,"expected_file_ids":[local["files"][0]["id"]],"file_ids":[]})).send().await.unwrap().status(), StatusCode::OK);
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
            "{base}/records/{id}/resolved-preview?context_id={default_id}"
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
    let record = create_record(&client, &base, &bp).await;
    let url = format!(
        "{base}/records/{}/file-attributes/photos",
        record["id"].as_str().unwrap()
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

#[sqlx::test]
async fn gallery_changes_return_the_version_an_open_form_saves_against(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = true").await;
    let record = create_record(&client, &base, &bp).await;
    let record_id = record["id"].as_str().unwrap();
    let loaded_version = record["updated_at"].clone();
    let url = format!("{base}/records/{record_id}/file-attributes/photos");
    let uploaded: Value = upload(&client, &url)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let file = uploaded["files"][0]["id"].clone();
    let changed: Value = client
        .put(format!("{url}/references"))
        .json(
            &json!({"context_id":uploaded["context_id"],"expected_file_ids":[file],"file_ids":[]}),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let save = |version: &Value| {
        client
            .put(format!("{base}/v1/records/{record_id}"))
            .json(&json!({"expected_updated_at":version,"values":[]}))
    };
    assert_eq!(
        save(&loaded_version).send().await.unwrap().status(),
        StatusCode::CONFLICT,
        "the version loaded before the gallery change is stale"
    );
    let saved = save(&changed["record_updated_at"]).send().await.unwrap();
    assert_eq!(
        saved.status(),
        StatusCode::OK,
        "{}",
        saved.text().await.unwrap()
    );
    server.abort();
}

#[sqlx::test]
async fn gallery_writes_archive_history_and_emit_audited_value_changes(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = true").await;
    let record = create_record(&client, &base, &bp).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    let url = format!("{base}/records/{record_id}/file-attributes/photos");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let uploaded: Value = upload(&client, &url)
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        ids.push(uploaded["files"][0]["id"].as_str().unwrap().to_owned());
    }
    let (a, b) = (&ids[0], &ids[1]);
    client
        .put(format!("{url}/references"))
        .json(&json!({"expected_file_ids":[a, b],"file_ids":[b, a]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Appending archives the previous list just like a reorder does.
    let history: Vec<Vec<String>> = sqlx::query_scalar(
        "SELECT array_agg(r.file_id::text ORDER BY r.position) FROM attribute_value_history h JOIN attribute_file_reference_history r ON r.attribute_value_history_id = h.id AND r.attribute_value_history_archived_at = h.archived_at WHERE h.record_id = $1 GROUP BY h.id, h.created_at ORDER BY h.created_at",
    )
    .bind(record_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(history, vec![vec![a.clone()], vec![a.clone(), b.clone()]]);

    let events: Vec<(String, Value)> = sqlx::query_as(
        "SELECT event_type, payload FROM domain_events WHERE aggregate_id = $1 AND event_type = 'attribute_value.changed.v1' ORDER BY sequence",
    )
    .bind(record_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let facts: Vec<(Value, Value)> = events
        .iter()
        .map(|(_, payload)| {
            let fact = &payload["facts"][0];
            assert_eq!(fact["attribute_code"], "photos");
            (fact["before_value"].clone(), fact["after_value"].clone())
        })
        .collect();
    assert_eq!(
        facts,
        vec![
            (Value::Null, json!([a])),
            (json!([a]), json!([a, b])),
            (json!([a, b]), json!([b, a])),
        ]
    );
    let audited: Vec<String> = sqlx::query_scalar(
        "SELECT change_kind FROM audit_event_changes c WHERE c.record_id = $1 AND c.attribute_code = 'photos' ORDER BY c.created_at",
    )
    .bind(record_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audited, ["set", "replace", "replace"]);
    server.abort();
}

#[sqlx::test]
async fn duplicating_a_gallery_writes_its_list_once_without_history(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store).await;
    let client = authenticated_client();
    let bp = blueprint(&client, &base, "ordered = true").await;
    let record = create_record(&client, &base, &bp).await;
    let record_id = record["id"].as_str().unwrap();
    let url = format!("{base}/records/{record_id}/file-attributes/photos");
    let mut ids = Vec::new();
    for _ in 0..3 {
        let uploaded: Value = upload(&client, &url)
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        ids.push(uploaded["files"][0]["id"].as_str().unwrap().to_owned());
    }
    let reordered = vec![ids[2].clone(), ids[0].clone(), ids[1].clone()];
    client
        .put(format!("{url}/references"))
        .json(&json!({"expected_file_ids": ids, "file_ids": reordered}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let copy: Value = client
        .post(format!("{base}/v1/records/{record_id}/duplicate"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let copy_id: Uuid = copy["id"].as_str().unwrap().parse().unwrap();
    let current: Vec<String> = sqlx::query_scalar(
        "SELECT r.file_id::text FROM attribute_values v JOIN attribute_file_references r ON r.attribute_value_id = v.id WHERE v.record_id = $1 ORDER BY r.position",
    )
    .bind(copy_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(current, reordered);
    let archived: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM attribute_value_history WHERE record_id = $1")
            .bind(copy_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(archived, 0);
    server.abort();
}
