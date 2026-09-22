mod support;

use std::{collections::HashSet, sync::OnceLock};

use support::*;

const DEFAULT_CONTEXT_ID: &str = "00000000-0000-4000-8000-000000000001";

fn budget_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn set_values(client: &Client, base_url: &str, entity: &Value, values: Value) {
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            entity["id"].as_str().unwrap()
        ))
        .json(&json!({ "values": values }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

async fn search(
    client: &Client,
    base_url: &str,
    blueprint: &Value,
    query: &str,
) -> reqwest::Response {
    client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": { "code": blueprint["blueprint"]["code"] },
            "query": query,
            "page": { "size": 100 },
        }))
        .send()
        .await
        .unwrap()
}

async fn assert_budget_failure(
    pool: &PgPool,
    client: &Client,
    base_url: &str,
    blueprint: &Value,
    query: &str,
) {
    // Bulk fixture inserts bypass the normal write path, so refresh planner
    // statistics before exercising the production 250 ms statement limit.
    sqlx::query("ANALYZE entities").execute(pool).await.unwrap();
    sqlx::query("ANALYZE attribute_values")
        .execute(pool)
        .await
        .unwrap();
    let response = search(client, base_url, blueprint, query).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["error"]["code"],
        "global_relationship_search_budget_exceeded"
    );
    // A rejected resolver must not return an otherwise-valid, truncated page.
    assert!(body.get("items").is_none());
}

async fn create_node_blueprint(client: &Client, base_url: &str, code: &str) -> Value {
    create_blueprint(
        client,
        base_url,
        &format!(
            r#"
format_version = 1
code = "{code}"
name = "{code}"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["needle"]
[[attributes]]
code = "needle"
value_type = "string"
[[attributes]]
code = "link_one"
value_type = "relationship"
target_blueprint = "{code}"
[[attributes]]
code = "link_two"
value_type = "relationship"
target_blueprint = "{code}"
[[attributes]]
code = "link_three"
value_type = "relationship"
target_blueprint = "{code}"
"#
        ),
    )
    .await
}

fn blueprint_id(blueprint: &Value) -> Uuid {
    blueprint["blueprint"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

fn blueprint_version(blueprint: &Value) -> i64 {
    blueprint["blueprint"]["version"].as_i64().unwrap()
}

async fn attribute_id(pool: &PgPool, blueprint: &Value, code: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM attributes WHERE blueprint_id = $1 AND blueprint_version = $2 AND code = $3")
        .bind(blueprint_id(blueprint))
        .bind(blueprint_version(blueprint))
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_entities(pool: &PgPool, blueprint: &Value, count: i64) -> Vec<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO entities (id, workspace_id, blueprint_id, blueprint_version) \
         SELECT gen_random_uuid(), $1, $2, $3 FROM generate_series(1, $4) RETURNING id",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(blueprint_id(blueprint))
    .bind(blueprint_version(blueprint))
    .bind(count)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn insert_scalar_values(pool: &PgPool, entities: &[Uuid], attribute: Uuid, value: &str) {
    sqlx::query(
        "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active, value_text) \
         SELECT gen_random_uuid(), $1, entity_id, $2, $3, true, $4 FROM unnest($5::uuid[]) entity_id",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(attribute)
    .bind(DEFAULT_CONTEXT_ID.parse::<Uuid>().unwrap())
    .bind(value)
    .bind(entities)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_edges(pool: &PgPool, sources: &[Uuid], attribute: Uuid, target: Uuid) {
    sqlx::query(
        "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active, relationship_target_entity_id) \
         SELECT gen_random_uuid(), $1, entity_id, $2, $3, true, $4 FROM unnest($5::uuid[]) entity_id",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(attribute)
    .bind(DEFAULT_CONTEXT_ID.parse::<Uuid>().unwrap())
    .bind(target)
    .bind(sources)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_paired_edges(pool: &PgPool, sources: &[Uuid], attribute: Uuid, targets: &[Uuid]) {
    sqlx::query(
        "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active, relationship_target_entity_id) \
         SELECT gen_random_uuid(), $1, source_id, $2, $3, true, target_id \
         FROM unnest($4::uuid[], $5::uuid[]) AS edge(source_id, target_id)",
    )
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(attribute)
    .bind(DEFAULT_CONTEXT_ID.parse::<Uuid>().unwrap())
    .bind(sources)
    .bind(targets)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn global_search_rejects_root_budget_without_a_partial_page(pool: PgPool) {
    let _guard = budget_test_lock().lock().await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_node_blueprint(&client, &base_url, "global_budget_roots").await;
    let entities = insert_entities(&pool, &blueprint, 1_001).await;
    insert_scalar_values(
        &pool,
        &entities,
        attribute_id(&pool, &blueprint, "needle").await,
        "root-budget",
    )
    .await;

    assert_budget_failure(&pool, &client, &base_url, &blueprint, "*:root-budget").await;
    server.abort();
}

#[sqlx::test]
async fn global_search_rejects_frontier_budget_without_a_partial_page(pool: PgPool) {
    let _guard = budget_test_lock().lock().await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_node_blueprint(&client, &base_url, "global_budget_frontier").await;
    let target = create_entity(&client, &base_url, &blueprint).await;
    set_values(
        &client,
        &base_url,
        &target,
        json!([{ "kind": "scalar", "attribute_code": "needle", "value": "frontier-budget" }]),
    )
    .await;
    let sources = insert_entities(&pool, &blueprint, 5_001).await;
    insert_edges(
        &pool,
        &sources,
        attribute_id(&pool, &blueprint, "link_one").await,
        target["id"].as_str().unwrap().parse().unwrap(),
    )
    .await;

    assert_budget_failure(&pool, &client, &base_url, &blueprint, "*:frontier-budget").await;
    server.abort();
}

#[sqlx::test]
async fn global_search_rejects_edge_budget_without_a_partial_page(pool: PgPool) {
    let _guard = budget_test_lock().lock().await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_node_blueprint(&client, &base_url, "global_budget_edges").await;
    let target = create_entity(&client, &base_url, &blueprint).await;
    set_values(
        &client,
        &base_url,
        &target,
        json!([{ "kind": "scalar", "attribute_code": "needle", "value": "edge-budget" }]),
    )
    .await;
    let sources = insert_entities(&pool, &blueprint, 3_334).await;
    let target_id = target["id"].as_str().unwrap().parse().unwrap();
    for code in ["link_one", "link_two", "link_three"] {
        insert_edges(
            &pool,
            &sources,
            attribute_id(&pool, &blueprint, code).await,
            target_id,
        )
        .await;
    }

    assert_budget_failure(&pool, &client, &base_url, &blueprint, "*:edge-budget").await;
    server.abort();
}

#[sqlx::test]
async fn global_search_rejects_visited_budget_without_a_partial_page(pool: PgPool) {
    let _guard = budget_test_lock().lock().await;
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_node_blueprint(&client, &base_url, "global_budget_visited").await;
    let target = create_entity(&client, &base_url, &blueprint).await;
    set_values(
        &client,
        &base_url,
        &target,
        json!([{ "kind": "scalar", "attribute_code": "needle", "value": "visited-budget" }]),
    )
    .await;
    let middle = insert_entities(&pool, &blueprint, 2_500).await;
    let roots = insert_entities(&pool, &blueprint, 2_500).await;
    let link = attribute_id(&pool, &blueprint, "link_one").await;
    let target_id = target["id"].as_str().unwrap().parse().unwrap();
    insert_edges(&pool, &middle, link, target_id).await;
    insert_paired_edges(&pool, &roots, link, &middle).await;

    assert_budget_failure(&pool, &client, &base_url, &blueprint, "*:visited-budget").await;
    server.abort();
}

#[sqlx::test]
async fn global_search_returns_an_exact_three_level_witness(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_node_blueprint(&client, &base_url, "global_budget_three_levels").await;
    let leaf = create_entity(&client, &base_url, &blueprint).await;
    let one = create_entity(&client, &base_url, &blueprint).await;
    let two = create_entity(&client, &base_url, &blueprint).await;
    let three = create_entity(&client, &base_url, &blueprint).await;
    let four = create_entity(&client, &base_url, &blueprint).await;
    set_values(
        &client,
        &base_url,
        &leaf,
        json!([{ "kind": "scalar", "attribute_code": "needle", "value": "three-level-success" }]),
    )
    .await;
    for (source, target) in [(&one, &leaf), (&two, &one), (&three, &two), (&four, &three)] {
        client.put(format!("{base_url}/v1/entities/{}", source["id"].as_str().unwrap()))
            .json(&json!({ "relationships": [{ "attribute_code": "link_one", "target_entity_ids": [target["id"]] }] }))
            .send().await.unwrap().error_for_status().unwrap();
    }

    let response: Value = search(&client, &base_url, &blueprint, "*:three-level-success")
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = response["items"].as_array().unwrap();
    let ids: HashSet<_> = items
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 4);
    assert!(ids.contains(three["id"].as_str().unwrap()));
    assert!(!ids.contains(four["id"].as_str().unwrap()));
    let witness = items.iter().find(|item| item["id"] == three["id"]).unwrap();
    assert_eq!(witness["match_explanations"][0]["traversal_depth"], 3);
    assert_eq!(
        witness["match_explanations"][0]["relationship_path"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    server.abort();
}
