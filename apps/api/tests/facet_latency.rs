//! Repeatable latency measurements for relationship tree facets: the Browse
//! rail's children pages and searches that select tree nodes.
//!
//! ```sh
//! set -a; source .env; set +a
//! RUST_LOG=warn cargo test --release -p api --test facet_latency -- --ignored --nocapture
//! ```
//!
//! `FACET_BENCH_PRODUCTS` (default 100000) sets the number of products. They
//! link to a four-level category tree of 4,110 nodes, and every tenth product
//! overrides its category in a child context. Products also link to one of
//! 1,000 product lines, each in a leaf category, for the two-hop path
//! `line.category`; every tenth product and line overrides its hop in the
//! child context. Each product also has its own listing, in the product's
//! category and line, for paths whose middle hop is as large as the source:
//! `listing.category` and `listing.line.category`. See `perf/facets.md` for
//! results.

mod support;

use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{Value, json};
use sqlx::PgPool;
use support::*;
use uuid::Uuid;

const CATEGORY: &str = r#"
format_version = 1
code = "bench_category"
name = "Bench category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "bench_category"
cardinality = "one"
tree = true
context_editable = "default"
"#;

const PRODUCT: &str = r#"
format_version = 1
code = "bench_product"
name = "Bench product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "bench_category"

[[attributes]]
code = "line"
value_type = "relationship"
target_blueprint = "bench_line"
cardinality = "one"

[[attributes]]
code = "listing"
value_type = "relationship"
target_blueprint = "bench_listing"
cardinality = "one"
"#;

const LISTING: &str = r#"
format_version = 1
code = "bench_listing"
name = "Bench listing"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "bench_category"
cardinality = "one"

[[attributes]]
code = "line"
value_type = "relationship"
target_blueprint = "bench_line"
cardinality = "one"
"#;

const LINE: &str = r#"
format_version = 1
code = "bench_line"
name = "Bench line"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "bench_category"
cardinality = "one"
"#;

/// Bulk rows bypass the record API, which would take minutes at this size.
/// Blueprints, attributes and contexts still come from the API.
const SEED: &str = r#"
CREATE TEMP TABLE bench_category (id uuid, parent uuid, depth int, n int);
INSERT INTO bench_category SELECT gen_random_uuid(), NULL, 0, i FROM generate_series(1, 10) i;
INSERT INTO bench_category SELECT gen_random_uuid(), c.id, 1, i FROM bench_category c, generate_series(1, 10) i WHERE c.depth = 0;
INSERT INTO bench_category SELECT gen_random_uuid(), c.id, 2, i FROM bench_category c, generate_series(1, 8) i WHERE c.depth = 1;
INSERT INTO bench_category SELECT gen_random_uuid(), c.id, 3, i FROM bench_category c, generate_series(1, 4) i WHERE c.depth = 2;
INSERT INTO records (id, blueprint_id, blueprint_version, projections)
SELECT id, $1, 1, jsonb_build_object('preview', jsonb_build_object('name', 'Category ' || depth || '-' || n))
FROM bench_category;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, value_text)
SELECT gen_random_uuid(), id, $3, $8, 'Category ' || depth || '-' || n FROM bench_category;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $4, $8, parent FROM bench_category WHERE parent IS NOT NULL;

CREATE TEMP TABLE bench_leaf AS SELECT row_number() OVER () AS rn, id FROM bench_category WHERE depth >= 2;
CREATE TEMP TABLE bench_product AS
SELECT gen_random_uuid() AS id, i,
       (SELECT id FROM bench_leaf WHERE rn = 1 + (hashint4(i) & 2147483647) % (SELECT count(*) FROM bench_leaf)) AS category,
       (SELECT id FROM bench_leaf WHERE rn = 1 + (hashint4(i + 7) & 2147483647) % (SELECT count(*) FROM bench_leaf)) AS override
FROM generate_series(1, $10) i;
INSERT INTO records (id, blueprint_id, blueprint_version, projections)
SELECT id, $2, 1, jsonb_build_object('preview', jsonb_build_object('name', 'Product ' || i))
FROM bench_product;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, value_text)
SELECT gen_random_uuid(), p.id, a.id, $8,
       CASE a.id WHEN $5 THEN 'Product ' || p.i ELSE 'Value ' || p.i END
FROM bench_product p, unnest(ARRAY[$5, $6, $7]::uuid[]) a(id);
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $11, $8, category FROM bench_product;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $11, $9, override FROM bench_product WHERE i % 10 = 0;

CREATE TEMP TABLE bench_line AS
SELECT gen_random_uuid() AS id, i,
       (SELECT id FROM bench_leaf WHERE rn = 1 + (hashint4(i + 3) & 2147483647) % (SELECT count(*) FROM bench_leaf)) AS category,
       (SELECT id FROM bench_leaf WHERE rn = 1 + (hashint4(i + 5) & 2147483647) % (SELECT count(*) FROM bench_leaf)) AS override
FROM generate_series(1, 1000) i;
INSERT INTO records (id, blueprint_id, blueprint_version, projections)
SELECT id, $12, 1, jsonb_build_object('preview', jsonb_build_object('name', 'Line ' || i))
FROM bench_line;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, value_text)
SELECT gen_random_uuid(), id, $13, $8, 'Line ' || i FROM bench_line;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $14, $8, category FROM bench_line;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $14, $9, override FROM bench_line WHERE i % 10 = 0;
CREATE TEMP TABLE bench_line_rn AS SELECT row_number() OVER () AS rn, id FROM bench_line;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), p.id, $15, $8, l.id
FROM bench_product p JOIN bench_line_rn l ON l.rn = 1 + (hashint4(p.i + 11) & 2147483647) % 1000;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), p.id, $15, $9, l.id
FROM bench_product p JOIN bench_line_rn l ON l.rn = 1 + (hashint4(p.i + 13) & 2147483647) % 1000
WHERE p.i % 10 = 0;

CREATE TEMP TABLE bench_listing AS
SELECT gen_random_uuid() AS id, p.id AS product, p.category, l.id AS line
FROM bench_product p JOIN bench_line_rn l ON l.rn = 1 + (hashint4(p.i + 11) & 2147483647) % 1000;
INSERT INTO records (id, blueprint_id, blueprint_version, projections)
SELECT id, $16, 1, jsonb_build_object('preview', '{}'::jsonb)
FROM bench_listing;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $17, $8, category FROM bench_listing;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), id, $18, $8, line FROM bench_listing;
INSERT INTO attribute_values (id, record_id, attribute_id, context_id, relationship_target_record_id)
SELECT gen_random_uuid(), product, $19, $8, id FROM bench_listing;
"#;

#[sqlx::test]
#[ignore = "benchmark; run explicitly with a controlled PostgreSQL instance"]
async fn relationship_tree_facet_latency(pool: PgPool) {
    let (base_url, server) = start_server_with_devtools(pool.clone()).await;
    let client = authenticated_client();
    let category = create_blueprint(&client, &base_url, CATEGORY).await;
    let line = create_blueprint(&client, &base_url, LINE).await;
    let listing = create_blueprint(&client, &base_url, LISTING).await;
    let product = create_blueprint(&client, &base_url, PRODUCT).await;
    let child_context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "bench_child", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let default_context: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    let child_context: Uuid = child_context["id"].as_str().unwrap().parse().unwrap();
    let category_id = blueprint_id(&category);
    let product_id = blueprint_id(&product);
    let line_id = blueprint_id(&line);
    let listing_id = blueprint_id(&listing);
    let attribute = |blueprint: Uuid, code: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM attributes WHERE blueprint_id = $1 AND code = $2",
            )
            .bind(blueprint)
            .bind(code)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let products: i32 = std::env::var("FACET_BENCH_PRODUCTS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(100_000);
    let seeded = Instant::now();
    let mut connection = pool.acquire().await.unwrap();
    for statement in SEED.split(";\n").map(str::trim).filter(|s| !s.is_empty()) {
        let (statement, parameters) = renumber_parameters(statement);
        let mut query = sqlx::query(&statement);
        for parameter in parameters {
            query = match parameter {
                1 => query.bind(category_id),
                2 => query.bind(product_id),
                3 => query.bind(attribute(category_id, "name").await),
                4 => query.bind(attribute(category_id, "parent").await),
                5 => query.bind(attribute(product_id, "name").await),
                6 => query.bind(attribute(product_id, "sku").await),
                7 => query.bind(attribute(product_id, "description").await),
                8 => query.bind(default_context),
                9 => query.bind(child_context),
                10 => query.bind(products),
                11 => query.bind(attribute(product_id, "category").await),
                12 => query.bind(line_id),
                13 => query.bind(attribute(line_id, "name").await),
                14 => query.bind(attribute(line_id, "category").await),
                15 => query.bind(attribute(product_id, "line").await),
                16 => query.bind(listing_id),
                17 => query.bind(attribute(listing_id, "category").await),
                18 => query.bind(attribute(listing_id, "line").await),
                19 => query.bind(attribute(product_id, "listing").await),
                _ => unreachable!(),
            };
        }
        query.execute(&mut *connection).await.unwrap();
    }
    drop(connection);
    sqlx::query("VACUUM ANALYZE").execute(&pool).await.unwrap();
    println!("seeded {products} products in {:?}", seeded.elapsed());

    let node = |name: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM records WHERE blueprint_id = $1 AND projections -> 'preview' ->> 'name' = $2 ORDER BY id LIMIT 1",
            )
            .bind(category_id)
            .bind(name)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let root = node("Category 0-1").await;
    let first_child = |parent: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT record_id FROM attribute_values WHERE relationship_target_record_id = $1 AND context_id = $2 ORDER BY record_id LIMIT 1",
            )
            .bind(parent)
            .bind(default_context)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let level_1 = first_child(root).await;
    let level_2 = first_child(level_1).await;

    let children = |source: &str, context: Uuid, parent: Option<Uuid>, query: Option<&str>| {
        let mut body = json!({
            "blueprint": { "code": "bench_product" },
            "source_relationship_field": source,
            "hierarchy_field": "parent",
            "context_id": context,
        });
        if let Some(parent) = parent {
            body["parent_id"] = json!(parent);
        }
        if let Some(query) = query {
            body["query"] = json!(query);
        }
        ("/v1/records/facets/relationship-tree/children", body)
    };
    let search = |source: &str, context: Uuid, selected: Option<Uuid>, version: Option<i64>| {
        let mut body = json!({ "blueprint": { "code": "bench_product" }, "include_total": true });
        if let Some(version) = version {
            body["blueprint"]["version"] = json!(version);
        }
        if let Some(selected) = selected {
            body["relationship_tree_facets"] = json!([{
                "source_relationship_field": source,
                "hierarchy_field": "parent",
                "context_id": context,
                "selected_target_ids": [selected],
            }]);
        }
        ("/v1/records/search", body)
    };
    let scenarios = [
        (
            "browse: root level",
            children("category", default_context, None, None),
        ),
        (
            "browse: root level, child context",
            children("category", child_context, None, None),
        ),
        (
            "browse: expand root",
            children("category", default_context, Some(root), None),
        ),
        (
            "browse: expand level 1",
            children("category", default_context, Some(level_1), None),
        ),
        (
            "browse: expand level 2",
            children("category", default_context, Some(level_2), None),
        ),
        (
            "browse: root level, query \"Product 1\"",
            children("category", default_context, None, Some("Product 1")),
        ),
        // "12" matches about 4% of products, "7" about 41%, "Product 1" all.
        (
            "browse: root level, query \"12\"",
            children("category", default_context, None, Some("12")),
        ),
        (
            "search: no facet (reference)",
            search("category", default_context, None, None),
        ),
        (
            "search: root selected",
            search("category", default_context, Some(root), None),
        ),
        (
            "search: root selected, child context",
            search("category", child_context, Some(root), None),
        ),
        (
            "search: root selected, version 1",
            search("category", default_context, Some(root), Some(1)),
        ),
        (
            "search: level 2 selected",
            search("category", default_context, Some(level_2), None),
        ),
        // The two-hop path keeps its own cached counts.
        (
            "path browse: root level",
            children("line.category", default_context, None, None),
        ),
        (
            "path browse: root level, child context",
            children("line.category", child_context, None, None),
        ),
        (
            "path browse: expand level 2",
            children("line.category", default_context, Some(level_2), None),
        ),
        (
            "path browse: root level, query \"Product 1\"",
            children("line.category", default_context, None, Some("Product 1")),
        ),
        (
            "path browse: root level, query \"12\"",
            children("line.category", default_context, None, Some("12")),
        ),
        (
            "path search: root selected",
            search("line.category", default_context, Some(root), None),
        ),
        (
            "path search: root selected, child context",
            search("line.category", child_context, Some(root), None),
        ),
        (
            "path search: level 2 selected",
            search("line.category", default_context, Some(level_2), None),
        ),
        // A middle hop with a record per product.
        (
            "large-hop browse: root level",
            children("listing.category", default_context, None, None),
        ),
        (
            "large-hop browse: root level, query \"Product 1\"",
            children("listing.category", default_context, None, Some("Product 1")),
        ),
        (
            "large-hop browse: root level, query \"12\"",
            children("listing.category", default_context, None, Some("12")),
        ),
        (
            "large-hop browse: root level, query \"7\"",
            children("listing.category", default_context, None, Some("7")),
        ),
        (
            "large-hop search: root selected",
            search("listing.category", default_context, Some(root), None),
        ),
        (
            "three-hop browse: root level",
            children("listing.line.category", default_context, None, None),
        ),
        (
            "three-hop browse: root level, query \"12\"",
            children("listing.line.category", default_context, None, Some("12")),
        ),
        (
            "three-hop search: root selected",
            search("listing.line.category", default_context, Some(root), None),
        ),
    ];
    // Ten samples after the first exceed the five executions after which
    // PostgreSQL may switch a pooled prepared statement to a generic plan.
    // `First` is each scenario's first request. Browse scenarios without a
    // query share cached counts, so only the first of them misses the cache.
    println!("| Scenario | First | Median | Min | Max |\n| --- | ---: | ---: | ---: | ---: |");
    let mut slowest = Vec::new();
    for (name, (path, body)) in scenarios {
        let started = Instant::now();
        post(&client, &base_url, path, &body).await;
        let first = started.elapsed();
        let mut samples = Vec::new();
        for _ in 0..10 {
            let started = Instant::now();
            let timing = post(&client, &base_url, path, &body).await;
            samples.push((started.elapsed(), timing));
        }
        samples.sort_by_key(|(elapsed, _)| *elapsed);
        let (max, max_timing) = samples.last().unwrap().clone();
        println!(
            "| {name} | {} | {} | {} | {} |",
            millis(first),
            millis(samples[samples.len() / 2].0),
            millis(samples[0].0),
            millis(max),
        );
        slowest.push(format!("{name}: {max_timing}"));
    }
    println!("\nServer-Timing of each scenario's slowest sample:");
    for line in slowest {
        println!("  {line}");
    }
    server.abort();
}

fn blueprint_id(blueprint: &Value) -> Uuid {
    blueprint["blueprint"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// Renumbers a statement's `$n` placeholders to `$1..` in ascending order of
/// `n`, returning the original numbers to bind in that order.
fn renumber_parameters(statement: &str) -> (String, Vec<usize>) {
    let mut parameters: Vec<usize> = Vec::new();
    let mut parts = statement.split('$');
    let mut pieces = vec![parts.next().unwrap_or_default().to_owned()];
    for part in parts {
        let digits = part.chars().take_while(char::is_ascii_digit).count();
        let parameter: usize = part[..digits].parse().unwrap();
        if !parameters.contains(&parameter) {
            parameters.push(parameter);
        }
        pieces.push(format!("{parameter}\0{}", &part[digits..]));
    }
    parameters.sort_unstable();
    let mut renumbered = pieces[0].clone();
    for piece in &pieces[1..] {
        let (parameter, rest) = piece.split_once('\0').unwrap();
        let parameter: usize = parameter.parse().unwrap();
        let position = parameters.iter().position(|p| *p == parameter).unwrap() + 1;
        renumbered.push_str(&format!("${position}{rest}"));
    }
    (renumbered, parameters)
}

/// Sends one request and returns its `server-timing` header.
async fn post(client: &Client, base_url: &str, path: &str, body: &Value) -> String {
    let response = client
        .post(format!("{base_url}{path}"))
        .json(body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let timing = response
        .headers()
        .get("server-timing")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    response.bytes().await.unwrap();
    timing
}

fn millis(duration: Duration) -> String {
    format!("{:.0} ms", duration.as_secs_f64() * 1000.0)
}
