use crate::support;

use chrono::{Datelike, Utc};
use support::*;

const INVOICE: &str = r#"
format_version = 1
code = "gs_invoice"
name = "Invoice"
kind = "record"
record_schema = '{"type":"object","required":["number","title"]}'

[views.dropdown_option]
type = "dropdown_option"
fields = ["number"]

[[unique_keys]]
code = "number"
attributes = ["number"]

[[attributes]]
code = "number"
value_type = "string"
generated = { format = "INV-{n:04}" }

[[attributes]]
code = "position"
value_type = "integer"
generated = { start = 1000 }

[[attributes]]
code = "title"
value_type = "string"
"#;

fn value_of(form: &Value, code: &str) -> Value {
    form["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["attribute_code"] == code)
        .map(|value| value["value"].clone())
        .unwrap_or(Value::Null)
}

async fn form(client: &Client, base_url: &str, record: &Value) -> Value {
    get_json(
        client,
        format!("{base_url}/v1/records/{}", record["id"].as_str().unwrap()),
    )
    .await
}

async fn invoice(client: &Client, base_url: &str, title: &str) -> Value {
    create_record_with(
        client,
        base_url,
        "gs_invoice",
        json!([scalar("title", json!(title))]),
    )
    .await
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn creates_number_records_gaplessly_inside_the_transaction(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, INVOICE).await;
    let attribute = |code: &str| {
        blueprint["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|attribute| attribute["code"] == code)
            .unwrap()
            .clone()
    };
    assert_eq!(attribute("number")["readonly"], true);
    assert_eq!(attribute("number")["context_editable"], "default");
    assert_eq!(
        attribute("number")["generated"],
        json!({"format": "INV-{n:04}", "start": 1, "reset": "never", "time_zone": "UTC"})
    );
    assert_eq!(attribute("title")["generated"], Value::Null);

    let first = invoice(&client, &base_url, "first").await;
    let first_form = form(&client, &base_url, &first).await;
    assert_eq!(value_of(&first_form, "number"), "INV-0001");
    assert_eq!(value_of(&first_form, "position"), 1000);
    let created = latest_domain_event(
        &pool,
        first["id"].as_str().unwrap().parse().unwrap(),
        Some("record.created.v1"),
    )
    .await;
    assert!(
        created.payload["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["attribute_code"] == "number"),
        "{}",
        created.payload
    );

    // A create that fails validation hands its number back.
    expect_error(
        post_record(&client, &base_url, "gs_invoice", json!([])).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "record_schema_mismatch",
    )
    .await;
    let second = invoice(&client, &base_url, "second").await;
    let second_form = form(&client, &base_url, &second).await;
    assert_eq!(value_of(&second_form, "number"), "INV-0002");
    assert_eq!(value_of(&second_form, "position"), 1001);

    // A duplicate is numbered like any new record.
    let copy: Value = client
        .post(format!(
            "{base_url}/v1/records/{}/duplicate",
            first["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let copy_form = form(&client, &base_url, &copy).await;
    assert_eq!(value_of(&copy_form, "number"), "INV-0003");
    assert_eq!(value_of(&copy_form, "title"), "first");

    // A batch numbers its creates in order, in one transaction.
    let batch = expect_status(
        client
            .post(format!("{base_url}/v1/records/batch"))
            .json(&json!({ "operations": [
                {"op": "create", "blueprint": {"code": "gs_invoice"}, "values": [scalar("title", json!("b1"))]},
                {"op": "create", "blueprint": {"code": "gs_invoice"}, "values": [scalar("title", json!("b2"))]},
            ]}))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    let mut numbers = Vec::new();
    for operation in batch["operations"].as_array().unwrap() {
        numbers.push(value_of(
            &form(&client, &base_url, &operation["record"]).await,
            "number",
        ));
    }
    assert_eq!(numbers, [json!("INV-0004"), json!("INV-0005")]);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn clients_cannot_write_generated_values(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, INVOICE).await;
    let record = invoice(&client, &base_url, "first").await;
    let id = record["id"].as_str().unwrap();

    let body = expect_error(
        post_record(
            &client,
            &base_url,
            "gs_invoice",
            json!([
                scalar("title", json!("x")),
                scalar("number", json!("INV-9999"))
            ]),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "generated_attribute_readonly",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "number");

    for request in [
        client
            .put(format!("{base_url}/v1/records/{id}"))
            .json(&json!({"values": [scalar("position", json!(5))]})),
        client
            .put(format!("{base_url}/v1/records/{id}"))
            .json(&json!({"remove_values": [{"attribute_code": "number"}]})),
        client
            .post(format!("{base_url}/records/{id}/values"))
            .json(&json!({"values": [scalar("number", json!("INV-0042"))]})),
    ] {
        expect_error(
            request.send().await.unwrap(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "generated_attribute_readonly",
        )
        .await;
    }
    assert_eq!(
        value_of(&form(&client, &base_url, &record).await, "number"),
        "INV-0001"
    );

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn concurrent_creates_take_distinct_contiguous_numbers(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, INVOICE).await;

    let mut creates = tokio::task::JoinSet::new();
    for index in 0..20 {
        let client = client.clone();
        let base_url = base_url.clone();
        creates.spawn(async move {
            let record = invoice(&client, &base_url, &format!("parallel {index}")).await;
            value_of(&form(&client, &base_url, &record).await, "position")
                .as_i64()
                .unwrap()
        });
    }
    let mut numbers = creates.join_all().await;
    numbers.sort_unstable();
    assert_eq!(numbers, (1000..1020).collect::<Vec<_>>());

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn counters_can_be_listed_and_only_raised(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, INVOICE).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let sequences = format!("{base_url}/blueprints/{blueprint_id}/sequences");

    // Counters that no record has used yet are listed at their start.
    assert_eq!(
        get_json(&client, &sequences).await,
        json!([
            {"attribute_code": "number", "period": "", "next_value": 1, "updated_at": null},
            {"attribute_code": "position", "period": "", "next_value": 1000, "updated_at": null},
        ])
    );

    invoice(&client, &base_url, "first").await;
    let advance = |code: &str, body: Value| {
        client
            .post(format!("{sequences}/{code}/advance"))
            .json(&body)
            .send()
    };
    let raised = expect_status(
        advance("number", json!({"next_value": 500})).await.unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        raised,
        json!({"attribute_code": "number", "period": "", "previous": 2, "next_value": 500})
    );
    let audited: Value = sqlx::query_scalar(
        "SELECT metadata->'generated_sequence' FROM audit_events WHERE action LIKE '%sequences%' ORDER BY occurred_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, raised);

    let next = invoice(&client, &base_url, "after advance").await;
    assert_eq!(
        value_of(&form(&client, &base_url, &next).await, "number"),
        "INV-0500"
    );

    let lowered = expect_error(
        advance("number", json!({"next_value": 10})).await.unwrap(),
        StatusCode::CONFLICT,
        "generated_sequence_not_raised",
    )
    .await;
    assert_eq!(
        lowered["error"]["details"],
        json!({"current": 501, "requested": 10})
    );
    expect_status(
        advance("number", json!({"next_value": 501})).await.unwrap(),
        StatusCode::OK,
    )
    .await;
    expect_error(
        advance("number", json!({"next_value": 600, "period": "2026"}))
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    expect_error(
        advance("title", json!({"next_value": 600})).await.unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;

    let listed = get_json(&client, &sequences).await;
    assert_eq!(listed[0]["next_value"], 501);
    assert_eq!(listed[1]["next_value"], 1002);
    assert!(listed[0]["updated_at"].is_string());

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn records_are_numbered_when_they_migrate_onto_a_generating_revision(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let first_revision = INVOICE
        .replace("generated = { format = \"INV-{n:04}\" }", "")
        .replace("generated = { start = 1000 }", "")
        .replace(
            r#"record_schema = '{"type":"object","required":["number","title"]}'"#,
            "",
        )
        .replace(
            "[[unique_keys]]\ncode = \"number\"\nattributes = [\"number\"]\n",
            "",
        );
    let blueprint = create_blueprint(&client, &base_url, &first_revision).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let old = invoice(&client, &base_url, "old").await;
    let id = old["id"].as_str().unwrap();
    assert_eq!(
        value_of(&form(&client, &base_url, &old).await, "number"),
        Value::Null
    );

    let yearly = INVOICE.replace(
        "generated = { format = \"INV-{n:04}\" }",
        "generated = { format = \"INV-{yyyy}-{n:03}\", reset = \"yearly\" }",
    );
    let revision = expect_status(
        client
            .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
            .json(&json!({ "definition": yearly }))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{blueprint_id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let preview = get_json_post(
        &client,
        format!("{base_url}/v1/records/{id}/blueprint-migration/preview"),
    )
    .await;
    expect_status(
        client
            .post(format!("{base_url}/v1/records/{id}/blueprint-migration"))
            .json(&json!({
                "migration_id": preview["migration_id"],
                "expected_target_version": version,
            }))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;

    let year = Utc::now().year();
    let migrated = form(&client, &base_url, &old).await;
    assert_eq!(value_of(&migrated, "number"), format!("INV-{year}-001"));
    assert_eq!(value_of(&migrated, "position"), 1000);
    let fresh = invoice(&client, &base_url, "new").await;
    assert_eq!(
        value_of(&form(&client, &base_url, &fresh).await, "number"),
        format!("INV-{year}-002")
    );
    assert_eq!(
        get_json(
            &client,
            format!("{base_url}/blueprints/{blueprint_id}/sequences")
        )
        .await[0]["period"],
        year.to_string()
    );

    server.abort();
}

async fn get_json_post(client: &Client, url: String) -> Value {
    client
        .post(url)
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
async fn publication_alongside_creates_does_not_deadlock(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url, INVOICE).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();

    let mut work = tokio::task::JoinSet::new();
    for index in 0..10 {
        let client = client.clone();
        let base_url = base_url.clone();
        work.spawn(async move {
            invoice(&client, &base_url, &format!("racing {index}")).await;
        });
    }
    {
        // A revision with a new unique key re-indexes the family under its
        // exclusive key lock while the creates hold counter rows.
        let client = client.clone();
        let base_url = base_url.clone();
        work.spawn(async move {
            let keyed = INVOICE.replace(
                "[[attributes]]\ncode = \"number\"",
                "[[unique_keys]]\ncode = \"title\"\nattributes = [\"title\"]\n\n[[attributes]]\ncode = \"number\"",
            );
            let revision = expect_status(
                client
                    .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
                    .json(&json!({ "definition": keyed }))
                    .send()
                    .await
                    .unwrap(),
                StatusCode::CREATED,
            )
            .await;
            let version = revision["blueprint"]["version"].as_i64().unwrap();
            expect_status(
                client
                    .post(format!(
                        "{base_url}/blueprints/{blueprint_id}/versions/{version}/publish"
                    ))
                    .send()
                    .await
                    .unwrap(),
                StatusCode::OK,
            )
            .await;
        });
    }
    tokio::time::timeout(std::time::Duration::from_secs(30), work.join_all())
        .await
        .expect("creates and publication finish");
    let after = invoice(&client, &base_url, "after").await;
    assert_eq!(
        value_of(&form(&client, &base_url, &after).await, "position"),
        1010
    );

    server.abort();
}
