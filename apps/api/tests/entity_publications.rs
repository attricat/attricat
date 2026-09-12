mod support;

use support::*;

const PUBLICATION_BLUEPRINT: &str = r#"
format_version = 1
code = "publication_product"
name = "Publication product"
kind = "entity"

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
async fn channel_publication_is_authorized_snapshotted_and_protects_dependencies(pool: PgPool) {
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
    let target = create_entity(&client, &base_url, &blueprint).await;
    let target_id = target["id"].as_str().unwrap();

    let publish = |entity_id: &str| {
        client
            .post(format!("{base_url}/v1/entities/{entity_id}/publications"))
            .json(&json!({ "context_id": context_id }))
    };
    let target_status = publish(target_id)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(target_status["status"], "published");
    assert_eq!(target_status["revision"], 1);

    let source = client
        .post(format!("{base_url}/v1/entities"))
        .json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"],
            },
            "values": [{
                "kind": "relationship",
                "attribute_code": "related",
                "context_id": context_id,
                "target_entity_id": target_id,
            }],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let source_id = source["id"].as_str().unwrap();
    assert_eq!(
        publish(source_id).send().await.unwrap().status(),
        StatusCode::OK
    );

    let statuses = client
        .get(format!("{base_url}/v1/entities/{source_id}/publications"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap();
    assert_eq!(statuses[0]["status"], "published");

    let unpublish = client
        .post(format!(
            "{base_url}/v1/entities/{target_id}/publications/unpublish"
        ))
        .json(&json!({ "context_id": context_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(unpublish.status(), StatusCode::CONFLICT);
    assert_eq!(
        client
            .delete(format!("{base_url}/entities/{target_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    let republished = publish(target_id)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(republished["revision"], 2);

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE workspace_id = $1 AND target ->> 'type' IN ('entity', 'context')", 
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        audit_count >= 4,
        "channel configuration and publication mutations are audited"
    );

    server.abort();
}

#[sqlx::test]
async fn publication_endpoints_require_an_authenticated_publisher(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let response = Client::new()
        .get(format!(
            "{base_url}/v1/entities/{}/publications",
            Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    server.abort();
}
