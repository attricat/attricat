mod support;

use serde_json::{Value, json};
use sqlx::PgPool;
use support::*;

const PRODUCT: &str = r#"format_version = 1
code = "reusable_product"
name = "Reusable product"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string""#;

fn reusable_definition(input: &Value) -> String {
    let mut definition = format!(
        "code = \"{}\"\nname = \"{}\"\nvalue_type = \"{}\"\n",
        input["code"].as_str().unwrap(),
        input["name"].as_str().unwrap(),
        input["value_type"].as_str().unwrap(),
    );
    for key in ["searchable", "facetable"] {
        if let Some(value) = input[key].as_bool() {
            definition.push_str(&format!("{key} = {value}\n"));
        }
    }
    if let Some(value) = input.get("default_value") {
        definition.push_str(&format!("default_value = {value}\n"));
    }
    definition
}

async fn create_published_reusable(
    client: &reqwest::Client,
    base_url: &str,
    definition: Value,
) -> Value {
    let draft: Value = client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&json!({ "definition": reusable_definition(&definition) }))
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
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    let attached: Value = client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
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
    assert_eq!(attached["code"], "default:weight");
    let form: Value = client
        .get(format!("{base_url}/v1/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(form["values"].as_array().unwrap().len(), 0);
    assert_eq!(form["reusable_attributes"][0]["code"], "default:weight");
    assert_eq!(form["reusable_values"][0]["value"], 1.5);
    // The registry can advance to a non-searchable revision without changing
    // the searchable revision that this record already pinned.
    let newer_draft: Value = client
        .post(format!("{base_url}/reusable-attributes/{}/versions", published["definition_id"].as_str().unwrap()))
        .json(&json!({ "definition": "code = \"weight\"\nname = \"Weight\"\nvalue_type = \"number\"\nsearchable = false" }))
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
    let filtered: Value = client.post(format!("{base_url}/v1/records/search"))
        .json(&json!({ "blueprint": { "code": "reusable_product" }, "filters": [{ "field": "default:weight", "operator": "gte", "value": 1 }], "page": { "size": 25 } }))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    assert_eq!(filtered["items"].as_array().unwrap().len(), 1);
    let duplicate = client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": published["id"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), reqwest::StatusCode::CONFLICT);
    server.abort();
}

#[sqlx::test]
async fn reusable_relationships_and_group_attachments_use_record_owned_attributes(pool: PgPool) {
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
    let record = create_record(&client, &base_url, &blueprint).await;
    let target = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": relationship["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client.put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({ "relationships": [{ "attribute_code": "default:related", "target_record_ids": [target_id] }] }))
        .send().await.unwrap().error_for_status().unwrap();
    let values: Vec<Value> = client
        .get(format!("{base_url}/records/{record_id}/values/current"))
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
            .any(|value| value["relationship_target_record_id"] == target_id)
    );

    let relationship_v2: Value = client
        .post(format!("{base_url}/reusable-attributes/{}/versions", relationship["definition_id"].as_str().unwrap()))
        .json(&json!({ "definition": "code = \"related\"\nname = \"Related\"\nvalue_type = \"relationship\"" }))
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
            "{base_url}/v1/records/{record_id}/reusable-attribute-groups/{}",
            group["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(atomic.status(), reqwest::StatusCode::CONFLICT);
    let form: Value = client
        .get(format!("{base_url}/v1/records/{record_id}"))
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

#[sqlx::test]
async fn migration_preserves_reusable_attribute_values(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let reusable = create_published_reusable(
        &client,
        &base_url,
        json!({
            "namespace": "acme", "code": "weight", "name": "Weight", "value_type": "number",
            "default_value": 1.5
        }),
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": reusable["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let before = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT av.id, av.attribute_id, av.created_at
           FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           WHERE av.record_id = $1 AND a.record_id = $1"#,
    )
    .bind(record_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    let revision: Value = client
        .post(format!(
            "{base_url}/blueprints/{}/versions",
            blueprint["blueprint"]["id"].as_str().unwrap()
        ))
        .json(&json!({ "definition": PRODUCT.replace("name = \"Reusable product\"", "name = \"Reusable product v2\"") }))
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
            "{base_url}/blueprints/{}/versions/{}/publish",
            blueprint["blueprint"]["id"].as_str().unwrap(),
            revision["blueprint"]["version"].as_i64().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let preview: Value = client
        .post(format!(
            "{base_url}/v1/records/{record_id}/blueprint-migration/preview"
        ))
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
            "{base_url}/v1/records/{record_id}/blueprint-migration"
        ))
        .json(&json!({
            "migration_id": preview["migration_id"],
            "expected_target_version": preview["target"]["blueprint"]["version"],
            "values": [],
            "relationships": [],
            "discard_attributes": []
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let record: Value = client
        .get(format!("{base_url}/v1/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(record["reusable_attributes"].as_array().unwrap().len(), 1);
    assert_eq!(record["reusable_values"][0]["value"], 1.5);
    let after = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, chrono::DateTime<chrono::Utc>)>(
        r#"SELECT av.id, av.attribute_id, av.created_at
           FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           WHERE av.record_id = $1 AND a.record_id = $1"#,
    )
    .bind(record_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        after, before,
        "reusable values are not rewritten or remapped"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attribute_value_history WHERE id = $1",)
            .bind(before.0)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    server.abort();
}

#[sqlx::test]
async fn attaching_a_reusable_attribute_audits_its_default_and_emits_an_record_update(
    pool: PgPool,
) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let published = create_published_reusable(
        &client,
        &base_url,
        json!({ "code": "depth", "name": "Depth", "value_type": "number", "default_value": 2.5 }),
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": published["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let payload: Value = sqlx::query_scalar(
        "SELECT payload FROM domain_events WHERE aggregate_id = $1 AND event_type = 'record.updated.v1' ORDER BY sequence DESC LIMIT 1",
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(payload["facts"][0]["attribute_code"], "default:depth");
    assert_eq!(payload["facts"][0]["after_value"], 2.5);
    let change_kind: String = sqlx::query_scalar(
        "SELECT change_kind FROM audit_event_changes WHERE record_id = $1 AND attribute_code = 'default:depth'",
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(change_kind, "set");
    server.abort();
}
