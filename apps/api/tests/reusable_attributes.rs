mod support;

use serde_json::{Value, json};
use sqlx::PgPool;
use support::*;

const PRODUCT: &str = r#"format_version = 1
code = "reusable_product"
name = "Reusable product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string""#;

async fn create_published_reusable(
    client: &reqwest::Client,
    base_url: &str,
    definition: Value,
) -> Value {
    let draft: Value = client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&definition)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base_url}/reusable-attribute-revisions/{}/publish",
            draft["id"].as_str().unwrap()
        ))
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
async fn attaches_published_reusable_attributes_as_distinct_typed_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let published = create_published_reusable(
        &client,
        &base_url,
        json!({
            "namespace": "acme", "code": "weight", "name": "Weight", "value_type": "number",
            "searchable": true, "facetable": true, "default_value": 1.5
        }),
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let attached: Value = client
        .post(format!(
            "{base_url}/v1/entities/{entity_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": published["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(attached["code"], "acme:weight");
    let form: Value = client
        .get(format!("{base_url}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(form["values"].as_array().unwrap().len(), 0);
    assert_eq!(form["reusable_attributes"][0]["code"], "acme:weight");
    assert_eq!(form["reusable_values"][0]["value"], 1.5);
    // The registry can advance to a non-searchable revision without changing
    // the searchable revision that this entity already pinned.
    let newer_draft: Value = client
        .post(format!("{base_url}/reusable-attributes/{}/versions", published["definition_id"].as_str().unwrap()))
        .json(&json!({ "namespace": "acme", "code": "weight", "name": "Weight", "value_type": "number", "searchable": false }))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    client
        .post(format!(
            "{base_url}/reusable-attribute-revisions/{}/publish",
            newer_draft["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let filtered: Value = client.post(format!("{base_url}/v1/entities/search"))
        .json(&json!({ "blueprint": { "code": "reusable_product" }, "filters": [{ "field": "acme:weight", "operator": "gte", "value": 1 }], "page": { "size": 25 } }))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(filtered["items"].as_array().unwrap().len(), 1);
    let duplicate = client
        .post(format!(
            "{base_url}/v1/entities/{entity_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": published["id"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), reqwest::StatusCode::CONFLICT);
    server.abort();
}

#[sqlx::test]
async fn reusable_relationships_and_group_attachments_use_entity_owned_attributes(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let relationship = create_published_reusable(
        &client,
        &base_url,
        json!({
            "namespace": "acme", "code": "related", "name": "Related", "value_type": "relationship"
        }),
    )
    .await;
    let scalar = create_published_reusable(
        &client,
        &base_url,
        json!({
            "namespace": "acme", "code": "rating", "name": "Rating", "value_type": "integer"
        }),
    )
    .await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let target = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/v1/entities/{entity_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": relationship["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client.put(format!("{base_url}/v1/entities/{entity_id}"))
        .json(&json!({ "relationships": [{ "attribute_code": "acme:related", "target_entity_ids": [target_id] }] }))
        .send().await.unwrap().error_for_status().unwrap();
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
    assert!(
        values
            .iter()
            .any(|value| value["relationship_target_entity_id"] == target_id)
    );

    let relationship_v2: Value = client
        .post(format!("{base_url}/reusable-attributes/{}/versions", relationship["definition_id"].as_str().unwrap()))
        .json(&json!({ "namespace": "acme", "code": "related", "name": "Related", "value_type": "relationship" }))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let relationship_v2: Value = client
        .post(format!(
            "{base_url}/reusable-attribute-revisions/{}/publish",
            relationship_v2["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let duplicate_group = client.post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "invalid", "name": "Invalid", "reusable_attribute_revision_ids": [relationship["id"], relationship_v2["id"]] }))
        .send().await.unwrap();
    assert_eq!(duplicate_group.status(), reqwest::StatusCode::CONFLICT);
    let group: Value = client.post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "specs", "name": "Specs", "reusable_attribute_revision_ids": [relationship["id"], scalar["id"]] }))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let atomic = client
        .post(format!(
            "{base_url}/v1/entities/{entity_id}/reusable-attribute-groups/{}",
            group["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(atomic.status(), reqwest::StatusCode::CONFLICT);
    let form: Value = client
        .get(format!("{base_url}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        form["reusable_attributes"].as_array().unwrap().len(),
        1,
        "group attach rolls back every member"
    );
    server.abort();
}
