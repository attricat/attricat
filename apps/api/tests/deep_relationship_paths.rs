mod support;

use api::{agent_tools, repository::CatalogRepository};
use support::*;

#[sqlx::test]
async fn search_resolves_three_hop_table_query_filter_and_sort_paths(pool: PgPool) {
    let repository_pool = pool.clone();
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();

    let kind = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_kind"
name = "Deep kind"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let class = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_class"
name = "Deep class"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "kind"
value_type = "relationship"
target_blueprint = "deep_kind"
cardinality = "one""#,
    )
    .await;
    let family = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_family"
name = "Deep family"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "class"
value_type = "relationship"
target_blueprint = "deep_class"
cardinality = "one""#,
    )
    .await;
    let sku = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "deep_sku"
name = "Deep SKU"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[views.table]
type = "table"
columns = [
  { field = "sku", label = "SKU" },
  { field = "family.class.kind.name", label = "Kind" }
]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "family"
value_type = "relationship"
target_blueprint = "deep_family"
cardinality = "one""#,
    )
    .await;

    let kind_entity = create_entity(&client, &base_url, &kind).await;
    let class_entity = create_entity(&client, &base_url, &class).await;
    let family_entity = create_entity(&client, &base_url, &family).await;
    let sku_entity = create_entity(&client, &base_url, &sku).await;
    for (entity, attribute, value) in [
        (&kind_entity, "name", "Graphics Card"),
        (&class_entity, "name", "Component"),
        (&family_entity, "name", "Arc"),
        (&sku_entity, "sku", "ARC-1"),
    ] {
        client
            .post(format!("{base_url}/entities/{}/values", entity["id"].as_str().unwrap()))
            .json(&json!({"values": [{"kind": "scalar", "attribute_code": attribute, "value": value}]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    for (source, field, target) in [
        (&class_entity, "kind", &kind_entity),
        (&family_entity, "class", &class_entity),
        (&sku_entity, "family", &family_entity),
    ] {
        client
            .post(format!(
                "{base_url}/entities/{}/relationships/replace",
                source["id"].as_str().unwrap()
            ))
            .json(&json!({"relationships": [{
                "attribute_code": field,
                "target_entity_ids": [target["id"]]
            }]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    for body in [
        json!({
            "blueprint": {"code": "deep_sku"},
            "query": "family.class.kind.name:Graphics",
            "page": {"size": 25}
        }),
        json!({
            "blueprint": {"code": "deep_sku"},
            "filters": [{"field": "family.class.kind.name", "operator": "eq", "value": "Graphics Card"}],
            "page": {"size": 25}
        }),
        json!({
            "blueprint": {"code": "deep_sku"},
            "sort": {"field": "family.class.kind.name", "direction": "asc"},
            "page": {"size": 25}
        }),
    ] {
        let raw = client
            .post(format!("{base_url}/v1/entities/search"))
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = raw.status();
        let text = raw.text().await.unwrap();
        assert!(status.is_success(), "search failed with {status}: {text}");
        let response: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(response["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            response["blueprint"]["table_path_attributes"][1],
            json!({
                "code": "family.class.kind.name",
                "value_type": "string",
                "sortable": true
            })
        );
        assert_eq!(
            response["items"][0]["table_values"]["family.class.kind.name"],
            json!(["Graphics Card"])
        );
    }

    let workspace = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let repository = CatalogRepository::system(repository_pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let agent_result = agent_tools::execute_read(
        &repository,
        BOOTSTRAP_OWNER_ID.parse().unwrap(),
        workspace,
        "search_entities",
        json!({
            "blueprint": {"code": "deep_sku"},
            "sort": {"field": "family.class.kind.name", "direction": "asc"},
            "page": {"size": 25}
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        agent_result["items"][0]["table_values"]["family.class.kind.name"],
        json!(["Graphics Card"]),
        "agent results hydrate the related value that explains their order"
    );

    let actor = BOOTSTRAP_OWNER_ID.parse().unwrap();
    for (filters, relationship_filters, expected) in [
        (
            json!([{"field":"family.class.kind.name","operator":"eq","value":"Graphics Card"}]),
            json!([]),
            1,
        ),
        (
            json!([{"field":"sku","operator":"eq","value":"ARC-1"}]),
            json!([{"field":"family.class.kind","selected_target_ids":[kind_entity["id"]]}]),
            1,
        ),
        (
            json!([{"field":"sku","operator":"eq","value":"missing"}]),
            json!([]),
            0,
        ),
    ] {
        let result = agent_tools::execute_read(
            &repository,
            actor,
            workspace,
            "search_entities",
            json!({
                "blueprint":{"code":"deep_sku"}, "filters":filters,
                "relationship_filters":relationship_filters,
            }),
        )
        .await
        .unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), expected);
    }
    let saved = agent_tools::execute_mutation(&repository, actor, "create_saved_search", json!({
        "name":"Graphics SKUs", "blueprint":"deep_sku",
        "attributeFilters":[{"field":"family.class.kind.name","operator":"eq","value":"Graphics Card"}],
        "relationshipFacets":[{"field":"family.class.kind","selectedIds":[kind_entity["id"]]}],
    })).await.unwrap();
    let saved_id = saved["id"].as_str().unwrap().parse().unwrap();
    let loaded = repository
        .get_saved_view(actor, saved_id, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.state["attributeFilters"][0]["value"],
        json!("Graphics Card")
    );
    assert_eq!(
        loaded.state["relationshipFacets"][0]["selectedIds"][0],
        kind_entity["id"]
    );
    assert!(
        agent_tools::execute_mutation(
            &repository,
            actor,
            "create_saved_search",
            json!({
                "name":"Bad filter", "blueprint":"deep_sku",
                "attributeFilters":[{"field":"sku","operator":"gt","value":"ARC-1"}],
            })
        )
        .await
        .is_err()
    );

    let owned = agent_tools::execute_read(
        &repository,
        actor,
        workspace,
        "list_saved_searches",
        json!({}),
    )
    .await
    .unwrap();
    assert!(
        owned
            .as_array()
            .unwrap()
            .iter()
            .any(|view| view["id"] == saved["id"])
    );
    let before = agent_tools::execute_read(
        &repository,
        actor,
        workspace,
        "get_saved_search",
        json!({"saved_view_id":saved_id}),
    )
    .await
    .unwrap();
    assert_eq!(
        before["state"]["relationshipFacets"][0]["selectedIds"][0],
        kind_entity["id"]
    );
    let mut browser_state = loaded.state.clone();
    browser_state["sort"] = json!({"field":"sku","direction":"asc"});
    browser_state["context"] = json!("default-channel");
    repository
        .update_saved_view(
            actor,
            saved_id,
            "Graphics SKUs",
            None,
            "private",
            &browser_state,
        )
        .await
        .unwrap();
    let updated = agent_tools::execute_mutation(
        &repository,
        actor,
        "update_saved_search",
        json!({
            "saved_view_id": saved_id, "name": "Graphics SKUs updated", "query": "ARC*",
            "relationshipFacets": [],
        }),
    )
    .await
    .unwrap();
    assert_eq!(updated["id"], saved["id"]);
    assert_eq!(updated["url"], saved["url"]);
    assert_eq!(
        updated["state"]["attributeFilters"][0]["value"],
        json!("Graphics Card")
    );
    assert!(updated["state"].get("relationshipFacets").is_none());
    assert_eq!(updated["state"]["query"], json!("ARC*"));
    assert_eq!(updated["state"]["sort"], browser_state["sort"]);
    assert_eq!(updated["state"]["context"], browser_state["context"]);
    assert!(
        agent_tools::execute_mutation(
            &repository,
            uuid::Uuid::new_v4(),
            "update_saved_search",
            json!({
                "saved_view_id": saved_id, "name": "Unauthorized change",
            })
        )
        .await
        .is_err()
    );
    let after = repository
        .get_saved_view(actor, saved_id, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.name.as_deref(), Some("Graphics SKUs updated"));

    let kind_blueprint_id = kind["blueprint"]["id"].as_str().unwrap();
    let kind_draft: Value = client
        .post(format!(
            "{base_url}/blueprints/{kind_blueprint_id}/versions"
        ))
        .json(&json!({"definition": r#"format_version = 1
code = "deep_kind"
name = "Deep kind"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "integer""#}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let before_publish: Value = client
        .get(format!("{base_url}/blueprints/by-code/deep_sku"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        before_publish["table_path_attributes"][1]["value_type"], "string",
        "newer drafts must not affect published path metadata"
    );

    let kind_draft_version = kind_draft["blueprint"]["version"].as_i64().unwrap();
    client
        .post(format!(
            "{base_url}/blueprints/{kind_blueprint_id}/versions/{kind_draft_version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let incompatible: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "deep_sku"},
            "page": {"size": 25}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        incompatible["blueprint"]["table_path_attributes"][1]["value_type"],
        "integer"
    );
    assert!(
        incompatible["items"][0]["table_values"]
            .get("family.class.kind.name")
            .is_none(),
        "a pinned string leaf must not be hydrated under integer path metadata"
    );

    server.abort();
}

#[sqlx::test]
async fn relationship_sort_requires_one_effective_source_version(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let leaf = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "versioned_sort_leaf"
name = "Versioned sort leaf"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string""#,
    )
    .await;
    let middle = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "versioned_sort_middle"
name = "Versioned sort middle"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "leaf"
value_type = "relationship"
target_blueprint = "versioned_sort_leaf"
cardinality = "one""#,
    )
    .await;
    let source_v1_definition = r#"format_version = 1
code = "versioned_sort_source"
name = "Versioned sort source"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "middle.leaf.name", label = "Leaf" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "middle"
value_type = "relationship"
target_blueprint = "versioned_sort_middle""#;
    let source_v1 = create_blueprint(&client, &base_url, source_v1_definition).await;
    let leaf_entity = create_entity(&client, &base_url, &leaf).await;
    let middle_entity = create_entity(&client, &base_url, &middle).await;
    let old_source = create_entity(&client, &base_url, &source_v1).await;
    client
        .post(format!(
            "{base_url}/entities/{}/values",
            leaf_entity["id"].as_str().unwrap()
        ))
        .json(&json!({"values": [{"kind": "scalar", "attribute_code": "name", "value": "Alpha"}]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    for (source, field, target) in [
        (&middle_entity, "leaf", &leaf_entity),
        (&old_source, "middle", &middle_entity),
    ] {
        client
            .post(format!("{base_url}/entities/{}/relationships/replace", source["id"].as_str().unwrap()))
            .json(&json!({"relationships": [{"attribute_code": field, "target_entity_ids": [target["id"]]}]}))
            .send().await.unwrap().error_for_status().unwrap();
    }

    let source_id = source_v1["blueprint"]["id"].as_str().unwrap();
    let revision: Value = client
        .post(format!("{base_url}/blueprints/{source_id}/versions"))
        .json(&json!({"definition": source_v1_definition.replace(
            "target_blueprint = \"versioned_sort_middle\"",
            "target_blueprint = \"versioned_sort_middle\"\ncardinality = \"one\"",
        )}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let version = revision["blueprint"]["version"].as_i64().unwrap();
    let source_v2: Value = client
        .post(format!(
            "{base_url}/blueprints/{source_id}/versions/{version}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let current_source = create_entity(&client, &base_url, &source_v2).await;
    client
        .post(format!("{base_url}/entities/{}/relationships/replace", current_source["id"].as_str().unwrap()))
        .json(&json!({"relationships": [{"attribute_code": "middle", "target_entity_ids": [middle_entity["id"]]}]}))
        .send().await.unwrap().error_for_status().unwrap();

    let historical_sort = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source", "version": 1},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(historical_sort.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let all_versions: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "page": {"size": 25}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all_versions["result_version_scope"]["kind"], "multiple");

    let all_version_sort = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(all_version_sort.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error: Value = all_version_sort.json().await.unwrap();
    assert_eq!(
        error["error"]["code"],
        "relationship_path_sort_requires_single_result_version"
    );

    let current_only: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source", "version": version},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current_only["items"][0]["id"], current_source["id"]);
    assert_eq!(current_only["result_version_scope"]["version"], version);
    assert_eq!(current_only["hidden_outdated_count"], 1);
    assert_eq!(current_only["hidden_outdated_count_capped"], false);

    client
        .delete(format!(
            "{base_url}/entities/{}",
            old_source["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let second_current = create_entity(&client, &base_url, &source_v2).await;
    client
        .post(format!(
            "{base_url}/entities/{}/relationships/replace",
            second_current["id"].as_str().unwrap()
        ))
        .json(&json!({"relationships": [{
            "attribute_code": "middle",
            "target_entity_ids": [middle_entity["id"]]
        }]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let single_version_page: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        single_version_page["result_version_scope"]["version"],
        version
    );
    let stale_cursor = single_version_page["next_cursor"]
        .as_str()
        .expect("two current entities produce a cursor");
    let shared_target_next_page: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1, "cursor": stale_cursor}
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(
        shared_target_next_page["items"][0]["id"], single_version_page["items"][0]["id"],
        "the target/source cursor must page distinct sources that share one leaf target"
    );

    let revision_v3: Value = client
        .post(format!("{base_url}/blueprints/{source_id}/versions"))
        .json(&json!({"definition": source_v1_definition
        .replace(
            "target_blueprint = \"versioned_sort_middle\"",
            "target_blueprint = \"versioned_sort_middle\"\ncardinality = \"one\"",
        )
        .replace(
            "name = \"Versioned sort source\"",
            "name = \"Versioned sort source v3\"",
        )}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let version_v3 = revision_v3["blueprint"]["version"].as_i64().unwrap();
    let source_v3: Value = client
        .post(format!(
            "{base_url}/blueprints/{source_id}/versions/{version_v3}/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    create_entity(&client, &base_url, &source_v3).await;

    let stale = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({
            "blueprint": {"code": "versioned_sort_source"},
            "sort": {"field": "middle.leaf.name", "direction": "asc"},
            "page": {"size": 1, "cursor": stale_cursor}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}
