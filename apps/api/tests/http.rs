use std::net::SocketAddr;

use api::{
    http::{AppState, router},
    repository::CatalogRepository,
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::{net::TcpListener, task::JoinHandle};
use uuid::Uuid;

async fn start_server(pool: PgPool) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let router = router(AppState {
        repository: CatalogRepository::new(pool),
        max_preview_relationship_depth: 3,
        max_preview_relationship_items: 10,
        max_entity_page_size: 100,
    });
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

    (format!("http://{address}"), server)
}

async fn create_blueprint(client: &Client, base_url: &str, definition: &str) -> Value {
    client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn create_entity(client: &Client, base_url: &str, blueprint: &Value) -> Value {
    client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "blueprint_id": blueprint["blueprint"]["id"],
            "blueprint_version": blueprint["blueprint"]["version"],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[sqlx::test]
async fn catalog_workflow_compiles_explicit_toml_selections_and_rebuilds_preview(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = Client::new();

    assert_eq!(
        client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        json!({ "status": "ok" })
    );

    let seo = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "seo"
name = "SEO metadata"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"

[[attributes]]
code = "meta_description"
value_type = "string"
"#,
    )
    .await;
    assert_eq!(seo["blueprint"]["kind"], "mixin");

    let product_definition = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "meta_title"
from = "seo.meta_title"

[[attributes]]
code = "related_products"
value_type = "relationship"

[[attributes]]
code = "stock"
value_type = "integer"
context_editable = "default"
"#;
    let blueprint = create_blueprint(&client, &base_url, product_definition).await;
    let blueprint_id: Uuid = blueprint["blueprint"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(blueprint["blueprint"]["code"], "product");
    assert_eq!(blueprint["blueprint"]["kind"], "entity");
    assert_eq!(
        blueprint["blueprint"]["display"],
        json!({ "dropdown_option": { "fields": ["title"], "separator": " · " } })
    );
    assert_eq!(
        blueprint["blueprint"]["includes"],
        json!([{ "alias": "seo", "code": "seo", "version": 1 }])
    );
    let attribute_contract: Vec<_> = blueprint["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|attribute| {
            json!({
                "code": attribute["code"],
                "value_type": attribute["value_type"],
                "position": attribute["position"],
            })
        })
        .collect();
    assert_eq!(
        attribute_contract,
        vec![
            json!({ "code": "title", "value_type": "string", "position": 0 }),
            json!({ "code": "meta_title", "value_type": "string", "position": 1 }),
            json!({ "code": "related_products", "value_type": "relationship", "position": 2 }),
            json!({ "code": "stock", "value_type": "integer", "position": 3 }),
        ],
        "attribute output should be ordered and only selected mixin fields should materialize"
    );
    let title_attribute_id = blueprint["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attribute| attribute["code"] == "title")
        .unwrap()["id"]
        .as_str()
        .unwrap();

    let resolved: Value = client
        .get(format!("{base_url}/blueprints/by-code/product"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resolved["blueprint"]["id"], blueprint["blueprint"]["id"]);
    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({
            "definition": r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["sku"]

[[attributes]]
code = "sku"
value_type = "string"
tags = ["searchable"]
"#
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revision["blueprint"]["version"], 2);
    let current_blueprint: Value = client
        .get(format!("{base_url}/blueprints/{blueprint_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current_blueprint["blueprint"]["version"], 2);
    let first_revision: Value = client
        .get(format!("{base_url}/blueprints/{blueprint_id}/versions/1"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first_revision["blueprint"]["version"], 1);

    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "en_GB", "data": { "language": "en-GB" } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!("{base_url}/contexts/en_GB"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["id"],
        context["id"]
    );
    let contexts: Value = client
        .get(format!("{base_url}/contexts"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(contexts, json!([context]));
    for data in [json!([]), json!("en-GB"), Value::Null] {
        let response = client
            .post(format!("{base_url}/contexts"))
            .json(&json!({ "code": Uuid::new_v4().to_string(), "data": data }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "invalid_input"
        );
    }
    for code in ["en-GB", "en GB", "en.GB", ""] {
        let response = client
            .post(format!("{base_url}/contexts"))
            .json(&json!({ "code": code, "data": {} }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "invalid_input"
        );
    }

    let source: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "blueprint_id": blueprint_id,
            "blueprint_version": 1
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let default_only_write = client
        .post(format!(
            "{base_url}/entities/{}/values",
            source["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": [{
            "kind": "scalar",
            "attribute_code": "stock",
            "context_id": context["id"],
            "value": 4
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        default_only_write.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        default_only_write.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_input"
    );
    assert_eq!(
        source["projections"],
        json!({ "preview": { "default": {} } })
    );

    let target: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "blueprint_id": blueprint_id,
            "blueprint_version": 1
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let response = client
        .post(format!(
            "{base_url}/entities/{}/values",
            source["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [
                {
                    "kind": "scalar",
                    "attribute_code": "title",
                    "context_id": null,
                    "value": "Blue shirt"
                },
                {
                    "kind": "scalar",
                    "attribute_id": title_attribute_id,
                    "context_id": context["id"],
                    "value": "Blue shirt (UK)"
                },
                {
                    "kind": "relationship",
                    "attribute_code": "related_products",
                    "context_id": null,
                    "target_entity_id": target["id"]
                }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    assert_eq!(
        client
            .get(format!(
                "{base_url}/entities/{}/values/current",
                source["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Vec<Value>>()
            .await
            .unwrap()
            .len(),
        3
    );

    let preview: Value = client
        .get(format!(
            "{base_url}/entities/{}/preview",
            source["id"].as_str().unwrap()
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
        preview["context"],
        json!({
            "default": {
                "title": "Blue shirt",
                "related_products": {
                    "items": [{ "id": target["id"], "display": "" }],
                    "truncated": false
                }
            },
            "en_GB": { "title": "Blue shirt (UK)" }
        })
    );
    assert_eq!(preview["entity"]["id"], source["id"]);
    assert_eq!(
        preview["entity"]["blueprint_id"],
        blueprint["blueprint"]["id"]
    );
    assert_eq!(
        preview["entity"]["blueprint_version"],
        blueprint["blueprint"]["version"]
    );

    let replacement = client
        .post(format!(
            "{base_url}/entities/{}/values",
            source["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [
                {
                    "kind": "scalar",
                    "attribute_code": "title",
                    "value": "Green shirt"
                },
                {
                    "kind": "scalar",
                    "attribute_code": "title",
                    "value": "Red shirt"
                }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(replacement.status(), StatusCode::CREATED);

    let current_values: Vec<Value> = client
        .get(format!(
            "{base_url}/entities/{}/values/current",
            source["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let current_default_title: Vec<_> = current_values
        .iter()
        .filter(|value| {
            value["attribute_id"] == title_attribute_id && value["context_id"].is_null()
        })
        .collect();
    assert_eq!(current_default_title.len(), 1);
    assert_eq!(current_default_title[0]["value"], "Red shirt");

    let current_preview: Value = client
        .get(format!(
            "{base_url}/entities/{}/preview?relationship_depth=0",
            source["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current_preview["context"]["default"]["title"], "Red shirt");

    let source_id: Uuid = source["id"].as_str().unwrap().parse().unwrap();
    let title_attribute_id: Uuid = title_attribute_id.parse().unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attribute_values WHERE entity_id = $1 AND attribute_id = $2 AND context_id IS NULL AND relationship_target_entity_id IS NULL AND latest"
        )
        .bind(source_id)
        .bind(title_attribute_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attribute_values WHERE entity_id = $1 AND attribute_id = $2 AND context_id IS NULL AND relationship_target_entity_id IS NULL"
        )
        .bind(source_id)
        .bind(title_attribute_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );

    let search_page: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "product", "version": 1 },
            "query": "red shirt",
            "filters": [],
            "page": { "size": 1, "cursor": null }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search_page["blueprint"]["blueprint"]["version"], 1);
    assert_eq!(search_page["items"][0]["id"], source["id"]);
    assert_eq!(
        search_page["items"][0]["display"],
        json!({ "default": "Red shirt", "en_GB": "Blue shirt (UK)" })
    );
    assert!(search_page["next_cursor"].is_null());

    let created_from_form: Value = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": { "code": "product", "version": 1 },
            "values": [{
                "kind": "scalar",
                "attribute_code": "title",
                "value": "Created from form"
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let form_entity_id = created_from_form["id"].as_str().unwrap();
    let form: Value = client
        .get(format!("{base_url}/v1/entities/{form_entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        form["values"][0],
        json!({ "kind": "scalar", "attribute_code": "title", "context_id": null, "value": "Created from form" })
    );
    assert_eq!(form["context"]["default"]["title"], "Created from form");

    let updated: Value = client
        .put(format!("{base_url}/v1/entities/{form_entity_id}"))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_code": "title",
                "value": "Updated from form"
            }]
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
        updated["projections"]["preview"]["default"]["title"],
        "Updated from form"
    );

    client
        .put(format!(
            "{base_url}/v1/entities/{}",
            source["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [],
            "relationships": [{
                "attribute_code": "related_products",
                "target_entity_ids": []
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let removed_relationship_preview: Value = client
        .get(format!(
            "{base_url}/entities/{}/preview",
            source["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        removed_relationship_preview["context"]["default"]
            .get("related_products")
            .is_none()
    );

    server.abort();
}

#[sqlx::test]
async fn rejects_invalid_toml_and_mixin_entity_creation(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

    let invalid = client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({
            "definition": "format_version = 1\ncode = 'bad'\nname = 'Bad'\nkind = 'entity'"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_blueprint_definition"
    );

    let mixin = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "visibility"
name = "Visibility"
kind = "mixin"

[[attributes]]
code = "visible"
value_type = "boolean"
"#,
    )
    .await;
    let entity = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "blueprint_id": mixin["blueprint"]["id"],
            "blueprint_version": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(entity.status(), StatusCode::NOT_FOUND);

    server.abort();
}

#[sqlx::test]
async fn stores_typed_scalar_values_in_native_columns(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = Client::new();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "measurement"
name = "Measurement"
kind = "entity"

[display.dropdown_option]
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "quantity"
value_type = "integer"

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "available_on"
value_type = "date"

[[attributes]]
code = "released_at"
value_type = "datetime"

[[attributes]]
code = "cutoff"
value_type = "time"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id: Uuid = entity["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base_url}/entities/{entity_id}/values"))
        .json(&json!({ "values": [
            { "kind": "scalar", "attribute_code": "name", "value": "Widget" },
            { "kind": "scalar", "attribute_code": "price", "value": 12.50 },
            { "kind": "scalar", "attribute_code": "quantity", "value": 3 },
            { "kind": "scalar", "attribute_code": "available", "value": true },
            { "kind": "scalar", "attribute_code": "available_on", "value": "2026-08-12" },
            { "kind": "scalar", "attribute_code": "released_at", "value": "2026-08-12T14:30:00Z" },
            { "kind": "scalar", "attribute_code": "cutoff", "value": { "time": "09:30:00", "time_zone": "America/New_York" } }
        ] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let values: Vec<Value> = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(values.iter().any(|value| value["value"] == json!(true)));
    assert!(values.iter().any(|value| value["value"] == json!(12.50)));
    assert!(
        values
            .iter()
            .any(|value| value["value"] == json!("2026-08-12T14:30:00+00:00"))
    );
    assert!(
        values.iter().any(|value| value["value"]
            == json!({ "time": "09:30:00", "time_zone": "America/New_York" }))
    );
    assert_eq!(
        sqlx::query_scalar::<_, rust_decimal::Decimal>(
            "SELECT SUM(value_number) FROM attribute_values WHERE entity_id = $1 AND latest"
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "12.50".parse().unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, bool>(
            "SELECT value_boolean FROM attribute_values WHERE entity_id = $1 AND latest AND value_boolean IS NOT NULL"
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        true
    );

    server.abort();
}

#[sqlx::test]
async fn rejects_mismatched_native_value_storage_on_read(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = Client::new();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "typed-read"
name = "Typed read"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id: Uuid = entity["id"].as_str().unwrap().parse().unwrap();
    let attribute_id: Uuid = blueprint["attributes"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    sqlx::query(
        "INSERT INTO attribute_values (id, entity_id, attribute_id, value_number) VALUES ($1, $2, $3, 12.50)",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(attribute_id)
    .execute(&pool)
    .await
    .unwrap();

    let response = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    server.abort();
}

#[sqlx::test]
async fn rejects_values_from_another_blueprint_version(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

    let first = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "first"
name = "First"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let second = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "second"
name = "Second"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let entity: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "blueprint_id": first["blueprint"]["id"],
            "blueprint_version": 1
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let response = client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_id": second["attributes"][0]["id"],
                "context_id": null,
                "value": "invalid"
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "error": {
                "code": "attribute_not_applicable",
                "message": "attribute does not belong to the entity blueprint version"
            }
        })
    );

    let response = client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_id": first["attributes"][0]["id"],
                "attribute_code": "title",
                "value": "invalid"
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "error": {
                "code": "invalid_input",
                "message": "provide exactly one of attribute_id or attribute_code"
            }
        })
    );

    server.abort();
}

#[sqlx::test]
async fn typed_category_and_color_relationships_can_be_replaced_and_removed(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();
    let category = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[display.dropdown_option]
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]
"#,
    )
    .await;
    let color = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "color"
name = "Color"
kind = "entity"

[display.dropdown_option]
fields = ["name", "hex"]

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "hex"
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

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "colors"
value_type = "relationship"
target_blueprint = "color"
"#,
    )
    .await;
    assert_eq!(
        product["attributes"][1]["target_blueprint_code"],
        "category"
    );

    let shirts = create_entity(&client, &base_url, &category).await;
    let sale = create_entity(&client, &base_url, &category).await;
    let navy = create_entity(&client, &base_url, &color).await;
    let shirt = create_entity(&client, &base_url, &product).await;
    let shirt_id = shirt["id"].as_str().unwrap();

    for (entity, values) in [
        (
            &shirts,
            json!({ "values": [{ "kind": "scalar", "attribute_code": "name", "value": "Shirts" }] }),
        ),
        (
            &sale,
            json!({ "values": [{ "kind": "scalar", "attribute_code": "name", "value": "Sale" }] }),
        ),
        (
            &navy,
            json!({ "values": [
            { "kind": "scalar", "attribute_code": "name", "value": "Navy" },
            { "kind": "scalar", "attribute_code": "hex", "value": "#1c2d4a" }
        ] }),
        ),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&values)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    client
        .post(format!("{base_url}/entities/{shirt_id}/values"))
        .json(&json!({ "values": [{
            "kind": "scalar", "attribute_code": "title", "value": "Navy shirt"
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let invalid = client
        .post(format!("{base_url}/entities/{shirt_id}/values"))
        .json(&json!({ "values": [{
            "kind": "relationship", "attribute_code": "categories", "target_entity_id": navy["id"]
        }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["code"],
        "relationship_target_type_mismatch"
    );

    let replace = client
        .post(format!(
            "{base_url}/entities/{shirt_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [
            { "attribute_code": "categories", "target_entity_ids": [shirts["id"], sale["id"]] },
            { "attribute_code": "colors", "target_entity_ids": [navy["id"]] }
        ] }))
        .send()
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::CREATED);

    let preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        preview["context"]["default"]["categories"]["truncated"],
        false
    );
    assert_eq!(
        preview["context"]["default"]["colors"]["items"][0]["display"],
        "Navy · #1c2d4a"
    );

    let bounded_preview: Value = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_limit=1"
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
        bounded_preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        bounded_preview["context"]["default"]["categories"]["truncated"],
        true
    );

    let scalar_preview: Value = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_depth=0"
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
        scalar_preview["context"],
        json!({
            "default": {
                "title": "Navy shirt",
                "categories": { "items": [], "truncated": true },
                "colors": { "items": [], "truncated": true }
            }
        })
    );

    let excessive_depth = client
        .get(format!(
            "{base_url}/entities/{shirt_id}/preview?relationship_depth=4"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(excessive_depth.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let first_page: Value = client
        .get(format!(
            "{base_url}/entities?blueprint=category&related_from={shirt_id}&relationship=categories&limit=1"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let second_page: Value = client
        .get(format!(
            "{base_url}/entities?blueprint=category&related_from={shirt_id}&relationship=categories&limit=1&cursor={cursor}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
    assert!(second_page["next_cursor"].is_null());

    client
        .post(format!(
            "{base_url}/entities/{shirt_id}/relationships/remove"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "categories", "target_entity_ids": [sale["id"]]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let preview: Value = client
        .get(format!("{base_url}/entities/{shirt_id}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        preview["context"]["default"]["categories"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        preview["context"]["default"]["categories"]["items"][0]["display"],
        "Shirts"
    );

    server.abort();
}
