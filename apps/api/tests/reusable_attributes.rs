use crate::support;

use serde_json::{Value, json};
use sqlx::PgPool;
use support::*;

const PRODUCT: &str = r#"format_version: 1
code: reusable_product
name: Reusable product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string"#;

fn reusable_definition(input: &Value) -> String {
    let mut definition = format!(
        "code: '{}'\nname: '{}'\nvalue_type: '{}'\n",
        input["code"].as_str().unwrap(),
        input["name"].as_str().unwrap(),
        input["value_type"].as_str().unwrap(),
    );
    for key in ["searchable", "facetable"] {
        if let Some(value) = input[key].as_bool() {
            definition.push_str(&format!("{key}: {value}\n"));
        }
    }
    if let Some(value) = input.get("default_value") {
        definition.push_str(&format!("default_value: {value}\n"));
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
        .json(&json!({ "definition": "code: weight\nname: Weight\nvalue_type: number\nsearchable: false" }))
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn lists_facetable_searchable_attributes_as_blueprint_filter_fields(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    for (code, searchable, facetable) in [
        ("width", true, true),
        ("height", true, false),
        ("depth", false, true),
    ] {
        let published = create_published_reusable(
            &client,
            &base_url,
            json!({
                "code": code, "name": code, "value_type": "number",
                "searchable": searchable, "facetable": facetable
            }),
        )
        .await;
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
    }
    // Attached to no record of the blueprint, so no filter can match it.
    create_published_reusable(
        &client,
        &base_url,
        json!({
            "code": "length", "name": "Length", "value_type": "number",
            "searchable": true, "facetable": true
        }),
    )
    .await;

    let fields: Vec<Value> = client
        .get(format!(
            "{base_url}/v1/records/reusable-attributes?blueprint=reusable_product"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let codes = fields
        .iter()
        .map(|field| field["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(codes, ["width"]);
    let unknown = client
        .get(format!(
            "{base_url}/v1/records/reusable-attributes?blueprint=missing"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), reqwest::StatusCode::NOT_FOUND);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    create_published_reusable(
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
        .post(format!(
            "{base_url}/reusable-attributes/{}/versions",
            relationship["definition_id"].as_str().unwrap()
        ))
        .json(&json!({ "definition": "code: related\nname: Related\nvalue_type: relationship" }))
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
            relationship_v2["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let duplicate_group = client.post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "invalid", "name": "Invalid", "reusable_attribute_codes": ["default:related", "default:related"] }))
        .send().await.unwrap();
    assert_eq!(duplicate_group.status(), reqwest::StatusCode::CONFLICT);
    let group: Value = client.post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "specs", "name": "Specs", "reusable_attribute_codes": ["default:related", "default:rating"] }))
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
        .json(&json!({ "definition": PRODUCT.replace("name: Reusable product", "name: Reusable product v2") }))
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn validates_definitions_without_storing_them(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition = |code: &str, value_type: &str| {
        format!("code: '{code}'\nname: Checked\nvalue_type: '{value_type}'\n")
    };
    let list = || async {
        client
            .get(format!(
                "{base_url}/reusable-attributes?include_drafts=true"
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };

    let checked: Value = client
        .post(format!("{base_url}/reusable-attributes/validate"))
        .json(&json!({ "definition": definition("checked", "number") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(checked["code"], "checked");
    assert_eq!(checked["value_type"], "number");
    assert_eq!(checked["version"], 1);
    assert_eq!(list().await.as_array().unwrap().len(), 0);

    let invalid = client
        .post(format!("{base_url}/reusable-attributes/validate"))
        .json(&json!({ "definition": definition("checked", "colour") }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.json::<Value>().await.unwrap()["error"]["message"],
        "invalid reusable attribute definition: attribute 'checked' has unsupported value type 'colour'"
    );

    let created: Value = client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&json!({ "definition": definition("checked", "number") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let definition_id = created["definition_id"].as_str().unwrap();
    let taken = client
        .post(format!("{base_url}/reusable-attributes/validate"))
        .json(&json!({ "definition": definition("checked", "number") }))
        .send()
        .await
        .unwrap();
    assert!(taken.status().is_client_error());
    let revision: Value = client
        .post(format!(
            "{base_url}/reusable-attributes/{definition_id}/versions/validate"
        ))
        .json(&json!({ "definition": definition("checked", "integer") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revision["version"], 2);
    assert_eq!(revision["value_type"], "integer");
    let renamed = client
        .post(format!(
            "{base_url}/reusable-attributes/{definition_id}/versions/validate"
        ))
        .json(&json!({ "definition": definition("renamed", "number") }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(list().await.as_array().unwrap().len(), 1);

    server.abort();
}

/// The web editors offer these as starting points, so each must save as is.
#[sqlx::test(migrator = "api::MIGRATOR")]
async fn editor_templates_pass_validation(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let web = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/src/features");
    for (directory, route) in [
        ("blueprints/templates", "blueprints/validate"),
        (
            "reusable-attributes/templates",
            "reusable-attributes/validate",
        ),
    ] {
        let mut paths = std::fs::read_dir(web.join(directory))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        assert!(!paths.is_empty(), "{directory} has no templates");
        for path in paths {
            let definition = std::fs::read_to_string(&path).unwrap();
            let response = client
                .post(format!("{base_url}/{route}"))
                .json(&json!({ "definition": definition }))
                .send()
                .await
                .unwrap();
            let status = response.status();
            assert!(
                status.is_success(),
                "{}: {status} {}",
                path.display(),
                response.text().await.unwrap()
            );
        }
    }

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_taken_reusable_attribute_code_is_reported_as_code_taken(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let definition =
        reusable_definition(&json!({ "code": "weight", "name": "Weight", "value_type": "number" }));
    client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    for path in ["reusable-attributes/validate", "reusable-attributes"] {
        let duplicate = client
            .post(format!("{base_url}/{path}"))
            .json(&json!({ "definition": definition }))
            .send()
            .await
            .unwrap();
        assert_eq!(duplicate.status(), reqwest::StatusCode::CONFLICT, "{path}");
        let duplicate: Value = duplicate.json().await.unwrap();
        assert_eq!(duplicate["error"]["code"], "code_taken", "{path}");
        assert_eq!(
            duplicate["error"]["details"]["resource"],
            "reusable_attribute"
        );
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn reusable_attribute_group_codes_report_why_they_are_refused(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let create = |code: &str| {
        client
            .post(format!("{base_url}/reusable-attribute-groups"))
            .json(&json!({ "code": code, "name": "Specs", "reusable_attribute_codes": [] }))
            .send()
    };
    create("specs").await.unwrap().error_for_status().unwrap();
    for (code, status, error_code) in [
        (
            "Bad code!",
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_code",
        ),
        ("specs", reqwest::StatusCode::CONFLICT, "code_taken"),
    ] {
        let response = create(code).await.unwrap();
        assert_eq!(response.status(), status, "{code}");
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], error_code, "{code}");
    }
    server.abort();
}

async fn publish_next_revision(
    client: &reqwest::Client,
    base_url: &str,
    attribute: &Value,
) -> Value {
    let draft = create_next_draft(client, base_url, attribute).await;
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

async fn create_next_draft(client: &reqwest::Client, base_url: &str, attribute: &Value) -> Value {
    client
        .post(format!(
            "{base_url}/reusable-attributes/{}/versions",
            attribute["definition_id"].as_str().unwrap()
        ))
        .json(&json!({ "definition": attribute["definition"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn applying_a_group_uses_each_attributes_latest_published_revision(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let attribute = create_published_reusable(
        &client,
        &base_url,
        json!({ "code": "weight", "name": "Weight", "value_type": "number" }),
    )
    .await;
    let group: Value = client
        .post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "specs", "name": "Specs", "reusable_attribute_codes": ["default:weight"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(group["reusable_attribute_codes"], json!(["default:weight"]));
    let published = publish_next_revision(&client, &base_url, &attribute).await;
    // A newer draft is not ready for records.
    create_next_draft(&client, &base_url, &attribute).await;

    let record = create_record(&client, &base_url, &blueprint).await;
    let applied: Value = client
        .post(format!(
            "{base_url}/v1/records/{}/reusable-attribute-groups/{}",
            record["id"].as_str().unwrap(),
            group["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied[0]["revision_id"], published["id"]);
    assert_eq!(applied[0]["version"], 2);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn group_members_must_be_qualified_codes_with_a_published_revision(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&json!({ "definition": "code: draft_only\nname: Draft\nvalue_type: string" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    for (code, error_code) in [
        ("draft_only", "invalid_input"),
        ("default:bad code", "invalid_input"),
        ("default:draft_only", "reusable_attribute_not_published"),
        ("default:missing", "reusable_attribute_not_published"),
    ] {
        let refused = client
            .post(format!("{base_url}/reusable-attribute-groups"))
            .json(&json!({ "code": "specs", "name": "Specs", "reusable_attribute_codes": [code] }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            refused.status(),
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            "{code}"
        );
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body["error"]["code"], error_code, "{code}");
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn editing_and_deleting_a_group_leaves_records_alone(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    for code in ["width", "height"] {
        create_published_reusable(
            &client,
            &base_url,
            json!({ "code": code, "name": code, "value_type": "number" }),
        )
        .await;
    }
    let group: Value = client
        .post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "specs", "name": "Specs", "position": 3, "reusable_attribute_codes": ["default:width"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let group_url = format!(
        "{base_url}/reusable-attribute-groups/{}",
        group["id"].as_str().unwrap()
    );
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_url = format!("{base_url}/v1/records/{}", record["id"].as_str().unwrap());
    client
        .post(format!(
            "{record_url}/reusable-attribute-groups/{}",
            group["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let attached_codes = || async {
        let form: Value = client
            .get(&record_url)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        form["reusable_attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|attribute| attribute["code"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };

    // Fields left out keep their value.
    let renamed: Value = client
        .patch(&group_url)
        .json(&json!({ "name": "Renamed" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(renamed["name"], "Renamed");
    assert_eq!(renamed["code"], "specs");
    assert_eq!(renamed["position"], 3);
    assert_eq!(
        renamed["reusable_attribute_codes"],
        json!(["default:width"])
    );

    let edited: Value = client
        .patch(&group_url)
        .json(&json!({ "position": 1, "reusable_attribute_codes": ["default:height", "default:width"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited["position"], 1);
    assert_eq!(
        edited["reusable_attribute_codes"],
        json!(["default:height", "default:width"])
    );
    assert_eq!(attached_codes().await, ["default:width"]);

    // A refused edit changes nothing, and the code cannot change.
    let refused = client
        .patch(&group_url)
        .json(&json!({ "name": "Refused", "reusable_attribute_codes": ["default:missing"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let code_change = client
        .patch(&group_url)
        .json(&json!({ "code": "other" }))
        .send()
        .await
        .unwrap();
    assert!(code_change.status().is_client_error());
    let groups: Value = client
        .get(format!("{base_url}/reusable-attribute-groups"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(groups[0]["name"], "Renamed");
    assert_eq!(groups[0]["code"], "specs");
    assert_eq!(
        groups[0]["reusable_attribute_codes"],
        json!(["default:height", "default:width"])
    );

    let deleted = client.delete(&group_url).send().await.unwrap();
    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(attached_codes().await, ["default:width"]);
    for response in [
        client.delete(&group_url).send().await.unwrap(),
        client
            .patch(&group_url)
            .json(&json!({ "name": "Gone" }))
            .send()
            .await
            .unwrap(),
    ] {
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    }
    // Its code is free again.
    client
        .post(format!("{base_url}/reusable-attribute-groups"))
        .json(&json!({ "code": "specs", "name": "Specs", "reusable_attribute_codes": [] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn selection_pages_include_attached_reusable_values(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, PRODUCT).await;
    let rating = create_published_reusable(
        &client,
        &base_url,
        json!({
            "namespace": "acme", "code": "rating", "name": "Rating", "value_type": "integer"
        }),
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/v1/records/{record_id}/reusable-attributes"
        ))
        .json(&json!({ "reusable_attribute_revision_id": rating["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({ "values": [{ "kind": "scalar", "attribute_code": "default:rating", "value": 4 }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let repository =
        api::repository::AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let scope = api::repository::InteractiveRunScope {
        scope: api::extensions::InteractiveScope::Selection,
        actor: api::repository::AuthorizationActor {
            user_id: BOOTSTRAP_OWNER_ID.parse().unwrap(),
            token_id: None,
        },
        record_ids: vec![record_id.parse().unwrap()],
        blueprint_id: blueprint["blueprint"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        blueprint_version: 1,
        context_id: None,
    };

    let page = repository
        .interactive_selection_page("acme.selection", &scope, "", 10)
        .await
        .unwrap();

    assert_eq!(
        page["records"][0]["reusable_values"][0]["value"],
        json!(4),
        "{page}"
    );
    server.abort();
}
