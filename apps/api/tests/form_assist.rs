//! Form assists: a blueprint opts in to an enabled extension's assist, and
//! the host calls its command with the form's watched and proposed fields
//! only, keeping proposals for declared, unedited fields that their
//! attributes accept.

use crate::support;

use std::sync::Arc;

use api::{repository::AttricatRepository, storage::FakeObjectStore};
use support::*;

const EXTENSION: &str = "acme.pricing";
const PERMISSIONS: &[&str] = &["client.commands", "client.form_assist"];

const PRODUCT: &str = r#"format_version: 1
code: fa_product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string"#;

/// The order line, using `assists` (YAML list items) under `form_assists`.
fn line(code: &str, assists: &str) -> String {
    format!(
        r#"format_version: 1
code: {code}
name: Order line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [note]
form_assists:{assists}
attributes:
  - code: note
    value_type: string
  - code: memo
    value_type: string
  - code: item
    value_type: relationship
    target_blueprint: fa_product
    cardinality: one
  - code: quantity
    value_type: number
  - code: unit_price_net
    value_type: number
  - code: discount
    value_type: integer
  - code: customer
    value_type: string"#
    )
}

fn assist(id: &str) -> String {
    format!("\n  - {{extension_id: {EXTENSION}, id: {id}, version: 1}}")
}

fn archive() -> Vec<u8> {
    let command = |id: &str| {
        json!({
            "id": id, "handler": id,
            "request_schema": {"type": "object", "required": ["version", "values", "edited"]},
            "response_schema": {"type": "object"}
        })
    };
    let assist = |id: &str, command: &str, watch: Value, proposes: Value| {
        json!({
            "id": id, "version": 1, "kind": "form_assist", "outlet": "record_form",
            "when": {"blueprints": ["fa_line"]},
            "form_assist": {"command": command, "watch": watch, "proposes": proposes}
        })
    };
    let manifest = serde_json::to_vec(&json!({
        "manifest_version": 1,
        "name": "Pricing",
        "version": "1.0.0",
        "description": "form assist integration test",
        "icons": {"48": "icon.png"},
        "attricat": {"id": EXTENSION, "host_api": ">=1.0.0, <2.0.0"},
        "permissions": PERMISSIONS,
        "configuration": {"version": 1, "schema": {"type": "object", "additionalProperties": false}},
        "artifacts": [{"id": "server", "kind": "server_wasm", "path": "server.wasm"}],
        "server": {"commands": [
            command("form-assist"), command("form-assist-echo"), command("form-assist-invalid")
        ]},
        "ui": [
            assist("customer_price", "form-assist", json!(["item", "quantity"]), json!(["unit_price_net", "discount"])),
            assist("echo", "form-assist-echo", json!(["item", "quantity"]), json!(["memo", "unit_price_net"])),
            assist("broken", "form-assist-invalid", json!(["quantity"]), json!(["unit_price_net"])),
            assist("links", "form-assist", json!(["quantity"]), json!(["item"])),
        ]
    }))
    .unwrap();
    tar_zst(&[
        ("manifest.json", &manifest),
        (
            "server.wasm",
            &build_test_component(
                "attricat-unified-test-component",
                "unified-test.component.wasm",
            ),
        ),
    ])
}

struct Form {
    base_url: String,
    client: Client,
    blueprint: Value,
    product: Value,
}

impl Form {
    /// Runs assist `id` with `body` added to the form's identity, retrying
    /// while the component first compiles.
    async fn run(&self, id: &str, body: Value) -> Value {
        let mut request = json!({
            "extension_id": EXTENSION,
            "id": id,
            "blueprint_id": self.blueprint["blueprint"]["id"],
            "blueprint_version": self.blueprint["blueprint"]["version"],
        });
        request
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        for _ in 0..60 {
            let answer = expect_success(
                self.client
                    .post(format!("{}/v1/records/form-assist", self.base_url))
                    .json(&request)
                    .send()
                    .await
                    .unwrap(),
            )
            .await;
            if answer["outcome"] != "timed_out" {
                return answer;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        panic!("the assist never answered");
    }
}

async fn form(pool: PgPool) -> (Form, tokio::task::JoinHandle<()>, AttricatRepository) {
    let repository =
        AttricatRepository::system(pool.clone()).for_workspace(bootstrap_workspace_id());
    let store = Arc::new(FakeObjectStore::available());
    install_extension(
        &repository,
        store.clone(),
        EXTENSION,
        &archive(),
        PERMISSIONS,
    )
    .await;
    let (base_url, server) = start_server_with_object_store(pool, store).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PRODUCT).await;
    let blueprint = create_blueprint(
        &client,
        &base_url,
        &line(
            "fa_line",
            &[assist("customer_price"), assist("echo"), assist("broken")].concat(),
        ),
    )
    .await;
    let product = create_record_with(
        &client,
        &base_url,
        "fa_product",
        json!([scalar("title", "Bolt")]),
    )
    .await;
    (
        Form {
            base_url,
            client,
            blueprint,
            product,
        },
        server,
        repository,
    )
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn saving_checks_assists_against_the_enabled_release(pool: PgPool) {
    let (form, server, _) = form(pool).await;
    let validate = |definition: String| {
        let request = form
            .client
            .post(format!("{}/blueprints/validate", form.base_url))
            .json(&json!({"definition": definition}));
        async move { request.send().await.unwrap() }
    };
    for (definition, message) in [
        (line("fa_line_check", &assist("missing")), "is not declared"),
        (
            line("fa_other", &assist("customer_price")),
            "does not apply to blueprint 'fa_other'",
        ),
    ] {
        let error = expect_error(
            validate(definition).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_blueprint_definition",
        )
        .await;
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains(message),
            "{error}"
        );
    }
    // Saving a new revision of the line itself checks the same rules.
    let error = expect_error(
        form.client
            .post(format!("{}/blueprints", form.base_url))
            .json(&json!({"definition": line("fa_line", &assist("links"))}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_blueprint_definition",
    )
    .await;
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("proposes 'item'"),
        "{error}"
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn the_form_lists_and_runs_its_assists(pool: PgPool) {
    let (form, server, repository) = form(pool).await;
    let listed = get_json(
        &form.client,
        format!(
            "{}/v1/records/form-assists?blueprint_id={}&blueprint_version={}",
            form.base_url,
            form.blueprint["blueprint"]["id"].as_str().unwrap(),
            form.blueprint["blueprint"]["version"]
        ),
    )
    .await;
    let assists = listed["assists"].as_array().unwrap();
    assert_eq!(assists.len(), 3, "{listed}");
    assert_eq!(assists[0]["id"], "customer_price");
    assert_eq!(assists[0]["extension_name"], "Pricing");
    assert_eq!(assists[0]["outlet"], "record_form");
    assert_eq!(assists[0]["watch"], json!(["item", "quantity"]));
    assert_eq!(
        assists[0]["proposes"],
        json!(["unit_price_net", "discount"])
    );

    let working = json!({
        "values": {"quantity": 3},
        "relationships": {"item": [form.product["id"]]},
    });
    let answer = form.run("customer_price", working.clone()).await;
    assert_eq!(answer["outcome"], "answered");
    // The undeclared field and the value its attribute refuses are dropped.
    assert_eq!(
        answer["proposals"],
        json!([{"attribute_code": "unit_price_net", "value": 30.0, "note": "Customer price for a fa_product"}])
    );

    // A field the user edited keeps its value.
    let mut edited = working.clone();
    edited["edited"] = json!(["unit_price_net"]);
    assert_eq!(
        form.run("customer_price", edited).await["proposals"],
        json!([])
    );

    // The command sees only the assist's fields, with each target's blueprint.
    let mut seen = working.clone();
    seen["values"]["memo"] = json!("draft");
    seen["edited"] = json!(["unit_price_net", "customer"]);
    let echoed = form.run("echo", seen).await;
    let context: Value =
        serde_json::from_str(echoed["proposals"][0]["value"].as_str().unwrap()).unwrap();
    assert_eq!(context["version"], 1);
    assert_eq!(context["assist"], "echo");
    assert_eq!(context["blueprint"]["code"], "fa_line");
    assert_eq!(context["record_id"], Value::Null);
    assert_eq!(
        context["values"],
        json!({
            "item": [{"record_id": form.product["id"], "blueprint_code": "fa_product"}],
            "memo": "draft",
            "quantity": 3,
            "unit_price_net": null,
        })
    );
    assert_eq!(context["edited"], json!(["unit_price_net"]));

    // A field outside the assist is refused before the command runs.
    let mut outside = working.clone();
    outside["values"]["customer"] = json!("ACME");
    expect_error(
        form.client
            .post(format!("{}/v1/records/form-assist", form.base_url))
            .json(&json!({
                "extension_id": EXTENSION, "id": "customer_price",
                "blueprint_id": form.blueprint["blueprint"]["id"],
                "blueprint_version": form.blueprint["blueprint"]["version"],
                "values": outside["values"],
            }))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;

    // An answer breaking the contract proposes nothing.
    let broken = form.run("broken", json!({"values": {"quantity": 1}})).await;
    assert_eq!(broken["outcome"], "failed");
    assert_eq!(broken["proposals"], json!([]));

    // A disabled extension's assists are gone, and the form goes on without.
    repository.disable_extension(EXTENSION).await.unwrap();
    let listed = get_json(
        &form.client,
        format!(
            "{}/v1/records/form-assists?blueprint_id={}&blueprint_version={}",
            form.base_url,
            form.blueprint["blueprint"]["id"].as_str().unwrap(),
            form.blueprint["blueprint"]["version"]
        ),
    )
    .await;
    assert_eq!(listed["assists"], json!([]));
    expect_error(
        form.client
            .post(format!("{}/v1/records/form-assist", form.base_url))
            .json(&json!({
                "extension_id": EXTENSION, "id": "customer_price",
                "blueprint_id": form.blueprint["blueprint"]["id"],
                "blueprint_version": form.blueprint["blueprint"]["version"],
            }))
            .send()
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    server.abort();
}
