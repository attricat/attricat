mod support;
use support::*;

const DEFINITION: &str = r#"format_version = 1
code = 'status_item'
name = 'Status item'
kind = 'entity'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['status']
[[attributes]]
code = 'status'
value_type = 'string'
value_schema = '''{"type":"string","enum":["draft","live","done"],"x-attricat-status":{"version":1,"options":[{"code":"draft","label":"Draft"},{"code":"live","label":"Live","tone":"success"},{"code":"done","label":"Done"}],"transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"live"},{"from":"live","to":"done"}]}}'''
[[attributes]]
code = 'title'
value_type = 'string'
"#;
fn value(status: &str) -> Value {
    json!({"kind":"scalar","attribute_code":"status","value":status})
}

#[sqlx::test]
async fn status_transitions_are_atomic_and_stale_edits_are_rejected(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    let response = client
        .post(format!("{base}/blueprints"))
        .json(&json!({"definition": DEFINITION}))
        .send()
        .await
        .unwrap();
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap()
    );
    let blueprint: Value = response.json().await.unwrap();
    client
        .post(format!(
            "{base}/blueprints/{}/versions/{}/publish",
            blueprint["blueprint"]["id"].as_str().unwrap(),
            blueprint["blueprint"]["version"].as_i64().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let create = |status: &str| {
        client
            .post(format!("{base}/v1/entities"))
            .json(&json!({"blueprint":{"code":"status_item"},"values":[value(status)]}))
    };
    assert_eq!(
        create("live").send().await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let entity: Value = create("draft")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!("{base}/v1/entities/{}", entity["id"].as_str().unwrap());
    let version = &entity["updated_at"];
    // Intermediate writes cannot turn draft -> live -> done into one valid save.
    let denied = client
        .put(&url)
        .json(&json!({"expected_updated_at":version,"values":[value("live"),value("done")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        denied.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        denied.text().await.unwrap()
    );
    let missing = client
        .put(&url)
        .json(&json!({"values":[value("live")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::PRECONDITION_REQUIRED);
    let live: Value = client
        .put(&url)
        .json(&json!({"expected_updated_at":version,"values":[value("live")]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(live["projections"]["preview"]["default"]["status"], "live");
    // live -> done is valid, but the old form version must still conflict.
    let stale = client
        .put(&url)
        .json(&json!({"expected_updated_at":version,"values":[value("done")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let clear = client.put(&url).json(&json!({"expected_updated_at":live["updated_at"],"remove_values":[{"attribute_code":"status"}]})).send().await.unwrap();
    assert_eq!(clear.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let done: Value = client
        .put(&url)
        .json(&json!({"expected_updated_at":live["updated_at"],"values":[value("done")]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(done["projections"]["preview"]["default"]["status"], "done");
    // The append endpoint has the same precondition and transition rules.
    let append_url = format!("{base}/entities/{}/values", entity["id"].as_str().unwrap());
    let append = client
        .post(&append_url)
        .json(&json!({"expected_updated_at":done["updated_at"],"values":[value("draft")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(append.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let missing = client
        .post(&append_url)
        .json(&json!({"values":[value("draft")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::PRECONDITION_REQUIRED);
    let unrelated = client.post(&append_url).json(&json!({"values":[{"kind":"scalar","attribute_code":"title","value":"Still editable"}]})).send().await.unwrap();
    assert!(
        unrelated.status().is_success(),
        "{}",
        unrelated.text().await.unwrap()
    );
    server.abort();
}

#[sqlx::test]
async fn inherited_status_and_removing_an_override_use_effective_values(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, DEFINITION).await;
    let context: Value = client
        .post(format!("{base}/contexts"))
        .json(&json!({"code":"web","data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let entity: Value = client
        .post(format!("{base}/v1/entities"))
        .json(&json!({"blueprint":{"code":"status_item"},"values":[value("draft")]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!("{base}/v1/entities/{}", entity["id"].as_str().unwrap());
    let mut live_value = value("live");
    live_value["context_id"] = context["id"].clone();
    let live: Value = client
        .put(&url)
        .json(&json!({"expected_updated_at":entity["updated_at"],"values":[live_value]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(live["projections"]["preview"]["web"]["status"], "live");
    // Removing the override would move live back to inherited draft.
    let clear = client.put(&url).json(&json!({"expected_updated_at":live["updated_at"],"remove_values":[{"attribute_code":"status","context_id":context["id"]}]})).send().await.unwrap();
    assert_eq!(clear.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}

#[sqlx::test]
async fn concurrent_status_edits_have_one_winner(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, DEFINITION).await;
    let entity: Value = client
        .post(format!("{base}/v1/entities"))
        .json(&json!({"blueprint":{"code":"status_item"},"values":[value("draft")]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!("{base}/v1/entities/{}", entity["id"].as_str().unwrap());
    let body = json!({"expected_updated_at":entity["updated_at"],"values":[value("live")]});
    let (a, b) = tokio::join!(
        client.put(&url).json(&body).send(),
        client.put(&url).json(&body).send()
    );
    let statuses = [a.unwrap().status(), b.unwrap().status()];
    assert_eq!(
        statuses.iter().filter(|status| status.is_success()).count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CONFLICT)
            .count(),
        1
    );
    server.abort();
}

#[sqlx::test]
async fn migration_does_not_silently_clear_a_status(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, DEFINITION).await;
    let entity: Value = client
        .post(format!("{base}/v1/entities"))
        .json(&json!({"blueprint":{"code":"status_item"},"values":[value("draft")]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let definition = "format_version = 1\ncode = 'status_item'\nname = 'Status item'\nkind = 'entity'\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'";
    client
        .post(format!("{base}/blueprints/{blueprint_id}/versions"))
        .json(&json!({"definition":definition}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!(
            "{base}/blueprints/{blueprint_id}/versions/2/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let url = format!(
        "{base}/v1/entities/{}/blueprint-migration",
        entity["id"].as_str().unwrap()
    );
    let preview: Value = client
        .post(format!("{url}/preview"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = client.post(&url).query(&[("expected_updated_at", preview["source_updated_at"].as_str().unwrap())]).json(&json!({"migration_id":preview["migration_id"],"expected_target_version":2,"discard_attributes":["status"]})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("status transition is not allowed")
    );
    server.abort();
}
