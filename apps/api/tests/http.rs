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

#[sqlx::test]
async fn catalog_workflow_compiles_explicit_toml_selections_and_rebuilds_preview(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = Client::new();

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

[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "meta_title"
from = "seo.meta_title"

[[attributes]]
code = "related_products"
value_type = "relationship"
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
    let relationship_attribute_id = blueprint["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attribute| attribute["code"] == "related_products")
        .unwrap()["id"]
        .as_str()
        .unwrap();

    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({
            "definition": r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "sku"
value_type = "string"
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

    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "en-GB", "data": { "language": "en-GB" } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let source: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "code": "source",
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
    assert_eq!(
        source["projections"],
        json!({ "preview": { "default": {} } })
    );

    let target: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "code": "target",
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
                    "attribute_id": title_attribute_id,
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
                    "attribute_id": relationship_attribute_id,
                    "context_id": null,
                    "target_entity_id": target["id"]
                }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let preview: Value = client
        .get(format!(
            "{base_url}/entities/{}/projections/preview",
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
        preview,
        json!({
            "default": { "title": "Blue shirt" },
            "en-GB": { "title": "Blue shirt (UK)" }
        })
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
            "code": "not-allowed",
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

[[attributes]]
code = "title"
value_type = "string"
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

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let entity: Value = client
        .post(format!("{base_url}/entities"))
        .json(&json!({
            "code": "entity",
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

    server.abort();
}
