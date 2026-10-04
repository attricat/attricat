//! Generation-keyed caches never serve state older than the generation a
//! request reads: each change below is visible to the very next request.

mod support;

use std::time::Duration;

use api::{
    repository::CatalogRepository, rule_runtime, task_queue::TaskKind, task_worker::TaskOutcome,
};
use support::*;

async fn drain_rule_tasks(pool: &PgPool) {
    let repository = CatalogRepository::new(pool.clone(), BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let handler = rule_runtime::task_handler(repository.clone());
    for _ in 0..50 {
        let Some(task) = repository
            .claim_task("generation-test", Duration::from_secs(30))
            .await
            .unwrap()
        else {
            return;
        };
        if task.kind == TaskKind::RuleRunV1
            && let TaskOutcome::Reschedule { .. } = handler.handle(task.clone()).await.unwrap()
        {
            repository
                .reschedule_task_at(
                    task.id,
                    &task.lease_owner,
                    task.lease_token,
                    chrono::Utc::now() - chrono::Duration::seconds(1),
                )
                .await
                .unwrap();
            continue;
        }
        repository
            .complete_task(task.id, &task.lease_owner, task.lease_token)
            .await
            .unwrap();
    }
}

const DEFINITION: &str = r#"format_version = 1
code = "generation_item"
name = "Generation item"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"
"#;

async fn send(request: reqwest::RequestBuilder) -> (StatusCode, Value) {
    let response = request.send().await.unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

#[sqlx::test]
async fn published_versions_contexts_and_rules_apply_to_the_next_request(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, DEFINITION).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();

    // The latest published revision by code.
    let (status, latest) =
        send(client.get(format!("{base}/blueprints/by-code/generation_item"))).await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["blueprint"]["version"], 1);
    let (status, draft) = send(
        client
            .post(format!("{base}/blueprints/{blueprint_id}/versions"))
            .json(&json!({ "definition": DEFINITION.replace("Generation item", "Generation item v2") })),
    )
    .await;
    assert!(status.is_success(), "{draft}");
    let (status, latest) =
        send(client.get(format!("{base}/blueprints/by-code/generation_item"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        latest["blueprint"]["version"], 1,
        "a draft is not published"
    );
    let (status, published) = send(client.post(format!(
        "{base}/blueprints/{blueprint_id}/versions/2/publish"
    )))
    .await;
    assert!(status.is_success(), "{published}");
    let (status, latest) =
        send(client.get(format!("{base}/blueprints/by-code/generation_item"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(latest["blueprint"]["version"], 2);
    assert_eq!(latest["blueprint"]["name"], "Generation item v2");

    // An entity written before and after a new context exists.
    let entity = create_entity(&client, &base, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap().to_owned();
    let update = |values: Value| {
        client
            .put(format!("{base}/v1/entities/{entity_id}"))
            .json(&json!({ "values": values }))
    };
    let (status, body) = send(update(json!([
        { "kind": "scalar", "attribute_code": "title", "value": "First" }
    ])))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, context) = send(
        client
            .post(format!("{base}/contexts"))
            .json(&json!({ "code": "generation-fr", "data": {} })),
    )
    .await;
    assert!(status.is_success(), "{context}");
    let (status, body) = send(update(json!([
        { "kind": "scalar", "attribute_code": "title", "context_id": context["id"], "value": "Premier" }
    ])))
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a new context is usable at once: {body}"
    );
    assert_eq!(
        body["projections"]["preview"]["generation-fr"]["title"],
        "Premier"
    );

    // An enforcing rule enabled after writes applies to the next write.
    let (status, body) = send(update(json!([
        { "kind": "scalar", "attribute_code": "price", "value": 5 }
    ])))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, rule) = send(client.post(format!("{base}/rules")).json(&json!({
        "blueprint_id": blueprint_id,
        "blueprint_version": 1,
        "context_id": null,
        "definition": "format_version = 1\ncode = \"price-positive\"\nname = \"price-positive\"\nseverity = \"error\"\n[[triggers]]\ntype = \"manual\"\n[predicate]\ntype = \"compare\"\nattribute_code = \"price\"\nop = \"gte\"\nvalue = 0\n[enforcement]\non_save = true",
    })))
    .await;
    assert_eq!(status, StatusCode::CREATED, "{rule}");
    let rule_id = rule["id"].as_str().unwrap();
    let (status, body) = send(
        client
            .post(format!("{base}/rules/{rule_id}/versions/1/publish"))
            .json(&json!({})),
    )
    .await;
    assert!(status.is_success(), "{body}");
    // An enforcing rule needs a completed dry run before it is enabled.
    let (status, body) = send(
        client
            .post(format!("{base}/rules/{rule_id}/run-now"))
            .json(&json!({"dry_run": true, "idempotency_key": "preview", "version": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    drain_rule_tasks(&pool).await;
    let (status, body) = send(
        client
            .post(format!("{base}/rules/{rule_id}/versions/1/enable"))
            .json(&json!({})),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let (status, body) = send(update(json!([
        { "kind": "scalar", "attribute_code": "price", "value": -2 }
    ])))
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "rule_violation", "{body}");

    // Disabling it lifts the check for the next write.
    let (status, body) = send(
        client
            .post(format!("{base}/rules/{rule_id}/disable"))
            .json(&json!({})),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let (status, body) = send(update(json!([
        { "kind": "scalar", "attribute_code": "price", "value": -3 }
    ])))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    server.abort();
}
