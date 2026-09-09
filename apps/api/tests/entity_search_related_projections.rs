mod support;

use support::*;

#[sqlx::test]
async fn search_bulk_hydrates_direct_related_table_previews_without_multiplying_sources(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let category = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "search_projection_category"
name = "Search projection category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"
"#,
    )
    .await;
    let first_category = create_entity(&client, &base_url, &category).await;
    let second_category = create_entity(&client, &base_url, &category).await;
    let context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "regional", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for (entity, values) in [
        (
            &first_category,
            json!([
                { "kind": "scalar", "attribute_code": "name", "value": "First" },
                { "kind": "scalar", "attribute_code": "sku", "value": "A-1" },
                { "kind": "scalar", "attribute_code": "name", "context_id": context["id"], "value": "First regional" }
            ]),
        ),
        (
            &second_category,
            json!([
                { "kind": "scalar", "attribute_code": "name", "value": "Second" },
                { "kind": "scalar", "attribute_code": "sku", "value": "B-2" }
            ]),
        ),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/values",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": values }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let source_v1 = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "search_projection_product"
name = "Search projection product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#,
    )
    .await;
    let old_source = create_entity(&client, &base_url, &source_v1).await;
    let source_id = source_v1["blueprint"]["id"].as_str().unwrap();
    let source_v2: Value = client
        .post(format!("{base_url}/blueprints/{source_id}/versions"))
        .json(&json!({ "definition": r#"
format_version = 1
code = "search_projection_product"
name = "Search projection product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [
  { field = "categories.name", label = "Category" },
  { field = "categories.sku", label = "Category SKU" }
]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "search_projection_category"
cardinality = "one_to_one"
"# }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let source_v2: Value = client
        .post(format!(
            "{base_url}/blueprints/{source_id}/versions/{}/publish",
            source_v2["blueprint"]["version"].as_i64().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_source = create_entity(&client, &base_url, &source_v2).await;
    let second_source = create_entity(&client, &base_url, &source_v2).await;
    for (entity, targets) in [
        (&first_source, vec![first_category["id"].clone()]),
        (&second_source, vec![second_category["id"].clone()]),
    ] {
        client
            .put(format!(
                "{base_url}/v1/entities/{}",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({
                "relationships": [{ "attribute_code": "categories", "target_entity_ids": targets }]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let search: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": "search_projection_product" },
            "page": { "size": 25 }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    // Two table columns share `categories`, but the source page remains one row per entity.
    assert_eq!(search["items"].as_array().unwrap().len(), 3);
    let old = search["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == old_source["id"])
        .unwrap();
    assert_eq!(old["related"]["categories"], json!([]));
    let first = search["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == first_source["id"])
        .unwrap();
    let categories = first["related"]["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1);
    let first_target = categories
        .iter()
        .find(|target| target["id"] == first_category["id"])
        .unwrap();
    assert_eq!(
        first_target["blueprint_version"],
        category["blueprint"]["version"]
    );
    assert_eq!(first_target["display"]["default"], "First");
    assert_eq!(first_target["preview"]["default"]["sku"], "A-1");
    assert_eq!(
        first_target["preview"]["regional"]["name"],
        "First regional"
    );
    assert!(
        first_target["preview"]["default"]
            .get("categories")
            .is_none()
    );

    server.abort();
}
