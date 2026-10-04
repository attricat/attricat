//! One predicate engine enforced as entity-schema checks, transition
//! conditions, enforcing rules and publication gates, and reported by rules.
mod support;

use std::time::Duration;

use api::{
    repository::CatalogRepository, rule_runtime, task_queue::TaskKind, task_worker::TaskOutcome,
};
use support::*;

async fn post(client: &Client, url: String, body: Value) -> (StatusCode, Value) {
    status_json(client.post(url).json(&body).send().await.unwrap()).await
}

async fn put(client: &Client, url: String, body: Value) -> (StatusCode, Value) {
    status_json(client.put(url).json(&body).send().await.unwrap()).await
}

async fn create(
    client: &Client,
    base: &str,
    blueprint: &str,
    values: Value,
) -> (StatusCode, Value) {
    status_json(post_entity(client, base, blueprint, values).await).await
}

/// Runs queued rule tasks to completion; other task kinds are acknowledged.
async fn drain_rule_tasks(pool: &PgPool) {
    let repository = CatalogRepository::new(pool.clone(), BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let handler = rule_runtime::task_handler(repository.clone());
    for _ in 0..50 {
        let Some(task) = repository
            .claim_task("checks-test", Duration::from_secs(30))
            .await
            .unwrap()
        else {
            return;
        };
        if task.kind == TaskKind::RuleRunV1 {
            match handler.handle(task.clone()).await.unwrap() {
                TaskOutcome::Complete => {}
                TaskOutcome::Reschedule { .. } => {
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
                other => panic!("unexpected rule task outcome {other:?}"),
            }
        }
        repository
            .complete_task(task.id, &task.lease_owner, task.lease_token)
            .await
            .unwrap();
    }
}

const VALIDITY: &str = r#"format_version = 1
code = "certificate"
name = "Certificate"
kind = "entity"
entity_schema = '''{
  "type": "object",
  "x-attricat-checks": [
    {"code": "valid-range", "message": "Valid until must not be before valid from",
     "predicate": {"type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from"}},
    {"code": "limits", "predicate": {"type": "compare", "attribute_code": "lower", "op": "lte", "other_attribute_code": "upper"}},
    {"code": "distinct-ends", "predicate": {"type": "compare", "attribute_code": "source", "op": "disjoint", "other_attribute_code": "target"}}
  ]
}'''
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "valid_from"
value_type = "date"
[[attributes]]
code = "valid_until"
value_type = "date"
[[attributes]]
code = "lower"
value_type = "number"
[[attributes]]
code = "upper"
value_type = "integer"
[[attributes]]
code = "source"
value_type = "relationship"
target_blueprint = "certificate"
[[attributes]]
code = "target"
value_type = "relationship"
target_blueprint = "certificate"
"#;

#[sqlx::test]
async fn entity_schema_checks_compare_attributes_on_every_write(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();

    let (status, body) = post(
        &client,
        format!("{base}/blueprints"),
        json!({"definition": VALIDITY.replace(r#""other_attribute_code": "upper""#, r#""other_attribute_code": "title""#)}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "invalid_blueprint_definition");
    create_blueprint(&client, &base, VALIDITY).await;

    let (status, body) = create(
        &client,
        &base,
        "certificate",
        json!([
            scalar("valid_from", json!("2026-05-01")),
            scalar("valid_until", json!("2026-04-01"))
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "entity_check_failed");
    let violation = &body["error"]["details"]["violations"][0];
    assert_eq!(violation["source"], "entity_check");
    assert_eq!(violation["code"], "valid-range");
    assert_eq!(
        violation["message"],
        "Valid until must not be before valid from"
    );
    assert_eq!(
        violation["attributes"],
        json!(["valid_until", "valid_from"])
    );
    assert_eq!(violation["contexts"], json!(["default"]));

    let valid = create_entity_with(
        &client,
        &base,
        "certificate",
        json!([
            scalar("valid_from", json!("2026-04-01")),
            scalar("valid_until", json!("2026-05-01")),
            scalar("lower", json!(1.5)),
            scalar("upper", json!(2))
        ]),
    )
    .await;
    let url = format!("{base}/v1/entities/{}", valid["id"].as_str().unwrap());
    let (status, body) = put(
        &client,
        url.clone(),
        json!({"values": [scalar("lower", json!(3))]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["details"]["violations"][0]["code"], "limits");
    // The rejected write is not committed.
    let form = get_json(&client, url.clone()).await;
    assert!(form.to_string().contains("1.5"), "{form}");

    let other = create_entity_with(&client, &base, "certificate", json!([])).await;
    let (status, body) = create(
        &client,
        &base,
        "certificate",
        json!([
            relationship("source", &other),
            relationship("target", &other)
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["error"]["details"]["violations"][0]["code"],
        "distinct-ends"
    );
    server.abort();
}

const SUPPLY: &str = r#"format_version = 1
code = "supplier"
name = "Supplier"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#;

const FACILITY: &str = r#"format_version = 1
code = "facility"
name = "Facility"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "state"
value_type = "string"
[[attributes]]
code = "supplier"
value_type = "relationship"
target_blueprint = "supplier"
cardinality = "one"
"#;

const ASSESSMENT: &str = r#"format_version = 1
code = "assessment"
name = "Assessment"
kind = "entity"
entity_schema = '''{
  "x-attricat-checks": [
    {"code": "facility-of-supplier", "message": "The facility must belong to the selected supplier",
     "predicate": {"type": "linked", "relationship_code": "facility",
       "predicate": {"type": "compare", "attribute_code": "supplier", "op": "eq", "subject_attribute_code": "supplier"}}},
    {"code": "facility-approved",
     "predicate": {"type": "linked", "relationship_code": "facility",
       "predicate": {"type": "one_of", "attribute_code": "state", "values": ["approved"]}}}
  ]
}'''
[views.dropdown_option]
type = "dropdown_option"
fields = ["reference"]
[[attributes]]
code = "reference"
value_type = "string"
[[attributes]]
code = "supplier"
value_type = "relationship"
target_blueprint = "supplier"
cardinality = "one"
[[attributes]]
code = "facility"
value_type = "relationship"
target_blueprint = "facility"
cardinality = "one"
"#;

#[sqlx::test]
async fn linked_record_checks_read_one_hop_on_save(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    for definition in [SUPPLY, FACILITY, ASSESSMENT] {
        create_blueprint(&client, &base, definition).await;
    }
    let acme = create_entity_with(
        &client,
        &base,
        "supplier",
        json!([scalar("name", json!("Acme"))]),
    )
    .await;
    let globex = create_entity_with(
        &client,
        &base,
        "supplier",
        json!([scalar("name", json!("Globex"))]),
    )
    .await;
    let plant = create_entity_with(
        &client,
        &base,
        "facility",
        json!([
            scalar("name", json!("Plant")),
            scalar("state", json!("approved")),
            relationship("supplier", &acme)
        ]),
    )
    .await;

    let (status, body) = create(
        &client,
        &base,
        "assessment",
        json!([
            relationship("supplier", &globex),
            relationship("facility", &plant)
        ]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let violation = &body["error"]["details"]["violations"][0];
    assert_eq!(violation["code"], "facility-of-supplier");
    assert_eq!(violation["attributes"], json!(["facility", "supplier"]));
    assert_eq!(
        violation["evidence"]["failing_entity_ids"],
        json!([plant["id"]])
    );

    create_entity_with(
        &client,
        &base,
        "assessment",
        json!([
            relationship("supplier", &acme),
            relationship("facility", &plant)
        ]),
    )
    .await;

    // A later change to the linked record is not rejected by the dependent's
    // checks; rules report it as a finding instead.
    let (status, body) = put(
        &client,
        format!("{base}/v1/entities/{}", plant["id"].as_str().unwrap()),
        json!({"values": [scalar("state", json!("suspended"))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    server.abort();
}

const NONCONFORMANCE: &str = r#"format_version = 1
code = "nonconformance"
name = "Nonconformance"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "root_cause"
value_type = "string"
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{"type":"string","enum":["open","closed"],"x-attricat-status":{"version":1,
  "options":[{"code":"open","label":"Open"},{"code":"closed","label":"Closed"}],
  "transitions":[{"from":null,"to":"open"},{"from":"closed","to":"open"},
    {"from":"open","to":"closed","conditions":[
      {"code":"root-cause","message":"Record the root cause","predicate":{"type":"required","attribute_code":"root_cause"}},
      {"code":"actions-closed","predicate":{"type":"referenced_by","blueprint_code":"corrective_action","relationship_code":"nonconformance","max":0,
        "predicate":{"type":"one_of","attribute_code":"state","values":["open"]}}}
    ]}]}}'''
"#;

const ACTION: &str = r#"format_version = 1
code = "corrective_action"
name = "Corrective action"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["state"]
[[attributes]]
code = "state"
value_type = "string"
[[attributes]]
code = "nonconformance"
value_type = "relationship"
target_blueprint = "nonconformance"
cardinality = "one"
"#;

#[sqlx::test]
async fn transition_conditions_block_and_explain_status_changes(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, NONCONFORMANCE).await;
    create_blueprint(&client, &base, ACTION).await;
    let nc = create_entity_with(
        &client,
        &base,
        "nonconformance",
        json!([scalar("status", json!("open"))]),
    )
    .await;
    let action = create_entity_with(
        &client,
        &base,
        "corrective_action",
        json!([
            scalar("state", json!("open")),
            relationship("nonconformance", &nc)
        ]),
    )
    .await;
    let nc_url = format!("{base}/v1/entities/{}", nc["id"].as_str().unwrap());

    // The documented controlled-records shape: one item per declared edge
    // leaving the saved status.
    let options = get_json(&client, format!("{nc_url}/status-transitions")).await;
    let closed = options["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["to"] == "closed")
        .unwrap();
    assert_eq!(closed["from"], "open");
    assert_eq!(closed["allowed"], false);
    assert_eq!(closed["denial_code"], "transition_conditions_unmet");
    assert_eq!(closed["unmet"].as_array().unwrap().len(), 2);

    let (status, body) = put(
        &client,
        nc_url.clone(),
        json!({"expected_updated_at": nc["updated_at"], "values": [scalar("status", json!("closed"))]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "transition_conditions_unmet");
    let unmet = body["error"]["details"]["violations"].as_array().unwrap();
    let codes: Vec<_> = unmet
        .iter()
        .map(|violation| violation["code"].clone())
        .collect();
    assert_eq!(codes, vec![json!("root-cause"), json!("actions-closed")]);
    assert_eq!(
        unmet[0]["transition"],
        json!({"attribute_code": "status", "from": "open", "to": "closed"})
    );
    assert_eq!(unmet[1]["evidence"]["count"], 1);

    let (status, body) = put(
        &client,
        format!("{base}/v1/entities/{}", action["id"].as_str().unwrap()),
        json!({"values": [scalar("state", json!("done"))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    // Conditions see the transaction's final state, so the root cause can be
    // recorded in the same save as the transition.
    let (status, body) = put(
        &client,
        nc_url.clone(),
        json!({"expected_updated_at": nc["updated_at"], "values": [scalar("root_cause", json!("Worn tool")), scalar("status", json!("closed"))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    server.abort();
}

const PRODUCT: &str = r#"format_version = 1
code = "rule_product"
name = "Rule product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "price"
value_type = "number"
[[attributes]]
code = "expires_on"
value_type = "date"
"#;

async fn create_rule(
    client: &Client,
    base: &str,
    blueprint: &Value,
    definition: &str,
) -> (StatusCode, Value) {
    post(
        client,
        format!("{base}/rules"),
        json!({
            "blueprint_id": blueprint["blueprint"]["id"],
            "blueprint_version": blueprint["blueprint"]["version"],
            "context_id": null,
            "definition": definition,
        }),
    )
    .await
}

async fn publish_rule(client: &Client, base: &str, rule: &Value) {
    let id = rule["id"].as_str().unwrap();
    let (status, body) = post(
        client,
        format!("{base}/rules/{id}/versions/1/publish"),
        json!({}),
    )
    .await;
    assert!(status.is_success(), "{body}");
}

/// Publishes and enables version 1 of a rule.
async fn enable_rule(client: &Client, base: &str, rule: &Value) {
    publish_rule(client, base, rule).await;
    let id = rule["id"].as_str().unwrap();
    let (status, body) = post(
        client,
        format!("{base}/rules/{id}/versions/1/enable"),
        json!({}),
    )
    .await;
    assert!(status.is_success(), "{body}");
}

/// Queues a reporting run of the rule.
async fn run_rule(client: &Client, base: &str, rule_id: &str, idempotency_key: &str) {
    let (status, body) = post(
        client,
        format!("{base}/rules/{rule_id}/run-now"),
        json!({"dry_run": false, "idempotency_key": idempotency_key}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
}

fn rule(code: &str, predicate: &str, extra: &str) -> String {
    format!(
        "format_version = 1\ncode = \"{code}\"\nname = \"{code}\"\nseverity = \"error\"\n[[triggers]]\ntype = \"manual\"\n[predicate]\n{predicate}\n{extra}"
    )
}

#[sqlx::test]
async fn new_rule_predicates_open_and_resolve_findings(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, PRODUCT).await;
    let first = create_entity_with(
        &client,
        &base,
        "rule_product",
        json!([
            scalar("sku", json!("A-1")),
            scalar("expires_on", json!("2000-01-01"))
        ]),
    )
    .await;
    // `unique` compares values as `[[unique_keys]]` do: trimmed, whitespace
    // collapsed and case-insensitive.
    let second = create_entity_with(
        &client,
        &base,
        "rule_product",
        json!([
            scalar("sku", json!("  a-1 ")),
            scalar("expires_on", json!("2999-01-01"))
        ]),
    )
    .await;

    let (status, body) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "bad",
            "type = \"unique\"\nattribute_codes = [\"missing\"]",
            "",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let mut rules = Vec::new();
    for definition in [
        rule(
            "unique-sku",
            "type = \"unique\"\nattribute_codes = [\"sku\"]",
            "",
        ),
        rule(
            "not-expired",
            "type = \"relative_date\"\nattribute_code = \"expires_on\"\nop = \"gt\"\noffset_days = 30",
            "",
        ),
    ] {
        let (status, created) = create_rule(&client, &base, &blueprint, &definition).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        enable_rule(&client, &base, &created).await;
        run_rule(&client, &base, created["id"].as_str().unwrap(), "first").await;
        rules.push(created);
    }
    drain_rule_tasks(&pool).await;
    let findings: Vec<(String, Uuid, String)> = sqlx::query_as("SELECT r.code, f.entity_id, f.state FROM rule_findings f JOIN rules r ON r.id = f.rule_id AND r.version = f.rule_version ORDER BY r.code, f.entity_id")
        .fetch_all(&pool).await.unwrap();
    let first_id: Uuid = first["id"].as_str().unwrap().parse().unwrap();
    let second_id: Uuid = second["id"].as_str().unwrap().parse().unwrap();
    let mut duplicates = [first_id, second_id];
    duplicates.sort();
    assert_eq!(
        findings,
        vec![
            ("not-expired".into(), first_id, "open".into()),
            ("unique-sku".into(), duplicates[0], "open".into()),
            ("unique-sku".into(), duplicates[1], "open".into()),
        ]
    );

    let (status, body) = put(
        &client,
        format!("{base}/v1/entities/{second_id}"),
        json!({"values": [scalar("sku", json!("B-2"))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let id = rules[0]["id"].as_str().unwrap();
    run_rule(&client, &base, id, "second").await;
    drain_rule_tasks(&pool).await;
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rule_findings WHERE rule_id = $1 AND state = 'open'",
    )
    .bind(id.parse::<Uuid>().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(open, 0);
    server.abort();
}

const GALLERY: &str = r#"format_version = 1
code = "gallery_item"
name = "Gallery item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "photo"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["png"]
max_bytes = 1048576
"#;

#[sqlx::test]
async fn predicates_see_file_values_and_explicit_empty_local_values(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, GALLERY).await;
    let (status, french) = post(
        &client,
        format!("{base}/contexts"),
        json!({"code": "gallery-fr", "data": {}}),
    )
    .await;
    assert!(status.is_success(), "{french}");
    let workspace_id: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let repository = CatalogRepository::new(pool.clone(), workspace_id);
    let mut ids = Vec::new();
    for title in ["with photo", "photo removed in fr"] {
        let item = create_entity_with(
            &client,
            &base,
            "gallery_item",
            json!([scalar("title", json!(title))]),
        )
        .await;
        let id: Uuid = item["id"].as_str().unwrap().parse().unwrap();
        let file_id = Uuid::new_v4();
        sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'p.png', 'p.png', 'image/png', 7, $3, 'files/p.png', 'ready')")
            .bind(file_id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
        repository
            .link_file_to_attribute(id, "photo", None, file_id)
            .await
            .unwrap();
        ids.push(id);
    }
    // An explicit empty local value (what removing every file in a context
    // stores) must not reveal the photo inherited from default.
    sqlx::query("INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) SELECT gen_random_uuid(), e.workspace_id, e.id, a.id, $2, true FROM entities e JOIN attributes a ON a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version AND a.code = 'photo' WHERE e.id = $1")
        .bind(ids[1])
        .bind(french["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let (status, created) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "has-photo",
            "type = \"required\"\nattribute_code = \"photo\"",
            "",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    enable_rule(&client, &base, &created).await;
    run_rule(&client, &base, created["id"].as_str().unwrap(), "photos").await;
    drain_rule_tasks(&pool).await;
    let settled = wait_until(|| async {
        count(
            &pool,
            "SELECT count(*) FROM rule_runs WHERE status NOT IN ('completed', 'dead_letter', 'cancelled')",
        )
        .await
            == 0
    })
    .await;
    assert!(settled, "the rule run did not settle");
    let findings: Vec<(Uuid, Value)> = sqlx::query_as(
        "SELECT entity_id, evidence FROM rule_findings WHERE state = 'open' ORDER BY entity_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].0, ids[1]);
    assert_eq!(findings[0].1["contexts"], json!(["gallery-fr"]));
    server.abort();
}

#[sqlx::test]
async fn enforcing_rules_dry_run_before_enabling_and_reject_writes(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, PRODUCT).await;
    let negative = create_entity_with(
        &client,
        &base,
        "rule_product",
        json!([scalar("price", json!(-5))]),
    )
    .await;

    let enforce = "[enforcement]\non_save = true";
    let (status, body) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "unique-enforced",
            "type = \"unique\"\nattribute_codes = [\"sku\"]",
            enforce,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("synchronous"),
        "{body}"
    );

    let (status, created) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "price-non-negative",
            "type = \"compare\"\nattribute_code = \"price\"\nop = \"gte\"\nvalue = 0",
            enforce,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    publish_rule(&client, &base, &created).await;
    let id = created["id"].as_str().unwrap();
    let enable = format!("{base}/rules/{id}/versions/1/enable");

    let (status, body) = post(&client, enable.clone(), json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "rule_dry_run_required");

    let (status, body) = post(
        &client,
        format!("{base}/rules/{id}/run-now"),
        json!({"dry_run": true, "idempotency_key": "preview", "version": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    drain_rule_tasks(&pool).await;
    // A dry run that stopped at the candidate cap does not cover every entity.
    sqlx::query("UPDATE rule_runs SET truncated = true WHERE dry_run")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&client, enable.clone(), json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "rule_dry_run_required");
    sqlx::query("UPDATE rule_runs SET truncated = false WHERE dry_run")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&client, enable.clone(), json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "rule_has_existing_violations");
    assert_eq!(body["error"]["details"]["existing_violations"], 1);
    let (status, body) = post(&client, enable, json!({"accept_existing_violations": true})).await;
    assert!(status.is_success(), "{body}");

    let (status, body) = create(
        &client,
        &base,
        "rule_product",
        json!([scalar("price", json!(-1))]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "rule_violation");
    let violation = &body["error"]["details"]["violations"][0];
    assert_eq!(violation["source"], "rule");
    assert_eq!(violation["code"], "price-non-negative");
    assert_eq!(violation["severity"], "error");
    assert_eq!(violation["attributes"], json!(["price"]));

    // The existing violation blocks unrelated edits until it is fixed.
    let url = format!("{base}/v1/entities/{}", negative["id"].as_str().unwrap());
    let (status, _) = put(
        &client,
        url.clone(),
        json!({"values": [scalar("sku", json!("X"))]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, body) = put(
        &client,
        url,
        json!({"values": [scalar("sku", json!("X")), scalar("price", json!(5))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    server.abort();
}

#[sqlx::test]
async fn channels_require_checks_before_publication(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, PRODUCT).await;
    let (status, created) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "has-sku",
            "type = \"required\"\nattribute_code = \"sku\"",
            "",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    enable_rule(&client, &base, &created).await;

    let context = get_json(&client, format!("{base}/contexts/default")).await;
    let context_id = context["id"].as_str().unwrap();
    // A code that names no rule would silently disable the gate.
    for codes in [json!(["has-skuu"]), json!(["has-sku", "has-sku"])] {
        let (status, body) = put(
            &client,
            format!("{base}/publication-channels/{context_id}"),
            json!({"enabled": true, "required_rule_codes": codes}),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["error"]["code"], "invalid_input", "{body}");
    }
    let (status, channel) = put(
        &client,
        format!("{base}/publication-channels/{context_id}"),
        json!({"enabled": true, "required_rule_codes": ["has-sku"], "require_valid_entity": true}),
    )
    .await;
    assert!(status.is_success(), "{channel}");
    assert_eq!(channel["required_rule_codes"], json!(["has-sku"]));

    let product = create_entity_with(&client, &base, "rule_product", json!([])).await;
    let entity_url = format!("{base}/v1/entities/{}", product["id"].as_str().unwrap());
    let readiness = get_json(&client, format!("{entity_url}/publications/readiness")).await;
    assert_eq!(readiness[0]["ready"], false);
    assert_eq!(readiness[0]["violations"][0]["code"], "has-sku");

    let (status, body) = post(
        &client,
        format!("{entity_url}/publications"),
        json!({"context_id": context_id}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "publication_checks_failed");
    assert_eq!(body["error"]["details"]["context"], "default");
    assert_eq!(body["error"]["details"]["violations"][0]["source"], "rule");

    let (status, body) = post(
        &client,
        format!(
            "{base}/blueprints/{}/versions/1/entity-publications",
            blueprint["blueprint"]["id"].as_str().unwrap()
        ),
        json!({"context_id": context_id}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["error"]["details"]["violations"][0]["evidence"]["entity_id"],
        product["id"]
    );

    let (status, body) = put(
        &client,
        entity_url.clone(),
        json!({"values": [scalar("sku", json!("A-1"))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let readiness = get_json(&client, format!("{entity_url}/publications/readiness")).await;
    assert_eq!(readiness[0]["ready"], true, "{readiness}");
    let (status, body) = post(
        &client,
        format!("{entity_url}/publications"),
        json!({"context_id": context_id}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    server.abort();
}

#[sqlx::test]
async fn retained_edits_withdraw_publications_whose_channel_checks_now_fail(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let definition = PRODUCT.replace("rule_product", "retained_product")
        + "[publication]\nretain_on_edit_roles = [\"owner\"]\n";
    let blueprint = create_blueprint(&client, &base, &definition).await;
    let (status, created) = create_rule(
        &client,
        &base,
        &blueprint,
        &rule(
            "retained-sku",
            "type = \"required\"\nattribute_code = \"sku\"",
            "",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    enable_rule(&client, &base, &created).await;

    let mut channels = Vec::new();
    for (code, required) in [
        ("gated-web", json!(["retained-sku"])),
        ("open-web", json!([])),
    ] {
        let (status, context) = post(
            &client,
            format!("{base}/contexts"),
            json!({"code": code, "data": {}}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{context}");
        let context_id = context["id"].as_str().unwrap().to_owned();
        let (status, body) = put(
            &client,
            format!("{base}/publication-channels/{context_id}"),
            json!({"enabled": true, "required_rule_codes": required}),
        )
        .await;
        assert!(status.is_success(), "{body}");
        channels.push(context_id);
    }
    let product = create_entity_with(
        &client,
        &base,
        "retained_product",
        json!([scalar("sku", json!("A-1")), scalar("price", json!(5))]),
    )
    .await;
    let entity_id: Uuid = product["id"].as_str().unwrap().parse().unwrap();
    let entity_url = format!("{base}/v1/entities/{entity_id}");
    let (status, body) = post(
        &client,
        format!("{entity_url}/publications/publish-all"),
        json!({}),
    )
    .await;
    assert!(status.is_success(), "{body}");

    // A retained edit that keeps the gate passing keeps both publications.
    let (status, body) = put(
        &client,
        entity_url.clone(),
        json!({"values": [scalar("price", json!(6))]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let published = |statuses: &Value| {
        statuses
            .as_array()
            .unwrap()
            .iter()
            .filter(|status| status["status"] == "published")
            .map(|status| status["context_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let statuses = get_json(&client, format!("{entity_url}/publications")).await;
    assert_eq!(published(&statuses).len(), 2, "{statuses}");

    // Removing the SKU fails the gated channel's rule: that publication is
    // withdrawn, the ungated one is retained.
    let default_context = get_json(&client, format!("{base}/contexts/default")).await;
    let (status, body) = put(
        &client,
        entity_url.clone(),
        json!({"remove_values": [{
            "attribute_code": "sku",
            "context_id": default_context["id"],
        }]}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let statuses = get_json(&client, format!("{entity_url}/publications")).await;
    assert_eq!(
        published(&statuses),
        vec![channels[1].clone()],
        "{statuses}"
    );
    let unpublished: Vec<Value> = sqlx::query_scalar(
        "SELECT payload FROM domain_events WHERE aggregate_id = $1 AND event_type = 'entity.unpublished.v1'",
    )
    .bind(entity_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(unpublished.len(), 1, "{unpublished:?}");
    assert_eq!(unpublished[0]["context_id"], channels[0].as_str());
    assert_eq!(unpublished[0]["reason"], "checks_failed");
    server.abort();
}

const PAGE_ITEM: &str = r#"format_version = 1
code = "page_item"
name = "Page item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["state"]
[[attributes]]
code = "state"
value_type = "string"
"#;

const PAGE_ORDER: &str = r#"format_version = 1
code = "page_order"
name = "Page order"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "items"
value_type = "relationship"
target_blueprint = "page_item"
cardinality = "many"
"#;

const PAGE_NOTE: &str = r#"format_version = 1
code = "page_note"
name = "Page note"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["state"]
[[attributes]]
code = "state"
value_type = "string"
[[attributes]]
code = "order"
value_type = "relationship"
target_blueprint = "page_order"
cardinality = "one"
"#;

/// A rule run reads linked and referencing records for a whole candidate
/// page at once; each candidate still sees only its own related records.
#[sqlx::test]
async fn rule_pages_resolve_related_records_per_candidate(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, PAGE_ITEM).await;
    let orders = create_blueprint(&client, &base, PAGE_ORDER).await;
    create_blueprint(&client, &base, PAGE_NOTE).await;
    let item = |state: &'static str| {
        let client = client.clone();
        let base = base.clone();
        async move {
            create_entity_with(
                &client,
                &base,
                "page_item",
                json!([scalar("state", json!(state))]),
            )
            .await
        }
    };
    let ok = item("ok").await;
    let also_ok = item("ok").await;
    let bad = item("bad").await;
    let order = |title: &'static str, items: Vec<&Value>| {
        let mut values = vec![scalar("title", json!(title))];
        values.extend(items.into_iter().map(|item| relationship("items", item)));
        create_entity_with(&client, &base, "page_order", Value::Array(values))
    };
    let empty = order("empty", vec![]).await;
    let open_note = order("open note", vec![&ok]).await;
    let bad_item = order("bad item", vec![&bad]).await;
    let closed_note = order("closed note", vec![&ok, &also_ok]).await;
    let mixed_notes = order("mixed notes", vec![&ok]).await;
    let note = |state: &'static str, order: &Value| {
        create_entity_with(
            &client,
            &base,
            "page_note",
            json!([scalar("state", json!(state)), relationship("order", order)]),
        )
    };
    let reopened = note("open", &open_note).await;
    note("closed", &closed_note).await;
    note("open", &mixed_notes).await;
    note("closed", &mixed_notes).await;

    let definition = rule(
        "page-related",
        r#"type = "all_of"
predicates = [
  {type = "linked", relationship_code = "items", quantifier = "all", predicate = {type = "one_of", attribute_code = "state", values = ["ok"]}},
  {type = "referenced_by", blueprint_code = "page_note", relationship_code = "order", max = 0, predicate = {type = "one_of", attribute_code = "state", values = ["open"]}},
]"#,
        "",
    );
    let (status, created) = create_rule(&client, &base, &orders, &definition).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    publish_rule(&client, &base, &created).await;
    let id = created["id"].as_str().unwrap();
    let rule_id: Uuid = id.parse().unwrap();
    let (status, body) = post(
        &client,
        format!("{base}/rules/{id}/versions/1/enable"),
        json!({}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    let run = |key: &'static str| {
        post(
            &client,
            format!("{base}/rules/{id}/run-now"),
            json!({"dry_run": false, "idempotency_key": key, "version": 1}),
        )
    };
    let (status, body) = run("first").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    drain_rule_tasks(&pool).await;
    let open_findings = || async {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT entity_id FROM rule_findings WHERE rule_id = $1 AND state = 'open' ORDER BY entity_id",
        )
        .bind(rule_id)
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    let id_of = |entity: &Value| entity["id"].as_str().unwrap().parse::<Uuid>().unwrap();
    let mut expected = vec![id_of(&open_note), id_of(&bad_item), id_of(&mixed_notes)];
    expected.sort();
    assert_eq!(open_findings().await, expected);
    // `empty` and `closed_note` pass: no items or notes, or only closed ones.
    assert!(
        ![id_of(&empty), id_of(&closed_note)]
            .iter()
            .any(|order| expected.contains(order))
    );

    // Closing the only open note resolves exactly that order's finding.
    let reopened_id = reopened["id"].as_str().unwrap();
    let (status, body) = put(
        &client,
        format!("{base}/v1/entities/{reopened_id}"),
        json!({"values": [scalar("state", json!("closed"))]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = run("second").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    drain_rule_tasks(&pool).await;
    expected.retain(|entity| *entity != id_of(&open_note));
    assert_eq!(open_findings().await, expected);
    let (created_count, resolved_count): (i64, i64) = sqlx::query_as(
        "SELECT sum(findings_created)::bigint, sum(findings_resolved)::bigint FROM rule_runs WHERE rule_id = $1",
    )
    .bind(rule_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((created_count, resolved_count), (3, 1));
    server.abort();
}
