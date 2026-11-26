//! `default_from`: an order line's price copied from its item while the line
//! is written, through every record write path.
use crate::support;

use support::*;

const TAX: &str = r#"format_version: 1
code: df_tax
name: Tax class
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [code]
attributes:
  - code: code
    value_type: string
  - code: rate
    value_type: integer"#;

const PRODUCT: &str = r#"format_version: 1
code: df_product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [sku]
attributes:
  - code: sku
    value_type: string
  - code: sale_price_net
    value_type: number
  - code: tax_class
    value_type: relationship
    target_blueprint: df_tax
    cardinality: one"#;

/// A second target blueprint without a price.
const SERVICE: &str = r#"format_version: 1
code: df_service
name: Service
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string"#;

const LINE: &str = r#"format_version: 1
code: df_line
name: Order line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [note]
attributes:
  - code: note
    value_type: string
  - code: item
    value_type: relationship
    target_blueprints: [df_product, df_service]
    cardinality: one
  - code: unit_price_net
    value_type: number
    value_schema: '{"minimum": 0}'
    default_from: item.sale_price_net
  - code: vat_rate
    value_type: number
    default_from: item.tax_class.rate"#;

struct Catalog {
    base_url: String,
    server: tokio::task::JoinHandle<()>,
    client: Client,
    line: Value,
    product: Value,
    default_context: Value,
}

async fn catalog(pool: PgPool) -> Catalog {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, TAX).await;
    let product = create_blueprint(&client, &base_url, PRODUCT).await;
    create_blueprint(&client, &base_url, SERVICE).await;
    let line = create_blueprint(&client, &base_url, LINE).await;
    let default_context = get_json(&client, format!("{base_url}/contexts"))
        .await
        .as_array()
        .unwrap()
        .iter()
        .find(|context| context["code"] == "default")
        .unwrap()
        .clone();
    Catalog {
        base_url,
        server,
        client,
        line,
        product,
        default_context,
    }
}

impl Catalog {
    async fn tax(&self, rate: i64) -> Value {
        create_record_with(
            &self.client,
            &self.base_url,
            "df_tax",
            json!([scalar("code", "standard"), scalar("rate", rate)]),
        )
        .await
    }

    async fn product(&self, price: f64, tax: Option<&Value>) -> Value {
        let mut values = vec![scalar("sku", "P-1"), scalar("sale_price_net", price)];
        values.extend(tax.map(|tax| relationship("tax_class", tax)));
        create_record_with(&self.client, &self.base_url, "df_product", json!(values)).await
    }

    async fn line(&self, values: Value) -> Value {
        create_record_with(&self.client, &self.base_url, "df_line", values).await
    }

    async fn form(&self, record: &Value) -> Value {
        get_json(
            &self.client,
            format!(
                "{}/v1/records/{}",
                self.base_url,
                record["id"].as_str().unwrap()
            ),
        )
        .await
    }

    /// The record's direct value of `code` in the default context.
    async fn value(&self, record: &Value, code: &str) -> Value {
        value_in(&self.form(record).await, code, &self.default_context)
    }

    async fn update(&self, record: &Value, body: Value) -> Value {
        expect_success(
            self.client
                .put(format!(
                    "{}/v1/records/{}",
                    self.base_url,
                    record["id"].as_str().unwrap()
                ))
                .json(&body)
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    async fn changes(&self, record: &Value) -> Vec<Value> {
        get_json(
            &self.client,
            format!(
                "{}/records/{}/changes",
                self.base_url,
                record["id"].as_str().unwrap()
            ),
        )
        .await
        .as_array()
        .unwrap()
        .clone()
    }
}

/// The direct value of `code` in `context`.
fn value_in(form: &Value, code: &str, context: &Value) -> Value {
    form["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["attribute_code"] == code && value["context_id"] == context["id"])
        .map(|value| {
            if value["kind"] == "relationship" {
                value["target_record_id"].clone()
            } else {
                value["value"].clone()
            }
        })
        .unwrap_or(Value::Null)
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn creating_a_line_copies_its_items_values_as_a_snapshot(pool: PgPool) {
    let catalog = catalog(pool.clone()).await;
    let tax = catalog.tax(23).await;
    let product = catalog.product(12.5, Some(&tax)).await;

    let line = catalog.line(json!([relationship("item", &product)])).await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 12.5);
    // An integer leaf fills a number attribute.
    assert_eq!(catalog.value(&line, "vat_rate").await, 23.0);

    let changes = catalog.changes(&line).await;
    for code in ["unit_price_net", "vat_rate"] {
        let change = changes
            .iter()
            .find(|change| change["attribute_code"] == code)
            .unwrap();
        assert_eq!(change["origin"], "default_from", "{change}");
    }
    let item = changes
        .iter()
        .find(|change| change["attribute_code"] == "item")
        .unwrap();
    assert!(item.get("origin").is_none(), "{item}");
    let event = latest_domain_event(&pool, record_uuid(&line), Some("record.created.v1")).await;
    let facts = event.payload["facts"].as_array().unwrap();
    assert!(
        facts
            .iter()
            .any(|fact| fact["attribute_code"] == "unit_price_net"
                && fact["origin"] == "default_from")
    );

    // Later source changes never reach existing lines.
    catalog
        .update(&product, json!({"values": [scalar("sale_price_net", 99)]}))
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 12.5);

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_supplied_value_always_wins(pool: PgPool) {
    let catalog = catalog(pool).await;
    let product = catalog.product(12.5, None).await;

    let line = catalog
        .line(json!([
            relationship("item", &product),
            scalar("unit_price_net", 10),
        ]))
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 10.0);
    // A product without a tax class yields nothing, without an error.
    assert_eq!(catalog.value(&line, "vat_rate").await, Value::Null);
    assert!(
        catalog
            .changes(&line)
            .await
            .iter()
            .all(|change| change.get("origin").is_none())
    );

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn updates_default_only_empty_fields_when_the_item_changes(pool: PgPool) {
    let catalog = catalog(pool).await;
    let first = catalog.product(10.0, None).await;
    let second = catalog.product(20.0, None).await;
    let third = catalog.product(30.0, None).await;

    let line = catalog.line(json!([scalar("note", "no item yet")])).await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, Value::Null);

    // Gaining an item fills the empty price.
    catalog
        .update(&line, json!({"values": [relationship("item", &first)]}))
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 10.0);

    // Replacing the item keeps a price that is already set.
    catalog
        .update(
            &line,
            json!({"relationships": [{"attribute_code": "item", "target_record_ids": [second["id"]]}]}),
        )
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 10.0);

    // A write that empties the price itself keeps it empty.
    catalog
        .update(
            &line,
            json!({
                "relationships": [{"attribute_code": "item", "target_record_ids": [third["id"]]}],
                "remove_values": [{"attribute_code": "unit_price_net"}],
            }),
        )
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, Value::Null);

    // A supplied price wins over the new item's.
    catalog
        .update(
            &line,
            json!({
                "relationships": [{"attribute_code": "item", "target_record_ids": [first["id"]]}],
                "values": [scalar("unit_price_net", 15)],
            }),
        )
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 15.0);

    // An emptied price is filled by the next item change, and an unrelated
    // write never fills it.
    catalog
        .update(
            &line,
            json!({"remove_values": [{"attribute_code": "unit_price_net"}]}),
        )
        .await;
    catalog
        .update(&line, json!({"values": [scalar("note", "edited")]}))
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, Value::Null);
    catalog
        .update(
            &line,
            json!({"relationships": [{"attribute_code": "item", "target_record_ids": [second["id"]]}]}),
        )
        .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 20.0);

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn relationship_replace_and_append_endpoints_default_too(pool: PgPool) {
    let catalog = catalog(pool.clone()).await;
    let product = catalog.product(7.0, None).await;

    let replaced = catalog.line(json!([])).await;
    expect_success(
        catalog
            .client
            .post(format!(
                "{}/records/{}/relationships/replace",
                catalog.base_url,
                replaced["id"].as_str().unwrap()
            ))
            .json(&json!({"relationships": [{"attribute_code": "item", "target_record_ids": [product["id"]]}]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(catalog.value(&replaced, "unit_price_net").await, 7.0);
    // The defaulted scalar makes it a record update, not only a link change.
    let event = latest_domain_event(&pool, record_uuid(&replaced), None).await;
    assert_eq!(event.event_type, "record.updated.v1");
    // The record's preview reflects the defaulted value's write.
    let preview = get_json(
        &catalog.client,
        format!(
            "{}/records/{}/preview",
            catalog.base_url,
            replaced["id"].as_str().unwrap()
        ),
    )
    .await;
    assert!(preview.to_string().contains("unit_price_net"), "{preview}");

    let appended = catalog.line(json!([])).await;
    expect_success(
        catalog
            .client
            .post(format!(
                "{}/records/{}/values",
                catalog.base_url,
                appended["id"].as_str().unwrap()
            ))
            .json(&json!({"values": [relationship("item", &product)]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(catalog.value(&appended, "unit_price_net").await, 7.0);

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn imports_default_like_single_writes(pool: PgPool) {
    let catalog = catalog(pool).await;
    let product = catalog.product(4.5, None).await;
    let line_id = Uuid::new_v4().to_string();
    expect_success(
        catalog
            .client
            .post(format!("{}/v1/records/batch", catalog.base_url))
            .json(&json!({"operations": [{
                "op": "create",
                "record_id": line_id,
                "blueprint": {"code": "df_line"},
                "values": [relationship("item", &product)],
            }]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        catalog
            .value(&json!({"id": line_id}), "unit_price_net")
            .await,
        4.5
    );

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_target_without_the_source_attribute_or_a_refused_value_yields_nothing(pool: PgPool) {
    let catalog = catalog(pool).await;
    let service = create_record_with(
        &catalog.client,
        &catalog.base_url,
        "df_service",
        json!([scalar("title", "Installation")]),
    )
    .await;
    let line = catalog.line(json!([relationship("item", &service)])).await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, Value::Null);

    // The line's schema refuses a negative price, so the source is ignored
    // rather than failing the write.
    let refund = catalog.product(-3.0, None).await;
    let line = catalog.line(json!([relationship("item", &refund)])).await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, Value::Null);

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_source_the_writer_cannot_read_yields_nothing(pool: PgPool) {
    let catalog = catalog(pool.clone()).await;
    let tax = catalog.tax(8).await;
    let product = catalog.product(12.5, Some(&tax)).await;
    let family = |blueprint: &Value| {
        blueprint["blueprint"]["id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap()
    };
    let (user, membership) = add_workspace_user(&pool).await;
    for blueprint in [&catalog.line, &catalog.product] {
        grant_role(
            &pool,
            membership,
            EDITOR_ROLE_ID,
            GrantScope::BlueprintFamily(family(blueprint)),
        )
        .await;
    }
    let editor = client_for(user);

    // Owner-created, so the editor's write is the one that links the item.
    let line = catalog.line(json!([])).await;
    expect_success(
        editor
            .post(format!(
                "{}/records/{}/relationships/replace",
                catalog.base_url,
                line["id"].as_str().unwrap()
            ))
            .json(&json!({"relationships": [{"attribute_code": "item", "target_record_ids": [product["id"]]}]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(catalog.value(&line, "unit_price_net").await, 12.5);
    // The tax class is outside the editor's grants.
    assert_eq!(catalog.value(&line, "vat_rate").await, Value::Null);

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn defaults_resolve_in_the_written_context_with_its_fallback(pool: PgPool) {
    let catalog = catalog(pool).await;
    let web = expect_success(
        catalog
            .client
            .post(format!("{}/contexts", catalog.base_url))
            .json(&json!({"code": "df_web", "data": {}}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    let inherited = catalog.product(10.0, None).await;
    let own = catalog.product(10.0, None).await;
    catalog
        .update(
            &own,
            json!({"values": [{"kind": "scalar", "attribute_code": "sale_price_net", "context_id": web["id"], "value": 8}]}),
        )
        .await;
    let link_in_web = |target: &Value| json!([{"kind": "relationship", "attribute_code": "item", "context_id": web["id"], "target_record_id": target["id"]}]);

    let line = catalog.line(link_in_web(&inherited)).await;
    let form = catalog.form(&line).await;
    // The web context inherits the product's default price.
    assert_eq!(value_in(&form, "unit_price_net", &web), 10.0);
    assert_eq!(
        value_in(&form, "unit_price_net", &catalog.default_context),
        Value::Null
    );

    let line = catalog.line(link_in_web(&own)).await;
    assert_eq!(
        value_in(&catalog.form(&line).await, "unit_price_net", &web),
        8.0
    );

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn validation_resolves_paths_against_published_targets(pool: PgPool) {
    let catalog = catalog(pool).await;
    let validate = |price: &'static str| {
        let request = catalog
            .client
            .post(format!("{}/blueprints/validate", catalog.base_url))
            .json(&json!({"definition": LINE.replace("code: df_line", "code: df_checked_line").replace(
                "    value_type: number\n    value_schema: '{\"minimum\": 0}'\n    default_from: item.sale_price_net",
                price,
            )}));
        async move { request.send().await.unwrap() }
    };
    for (price, message) in [
        (
            "    value_type: number\n    default_from: item.list_price",
            "list_price",
        ),
        (
            "    value_type: string\n    default_from: item.sale_price_net",
            "cannot fill a string attribute",
        ),
        (
            "    value_type: number\n    default_from: item.tax_class",
            "cannot fill a number attribute",
        ),
    ] {
        let error = expect_error(
            validate(price).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_blueprint_definition",
        )
        .await;
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains(message),
            "{price}: {error}"
        );
    }
    expect_success(validate("    value_type: number\n    default_from: item.sale_price_net").await)
        .await;

    catalog.server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn the_form_previews_the_values_a_save_would_default(pool: PgPool) {
    let catalog = catalog(pool).await;
    let tax = catalog.tax(23).await;
    let product = catalog.product(12.5, Some(&tax)).await;
    let preview = |relationship: &str, target: &Value| {
        let request = catalog
            .client
            .post(format!("{}/v1/records/default-values", catalog.base_url))
            .json(&json!({
                "blueprint_id": catalog.line["blueprint"]["id"],
                "blueprint_version": catalog.line["blueprint"]["version"],
                "relationship": relationship,
                "target_record_id": target["id"],
            }));
        async move { expect_success(request.send().await.unwrap()).await }
    };

    let mut values = preview("item", &product).await["values"]
        .as_array()
        .unwrap()
        .clone();
    values.sort_by_key(|value| value["attribute_code"].as_str().unwrap().to_owned());
    assert_eq!(
        values,
        vec![
            json!({"attribute_code": "unit_price_net", "value": 12.5}),
            json!({"attribute_code": "vat_rate", "value": 23}),
        ]
    );
    // Another relationship defaults nothing.
    assert_eq!(preview("note", &product).await["values"], json!([]));

    catalog.server.abort();
}
