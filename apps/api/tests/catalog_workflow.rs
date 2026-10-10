mod support;

use support::*;

#[sqlx::test]
async fn catalog_workflow_compiles_explicit_toml_selections_and_rebuilds_preview(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    assert_eq!(
        client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        json!({ "status": "live" })
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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
    assert_eq!(blueprint["blueprint"]["kind"], "record");
    assert_eq!(
        blueprint["blueprint"]["views"]["dropdown_option"],
        json!({ "type": "dropdown_option", "fields": ["title"], "separator": " · " })
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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
    client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/2/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
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
    assert!(contexts.as_array().unwrap().contains(&context));
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
    for code in ["en GB", "en.GB", ""] {
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

    let source = create_record(&client, &base_url, &blueprint).await;
    let default_only_write = client
        .post(format!(
            "{base_url}/records/{}/values",
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

    let target = create_record(&client, &base_url, &blueprint).await;

    let response = client
        .post(format!(
            "{base_url}/records/{}/values",
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
                    "target_record_id": target["id"]
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
                "{base_url}/records/{}/values/current",
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
            "{base_url}/records/{}/preview",
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
    assert_eq!(preview["record"]["id"], source["id"]);
    assert_eq!(
        preview["record"]["blueprint_id"],
        blueprint["blueprint"]["id"]
    );
    assert_eq!(
        preview["record"]["blueprint_version"],
        blueprint["blueprint"]["version"]
    );

    let replacement = client
        .post(format!(
            "{base_url}/records/{}/values",
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
            "{base_url}/records/{}/values/current",
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
            value["attribute_id"] == title_attribute_id
                && value["context_id"] == "00000000-0000-4000-8000-000000000001"
        })
        .collect();
    assert_eq!(current_default_title.len(), 1);
    assert_eq!(current_default_title[0]["value"], "Red shirt");

    let current_preview: Value = client
        .get(format!(
            "{base_url}/records/{}/preview?relationship_depth=0",
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
            "SELECT COUNT(*) FROM attribute_values WHERE record_id = $1 AND attribute_id = $2 AND context_id = $3 AND relationship_target_record_id IS NULL"
        )
        .bind(source_id)
        .bind(title_attribute_id)
        .bind(Uuid::from_u128(0x00000000000040008000000000000001))
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM attribute_value_history WHERE record_id = $1 AND attribute_id = $2 AND context_id = $3 AND relationship_target_record_id IS NULL"
        )
        .bind(source_id)
        .bind(title_attribute_id)
        .bind(Uuid::from_u128(0x00000000000040008000000000000001))
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );

    let search_page: Value = client
        .post(format!("{base_url}/v1/records/search"))
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
    assert_eq!(search_page["items"][0]["schema_outdated"], true);
    assert_eq!(
        search_page["items"][0]["display"],
        json!({ "default": "Red shirt", "en_GB": "Blue shirt (UK)" })
    );
    assert!(search_page["next_cursor"].is_null());

    let created_from_form: Value = client
        .post(format!("{base_url}/v1/records"))
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
    let form_record_id = created_from_form["id"].as_str().unwrap();
    let form: Value = client
        .get(format!("{base_url}/v1/records/{form_record_id}"))
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
        json!({ "kind": "scalar", "attribute_code": "title", "context_id": "00000000-0000-4000-8000-000000000001", "value": "Created from form" })
    );
    assert_eq!(form["context"]["default"]["title"], "Created from form");

    let updated: Value = client
        .put(format!("{base_url}/v1/records/{form_record_id}"))
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
            "{base_url}/v1/records/{}",
            source["id"].as_str().unwrap()
        ))
        .json(&json!({
            "values": [],
            "relationships": [{
                "attribute_code": "related_products",
                "target_record_ids": []
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let removed_relationship_preview: Value = client
        .get(format!(
            "{base_url}/records/{}/preview",
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
