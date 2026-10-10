mod support;
use support::*;

const CATEGORY: &str = r#"format_version = 1
code = 'ctx_category'
name = 'Context category'
kind = 'record'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['title']
[[attributes]]
code = 'title'
value_type = 'string'
"#;

const ITEM: &str = r#"format_version = 1
code = 'ctx_item'
name = 'Context item'
kind = 'record'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['name']
[views.table]
type = 'table'
[[views.table.columns]]
field = 'name'
[[views.table.columns]]
field = 'sku'
[[views.table.columns]]
field = 'category.title'
[[attributes]]
code = 'name'
value_type = 'string'
[[attributes]]
code = 'sku'
value_type = 'string'
context_fallback = 'none'
[[attributes]]
code = 'category'
value_type = 'relationship'
cardinality = 'one'
target_blueprint = 'ctx_category'
[[attributes]]
code = 'alt_category'
value_type = 'relationship'
cardinality = 'one'
target_blueprint = 'ctx_category'
"#;

async fn create_context(client: &Client, base: &str, code: &str, parent: Option<&Value>) -> Value {
    let mut body = json!({"code": code, "data": {}});
    if let Some(parent) = parent {
        body["parent_id"] = parent["id"].clone();
    }
    expect_status(
        client
            .post(format!("{base}/contexts"))
            .json(&body)
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await
}

async fn write_in_context(
    client: &Client,
    base: &str,
    record: &Value,
    context: &Value,
    value: Value,
) {
    let mut value = value;
    value["context_id"] = context["id"].clone();
    expect_status(
        client
            .put(format!(
                "{base}/v1/records/{}",
                record["id"].as_str().unwrap()
            ))
            .json(&json!({ "values": [value] }))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
}

async fn search(client: &Client, base: &str, context: Option<&str>, extra: Value) -> Value {
    let mut body = json!({"blueprint": {"code": "ctx_item"}, "page": {"size": 100}});
    if let Some(context) = context {
        body["context_code"] = json!(context);
    }
    for (key, value) in extra.as_object().unwrap() {
        body[key] = value.clone();
    }
    expect_status(
        client
            .post(format!("{base}/v1/records/search"))
            .json(&body)
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await
}

fn ids(response: &Value) -> Vec<String> {
    response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

fn id(record: &Value) -> String {
    record["id"].as_str().unwrap().to_owned()
}

fn cell(response: &Value, record: &Value, field: &str) -> Value {
    response["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == record["id"])
        .unwrap()["table_values"][field]
        .clone()
}

#[sqlx::test]
async fn search_resolves_filters_sorting_and_table_values_for_the_requested_context(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, CATEGORY).await;
    create_blueprint(&client, &base, ITEM).await;
    let pl = create_context(&client, &base, "pl", None).await;
    // A child without its own values resolves through `pl`.
    create_context(&client, &base, "pl-web", Some(&pl)).await;

    let shoes = create_record_with(
        &client,
        &base,
        "ctx_category",
        json!([scalar("title", "Shoes")]),
    )
    .await;
    let bags = create_record_with(
        &client,
        &base,
        "ctx_category",
        json!([scalar("title", "Bags")]),
    )
    .await;
    write_in_context(&client, &base, &shoes, &pl, scalar("title", "Buty")).await;
    write_in_context(&client, &base, &bags, &pl, scalar("title", "Torby")).await;
    let alpha = create_record_with(
        &client,
        &base,
        "ctx_item",
        json!([
            scalar("name", "Alpha"),
            scalar("sku", "A-1"),
            relationship("category", &shoes)
        ]),
    )
    .await;
    let beta = create_record_with(
        &client,
        &base,
        "ctx_item",
        json!([
            scalar("name", "Beta"),
            relationship("category", &bags),
            relationship("alt_category", &bags)
        ]),
    )
    .await;
    write_in_context(&client, &base, &alpha, &pl, scalar("name", "Zeta")).await;
    write_in_context(
        &client,
        &base,
        &beta,
        &pl,
        relationship("alt_category", &shoes),
    )
    .await;

    // Table values: translated names, inherited defaults, and non-inheriting attributes.
    let default = search(&client, &base, None, json!({})).await;
    assert_eq!(cell(&default, &alpha, "name"), json!(["Alpha"]));
    assert_eq!(cell(&default, &alpha, "sku"), json!(["A-1"]));
    assert_eq!(cell(&default, &alpha, "category.title"), json!(["Shoes"]));
    let web = search(&client, &base, Some("pl-web"), json!({})).await;
    assert_eq!(cell(&web, &alpha, "name"), json!(["Zeta"]));
    assert_eq!(cell(&web, &beta, "name"), json!(["Beta"]));
    assert_eq!(cell(&web, &alpha, "sku"), Value::Null);
    assert_eq!(cell(&web, &alpha, "category.title"), json!(["Buty"]));

    // Scalar filters.
    let by_name = json!({"filters": [{"field": "name", "operator": "eq", "value": "Zeta"}]});
    assert!(ids(&search(&client, &base, None, by_name.clone()).await).is_empty());
    assert_eq!(
        ids(&search(&client, &base, Some("pl-web"), by_name).await),
        [id(&alpha)]
    );
    let has_sku = json!({"filters": [{"field": "sku", "operator": "is_set", "value": true}]});
    assert_eq!(
        ids(&search(&client, &base, None, has_sku.clone()).await),
        [id(&alpha)]
    );
    assert!(ids(&search(&client, &base, Some("pl-web"), has_sku).await).is_empty());
    let by_title =
        json!({"filters": [{"field": "category.title", "operator": "eq", "value": "Buty"}]});
    assert_eq!(
        ids(&search(&client, &base, Some("pl"), by_title).await),
        [id(&alpha)]
    );

    // Relationship filters follow the edge resolved for the context.
    let alt_shoes = json!({"relationship_filters": [{"field": "alt_category", "selected_target_ids": [shoes["id"]]}]});
    assert!(ids(&search(&client, &base, None, alt_shoes.clone()).await).is_empty());
    assert_eq!(
        ids(&search(&client, &base, Some("pl-web"), alt_shoes).await),
        [id(&beta)]
    );

    // Local and relationship-path sorting.
    let by_name = json!({"sort": {"field": "name", "direction": "asc"}});
    assert_eq!(
        ids(&search(&client, &base, None, by_name.clone()).await),
        [id(&alpha), id(&beta)]
    );
    assert_eq!(
        ids(&search(&client, &base, Some("pl-web"), by_name).await),
        [id(&beta), id(&alpha)]
    );
    let by_category = json!({"blueprint": {"code": "ctx_item", "version": 1}, "sort": {"field": "category.title", "direction": "asc"}});
    assert_eq!(
        ids(&search(&client, &base, None, by_category.clone()).await),
        [id(&beta), id(&alpha)]
    );
    assert_eq!(
        ids(&search(&client, &base, Some("pl-web"), by_category).await),
        [id(&alpha), id(&beta)]
    );

    expect_error(
        client
            .post(format!("{base}/v1/records/search"))
            .json(&json!({"blueprint": {"code": "ctx_item"}, "context_code": "missing"}))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    server.abort();
}
