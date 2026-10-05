mod support;

use reqwest::header::{HeaderMap, HeaderValue};
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
    let (base_url, server) = start_server(pool.clone()).await;
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
        sqlx::query("UPDATE entities SET system_tags=ARRAY['attricat.sample']::text[] WHERE id=$1")
            .bind(source_id.parse::<Uuid>().unwrap())
            .execute(&pool)
            .await
            .unwrap();
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
    assert_eq!(first["items"][0]["is_sample"], true);

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
    assert_eq!(second["items"][0]["is_sample"], true);
    assert!(second["next_cursor"].is_null());

    server.abort();
}

#[sqlx::test]
async fn related_entity_previews_are_workspace_scoped_across_pages_and_deletions(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let other_workspace_id = Uuid::new_v4();
    let membership_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'related-preview-other', 'Related preview other', 'related-preview-other.test')",
    )
    .bind(other_workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    // A workspace inserted directly needs the default context that workspace
    // provisioning creates; entity creation resolves values against it.
    api::repository::CatalogRepository::system(pool.clone())
        .initialize_workspace(other_workspace_id)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership_id)
    .bind(other_workspace_id)
    .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000101', 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(other_workspace_id)
        .bind(membership_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut other_headers = HeaderMap::new();
    other_headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_static(BOOTSTRAP_OWNER_ID),
    );
    other_headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_str(&other_workspace_id.to_string()).unwrap(),
    );
    let other_client = Client::builder()
        .default_headers(other_headers)
        .build()
        .unwrap();

    let category_definition = r#"
format_version = 1
code = "previewcategory"
name = "Preview category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
"#;
    let product_definition = r#"
format_version = 1
code = "previewproduct"
name = "Preview product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "previewcategory"
"#;
    let category = create_blueprint(&client, &base_url, category_definition).await;
    let product = create_blueprint(&client, &base_url, product_definition).await;
    let first = create_entity(&client, &base_url, &category).await;
    let deleted = create_entity(&client, &base_url, &category).await;
    let last = create_entity(&client, &base_url, &category).await;
    let source = create_entity(&client, &base_url, &product).await;
    let source_id = source["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/entities/{source_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "categories",
            "target_entity_ids": [first["id"], deleted["id"], last["id"]]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .delete(format!(
            "{base_url}/entities/{}",
            deleted["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let other_category = create_blueprint(&other_client, &base_url, category_definition).await;
    let other_product = create_blueprint(&other_client, &base_url, product_definition).await;
    let other_target = create_entity(&other_client, &base_url, &other_category).await;
    let other_source = create_entity(&other_client, &base_url, &other_product).await;
    let other_source_id = other_source["id"].as_str().unwrap();
    other_client
        .post(format!(
            "{base_url}/entities/{other_source_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "categories",
            "target_entity_ids": [other_target["id"]]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let first_page: Value = client
        .get(format!("{base_url}/entities?blueprint=previewcategory&related_from={source_id}&relationship=categories&limit=1"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    assert_ne!(first_page["items"][0]["id"], deleted["id"]);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let second_page: Value = client
        .get(format!("{base_url}/entities?blueprint=previewcategory&related_from={source_id}&relationship=categories&limit=1&cursor={cursor}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
    assert_ne!(second_page["items"][0]["id"], deleted["id"]);
    assert!(second_page["next_cursor"].is_null());

    let foreign_source: Value = client
        .get(format!("{base_url}/entities?blueprint=previewcategory&related_from={other_source_id}&relationship=categories"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(foreign_source["items"].as_array().unwrap().is_empty());
    assert!(foreign_source["next_cursor"].is_null());
    assert_eq!(
        client
            .post(format!(
                "{base_url}/v1/entities/{}/incoming-relationships",
                other_target["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{
                    "source_blueprint": "previewproduct",
                    "field": "categories"
                }],
                "page": { "size": 1, "cursor": null }
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

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

#[sqlx::test]
async fn entity_labels_name_only_live_entities_the_caller_may_read(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base_url,
        r#"
format_version = 1
code = "labelled_product"
name = "Labelled product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let contexts: Vec<Value> = owner
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let default_context = contexts
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()["id"]
        .clone();
    let mut ids = Vec::new();
    for title in ["First", "Second", "Deleted"] {
        let entity: Value = owner
            .post(format!("{base_url}/v1/entities"))
            .json(&json!({
                "blueprint": {"code": "labelled_product", "version": blueprint["blueprint"]["version"]},
                "values": [{"kind": "scalar", "attribute_code": "title", "value": title, "context_id": default_context}],
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        ids.push(entity["id"].as_str().unwrap().parse::<Uuid>().unwrap());
    }
    let (first, second, deleted) = (ids[0], ids[1], ids[2]);
    owner
        .delete(format!("{base_url}/entities/{deleted}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let labels = |client: &Client, entity_ids: Vec<Uuid>| {
        client
            .post(format!("{base_url}/v1/entities/labels"))
            .json(&json!({ "entity_ids": entity_ids }))
            .send()
    };

    let named: Value = labels(&owner, vec![second, first, deleted, Uuid::new_v4(), first])
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut items = named["items"].as_array().unwrap().clone();
    items.sort_by_key(|item| item["display"]["default"].as_str().unwrap().to_owned());
    assert_eq!(items.len(), 2, "{named}");
    assert_eq!(items[0]["id"], json!(first));
    assert_eq!(items[0]["display"]["default"], "First");
    assert_eq!(items[0]["blueprint_code"], "labelled_product");
    assert_eq!(items[1]["id"], json!(second));
    assert_eq!(items[1]["display"]["default"], "Second");

    for entity_ids in [Vec::new(), (0..101).map(|_| Uuid::new_v4()).collect()] {
        let refused = labels(&owner, entity_ids).await.unwrap();
        assert!(refused.status().is_client_error(), "{}", refused.status());
    }

    // A viewer granted one entity learns nothing about the others.
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let mut clients = Vec::new();
    for (email, grant) in [
        ("label-entity-viewer@example.test", Some(first)),
        ("label-no-grant@example.test", None),
    ] {
        let user = Uuid::new_v4();
        let membership = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
            .bind(user)
            .bind(email)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
        )
        .bind(membership)
        .bind(workspace)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
        if let Some(entity) = grant {
            sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, '00000000-0000-4000-8000-000000000104', 'entity', $4)")
                .bind(Uuid::new_v4())
                .bind(workspace)
                .bind(membership)
                .bind(entity)
                .execute(&pool)
                .await
                .unwrap();
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-catalog-user-id",
            HeaderValue::from_str(&user.to_string()).unwrap(),
        );
        headers.insert(
            "x-catalog-workspace-id",
            HeaderValue::from_str(&workspace.to_string()).unwrap(),
        );
        clients.push(Client::builder().default_headers(headers).build().unwrap());
    }
    let scoped: Value = labels(&clients[0], vec![first, second])
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(scoped["items"].as_array().unwrap().len(), 1, "{scoped}");
    assert_eq!(scoped["items"][0]["id"], json!(first));
    // Without any read grant the lookup is filtered, like other entity lists.
    let ungranted: Value = labels(&clients[1], vec![first])
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ungranted["items"], json!([]));
    server.abort();
}
