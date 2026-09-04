mod support;

use serde_json::{Value, json};
use support::*;

async fn set_values(client: &reqwest::Client, base_url: &str, entity: &Value, values: Value) {
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

#[sqlx::test]
async fn relationship_aware_search_supports_traversal_selectors_wildcards_and_witnesses(
    pool: sqlx::PgPool,
) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let color = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "search_color"
name = "Search color"
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
code = "search_product"
name = "Search product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "color"
value_type = "relationship"
target_blueprint = "search_color"
"#,
    )
    .await;
    let red = create_entity(&client, &base_url, &color).await;
    let item = create_entity(&client, &base_url, &product).await;
    set_values(
        &client,
        &base_url,
        &red,
        json!([{ "kind": "scalar", "attribute_code": "name", "value": "Crimson" }]),
    )
    .await;
    set_values(
        &client,
        &base_url,
        &item,
        json!([{ "kind": "scalar", "attribute_code": "sku", "value": "ABC-123" }]),
    )
    .await;
    client.put(format!("{base_url}/v1/entities/{}", item["id"].as_str().unwrap()))
        .json(&json!({ "relationships": [{ "attribute_code": "color", "target_entity_ids": [red["id"]] }] }))
        .send().await.unwrap().error_for_status().unwrap();

    for query in ["Crimson", "color.name:Crimson", "sku:ABC*"] {
        let response: Value = client.post(format!("{base_url}/v1/entities/search"))
            .json(&json!({ "blueprint": { "code": "search_product" }, "query": query, "page": { "size": 25 } }))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        assert_eq!(response["items"].as_array().unwrap().len(), 1, "{query}");
        let witness = &response["items"][0]["match_explanations"][0];
        assert_eq!(witness["term"], query);
        if query.contains("Crimson") {
            assert_eq!(witness["matching_entity_id"], red["id"]);
            assert_eq!(witness["traversal_depth"], 1);
            assert_eq!(witness["relationship_path"][0]["attribute_code"], "color");
        }
    }
    let invalid = client.post(format!("{base_url}/v1/entities/search"))
        .json(&json!({ "blueprint": { "code": "search_product" }, "query": "color.name:bad*middle", "page": { "size": 25 } }))
        .send().await.unwrap();
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    server.abort();
}
