use crate::support;

use support::*;

const PRODUCT: &str = r#"
format_version: 1
code: sm_product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
attributes:
  - code: name
    value_type: string
"#;

const UNIT_CONVERSION: &str = r#"
format_version: 1
code: sm_unit_conversion
name: Unit conversion
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [unit]
unique_keys:
  - code: product_unit
    attributes: [product, unit]
attributes:
  - code: product
    value_type: relationship
    target_blueprint: sm_product
    cardinality: one
  - code: unit
    value_type: string
  - code: factor
    value_type: number
"#;

const STOCK_ITEM: &str = r#"
format_version: 1
code: sm_stock_item
name: Stock item
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [label]
unique_keys:
  - code: product
    attributes: [product]
attributes:
  - code: label
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: sm_product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0}
"#;

const GOODS_ISSUE: &str = r#"
format_version: 1
code: sm_goods_issue
name: Goods issue
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [number]
attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: sm_product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: sm_unit_conversion
    cardinality: one
    context_editable: default
    target_scope: {attribute: product, equals: product}
  - code: stock_item
    value_type: relationship
    target_blueprint: sm_stock_item
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: sm_goods_issue
    cardinality: one
    context_editable: default
movements:
  - code: stock
    direction: out
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: sm_stock_item
      balance: quantity
      key: product
      values: {product: product}
    reversal: {relationship: reverses}
"#;

const GOODS_RECEIPT: &str = r#"
format_version: 1
code: sm_goods_receipt
name: Goods receipt
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [number]
attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: sm_product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: sm_unit_conversion
    cardinality: one
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: sm_stock_item
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: sm_goods_receipt
    cardinality: one
    context_editable: default
movements:
  - code: stock
    direction: in
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: sm_stock_item
      balance: quantity
      key: product
      values: {product: product}
      create_missing: true
    reversal: {relationship: reverses}
"#;

/// `GOODS_ISSUE` under another code, so each attempt is a new family.
fn goods_issue_as(code: &str) -> String {
    GOODS_ISSUE.replace("sm_goods_issue", code)
}

/// Saves `definition` as a draft and asks to publish it.
async fn publish_response(client: &Client, base_url: &str, definition: &str) -> reqwest::Response {
    let draft = expect_status(
        client
            .post(format!("{base_url}/blueprints"))
            .json(&json!({ "definition": definition }))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    client
        .post(format!(
            "{base_url}/blueprints/{}/versions/{}/publish",
            draft["blueprint"]["id"].as_str().unwrap(),
            draft["blueprint"]["version"].as_i64().unwrap(),
        ))
        .send()
        .await
        .unwrap()
}

fn attribute<'a>(blueprint: &'a Value, code: &str) -> &'a Value {
    blueprint["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attribute| attribute["code"] == code)
        .unwrap()
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn publishes_balances_movements_and_target_scopes(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    for definition in [PRODUCT, UNIT_CONVERSION, STOCK_ITEM] {
        create_blueprint(&client, &base_url, definition).await;
    }
    let issue = create_blueprint(&client, &base_url, GOODS_ISSUE).await;
    assert_eq!(
        issue["blueprint"]["movements"][0]["direction"], "out",
        "{issue}"
    );
    assert_eq!(
        issue["blueprint"]["movements"][0]["target"]["values"],
        json!({"product": "product"})
    );
    assert_eq!(
        attribute(&issue, "conversion")["target_scope"],
        json!({"attribute": "product", "equals": "product"})
    );
    for code in ["stock_item", "base_quantity"] {
        assert_eq!(attribute(&issue, code)["readonly"], true, "{code}");
        assert_eq!(
            attribute(&issue, code)["context_editable"],
            "default",
            "{code}"
        );
    }

    let stock_items: Value = get_json(&client, format!("{base_url}/blueprints")).await;
    let stock_item_id = stock_items
        .as_array()
        .unwrap()
        .iter()
        .find(|blueprint| blueprint["code"] == "sm_stock_item")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let stock_item = get_json(&client, format!("{base_url}/blueprints/{stock_item_id}")).await;
    let quantity = attribute(&stock_item, "quantity");
    assert_eq!(quantity["balance"], json!({"min": "0"}));
    assert_eq!(quantity["readonly"], true);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn publication_checks_the_blueprints_a_movement_names(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PRODUCT).await;
    create_blueprint(&client, &base_url, UNIT_CONVERSION).await;

    // The target has no published revision yet.
    let body = expect_error(
        publish_response(&client, &base_url, &goods_issue_as("sm_issue_early")).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_blueprint_definition",
    )
    .await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("has no published revision"),
        "{body}"
    );

    create_blueprint(&client, &base_url, STOCK_ITEM).await;
    for (index, (from, to, expected)) in [
        (
            "balance: quantity",
            "balance: product",
            "is not a balance attribute",
        ),
        (
            "key: product",
            "key: missing",
            "has no unique key 'missing'",
        ),
        (
            "attribute: factor}",
            "attribute: unit}",
            "must be a number or integer attribute",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let definition = goods_issue_as(&format!("sm_issue_{index}")).replace(from, to);
        let body = expect_error(
            publish_response(&client, &base_url, &definition).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_blueprint_definition",
        )
        .await;
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{expected}: {body}"
        );
    }

    let mismatched_scope = goods_issue_as("sm_issue_scope").replace(
        "target_scope: {attribute: product, equals: product}",
        "target_scope: {attribute: unit, equals: product}",
    );
    let body = expect_error(
        publish_response(&client, &base_url, &mismatched_scope).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_blueprint_definition",
    )
    .await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("target_scope"),
        "{body}"
    );

    create_blueprint(&client, &base_url, GOODS_ISSUE).await;
    server.abort();
}

/// Publishes the inventory blueprints and returns a product.
async fn inventory(client: &Client, base_url: &str) -> Value {
    for definition in [
        PRODUCT,
        UNIT_CONVERSION,
        STOCK_ITEM,
        GOODS_RECEIPT,
        GOODS_ISSUE,
    ] {
        create_blueprint(client, base_url, definition).await;
    }
    product(client, base_url, "Bolt").await
}

async fn product(client: &Client, base_url: &str, name: &str) -> Value {
    create_record_with(
        client,
        base_url,
        "sm_product",
        json!([scalar("name", name)]),
    )
    .await
}

async fn conversion(
    client: &Client,
    base_url: &str,
    product: &Value,
    unit: &str,
    factor: Value,
) -> Value {
    create_record_with(
        client,
        base_url,
        "sm_unit_conversion",
        json!([
            relationship("product", product),
            scalar("unit", unit),
            scalar("factor", factor)
        ]),
    )
    .await
}

/// Posts a receipt or issue of `quantity`, optionally in a converted unit.
async fn movement(
    client: &Client,
    base_url: &str,
    blueprint: &str,
    product: &Value,
    quantity: Value,
    conversion: Option<&Value>,
) -> reqwest::Response {
    let mut values = vec![
        relationship("product", product),
        scalar("quantity", quantity),
    ];
    if let Some(conversion) = conversion {
        values.push(relationship("conversion", conversion));
    }
    post_record(client, base_url, blueprint, Value::Array(values)).await
}

/// The default-context number of a record attribute, exactly as stored.
async fn stored_number(pool: &PgPool, record_id: Uuid, code: &str) -> Option<String> {
    sqlx::query_scalar::<_, Option<String>>(
        r#"SELECT av.value_number::text FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           JOIN attribute_contexts c ON c.id = av.context_id AND c.code = 'default'
           WHERE av.record_id = $1 AND a.code = $2 AND av.active
             AND av.relationship_target_record_id IS NULL"#,
    )
    .bind(record_id)
    .bind(code)
    .fetch_optional(pool)
    .await
    .unwrap()
    .flatten()
}

/// The record a movement record's `stock_item` links to.
async fn stock_item_of(pool: &PgPool, record: &Value) -> Uuid {
    sqlx::query_scalar(
        r#"SELECT av.relationship_target_record_id FROM attribute_values av
           JOIN attributes a ON a.id = av.attribute_id
           WHERE av.record_id = $1 AND a.code = 'stock_item' AND av.active"#,
    )
    .bind(Uuid::parse_str(record["id"].as_str().unwrap()).unwrap())
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The balance of the stock item a movement record changed.
async fn balance_after(pool: &PgPool, record: &Value) -> String {
    let stock_item = stock_item_of(pool, record).await;
    stored_number(pool, stock_item, "quantity").await.unwrap()
}

async fn record_count(pool: &PgPool, blueprint: &str) -> i64 {
    sqlx::query_scalar(
        r#"SELECT count(*) FROM records r JOIN blueprints b
             ON b.id = r.blueprint_id AND b.version = r.blueprint_version
           WHERE b.code = $1 AND r.deleted_at IS NULL"#,
    )
    .bind(blueprint)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn success(response: reqwest::Response) -> Value {
    let (status, body) = status_json(response).await;
    assert!(status.is_success(), "{status}: {body}");
    body
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn receipts_create_and_add_to_the_stock_item_and_issues_subtract(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let box_of_12 = conversion(&client, &base_url, &bolt, "box", json!(12)).await;

    let first = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(2),
            Some(&box_of_12),
        )
        .await,
    )
    .await;
    assert_eq!(record_count(&pool, "sm_stock_item").await, 1);
    assert_eq!(balance_after(&pool, &first).await, "24");
    let first_id = Uuid::parse_str(first["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        stored_number(&pool, first_id, "base_quantity")
            .await
            .as_deref(),
        Some("24")
    );

    let second = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(1.5),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        record_count(&pool, "sm_stock_item").await,
        1,
        "the receipt adds to the existing item"
    );
    assert_eq!(
        stock_item_of(&pool, &first).await,
        stock_item_of(&pool, &second).await
    );
    assert_eq!(balance_after(&pool, &second).await, "25.5");

    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(5), None).await).await;
    assert_eq!(balance_after(&pool, &issue).await, "20.5");

    // The stock item's change is an ordinary, audited update.
    let stock_item = stock_item_of(&pool, &issue).await;
    let event = latest_domain_event(&pool, stock_item, Some("record.updated.v1")).await;
    assert_eq!(event.aggregate_id, stock_item);
    let action: String = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE target->>'id' = $1 ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(stock_item.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "attricat.records.movement.balance_update");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn issues_cannot_take_stock_below_its_minimum(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(10),
            None,
        )
        .await,
    )
    .await;

    let body = expect_error(
        movement(
            &client,
            &base_url,
            "sm_goods_issue",
            &bolt,
            json!(10.5),
            None,
        )
        .await,
        StatusCode::CONFLICT,
        "balance_below_minimum",
    )
    .await;
    assert_eq!(body["error"]["details"]["available"], "10");
    assert_eq!(body["error"]["details"]["requested"], "10.5");
    assert_eq!(body["error"]["details"]["minimum"], "0");
    assert_eq!(
        record_count(&pool, "sm_goods_issue").await,
        0,
        "the issue rolled back"
    );
    assert_eq!(balance_after(&pool, &receipt).await, "10");

    // Issuing everything leaves exactly the minimum.
    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(10), None).await).await;
    assert_eq!(balance_after(&pool, &issue).await, "0");

    // An issue needs an existing stock item; it never creates one.
    let nut = product(&client, &base_url, "Nut").await;
    let body = expect_error(
        movement(&client, &base_url, "sm_goods_issue", &nut, json!(1), None).await,
        StatusCode::CONFLICT,
        "movement_target_missing",
    )
    .await;
    assert_eq!(body["error"]["details"]["blueprint"], "sm_stock_item");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn concurrent_issues_cannot_overdraw_stock(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(10),
            None,
        )
        .await,
    )
    .await;

    let issues = futures_util::future::join_all(
        (0..4).map(|_| movement(&client, &base_url, "sm_goods_issue", &bolt, json!(6), None)),
    )
    .await;
    let statuses: Vec<StatusCode> = issues.iter().map(reqwest::Response::status).collect();
    assert_eq!(
        statuses.iter().filter(|status| status.is_success()).count(),
        1,
        "{statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|status| status.is_success() || *status == StatusCode::CONFLICT)
    );
    assert_eq!(balance_after(&pool, &receipt).await, "4");

    // Concurrent first receipts of a product create one stock item.
    let nut = product(&client, &base_url, "Nut").await;
    let receipts = futures_util::future::join_all(
        (0..4).map(|_| movement(&client, &base_url, "sm_goods_receipt", &nut, json!(1), None)),
    )
    .await;
    assert!(
        receipts
            .iter()
            .all(|response| response.status().is_success())
    );
    assert_eq!(record_count(&pool, "sm_stock_item").await, 2);

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn movement_amounts_are_exact_and_validated(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let tenth = conversion(&client, &base_url, &bolt, "tenth", json!(0.1)).await;
    let mut last = Value::Null;
    for _ in 0..3 {
        last = success(
            movement(
                &client,
                &base_url,
                "sm_goods_receipt",
                &bolt,
                json!(1),
                Some(&tenth),
            )
            .await,
        )
        .await;
    }
    assert_eq!(balance_after(&pool, &last).await, "0.3");

    for quantity in [json!(0), json!(-1)] {
        let body = expect_error(
            movement(
                &client,
                &base_url,
                "sm_goods_receipt",
                &bolt,
                quantity,
                None,
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "movement_quantity_invalid",
        )
        .await;
        assert_eq!(body["error"]["details"]["attribute"], "quantity");
    }
    let empty = conversion(&client, &base_url, &bolt, "empty", json!(0)).await;
    let body = expect_error(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(1),
            Some(&empty),
        )
        .await,
        StatusCode::CONFLICT,
        "movement_factor_invalid",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "factor");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn clients_cannot_write_balances_or_movement_values(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(3),
            None,
        )
        .await,
    )
    .await;
    let stock_item = stock_item_of(&pool, &receipt).await;

    for request in [
        client
            .put(format!("{base_url}/v1/records/{stock_item}"))
            .json(&json!({"values": [scalar("quantity", 100)]})),
        client
            .put(format!("{base_url}/v1/records/{stock_item}"))
            .json(&json!({"remove_values": [{"attribute_code": "quantity"}]})),
    ] {
        let body = expect_error(
            request.send().await.unwrap(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "balance_attribute_readonly",
        )
        .await;
        assert_eq!(body["error"]["details"]["attribute"], "quantity");
    }
    let other = json!({"id": stock_item});
    let body = expect_error(
        post_record(
            &client,
            &base_url,
            "sm_goods_receipt",
            json!([
                relationship("product", &bolt),
                scalar("quantity", 1),
                relationship("stock_item", &other)
            ]),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "movement_attribute_readonly",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "stock_item");
    let receipt_id = receipt["id"].as_str().unwrap();
    expect_error(
        client
            .put(format!("{base_url}/v1/records/{receipt_id}"))
            .json(&json!({"values": [scalar("base_quantity", 1)]}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "movement_attribute_readonly",
    )
    .await;
    assert_eq!(balance_after(&pool, &receipt).await, "3");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn duplicates_apply_their_own_movement(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(4),
            None,
        )
        .await,
    )
    .await;
    let receipt_id = receipt["id"].as_str().unwrap();
    let copy = success(
        client
            .post(format!("{base_url}/v1/records/{receipt_id}/duplicate"))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        stock_item_of(&pool, &copy).await,
        stock_item_of(&pool, &receipt).await
    );
    assert_eq!(balance_after(&pool, &copy).await, "8");

    // A stock item's copy has no key value and no balance of its own.
    let stock_item = stock_item_of(&pool, &receipt).await;
    let copy = success(
        client
            .post(format!("{base_url}/v1/records/{stock_item}/duplicate"))
            .send()
            .await
            .unwrap(),
    )
    .await;
    let copy_id = Uuid::parse_str(copy["id"].as_str().unwrap()).unwrap();
    assert_eq!(stored_number(&pool, copy_id, "quantity").await, None);

    server.abort();
}

/// Posts a reversal of `original`, with `extra` values.
async fn reversal(
    client: &Client,
    base_url: &str,
    blueprint: &str,
    original: &Value,
    extra: Vec<Value>,
) -> reqwest::Response {
    let mut values = vec![relationship("reverses", original)];
    values.extend(extra);
    post_record(client, base_url, blueprint, Value::Array(values)).await
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn reversals_move_back_what_the_original_moved(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let box_of_12 = conversion(&client, &base_url, &bolt, "box", json!(12)).await;
    success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(2),
            Some(&box_of_12),
        )
        .await,
    )
    .await;
    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(5), None).await).await;
    assert_eq!(balance_after(&pool, &issue).await, "19");

    // A mismatching input is refused; the reversal's inputs are the original's.
    let body = expect_error(
        reversal(
            &client,
            &base_url,
            "sm_goods_issue",
            &issue,
            vec![scalar("quantity", 4)],
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "movement_reversal_mismatch",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "quantity");
    let undo = success(
        reversal(
            &client,
            &base_url,
            "sm_goods_issue",
            &issue,
            vec![scalar("quantity", 5.0)],
        )
        .await,
    )
    .await;
    assert_eq!(balance_after(&pool, &undo).await, "24");
    let undo_id = Uuid::parse_str(undo["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        stored_number(&pool, undo_id, "base_quantity")
            .await
            .as_deref(),
        Some("5")
    );
    // The submitted `5.0` matched the original's 5 and is kept as written.
    assert_eq!(
        stored_number(&pool, undo_id, "quantity").await.as_deref(),
        Some("5.0")
    );

    let body = expect_error(
        reversal(&client, &base_url, "sm_goods_issue", &issue, Vec::new()).await,
        StatusCode::CONFLICT,
        "movement_already_reversed",
    )
    .await;
    assert_eq!(body["error"]["details"]["reversed_by"], undo["id"]);
    expect_error(
        reversal(&client, &base_url, "sm_goods_issue", &undo, Vec::new()).await,
        StatusCode::CONFLICT,
        "movement_reversal_of_reversal",
    )
    .await;

    // Concurrent reversals of one record: only one applies.
    let second =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(4), None).await).await;
    let reversals = futures_util::future::join_all(
        (0..3).map(|_| reversal(&client, &base_url, "sm_goods_issue", &second, Vec::new())),
    )
    .await;
    assert_eq!(
        reversals
            .iter()
            .filter(|response| response.status().is_success())
            .count(),
        1
    );
    assert_eq!(balance_after(&pool, &second).await, "24");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn reversing_a_receipt_needs_its_stock(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(10),
            None,
        )
        .await,
    )
    .await;
    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(8), None).await).await;

    let body = expect_error(
        reversal(&client, &base_url, "sm_goods_receipt", &receipt, Vec::new()).await,
        StatusCode::CONFLICT,
        "balance_below_minimum",
    )
    .await;
    assert_eq!(body["error"]["details"]["available"], "2");
    assert_eq!(body["error"]["details"]["requested"], "10");

    success(reversal(&client, &base_url, "sm_goods_issue", &issue, Vec::new()).await).await;
    let undo =
        success(reversal(&client, &base_url, "sm_goods_receipt", &receipt, Vec::new()).await).await;
    assert_eq!(balance_after(&pool, &undo).await, "0");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn applied_movement_values_never_change(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let nut = product(&client, &base_url, "Nut").await;
    success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(10),
            None,
        )
        .await,
    )
    .await;
    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(3), None).await).await;
    let id = issue["id"].as_str().unwrap();

    for (request, attribute) in [
        (json!({"values": [scalar("quantity", 1)]}), "quantity"),
        (
            json!({"remove_values": [{"attribute_code": "quantity"}]}),
            "quantity",
        ),
        (
            json!({"relationships": [{"attribute_code": "product", "target_record_ids": [nut["id"]]}]}),
            "product",
        ),
    ] {
        let body = expect_error(
            client
                .put(format!("{base_url}/v1/records/{id}"))
                .json(&request)
                .send()
                .await
                .unwrap(),
            StatusCode::CONFLICT,
            "movement_immutable",
        )
        .await;
        assert_eq!(
            body["error"]["details"]["attribute"], attribute,
            "{request}"
        );
    }
    // Other fields stay editable.
    success(
        client
            .put(format!("{base_url}/v1/records/{id}"))
            .json(&json!({"values": [scalar("number", "WZ/1")]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(balance_after(&pool, &issue).await, "7");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn movements_and_held_balances_are_not_deleted(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(3),
            None,
        )
        .await,
    )
    .await;
    let receipt_id = receipt["id"].as_str().unwrap();
    expect_error(
        client
            .delete(format!("{base_url}/records/{receipt_id}"))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "movement_record_undeletable",
    )
    .await;
    let stock_item = stock_item_of(&pool, &receipt).await;
    let body = expect_error(
        client
            .delete(format!("{base_url}/records/{stock_item}"))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "balance_record_not_empty",
    )
    .await;
    assert_eq!(body["error"]["details"]["balance"], "3");

    success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(3), None).await).await;
    let response = client
        .delete(format!("{base_url}/records/{stock_item}"))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "an empty stock item can go");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_movement_target_keeps_the_key_movements_find_it_by(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let bolt = inventory(&client, &base_url).await;
    let nut = product(&client, &base_url, "Nut").await;
    let receipt = success(
        movement(
            &client,
            &base_url,
            "sm_goods_receipt",
            &bolt,
            json!(5),
            None,
        )
        .await,
    )
    .await;
    let stock_item = stock_item_of(&pool, &receipt).await;

    let body = expect_error(
        client
            .put(format!("{base_url}/v1/records/{stock_item}"))
            .json(&json!({
                "relationships": [{"attribute_code": "product", "target_record_ids": [nut["id"]]}]
            }))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "movement_target_key_immutable",
    )
    .await;
    assert_eq!(body["error"]["details"]["attribute"], "product");
    // Other fields of the stock item stay editable.
    success(
        client
            .put(format!("{base_url}/v1/records/{stock_item}"))
            .json(&json!({"values": [scalar("label", "Bolts")]}))
            .send()
            .await
            .unwrap(),
    )
    .await;
    // The issue still finds the stock item by its product.
    let issue =
        success(movement(&client, &base_url, "sm_goods_issue", &bolt, json!(2), None).await).await;
    assert_eq!(balance_after(&pool, &issue).await, "3");

    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn movements_accept_items_of_several_blueprints(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, PRODUCT).await;
    create_blueprint(
        &client,
        &base_url,
        &PRODUCT
            .replace("sm_product", "sm_material")
            .replace("name: Product", "name: Material"),
    )
    .await;
    // The stock item and the receipt list the item's targets in different
    // orders; publication compares them as sets.
    create_blueprint(
        &client,
        &base_url,
        &STOCK_ITEM.replace(
            "    target_blueprint: sm_product\n",
            "    target_blueprints: [sm_product, sm_material]\n",
        ),
    )
    .await;
    let receipt = r#"
format_version: 1
code: sm_mixed_receipt
name: Mixed receipt
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [number]
attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprints: [sm_material, sm_product]
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: sm_stock_item
    cardinality: one
movements:
  - code: stock
    direction: in
    quantity: quantity
    target:
      relationship: stock_item
      blueprint: sm_stock_item
      balance: quantity
      key: product
      values: {product: product}
      create_missing: true
"#;
    create_blueprint(&client, &base_url, receipt).await;

    let bolt = product(&client, &base_url, "Bolt").await;
    let steel = create_record_with(
        &client,
        &base_url,
        "sm_material",
        json!([scalar("name", "Steel")]),
    )
    .await;
    let bolts = success(
        movement(
            &client,
            &base_url,
            "sm_mixed_receipt",
            &bolt,
            json!(4),
            None,
        )
        .await,
    )
    .await;
    let steel_in = success(
        movement(
            &client,
            &base_url,
            "sm_mixed_receipt",
            &steel,
            json!(2.5),
            None,
        )
        .await,
    )
    .await;
    assert_ne!(
        stock_item_of(&pool, &bolts).await,
        stock_item_of(&pool, &steel_in).await
    );
    assert_eq!(balance_after(&pool, &bolts).await, "4");
    assert_eq!(balance_after(&pool, &steel_in).await, "2.5");

    // A relationship filter can end in a relationship to several blueprints,
    // as a picker scoped by the item does.
    let found: Value = client
        .post(format!("{base_url}/v1/records/search"))
        .json(&json!({
            "blueprint": {"code": "sm_stock_item"},
            "query": "",
            "filters": [],
            "relationship_filters": [{"field": "product", "selected_target_ids": [steel["id"]]}],
            "page": {"size": 10, "cursor": null},
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = found["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{found}"))
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    let steel_item = stock_item_of(&pool, &steel_in).await.to_string();
    assert_eq!(ids, [steel_item.as_str()]);

    server.abort();
}
