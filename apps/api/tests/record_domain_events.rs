mod support;

use api::domain_events::{
    ATTRIBUTE_VALUE_CHANGED_V1, ATTRIBUTE_VALUE_RESTORED_V1, RECORD_CREATED_V1,
};
use support::*;

#[sqlx::test]
async fn record_and_scalar_value_writes_enqueue_one_normalized_event_each(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"evented_product\"\nname = \"Evented product\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"",
    )
    .await;
    let record = create_record(&client, &base_url, &blueprint).await;
    let record_id = record["id"].as_str().unwrap();

    for title in ["First title", "Second title"] {
        client
            .post(format!("{base_url}/records/{record_id}/values"))
            .json(&json!({ "values": [{
                "kind": "scalar", "attribute_code": "title", "value": title
            }] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let history_id = client
        .get(format!("{base_url}/records/{record_id}/values/history"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .await
        .unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    client
        .post(format!(
            "{base_url}/records/{record_id}/values/history/{history_id}/restore"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let events = sqlx::query_as::<_, (String, Value)>(
        "SELECT event_type, payload FROM domain_events WHERE aggregate_id = $1 ORDER BY sequence",
    )
    .bind(record_id.parse::<Uuid>().unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.0.as_str())
            .collect::<Vec<_>>(),
        [
            RECORD_CREATED_V1,
            ATTRIBUTE_VALUE_CHANGED_V1,
            ATTRIBUTE_VALUE_CHANGED_V1,
            ATTRIBUTE_VALUE_RESTORED_V1,
        ]
    );
    assert_eq!(events[2].1["facts"][0]["attribute_code"], "title");
    assert_eq!(events[2].1["facts"][0]["before_value"], "First title");
    assert_eq!(events[2].1["facts"][0]["after_value"], "Second title");
    assert_eq!(events[3].1["facts"][0]["change_kind"], "restore");
    assert_eq!(events[3].1["facts"][0]["before_value"], "Second title");
    assert_eq!(events[3].1["facts"][0]["after_value"], "First title");
    server.abort();
}

#[sqlx::test]
async fn relationship_set_mutation_emits_one_targeted_event(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let target_blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"event_target\"\nname = \"Event target\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"name\"]\n\n[[attributes]]\ncode = \"name\"\nvalue_type = \"string\"",
    )
    .await;
    let source_blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"event_source\"\nname = \"Event source\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"name\"]\n\n[[attributes]]\ncode = \"name\"\nvalue_type = \"string\"\n\n[[attributes]]\ncode = \"target\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"event_target\"",
    )
    .await;
    let target = create_record(&client, &base_url, &target_blueprint).await;
    let source = create_record(&client, &base_url, &source_blueprint).await;
    let source_id = source["id"].as_str().unwrap();
    let target_id = target["id"].as_str().unwrap();

    client
        .post(format!(
            "{base_url}/records/{source_id}/relationships/replace"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "target", "target_record_ids": [target_id]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .post(format!(
            "{base_url}/records/{source_id}/relationships/remove"
        ))
        .json(&json!({ "relationships": [{
            "attribute_code": "target", "target_record_ids": [target_id]
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{base_url}/records/{source_id}/values"))
        .json(&json!({ "values": [{
            "kind": "relationship", "attribute_code": "target", "target_record_id": target_id
        }] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let events = sqlx::query_as::<_, (String, Value)>(
        "SELECT event_type, payload FROM domain_events WHERE aggregate_id = $1 ORDER BY sequence",
    )
    .bind(source_id.parse::<Uuid>().unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(events.len(), 4);
    for event in events.iter().skip(1) {
        assert_eq!(event.0, "relationship.changed.v1");
        assert_eq!(
            event.1["facts"][0]["relationship_target_record_id"],
            target_id
        );
    }
    assert_eq!(events[1].1["facts"][0]["change_kind"], "relationship_add");
    assert_eq!(
        events[2].1["facts"][0]["change_kind"],
        "relationship_remove"
    );
    assert_eq!(events[3].1["facts"][0]["change_kind"], "relationship_add");
    assert!(events[2].1["facts"][0]["after_value"].is_null());
    assert_eq!(events[3].1["facts"][0]["after_value"], target_id);
    server.abort();
}

#[sqlx::test]
async fn record_outbox_failure_rolls_back_the_record_write(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        "format_version = 1\ncode = \"event_rollback_product\"\nname = \"Event rollback product\"\nkind = \"record\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"",
    )
    .await;
    let events_before = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "ALTER TABLE domain_events ADD CONSTRAINT record_events_test_reject CHECK (false) NOT VALID",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({
            "blueprint": {
                "code": blueprint["blueprint"]["code"],
                "version": blueprint["blueprint"]["version"]
            },
            "values": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM records")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domain_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        events_before
    );
    server.abort();
}
