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
async fn search_requires_an_explicit_relationship_selector_for_traversal(pool: sqlx::PgPool) {
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

    let bare_response: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({ "blueprint": { "code": "search_product" }, "query": "Crimson", "page": { "size": 25 } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(bare_response["items"].as_array().unwrap().is_empty());

    for query in ["ABC", "*:Crimson", "color.name:Crimson", "sku:ABC*"] {
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
        } else {
            assert_eq!(witness["matching_entity_id"], item["id"]);
            assert_eq!(witness["traversal_depth"], 0);
            assert!(witness["relationship_path"].as_array().unwrap().is_empty());
        }
    }
    let invalid = client.post(format!("{base_url}/v1/entities/search"))
        .json(&json!({ "blueprint": { "code": "search_product" }, "query": "color.name:bad*middle", "page": { "size": 25 } }))
        .send().await.unwrap();
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    server.abort();
}

#[sqlx::test]
async fn search_selects_entities_by_id_directly_and_through_relationships(pool: sqlx::PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let color = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "id_color"
name = "ID color"
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
code = "id_product"
name = "ID product"
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
target_blueprint = "id_color"
"#,
    )
    .await;
    let red = create_entity(&client, &base_url, &color).await;
    let blue = create_entity(&client, &base_url, &color).await;
    let first = create_entity(&client, &base_url, &product).await;
    let second = create_entity(&client, &base_url, &product).await;
    let third = create_entity(&client, &base_url, &product).await;
    for (item, target) in [(&first, &red), (&second, &blue)] {
        client
            .put(format!("{base_url}/v1/entities/{}", item["id"].as_str().unwrap()))
            .json(&json!({ "relationships": [{ "attribute_code": "color", "target_entity_ids": [target["id"]] }] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let id = |entity: &Value| entity["id"].as_str().unwrap().to_owned();
    let search = |query: String| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            client
                .post(format!("{base_url}/v1/entities/search"))
                .json(&json!({ "blueprint": { "code": "id_product" }, "query": query, "page": { "size": 25 } }))
                .send()
                .await
                .unwrap()
        }
    };
    let ids = |response: Value| {
        let mut ids: Vec<_> = response["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    };
    let sorted = |mut ids: Vec<String>| {
        ids.sort();
        ids
    };

    let direct: Value = search(format!("@id:{},{}", id(&first), id(&third)))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(ids(direct.clone()), sorted(vec![id(&first), id(&third)]));
    let witness = &direct["items"][0]["match_explanations"][0];
    assert_eq!(witness["traversal_depth"], 0);
    assert!(witness["matching_attribute_code"].is_null());

    // IDs of other blueprints never match the selected blueprint directly.
    let foreign: Value = search(format!("id_product.@ID:{}", id(&red)))
        .await
        .json()
        .await
        .unwrap();
    assert!(ids(foreign).is_empty());

    let related: Value = search(format!("color.@id:{},{}", id(&red), id(&blue)))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(ids(related.clone()), sorted(vec![id(&first), id(&second)]));
    let witness = &related["items"][0]["match_explanations"][0];
    assert_eq!(witness["traversal_depth"], 1);
    assert_eq!(witness["relationship_path"][0]["attribute_code"], "color");

    let intersected: Value = search(format!(
        "color.@id:{},{} @id:{}",
        id(&red),
        id(&blue),
        id(&second)
    ))
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(ids(intersected), vec![id(&second)]);

    for query in [
        "@id:not-a-uuid".to_owned(),
        format!("@id:{},", id(&first)),
        format!("@id:{}*", id(&first)),
        format!("sku.@id:{}", id(&first)),
        format!(
            "@id:{}",
            (0..101)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
    ] {
        assert_eq!(
            search(query.clone()).await.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
    server.abort();
}
