//! Relationship paths through a relationship to several target blueprints:
//! a product's `spec` links to one sheet, profile or part, whose shared
//! parameters come from one mixin.
use crate::support;

use api::{agent_tools, repository::AttricatRepository};
use support::*;

const GRADE: &str = r#"format_version: 1
code: mtp_grade
name: Grade
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [code]
attributes:
  - code: code
    value_type: string"#;

const PARAMETERS: &str = r#"format_version: 1
code: mtp_parameters
name: Spec parameters
kind: mixin
attributes:
  - code: thickness_mm
    name: Thickness
    value_type: number
  - code: grade
    value_type: relationship
    target_blueprint: mtp_grade
    cardinality: one"#;

/// A spec blueprint with the shared parameters and `extra` attributes.
fn spec(code: &str, extra: &str) -> String {
    format!(
        r#"format_version: 1
code: {code}
name: {code}
kind: record
includes:
  - alias: parameters
    code: mtp_parameters
    version: 1
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
attributes:
  - code: name
    value_type: string
  - code: thickness_mm
    from: parameters.thickness_mm
  - code: grade
    from: parameters.grade{extra}"#
    )
}

/// The product blueprint with table `columns`, each a field path.
fn product(columns: &[&str]) -> String {
    let columns: String = columns
        .iter()
        .map(|field| format!("\n      - {{field: {field}}}"))
        .collect();
    format!(
        r#"format_version: 1
code: mtp_product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [sku]
  table:
    type: table
    columns:{columns}
attributes:
  - code: sku
    value_type: string
  - code: spec
    value_type: relationship
    target_blueprints: [mtp_sheet, mtp_profile, mtp_part]
    cardinality: one"#
    )
}

const COLUMNS: &[&str] = &[
    "sku",
    "spec.thickness_mm",
    "spec.grade.code",
    "spec.width_mm",
];

struct Catalog {
    /// Products by SKU: sheet, profile, part, and one without a spec.
    products: Vec<(&'static str, Value)>,
}

async fn catalog(client: &Client, base_url: &str) -> Catalog {
    create_blueprint(client, base_url, GRADE).await;
    create_blueprint(client, base_url, PARAMETERS).await;
    let width = "\n  - code: width_mm\n    value_type: number";
    create_blueprint(client, base_url, &spec("mtp_sheet", width)).await;
    create_blueprint(client, base_url, &spec("mtp_profile", "")).await;
    create_blueprint(client, base_url, &spec("mtp_part", "")).await;
    create_blueprint(client, base_url, &product(COLUMNS)).await;

    let mut grades = Vec::new();
    for code in ["S235", "S355"] {
        grades.push(
            create_record_with(client, base_url, "mtp_grade", json!([scalar("code", code)])).await,
        );
    }
    let specs = [
        (
            "mtp_sheet",
            json!([
                scalar("name", "Sheet 2"),
                scalar("thickness_mm", 2),
                scalar("width_mm", 1000),
                relationship("grade", &grades[1]),
            ]),
        ),
        (
            "mtp_profile",
            json!([
                scalar("name", "Profile 5"),
                scalar("thickness_mm", 5),
                relationship("grade", &grades[0]),
            ]),
        ),
        (
            "mtp_part",
            json!([
                scalar("name", "Part 1"),
                scalar("thickness_mm", 1),
                relationship("grade", &grades[0]),
            ]),
        ),
    ];
    let mut products = Vec::new();
    for ((blueprint, values), sku) in specs.into_iter().zip(["P-SHEET", "P-PROFILE", "P-PART"]) {
        let target = create_record_with(client, base_url, blueprint, values).await;
        let record = create_record_with(
            client,
            base_url,
            "mtp_product",
            json!([scalar("sku", sku), relationship("spec", &target)]),
        )
        .await;
        products.push((sku, record));
    }
    let bare = create_record_with(
        client,
        base_url,
        "mtp_product",
        json!([scalar("sku", "P-NONE")]),
    )
    .await;
    products.push(("P-NONE", bare));
    Catalog { products }
}

/// Saves a blueprint draft, which validates its table column paths.
async fn save_draft(client: &Client, base_url: &str, definition: &str) -> reqwest::Response {
    client
        .post(format!("{base_url}/blueprints"))
        .json(&json!({ "definition": definition }))
        .send()
        .await
        .unwrap()
}

async fn search(client: &Client, base_url: &str, body: Value) -> reqwest::Response {
    client
        .post(format!("{base_url}/v1/records/search"))
        .json(&body)
        .send()
        .await
        .unwrap()
}

/// The SKUs of a search's items, in order.
fn skus(catalog: &Catalog, response: &Value) -> Vec<&'static str> {
    response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            catalog
                .products
                .iter()
                .find(|(_, record)| record["id"] == item["id"])
                .map(|(sku, _)| *sku)
                .unwrap()
        })
        .collect()
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn table_columns_show_and_sort_values_through_several_targets(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let catalog = catalog(&client, &base_url).await;

    let listed = expect_success(
        search(
            &client,
            &base_url,
            json!({"blueprint": {"code": "mtp_product"}, "page": {"size": 25}}),
        )
        .await,
    )
    .await;
    let paths = &listed["blueprint"]["table_path_attributes"];
    assert_eq!(
        paths[1],
        json!({"code": "spec.thickness_mm", "value_type": "number", "sortable": true})
    );
    assert_eq!(
        paths[2],
        json!({"code": "spec.grade.code", "value_type": "string", "sortable": true})
    );
    // Only sheets have a width: the path is valid and names where it is.
    assert_eq!(
        paths[3],
        json!({"code": "spec.width_mm", "value_type": "number", "sortable": true, "declared_on": ["mtp_sheet"]})
    );

    let cells = |sku: &str, path: &str| {
        let (_, record) = catalog
            .products
            .iter()
            .find(|(candidate, _)| *candidate == sku)
            .unwrap();
        listed["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == record["id"])
            .unwrap()["table_values"]
            .get(path)
            .cloned()
    };
    assert_eq!(cells("P-SHEET", "spec.thickness_mm"), Some(json!([2])));
    assert_eq!(cells("P-PROFILE", "spec.thickness_mm"), Some(json!([5])));
    assert_eq!(cells("P-PART", "spec.thickness_mm"), Some(json!([1])));
    assert_eq!(cells("P-SHEET", "spec.grade.code"), Some(json!(["S355"])));
    assert_eq!(cells("P-PART", "spec.grade.code"), Some(json!(["S235"])));
    assert_eq!(cells("P-SHEET", "spec.width_mm"), Some(json!([1000])));
    // A part has no width, and a product without a spec has nothing.
    assert_eq!(cells("P-PART", "spec.width_mm"), None);
    assert_eq!(cells("P-NONE", "spec.thickness_mm"), None);

    // Every product appears once in each order; those without a value last.
    for (field, direction, expected) in [
        (
            "spec.thickness_mm",
            "asc",
            ["P-PART", "P-SHEET", "P-PROFILE", "P-NONE"],
        ),
        (
            "spec.thickness_mm",
            "desc",
            ["P-PROFILE", "P-SHEET", "P-PART", "P-NONE"],
        ),
        (
            "spec.width_mm",
            "asc",
            ["P-SHEET", "P-PART", "P-PROFILE", "P-NONE"],
        ),
    ] {
        let mut seen = Vec::new();
        let mut cursor: Option<Value> = None;
        // Pages of one exercise the value and the missing-value phases.
        loop {
            let mut body = json!({
                "blueprint": {"code": "mtp_product"},
                "sort": {"field": field, "direction": direction},
                "page": {"size": 1},
            });
            if let Some(cursor) = &cursor {
                body["page"]["cursor"] = cursor.clone();
            }
            let page = expect_success(search(&client, &base_url, body).await).await;
            seen.extend(skus(&catalog, &page));
            match page["next_cursor"].as_str() {
                Some(next) => cursor = Some(json!(next)),
                None => break,
            }
        }
        assert_eq!(seen.len(), 4, "{field} {direction}: {seen:?}");
        if field == "spec.width_mm" {
            // Records without a width sort by ID after the one with it.
            assert_eq!(seen[0], expected[0], "{field}");
        } else {
            assert_eq!(seen, expected, "{field} {direction}");
        }
    }
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn a_segment_its_targets_declare_differently_is_rejected(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    catalog(&client, &base_url).await;
    // A gauge that is a number on one target and a string on the other,
    // whose grades link to different blueprints.
    for (code, value_type, grade_target) in [
        ("mtp_gauge_sheet", "number", "mtp_grade"),
        ("mtp_gauge_part", "string", "mtp_sheet"),
    ] {
        create_blueprint(
            &client,
            &base_url,
            &format!(
                r#"format_version: 1
code: {code}
name: {code}
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [gauge]
attributes:
  - code: gauge
    value_type: {value_type}
  - code: grade
    value_type: relationship
    target_blueprint: {grade_target}
    cardinality: one"#
            ),
        )
        .await;
    }
    let gauged = |columns: &str| {
        format!(
            r#"format_version: 1
code: mtp_gauged
name: Gauged
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [sku]
  table:
    type: table
    columns:
      - {{field: sku}}{columns}
attributes:
  - code: sku
    value_type: string
  - code: spec
    value_type: relationship
    target_blueprints: [mtp_gauge_sheet, mtp_gauge_part]
    cardinality: one
  - code: link
    value_type: relationship
    cardinality: one"#
        )
    };
    let body = expect_error(
        save_draft(&client, &base_url, &gauged("\n      - {field: spec.gauge}")).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "relationship_path_type_conflict",
    )
    .await;
    assert_eq!(
        body["error"]["details"],
        json!({
            "path": "spec.gauge",
            "segment": "gauge",
            "conflicts": [
                {"value_type": "number", "blueprints": ["mtp_gauge_sheet"]},
                {"value_type": "string", "blueprints": ["mtp_gauge_part"]},
            ],
        })
    );
    let body = expect_error(
        save_draft(
            &client,
            &base_url,
            &gauged("\n      - {field: spec.grade.code}"),
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "relationship_path_type_conflict",
    )
    .await;
    assert_eq!(
        body["error"]["details"]["conflicts"],
        json!([
            {"targets": ["mtp_grade"], "blueprints": ["mtp_gauge_sheet"]},
            {"targets": ["mtp_sheet"], "blueprints": ["mtp_gauge_part"]},
        ])
    );

    // Filters, relationship filters and the search syntax refuse them too.
    create_blueprint(&client, &base_url, &gauged("")).await;
    for body in [
        json!({"blueprint": {"code": "mtp_gauged"},
               "filters": [{"field": "spec.gauge", "operator": "eq", "value": "x"}]}),
        json!({"blueprint": {"code": "mtp_gauged"},
               "relationship_filters": [{"field": "spec.grade.grade",
                                         "selected_target_ids": [uuid::Uuid::new_v4()]}]}),
        json!({"blueprint": {"code": "mtp_gauged"}, "query": "spec.grade.code:S235"}),
    ] {
        expect_error(
            search(&client, &base_url, body.clone()).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "relationship_path_type_conflict",
        )
        .await;
    }
    // A relationship to any blueprint still ends a path.
    let body = json!({"blueprint": {"code": "mtp_gauged"},
                      "filters": [{"field": "link.code", "operator": "eq", "value": "x"}]});
    expect_error(
        search(&client, &base_url, body).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn filters_saved_searches_and_agents_follow_several_targets(pool: PgPool) {
    let repository_pool = pool.clone();
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let catalog = catalog(&client, &base_url).await;
    let grade_s235 = expect_success(
        search(
            &client,
            &base_url,
            json!({"blueprint": {"code": "mtp_grade"}, "query": "code:S235"}),
        )
        .await,
    )
    .await["items"][0]["id"]
        .clone();

    let thick = json!([{"field": "spec.thickness_mm", "operator": "gte", "value": 2}]);
    let grade_facet = json!([{"field": "spec.grade", "selected_target_ids": [grade_s235]}]);
    for (body, expected) in [
        (
            json!({"blueprint": {"code": "mtp_product"}, "filters": thick,
                   "sort": {"field": "sku", "direction": "asc"}}),
            vec!["P-PROFILE", "P-SHEET"],
        ),
        // A leaf only sheets have matches only their products.
        (
            json!({"blueprint": {"code": "mtp_product"},
                   "filters": [{"field": "spec.width_mm", "operator": "is_set", "value": true}]}),
            vec!["P-SHEET"],
        ),
        (
            json!({"blueprint": {"code": "mtp_product"}, "relationship_filters": grade_facet,
                   "sort": {"field": "sku", "direction": "asc"}}),
            vec!["P-PART", "P-PROFILE"],
        ),
        (
            json!({"blueprint": {"code": "mtp_product"}, "query": "spec.grade.code:S355"}),
            vec!["P-SHEET"],
        ),
    ] {
        let page = expect_success(search(&client, &base_url, body.clone()).await).await;
        assert_eq!(skus(&catalog, &page), expected, "{body}");
    }

    // A saved search through the hop keeps its state and runs as saved.
    let state = json!({"blueprint": "mtp_product", "attributeFilters": thick,
                       "sort": {"field": "spec.thickness_mm", "direction": "desc"}});
    let saved = expect_status(
        client
            .post(format!("{base_url}/saved-views"))
            .json(&json!({"kind": "explorer_search", "name": "Thick specs",
                          "visibility": "private", "state": state}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    let loaded = get_json(
        &client,
        format!("{base_url}/saved-views/{}", saved["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(loaded["state"], state);
    let page = expect_success(
        search(
            &client,
            &base_url,
            json!({"blueprint": {"code": "mtp_product"},
                   "filters": loaded["state"]["attributeFilters"],
                   "sort": loaded["state"]["sort"]}),
        )
        .await,
    )
    .await;
    assert_eq!(skus(&catalog, &page), ["P-PROFILE", "P-SHEET"]);

    // Agents resolve the same paths.
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = AttricatRepository::system(repository_pool).for_workspace(workspace);
    let result = agent_tools::execute_read(
        &repository,
        actor,
        workspace,
        "search_records",
        json!({"blueprint": {"code": "mtp_product"}, "filters": thick,
               "relationship_filters": grade_facet}),
    )
    .await
    .unwrap();
    assert_eq!(skus(&catalog, &result), ["P-PROFILE"]);
    let saved = agent_tools::execute_mutation(
        &repository,
        actor,
        "create_saved_search",
        json!({"name": "Agent thick specs", "blueprint": "mtp_product",
               "attributeFilters": thick}),
    )
    .await
    .unwrap();
    let saved = repository
        .get_saved_view(actor, saved["id"].as_str().unwrap().parse().unwrap(), false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.state["attributeFilters"], thick);
    server.abort();
}
