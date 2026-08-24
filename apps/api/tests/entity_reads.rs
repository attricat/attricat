mod support;

use support::*;

#[sqlx::test]
async fn resolved_preview_includes_entity_identity(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let resolved: Value = client
        .get(format!(
            "{base_url}/entities/{entity_id}/resolved-preview?context_id=00000000-0000-4000-8000-000000000001"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(resolved["entity"]["id"], entity["id"]);
    assert_eq!(
        resolved["entity"]["blueprint_id"],
        blueprint["blueprint"]["id"]
    );
    assert_eq!(resolved["entity"]["blueprint_version"], 1);

    server.abort();
}

#[sqlx::test]
async fn incoming_relationships_are_deduplicated_and_paginated(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let category = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#,
    )
    .await;
    let product = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "featured_category"
value_type = "relationship"
target_blueprint = "category"
"#,
    )
    .await;
    let target = create_entity(&client, &base_url, &category).await;
    let target_id = target["id"].as_str().unwrap();
    let sources = [
        create_entity(&client, &base_url, &product).await,
        create_entity(&client, &base_url, &product).await,
    ];
    for source in &sources {
        let source_id = source["id"].as_str().unwrap();
        client
            .put(format!("{base_url}/v1/entities/{source_id}"))
            .json(&json!({
                "values": [],
                "relationships": [
                    { "attribute_code": "categories", "target_entity_ids": [target["id"]] },
                    { "attribute_code": "featured_category", "target_entity_ids": [target["id"]] }
                ],
                "remove_values": []
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let request = json!({
        "relationships": [
            { "source_blueprint": "product", "field": "categories" },
            { "source_blueprint": "product", "field": "featured_category" }
        ],
        "page": { "size": 1, "cursor": null }
    });
    let first: Value = client
        .post(format!(
            "{base_url}/v1/entities/{target_id}/incoming-relationships"
        ))
        .json(&request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 1);

    let second: Value = client
        .post(format!(
            "{base_url}/v1/entities/{target_id}/incoming-relationships"
        ))
        .json(&json!({
            "relationships": request["relationships"],
            "page": { "size": 1, "cursor": first["next_cursor"] }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert!(second["next_cursor"].is_null());

    server.abort();
}

#[sqlx::test]
async fn legacy_entity_creation_route_is_unavailable(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let response = client
        .post(format!("{base_url}/entities"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

    server.abort();
}

#[sqlx::test]
async fn extractor_failures_use_the_api_error_shape(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let invalid_path = client
        .get(format!("{base_url}/entities/not-a-uuid"))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid_path.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid_path.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );

    let malformed_body = client
        .post(format!("{base_url}/blueprints"))
        .header("content-type", "application/json")
        .body("{")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed_body.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        malformed_body.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );

    server.abort();
}

#[sqlx::test]
async fn soft_deleted_entity_is_hidden_from_reads_and_relationship_previews(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let category = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"deletablecategory\"\nname = \"Deletable category\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"name\"]\n\n[[attributes]]\ncode = \"name\"\nvalue_type = \"string\"\ntags = [\"searchable\"]",
    )
    .await;
    let product = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"deletableproduct\"\nname = \"Deletable product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"\ntags = [\"searchable\"]\n\n[[attributes]]\ncode = \"categories\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"deletablecategory\"",
    )
    .await;
    let default_context = client
        .get(format!("{base_url}/contexts/default"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let category_entity = create_entity(&client, &base_url, &category).await;
    let product_entity = create_entity(&client, &base_url, &product).await;
    let category_id = category_entity["id"].as_str().unwrap();
    let product_id = product_entity["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/entities/{product_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "categories",
            "context_id": default_context["id"],
            "target_entity_ids": [category_id],
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    assert_eq!(
        client
            .delete(format!("{base_url}/entities/{category_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .get(format!("{base_url}/entities/{category_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let preview: Value = client
        .get(format!("{base_url}/entities/{product_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(preview["context"]["default"].get("categories").is_none());
    assert_eq!(
        client
            .delete(format!("{base_url}/entities/{category_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    server.abort();
}
