mod support;

use support::*;

#[sqlx::test]
async fn system_annotations_are_persisted_updated_and_searchable(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = 'annotated_product'\nname = 'Annotated product'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'",
    )
    .await;

    let entity: Value = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "system_tags": ["agent:pending", "debug"],
            "system_metadata": { "run_id": "run-42", "attempt": 1 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let entity_id = entity["id"].as_str().unwrap();
    assert_eq!(entity["system_tags"], json!(["agent:pending", "debug"]));
    assert_eq!(entity["system_metadata"]["run_id"], "run-42");

    let search: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "filters": [],
            "system_tags": ["agent:pending", "debug"],
            "page": { "size": 10 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["items"].as_array().unwrap().len(), 1);

    let updated: Value = client
        .put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&json!({ "system_metadata": { "run_id": "run-42", "result": "ok" } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["system_tags"], json!(["agent:pending", "debug"]));
    assert_eq!(updated["system_metadata"]["result"], "ok");

    let updated: Value = client
        .put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&json!({ "system_tags": ["agent:complete"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["system_tags"], json!(["agent:complete"]));
    assert_eq!(updated["system_metadata"]["result"], "ok");

    let search: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "annotated_product" },
            "filters": [],
            "system_tags": ["agent:complete"],
            "page": { "size": 10 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["items"].as_array().unwrap().len(), 1);
    assert_eq!(search["items"][0]["id"], entity["id"]);

    let cleared: Value = client
        .put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&json!({ "system_tags": [], "system_metadata": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cleared["system_tags"], json!([]));
    assert_eq!(cleared["system_metadata"], json!({}));

    assert_eq!(blueprint["blueprint"]["code"], "annotated_product");
    server.abort();
}

#[sqlx::test]
async fn system_annotations_require_an_object_and_unique_tags(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = 'system_annotation_validation'\nname = 'System annotation validation'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'", 
    )
    .await;

    let response = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": { "code": "system_annotation_validation" },
            "system_tags": ["duplicate", "duplicate"]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_system_tags");

    let response = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": { "code": "system_annotation_validation" },
            "system_metadata": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["code"], "invalid_system_metadata");

    server.abort();
}
