mod support;

use support::*;

const REVISION: &str = r#"
format_version = 1
code = "eb_revision"
name = "Document revision"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["label"]

[[unique_keys]]
code = "label"
attributes = ["label"]

[[attributes]]
code = "label"
value_type = "string"

[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "released", "superseded"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "released", "label": "Released" },
      { "code": "superseded", "label": "Superseded" }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": null, "to": "released" },
      { "from": "draft", "to": "released" },
      { "from": "released", "to": "superseded" }
    ]
  }
}'''

[[attributes]]
code = "previous"
value_type = "relationship"
target_blueprint = "eb_revision"
tree = true
context_editable = "default"
"#;

async fn released(client: &Client, base_url: &str, label: &str) -> Value {
    create_entity_with(
        client,
        base_url,
        "eb_revision",
        json!([scalar("label", label), scalar("status", "released")]),
    )
    .await
}

async fn batch(client: &Client, base_url: &str, operations: Value) -> reqwest::Response {
    client
        .post(format!("{base_url}/v1/entities/batch"))
        .json(&json!({ "operations": operations }))
        .send()
        .await
        .unwrap()
}

async fn form(client: &Client, base_url: &str, id: &str) -> Value {
    get_json(client, format!("{base_url}/v1/entities/{id}")).await
}

fn value_of(form: &Value, code: &str) -> Value {
    form["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["attribute_code"] == code)
        .map(|value| {
            if value["kind"] == "relationship" {
                value["target_entity_id"].clone()
            } else {
                value["value"].clone()
            }
        })
        .unwrap_or(Value::Null)
}

#[sqlx::test]
async fn releasing_a_revision_and_superseding_the_previous_one_is_atomic(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, REVISION).await;
    let previous = released(&client, &base_url, "A").await;
    let previous_id = previous["id"].as_str().unwrap().to_owned();
    let next_id = Uuid::new_v4().to_string();
    let events_before = count(&pool, "SELECT count(*) FROM domain_events").await;
    let audits_before = count(&pool, "SELECT count(*) FROM audit_events").await;

    let response = batch(
        &client,
        &base_url,
        json!([
            {
                "op": "create",
                "entity_id": next_id,
                "blueprint": { "code": "eb_revision" },
                "values": [
                    scalar("label", json!("B")),
                    scalar("status", json!("released")),
                    {"kind": "relationship", "attribute_code": "previous", "context_id": null, "target_entity_id": previous_id},
                ],
            },
            {
                "op": "update",
                "entity_id": previous_id,
                "expected_updated_at": previous["updated_at"],
                "values": [scalar("status", json!("superseded"))],
            },
        ]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["operations"][0]["op"], "create");
    assert_eq!(body["operations"][0]["entity"]["id"], json!(next_id));
    assert_eq!(body["operations"][1]["op"], "update");
    assert_eq!(body["operations"][1]["entity"]["id"], json!(previous_id));

    assert_eq!(
        value_of(&form(&client, &base_url, &previous_id).await, "status"),
        "superseded"
    );
    let next = form(&client, &base_url, &next_id).await;
    assert_eq!(value_of(&next, "status"), "released");
    assert_eq!(value_of(&next, "previous"), json!(previous_id));

    // One audit event and one domain event per operation, all committed together.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM domain_events").await - events_before,
        2
    );
    let audits = sqlx::query_as::<_, (Value, Value)>(
        "SELECT target, metadata FROM audit_events ORDER BY occurred_at DESC LIMIT 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM audit_events").await - audits_before,
        2
    );
    let mut audited: Vec<_> = audits
        .iter()
        .map(|(target, metadata)| {
            (
                target["id"].as_str().unwrap().to_owned(),
                metadata["batch"]["operation_index"].as_i64().unwrap(),
                metadata["batch"]["operation_count"].as_i64().unwrap(),
            )
        })
        .collect();
    audited.sort_by_key(|(_, index, _)| *index);
    assert_eq!(audited, [(next_id, 0, 2), (previous_id, 1, 2)]);

    server.abort();
}

#[sqlx::test]
async fn a_failing_operation_rolls_back_the_batch_and_names_the_operation(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, REVISION).await;
    let previous = released(&client, &base_url, "A").await;
    let previous_id = previous["id"].as_str().unwrap().to_owned();
    let other = released(&client, &base_url, "C").await;
    let new_id = Uuid::new_v4().to_string();
    let events_before = count(&pool, "SELECT count(*) FROM domain_events").await;
    let audits_before = count(&pool, "SELECT count(*) FROM audit_events").await;
    let create = json!({
        "op": "create",
        "entity_id": new_id,
        "blueprint": { "code": "eb_revision" },
        "values": [scalar("label", json!("B")), scalar("status", json!("released"))],
    });

    // A stale precondition on the second operation.
    let stale = batch(
        &client,
        &base_url,
        json!([
            create,
            {
                "op": "update",
                "entity_id": previous_id,
                "expected_updated_at": "2020-01-01T00:00:00Z",
                "values": [scalar("status", json!("superseded"))],
            },
        ]),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let body: Value = stale.json().await.unwrap();
    assert_eq!(body["error"]["code"], "stale_entity");
    assert_eq!(body["error"]["details"]["operation_index"], 1);
    assert_eq!(body["error"]["details"]["entity_id"], json!(previous_id));
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("operation 1:")
    );

    // A forbidden status transition and a unique-key conflict keep their codes.
    let transition = batch(
        &client,
        &base_url,
        json!([
            create,
            {
                "op": "update",
                "entity_id": previous_id,
                "expected_updated_at": previous["updated_at"],
                "values": [scalar("status", json!("draft"))],
            },
        ]),
    )
    .await;
    assert_eq!(transition.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        transition.json::<Value>().await.unwrap()["error"]["code"],
        "attribute_value_schema_mismatch"
    );
    let duplicate = batch(
        &client,
        &base_url,
        json!([
            create,
            {
                "op": "update",
                "entity_id": other["id"],
                "values": [scalar("label", json!("b"))],
            },
        ]),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let body: Value = duplicate.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unique_key_conflict");
    assert_eq!(body["error"]["details"]["operation_index"], 1);
    assert_eq!(
        body["error"]["details"]["conflicting_entity_id"],
        json!(new_id)
    );

    // Nothing from the failed batches was applied, audited or published.
    let missing = client
        .get(format!("{base_url}/v1/entities/{new_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        value_of(&form(&client, &base_url, &previous_id).await, "status"),
        "released"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM domain_events").await,
        events_before
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM audit_events").await,
        audits_before
    );

    // Deletes apply the same rules, including preconditions.
    let deleted = batch(
        &client,
        &base_url,
        json!([
            { "op": "delete", "entity_id": other["id"], "expected_updated_at": other["updated_at"] },
            { "op": "update", "entity_id": previous_id, "values": [scalar("label", json!("c"))] },
        ]),
    )
    .await;
    assert_eq!(
        deleted.status(),
        StatusCode::OK,
        "the deleted key is released"
    );
    assert_eq!(
        deleted.json::<Value>().await.unwrap()["operations"][0],
        json!({"op": "delete", "entity_id": other["id"]})
    );

    server.abort();
}

#[sqlx::test]
async fn batches_are_bounded_and_name_each_entity_once(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, REVISION).await;
    let existing = released(&client, &base_url, "A").await;

    let too_many: Vec<_> = (0..51)
        .map(|_| json!({"op": "create", "blueprint": {"code": "eb_revision"}}))
        .collect();
    for operations in [
        json!([]),
        json!(too_many),
        json!([
            {"op": "update", "entity_id": existing["id"], "values": [scalar("label", json!("x"))]},
            {"op": "delete", "entity_id": existing["id"]},
        ]),
    ] {
        let response = batch(&client, &base_url, operations).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "invalid_input"
        );
    }
    let taken = batch(
        &client,
        &base_url,
        json!([{"op": "create", "entity_id": existing["id"], "blueprint": {"code": "eb_revision"}}]),
    )
    .await;
    assert_eq!(taken.status(), StatusCode::CONFLICT);
    assert_eq!(
        taken.json::<Value>().await.unwrap()["error"]["code"],
        "entity_id_taken"
    );

    server.abort();
}

#[sqlx::test]
async fn every_operation_is_authorized_against_its_own_entity(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, REVISION).await;
    let granted = released(&client, &base_url, "A").await;
    let other = released(&client, &base_url, "B").await;

    let (editor_id, membership_id) = add_workspace_user(&pool).await;
    grant_role(
        &pool,
        membership_id,
        EDITOR_ROLE_ID,
        GrantScope::Entity(granted["id"].as_str().unwrap().parse().unwrap()),
    )
    .await;
    let editor_client = client_for(editor_id);
    let editor = |operations: Value| batch(&editor_client, &base_url, operations);
    let update = |entity: &Value, label: &str| json!({"op": "update", "entity_id": entity["id"], "values": [scalar("label", json!(label))]});

    let forbidden = editor(json!([update(&granted, "A2"), update(&other, "B2")])).await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        value_of(
            &form(&client, &base_url, granted["id"].as_str().unwrap()).await,
            "label"
        ),
        "A"
    );
    let allowed = editor(json!([update(&granted, "A2")])).await;
    assert_eq!(allowed.status(), StatusCode::OK);
    let create = editor(json!([{"op": "create", "blueprint": {"code": "eb_revision"}}])).await;
    assert_eq!(
        create.status(),
        StatusCode::FORBIDDEN,
        "creating needs a workspace grant"
    );

    server.abort();
}
