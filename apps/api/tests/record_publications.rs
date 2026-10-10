mod support;

use support::*;

const PUBLICATION_BLUEPRINT: &str = r#"
format_version = 1
code = "publication_product"
name = "Publication product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "related"
value_type = "relationship"
"#;

#[sqlx::test]
async fn channel_publication_is_authorized_and_record_changes_withdraw_approval(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();

    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "publication-web", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let context_id = context["id"].as_str().unwrap();
    client
        .put(format!("{base_url}/publication-channels/{context_id}"))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let blueprint = create_blueprint(&client, &base_url, PUBLICATION_BLUEPRINT).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    let publish = || {
        client
            .post(format!("{base_url}/v1/records/{record_id}/publications"))
            .json(&json!({ "context_id": context_id }))
    };

    let published = publish()
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(published["status"], "published");
    assert!(published["published_at"].is_string());
    assert!(published["published_by_user_id"].is_string());
    assert!(published.get("revision").is_none());

    client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_code": "title",
                "context_id": context_id,
                "value": "Changed",
            }],
            "relationships": [],
            "remove_values": [],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let statuses = client
        .get(format!("{base_url}/v1/records/{record_id}/publications"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    assert_eq!(statuses[0]["status"], "not_published");
    assert!(statuses[0]["published_at"].is_null());
    assert!(statuses[0]["published_by_user_id"].is_null());

    publish().send().await.unwrap().error_for_status().unwrap();
    client
        .put(format!("{base_url}/contexts/id/{context_id}"))
        .json(&json!({ "parent_id": context["parent_id"], "data": { "changed": true } }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let statuses = client
        .get(format!("{base_url}/v1/records/{record_id}/publications"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    assert_eq!(statuses[0]["status"], "not_published");

    client
        .delete(format!("{base_url}/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND target ->> 'type' IN ('record', 'context')",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(audit_count >= 4, "publication mutations are audited");

    server.abort();
}

#[sqlx::test]
async fn trusted_blueprint_role_retains_publication_after_an_record_edit(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "trusted-publication", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let context_id = context["id"].as_str().unwrap();
    client
        .put(format!("{base_url}/publication-channels/{context_id}"))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let definition =
        format!("{PUBLICATION_BLUEPRINT}\n[publication]\nretain_on_edit_roles = [\"owner\"]\n");
    let blueprint = create_blueprint(&client, &base_url, &definition).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();
    client
        .post(format!("{base_url}/v1/records/{record_id}/publications"))
        .json(&json!({ "context_id": context_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .put(format!("{base_url}/v1/records/{record_id}"))
        .json(&json!({
            "values": [{
                "kind": "scalar",
                "attribute_code": "title",
                "context_id": context_id,
                "value": "Retained",
            }],
            "relationships": [],
            "remove_values": [],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let statuses = client
        .get(format!("{base_url}/v1/records/{record_id}/publications"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    assert_eq!(statuses[0]["status"], "published");
    server.abort();
}

#[sqlx::test]
async fn blueprint_revision_can_publish_its_records_by_channel(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let web = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "blueprint-web", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let api = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "blueprint-api", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    for context in [&web, &api] {
        client
            .put(format!(
                "{base_url}/publication-channels/{}",
                context["id"].as_str().unwrap()
            ))
            .json(&json!({ "enabled": true }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let blueprint = create_blueprint(&client, &base_url, PUBLICATION_BLUEPRINT).await;
    let first = create_record(&client, &base_url, &blueprint).await;
    let second = create_record(&client, &base_url, &blueprint).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let version = blueprint["blueprint"]["version"].as_i64().unwrap();

    let one_channel = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{version}/record-publications"
        ))
        .json(&json!({ "context_id": web["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(
        one_channel,
        json!({
            "record_count": 2,
            "channel_count": 1,
            "publication_count": 2,
        })
    );

    let all_channels = client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{version}/record-publications/publish-all"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(
        all_channels,
        json!({
            "record_count": 2,
            "channel_count": 2,
            "publication_count": 4,
        })
    );
    for record in [&first, &second] {
        let statuses = client
            .get(format!(
                "{base_url}/v1/records/{}/publications",
                record["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<Vec<Value>>()
            .await
            .unwrap();
        assert!(
            statuses
                .iter()
                .all(|status| status["status"] == "published")
        );
    }
    server.abort();
}

#[sqlx::test]
async fn publication_endpoints_require_an_authenticated_publisher(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let response = Client::new()
        .get(format!(
            "{base_url}/v1/records/{}/publications",
            Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    server.abort();
}

#[sqlx::test]
async fn deleting_a_context_announces_its_withdrawn_publications(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "publication-retired", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let context_id = context["id"].as_str().unwrap();
    client
        .put(format!("{base_url}/publication-channels/{context_id}"))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let blueprint = create_blueprint(&client, &base_url, PUBLICATION_BLUEPRINT).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base_url}/v1/records/{record_id}/publications"))
        .json(&json!({ "context_id": context_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .delete(format!("{base_url}/contexts/id/{context_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let unpublished: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM domain_events WHERE aggregate_id = $1 AND event_type = 'record.unpublished.v1'",
    )
    .bind(record_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(unpublished.len(), 1);
    assert_eq!(unpublished[0]["context_id"], context_id);
    server.abort();
}

#[sqlx::test]
async fn deleting_an_record_announces_its_withdrawn_publications(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let context = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "publication-deleted-record", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let context_id = context["id"].as_str().unwrap();
    client
        .put(format!("{base_url}/publication-channels/{context_id}"))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let blueprint = create_blueprint(&client, &base_url, PUBLICATION_BLUEPRINT).await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id: Uuid = record["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base_url}/v1/records/{record_id}/publications"))
        .json(&json!({ "context_id": context_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .delete(format!("{base_url}/records/{record_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let unpublished: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM domain_events WHERE aggregate_id = $1 AND event_type = 'record.unpublished.v1'",
    )
    .bind(record_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(unpublished.len(), 1);
    assert_eq!(unpublished[0]["context_id"], context_id);
    assert_eq!(unpublished[0]["reason"], "record_deleted");
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM record_channel_publications WHERE record_id = $1")
            .bind(record_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 0);
    server.abort();
}
