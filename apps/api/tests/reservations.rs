//! Status-triggered movements and reservations: production orders reserve
//! material on approval, and release and consume it on completion.

use crate::support;

use support::*;

const MATERIAL: &str = r#"
format_version: 1
code: po_material
name: Material
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
attributes:
  - code: name
    value_type: string
"#;

const STOCK_ITEM: &str = r#"
format_version: 1
code: po_stock_item
name: Stock item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [quantity]
unique_keys:
  - code: item
    attributes: [item]
attributes:
  - code: item
    value_type: relationship
    target_blueprint: po_material
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0, reserved: reserved}
  - code: reserved
    value_type: number
    balance: {min: 0}
"#;

/// A receipt (`in`) or issue (`out`) applied when created.
fn stock_document(code: &str, direction: &str) -> String {
    format!(
        r#"
format_version: 1
code: {code}
name: {code}
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [quantity]
attributes:
  - code: item
    value_type: relationship
    target_blueprint: po_material
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: po_stock_item
    cardinality: one
movements:
  - code: stock
    direction: {direction}
    quantity: quantity
    target:
      relationship: stock_item
      blueprint: po_stock_item
      balance: quantity
      key: item
      values: {{item: item}}
      create_missing: true
"#
    )
}

const ORDER_STATUS: &str = r#"value_schema: |-
      {"type":"string","enum":["draft","approved","completed","cancelled"],"x-attricat-status":{"version":1,
        "options":[{"code":"draft","label":"Draft"},{"code":"approved","label":"Approved"},
          {"code":"completed","label":"Completed"},{"code":"cancelled","label":"Cancelled"}],
        "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"approved"},
          {"from":"draft","to":"cancelled"},{"from":"approved","to":"completed"},
          {"from":"approved","to":"cancelled"},{"from":"cancelled","to":"draft"}]}}"#;

/// The three movements of material for an order, triggered by `status` of
/// the record itself, or of the record `via` links to.
fn material_movements(via: Option<&str>) -> String {
    let trigger = |to: &str| match via {
        Some(relationship) => {
            format!("{{relationship: {relationship}, attribute: status, to: {to}}}")
        }
        None => format!("{{attribute: status, to: {to}}}"),
    };
    let target = |balance: &str| {
        format!(
            "    target: {{relationship: stock_item, blueprint: po_stock_item, balance: {balance}, key: item, values: {{item: item}}}}\n"
        )
    };
    format!(
        "movements:
  - code: reserve
    direction: in
    quantity: quantity
{reserve_target}    apply_on: [{approved}]
  - code: release
    direction: out
    quantity: quantity
{release_target}    apply_on: [{completed}, {cancelled}]
    requires: [reserve]
  - code: consume
    direction: out
    quantity: quantity
    result: consumed
{consume_target}    apply_on: [{completed}]
    requires: [reserve]
",
        reserve_target = target("reserved"),
        release_target = target("reserved"),
        consume_target = target("quantity"),
        approved = trigger("approved"),
        completed = trigger("completed"),
        cancelled = trigger("cancelled"),
    )
}

/// A single-material order: its own status applies its movements.
fn order() -> String {
    format!(
        r#"
format_version: 1
code: po_order
name: Order
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [quantity]
attributes:
  - code: status
    value_type: string
    {ORDER_STATUS}
  - code: item
    value_type: relationship
    target_blueprint: po_material
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: po_stock_item
    cardinality: one
  - code: consumed
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: po_order
    cardinality: one
    context_editable: default
{}"#,
        material_movements(None).replace(
            "    result: consumed\n",
            "    result: consumed\n    reversal: {relationship: reverses}\n"
        )
    )
}

/// A production order whose lines carry the movements.
fn production() -> String {
    format!(
        r#"
format_version: 1
code: po_production
name: Production
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [number]
attributes:
  - code: number
    value_type: string
  - code: status
    value_type: string
    {ORDER_STATUS}
"#
    )
}

/// A material line, reserved and consumed by its production's status.
fn material_line() -> String {
    format!(
        r#"
format_version: 1
code: po_material_line
name: Material line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [quantity]
attributes:
  - code: production
    value_type: relationship
    target_blueprint: po_production
    cardinality: one
    context_editable: default
  - code: item
    value_type: relationship
    target_blueprint: po_material
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: po_stock_item
    cardinality: one
  - code: consumed
    value_type: number
{}"#,
        material_movements(Some("production"))
    )
}

/// An output line, such as the finished product or scrap, received when its
/// production completes.
const OUTPUT_LINE: &str = r#"
format_version: 1
code: po_output_line
name: Output line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [quantity]
attributes:
  - code: production
    value_type: relationship
    target_blueprint: po_production
    cardinality: one
    context_editable: default
  - code: item
    value_type: relationship
    target_blueprint: po_material
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: po_stock_item
    cardinality: one
movements:
  - code: produce
    direction: in
    quantity: quantity
    target:
      relationship: stock_item
      blueprint: po_stock_item
      balance: quantity
      key: item
      values: {item: item}
      create_missing: true
    apply_on: [{relationship: production, attribute: status, to: completed}]
"#;

struct Fixture {
    client: Client,
    base_url: String,
    pool: PgPool,
}

impl Fixture {
    async fn new(pool: PgPool) -> (Self, tokio::task::JoinHandle<()>) {
        let (base_url, server) = start_server(pool.clone()).await;
        let client = authenticated_client();
        for definition in [
            MATERIAL.to_owned(),
            STOCK_ITEM.to_owned(),
            stock_document("po_receipt", "in"),
            stock_document("po_issue", "out"),
            order(),
            production(),
            material_line(),
            OUTPUT_LINE.to_owned(),
        ] {
            expect_status(
                publish_response(&client, &base_url, &definition).await,
                StatusCode::OK,
            )
            .await;
        }
        (
            Self {
                client,
                base_url,
                pool,
            },
            server,
        )
    }

    async fn material(&self, name: &str) -> Value {
        create_record_with(
            &self.client,
            &self.base_url,
            "po_material",
            json!([scalar("name", name)]),
        )
        .await
    }

    async fn post(&self, blueprint: &str, values: Value) -> reqwest::Response {
        post_record(&self.client, &self.base_url, blueprint, values).await
    }

    async fn create(&self, blueprint: &str, values: Value) -> Value {
        create_record_with(&self.client, &self.base_url, blueprint, values).await
    }

    /// Receives `quantity` of `material` into stock.
    async fn receive(&self, material: &Value, quantity: i64) {
        self.create(
            "po_receipt",
            json!([relationship("item", material), scalar("quantity", quantity)]),
        )
        .await;
    }

    async fn issue(&self, material: &Value, quantity: i64) -> reqwest::Response {
        self.post(
            "po_issue",
            json!([relationship("item", material), scalar("quantity", quantity)]),
        )
        .await
    }

    async fn order(&self, material: &Value, quantity: i64) -> Value {
        self.create(
            "po_order",
            json!([
                scalar("status", "draft"),
                relationship("item", material),
                scalar("quantity", quantity)
            ]),
        )
        .await
    }

    fn record_url(&self, record: &Value) -> String {
        format!(
            "{}/v1/records/{}",
            self.base_url,
            record["id"].as_str().unwrap()
        )
    }

    async fn put(&self, record: &Value, body: Value) -> reqwest::Response {
        self.client
            .put(self.record_url(record))
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    async fn set_status(&self, record: &Value, status: &str) -> reqwest::Response {
        let current = get_json(&self.client, self.record_url(record)).await;
        self.put(
            record,
            json!({
                "expected_updated_at": current["record"]["updated_at"],
                "values": [scalar("status", status)],
            }),
        )
        .await
    }

    async fn delete(&self, record: &Value) -> reqwest::Response {
        self.client
            .delete(format!(
                "{}/records/{}",
                self.base_url,
                record["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
    }

    /// The stock item of `material`: its quantity and reserved balances.
    async fn stock(&self, material: &Value) -> (String, String) {
        let balance = |code: &'static str| {
            let pool = self.pool.clone();
            let material = material["id"].as_str().unwrap().to_owned();
            async move {
                sqlx::query_scalar::<_, Option<String>>(
                    r#"SELECT (
                         SELECT av.value_number::text FROM attribute_values av
                         JOIN attributes a ON a.id = av.attribute_id
                         WHERE av.record_id = s.record_id AND a.code = $2 AND av.active)
                       FROM attribute_values s
                       JOIN attributes sa ON sa.id = s.attribute_id AND sa.code = 'item'
                       JOIN records r ON r.id = s.record_id
                       JOIN blueprints b ON b.id = r.blueprint_id AND b.version = r.blueprint_version
                        AND b.code = 'po_stock_item'
                       WHERE s.relationship_target_record_id = $1::uuid AND s.active"#,
                )
                .bind(material)
                .bind(code)
                .fetch_one(&pool)
                .await
                .unwrap()
                .unwrap_or_else(|| "0".to_owned())
            }
        };
        (balance("quantity").await, balance("reserved").await)
    }

    async fn status_of(&self, record: &Value) -> Value {
        let body = get_json(&self.client, self.record_url(record)).await;
        body["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["attribute_code"] == "status")
            .map(|value| value["value"].clone())
            .unwrap_or(Value::Null)
    }

    /// The record form's settled line relationships.
    async fn settled_lines(&self, record: &Value) -> Value {
        get_json(&self.client, self.record_url(record)).await["settled_lines"].clone()
    }

    async fn applied(&self, record: &Value) -> Vec<String> {
        sqlx::query_scalar(
            r#"SELECT movement_code FROM record_movement_applications
               WHERE record_id = $1::uuid ORDER BY movement_code"#,
        )
        .bind(record["id"].as_str().unwrap())
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }
}

fn stock(quantity: &str, reserved: &str) -> (String, String) {
    (quantity.to_owned(), reserved.to_owned())
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn approval_reserves_stock_that_issues_cannot_take(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    fixture.receive(&steel, 10).await;
    let order = fixture.order(&steel, 6).await;
    // A draft holds nothing.
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    assert!(fixture.applied(&order).await.is_empty());

    expect_success(fixture.set_status(&order, "approved").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "6"));
    assert_eq!(fixture.applied(&order).await, ["reserve"]);

    // Only the unreserved 4 can be issued.
    let body = expect_error(
        fixture.issue(&steel, 5).await,
        StatusCode::CONFLICT,
        "balance_below_minimum",
    )
    .await;
    let details = &body["error"]["details"];
    assert_eq!(details["available"], "4", "{body}");
    assert_eq!(details["reserved"], "6", "{body}");
    assert_eq!(details["requested"], "5", "{body}");
    expect_success(fixture.issue(&steel, 4).await).await;
    assert_eq!(fixture.stock(&steel).await, stock("6", "6"));

    // Another approval finds nothing free: it fails and changes nothing.
    let second = fixture.order(&steel, 1).await;
    let body = expect_error(
        fixture.set_status(&second, "approved").await,
        StatusCode::CONFLICT,
        "balance_below_minimum",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "quantity", "{body}");
    assert_eq!(body["error"]["details"]["available"], "0", "{body}");
    assert_eq!(fixture.status_of(&second).await, "draft");
    assert!(fixture.applied(&second).await.is_empty());

    // Completion releases the reservation and consumes the material.
    expect_success(fixture.set_status(&order, "completed").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("0", "0"));
    assert_eq!(
        fixture.applied(&order).await,
        ["consume", "release", "reserve"]
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn cancelling_releases_only_a_reservation_and_movements_apply_once(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    fixture.receive(&steel, 10).await;

    // A draft cancelled before approval has nothing to release.
    let draft = fixture.order(&steel, 3).await;
    expect_success(fixture.set_status(&draft, "cancelled").await).await;
    assert!(fixture.applied(&draft).await.is_empty());
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));

    let order = fixture.order(&steel, 4).await;
    expect_success(fixture.set_status(&order, "approved").await).await;
    expect_success(fixture.set_status(&order, "cancelled").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    assert_eq!(fixture.applied(&order).await, ["release", "reserve"]);

    // Approving again does not reserve again: each movement applies once.
    expect_success(fixture.set_status(&order, "draft").await).await;
    expect_success(fixture.set_status(&order, "approved").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));

    // Another order reserves its own quantity.
    let approved = fixture
        .create(
            "po_order",
            json!([
                scalar("status", "draft"),
                relationship("item", &steel),
                scalar("quantity", 2)
            ]),
        )
        .await;
    expect_success(fixture.set_status(&approved, "approved").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "2"));
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn drafts_stay_editable_until_their_movements_apply(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    let copper = fixture.material("Copper").await;
    fixture.receive(&steel, 10).await;
    fixture.receive(&copper, 10).await;

    let order = fixture.order(&steel, 6).await;
    expect_success(fixture
        .put(
            &order,
            json!({
                "values": [scalar("quantity", 3)],
                "relationships": [{"attribute_code": "item", "target_record_ids": [copper["id"]]}],
            }),
        )
        .await)
    .await;
    // A draft with nothing applied may be deleted.
    let scrapped = fixture.order(&steel, 1).await;
    expect_success(fixture.delete(&scrapped).await).await;

    expect_success(fixture.set_status(&order, "approved").await).await;
    assert_eq!(fixture.stock(&copper).await, stock("10", "3"));
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    for request in [
        json!({"values": [scalar("quantity", 4)]}),
        json!({"relationships": [{"attribute_code": "item", "target_record_ids": [steel["id"]]}]}),
    ] {
        expect_error(
            fixture.put(&order, request).await,
            StatusCode::CONFLICT,
            "movement_immutable",
        )
        .await;
    }
    expect_error(
        fixture.delete(&order).await,
        StatusCode::CONFLICT,
        "movement_record_undeletable",
    )
    .await;
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_reversal_moves_back_every_applied_movement(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    fixture.receive(&steel, 10).await;
    let order = fixture.order(&steel, 6).await;

    // Nothing applied yet: there is nothing to reverse.
    expect_error(
        fixture
            .post(
                "po_order",
                json!([scalar("status", "draft"), relationship("reverses", &order)]),
            )
            .await,
        StatusCode::CONFLICT,
        "movement_not_applied",
    )
    .await;

    expect_success(fixture.set_status(&order, "approved").await).await;
    expect_success(fixture.set_status(&order, "completed").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("4", "0"));

    let reversal = fixture
        .create(
            "po_order",
            json!([scalar("status", "draft"), relationship("reverses", &order)]),
        )
        .await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    assert_eq!(
        fixture.applied(&reversal).await,
        ["consume", "release", "reserve"]
    );
    // The reversal's own status never applies its movements again.
    expect_success(fixture.set_status(&reversal, "approved").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn concurrent_approvals_cannot_reserve_the_same_stock(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    fixture.receive(&steel, 10).await;
    let first = fixture.order(&steel, 6).await;
    let second = fixture.order(&steel, 6).await;

    let (a, b) = tokio::join!(
        fixture.set_status(&first, "approved"),
        fixture.set_status(&second, "approved")
    );
    let mut statuses = [a.status(), b.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    assert_eq!(fixture.stock(&steel).await, stock("10", "6"));
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_production_order_applies_its_lines(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let steel = fixture.material("Steel").await;
    let copper = fixture.material("Copper").await;
    let frame = fixture.material("Frame").await;
    let scrap = fixture.material("Scrap").await;
    fixture.receive(&steel, 10).await;
    fixture.receive(&copper, 2).await;

    let production = fixture
        .create(
            "po_production",
            json!([scalar("number", "ZP/1"), scalar("status", "draft")]),
        )
        .await;
    let line = |material: &Value, quantity: i64| {
        json!([
            relationship("production", &production),
            relationship("item", material),
            scalar("quantity", quantity)
        ])
    };
    let steel_line = fixture.create("po_material_line", line(&steel, 8)).await;
    let copper_line = fixture.create("po_material_line", line(&copper, 3)).await;
    fixture.create("po_output_line", line(&frame, 1)).await;
    fixture.create("po_output_line", line(&scrap, 2)).await;

    // One short line fails the whole approval.
    let body = expect_error(
        fixture.set_status(&production, "approved").await,
        StatusCode::CONFLICT,
        "balance_below_minimum",
    )
    .await;
    assert_eq!(body["error"]["details"]["requested"], "3", "{body}");
    assert_eq!(fixture.stock(&steel).await, stock("10", "0"));
    assert!(fixture.applied(&steel_line).await.is_empty());
    assert_eq!(fixture.status_of(&production).await, "draft");
    assert_eq!(fixture.settled_lines(&production).await, json!([]));

    // Lines of a draft stay editable.
    expect_success(
        fixture
            .put(&copper_line, json!({"values": [scalar("quantity", 2)]}))
            .await,
    )
    .await;
    expect_success(fixture.set_status(&production, "approved").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("10", "8"));
    assert_eq!(fixture.stock(&copper).await, stock("2", "2"));
    assert_eq!(fixture.applied(&steel_line).await, ["reserve"]);
    assert_eq!(
        fixture.settled_lines(&production).await,
        json!([{"blueprint": "po_material_line", "relationship": "production"}])
    );

    // An approved order's lines are settled.
    expect_error(
        fixture.post("po_material_line", line(&steel, 1)).await,
        StatusCode::CONFLICT,
        "movement_lines_locked",
    )
    .await;
    expect_error(
        fixture
            .put(&steel_line, json!({"values": [scalar("quantity", 1)]}))
            .await,
        StatusCode::CONFLICT,
        "movement_immutable",
    )
    .await;
    expect_error(
        fixture.delete(&steel_line).await,
        StatusCode::CONFLICT,
        "movement_record_undeletable",
    )
    .await;

    // Outputs settle at completion: an approved order still takes them.
    let offcut = fixture.create("po_output_line", line(&scrap, 1)).await;
    expect_success(
        fixture
            .put(&offcut, json!({"values": [scalar("quantity", 2)]}))
            .await,
    )
    .await;

    // Completion consumes the material and receives the outputs.
    expect_success(fixture.set_status(&production, "completed").await).await;
    assert_eq!(fixture.stock(&steel).await, stock("2", "0"));
    assert_eq!(fixture.stock(&copper).await, stock("0", "0"));
    assert_eq!(fixture.stock(&frame).await, stock("1", "0"));
    assert_eq!(fixture.stock(&scrap).await, stock("4", "0"));
    expect_error(
        fixture.post("po_output_line", line(&frame, 1)).await,
        StatusCode::CONFLICT,
        "movement_lines_locked",
    )
    .await;
    assert_eq!(
        fixture.settled_lines(&production).await,
        json!([
            {"blueprint": "po_material_line", "relationship": "production"},
            {"blueprint": "po_output_line", "relationship": "production"}
        ])
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn trigger_relationships_name_status_options_of_their_target(pool: PgPool) {
    let (fixture, server) = Fixture::new(pool).await;
    let invalid = OUTPUT_LINE
        .replace("po_output_line", "po_output_line_invalid")
        .replace("to: completed", "to: shipped");
    let body = expect_error(
        publish_response(&fixture.client, &fixture.base_url, &invalid).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_blueprint_definition",
    )
    .await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("'shipped' is not an option"),
        "{body}"
    );
    server.abort();
}
