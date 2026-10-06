mod support;
use support::*;

const ITEM: &str = r#"format_version = 1
code = 'presence_item'
name = 'Presence item'
kind = 'entity'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['name']
[[attributes]]
code = 'name'
value_type = 'string'
[[attributes]]
code = 'text'
value_type = 'string'
context_editable = 'all'
[[attributes]]
code = 'amount'
value_type = 'number'
[[attributes]]
code = 'count'
value_type = 'integer'
[[attributes]]
code = 'flag'
value_type = 'boolean'
[[attributes]]
code = 'day'
value_type = 'date'
[[attributes]]
code = 'instant'
value_type = 'datetime'
[[attributes]]
code = 'clock'
value_type = 'time'
[[attributes]]
code = 'assignee'
value_type = 'string'
value_schema = '''{"type":"string","x-attricat-principal":{"version":1,"kinds":["user","team"]}}'''
[[attributes]]
code = 'parent'
value_type = 'relationship'
cardinality = 'one'
target_blueprint = 'presence_item'
[[attributes]]
code = 'payload'
value_type = 'json'
[[attributes]]
code = 'files'
value_type = 'file'
"#;

async fn search(client: &Client, base: &str, filters: Value, version: Option<i64>) -> Value {
    expect_status(client.post(format!("{base}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"presence_item","version":version},"filters":filters,"include_total":true,"page":{"size":100}}))
        .send().await.unwrap(), StatusCode::OK).await
}

fn assert_ids(response: &Value, expected: &[&Value]) {
    let mut actual = response["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{response}");
    assert_eq!(response["total_count"], expected.len());
}

fn presence(field: &str, value: bool) -> Value {
    json!([{"field":field,"operator":"is_set","value":value}])
}

#[sqlx::test]
async fn scalar_presence_distinguishes_absence_from_false_zero_empty_and_restored_values(
    pool: PgPool,
) {
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, ITEM).await;
    let absent = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Absent")]),
    )
    .await;
    let present = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([
            scalar("name", "Present"),
            scalar("text", ""),
            scalar("amount", 0),
            scalar("count", 0),
            scalar("flag", false),
            scalar("day", "2026-10-01"),
            scalar("instant", "2026-10-01T10:00:00Z"),
            scalar("clock", json!({"time":"09:30","time_zone":"UTC"})),
            scalar("assignee", format!("user:{BOOTSTRAP_OWNER_ID}"))
        ]),
    )
    .await;
    let context = expect_status(
        client
            .post(format!("{base}/contexts"))
            .json(&json!({"code":"alternate","data":{}}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    expect_status(client.put(format!("{base}/v1/entities/{}",absent["id"].as_str().unwrap()))
        .json(&json!({"values":[{"kind":"scalar","attribute_code":"text","context_id":context["id"],"value":"Only in alternate context"}]}))
        .send().await.unwrap(),StatusCode::OK).await;
    let workspace = bootstrap_workspace_id();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = api::repository::CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    for field in [
        "text", "amount", "count", "flag", "day", "instant", "clock", "assignee",
    ] {
        for (value, expected) in [(true, &present), (false, &absent)] {
            let result = api::agent_tools::execute_read(
                &repository,
                actor,
                workspace,
                "search_entities",
                json!({"blueprint":{"code":"presence_item"},"filters":presence(field,value)}),
            )
            .await
            .unwrap();
            assert_eq!(result["items"].as_array().unwrap().len(), 1);
            assert_eq!(result["items"][0]["id"], expected["id"]);
        }
        assert_ids(
            &search(&client, &base, presence(field, true), None).await,
            &[&present],
        );
        assert_ids(
            &search(&client, &base, presence(field, false), None).await,
            &[&absent],
        );
    }
    assert_ids(
        &search(
            &client,
            &base,
            json!([
                {"field":"amount","operator":"is_set","value":false},
                {"field":"name","operator":"eq","value":"Absent"},
                {"field":"flag","operator":"is_set","value":false}
            ]),
            None,
        )
        .await,
        &[&absent],
    );
    assert_ids(
        &search(
            &client,
            &base,
            json!([{"field":"assignee","operator":"eq","value":"@me"}]),
            None,
        )
        .await,
        &[&present],
    );
    for (field, value) in [
        ("amount", json!("false")),
        ("assignee", json!(0)),
        ("text", Value::Null),
        ("parent", json!(false)),
        ("files", json!("true")),
        ("payload", json!(false)),
        ("unknown", json!(false)),
    ] {
        let response=client.post(format!("{base}/v1/entities/search")).json(&json!({"blueprint":{"code":"presence_item"},"filters":[{"field":field,"operator":"is_set","value":value}]})).send().await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}: {value}"
        );
        let result = api::agent_tools::execute_read(&repository, actor, workspace, "search_entities",
            json!({"blueprint":{"code":"presence_item"},"filters":[{"field":field,"operator":"is_set","value":value}]})).await;
        assert!(
            matches!(
                result,
                Err(api::agent_tools::ToolError::InvalidArguments(_))
            ),
            "{result:?}"
        );
    }
    let id = present["id"].as_str().unwrap();
    expect_status(
        client
            .put(format!("{base}/v1/entities/{id}"))
            .json(&json!({"remove_values":[{"attribute_code":"assignee","context_id":null}]}))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("assignee", false), None).await,
        &[&absent, &present],
    );
    let history = get_json(&client, format!("{base}/entities/{id}/values/history")).await;
    let entry = history
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["value"] == format!("user:{BOOTSTRAP_OWNER_ID}"))
        .unwrap();
    expect_status(
        client
            .post(format!(
                "{base}/entities/{id}/values/history/{}/restore",
                entry["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("assignee", true), None).await,
        &[&present],
    );
    let (user, membership) = add_workspace_user(&pool).await;
    let limited = client_for(user);
    grant_role(
        &pool,
        membership,
        VIEWER_ROLE_ID,
        GrantScope::Entity(Uuid::parse_str(id).unwrap()),
    )
    .await;
    let denied = limited
        .post(format!("{base}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"presence_item"},"filters":presence("assignee",false)}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let denied = api::agent_tools::execute_read(
        &repository,
        user,
        workspace,
        "search_entities",
        json!({"blueprint":{"code":"presence_item"},"filters":presence("assignee",false)}),
    )
    .await;
    assert!(matches!(
        denied,
        Err(api::agent_tools::ToolError::Forbidden)
    ));
    expect_status(
        client
            .delete(format!("{base}/entities/{id}"))
            .send()
            .await
            .unwrap(),
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("assignee", false), None).await,
        &[&absent],
    );
    assert_ids(
        &search(&client, &base, presence("assignee", true), None).await,
        &[],
    );
    server.abort();
}

#[sqlx::test]
async fn file_presence_requires_an_attached_file(pool: PgPool) {
    let (base, server) = start_server_with_object_store(
        pool,
        std::sync::Arc::new(api::storage::FakeObjectStore::available()),
    )
    .await;
    let client = authenticated_client();
    create_blueprint(&client, &base, ITEM).await;
    let named = |name: &str| json!([scalar("name", name)]);
    let absent = create_entity_with(&client, &base, "presence_item", named("Absent")).await;
    let attached = create_entity_with(&client, &base, "presence_item", named("Attached")).await;
    let cleared = create_entity_with(&client, &base, "presence_item", named("Cleared")).await;
    let mut file_ids = Vec::new();
    for entity in [&attached, &cleared] {
        let uploaded = expect_status(
            client
                .post(format!(
                    "{base}/entities/{}/file-attributes/files/uploads",
                    entity["id"].as_str().unwrap()
                ))
                .multipart(
                    reqwest::multipart::Form::new().part(
                        "file",
                        reqwest::multipart::Part::bytes(b"notes".to_vec())
                            .file_name("notes.txt")
                            .mime_str("text/plain")
                            .unwrap(),
                    ),
                )
                .send()
                .await
                .unwrap(),
            StatusCode::CREATED,
        )
        .await;
        file_ids.push(uploaded["files"][0]["id"].clone());
    }
    // Clearing every file leaves an active file value with no references.
    expect_status(
        client
            .put(format!(
                "{base}/entities/{}/file-attributes/files/references",
                cleared["id"].as_str().unwrap()
            ))
            .json(&json!({"expected_file_ids":[file_ids[1]],"file_ids":[]}))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("files", true), None).await,
        &[&attached],
    );
    assert_ids(
        &search(&client, &base, presence("files", false), None).await,
        &[&absent, &cleared],
    );
    let child = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Child"), relationship("parent", &attached)]),
    )
    .await;
    assert_ids(
        &search(
            &client,
            &base,
            json!([
                {"field":"parent.files","operator":"is_set","value":true}
            ]),
            None,
        )
        .await,
        &[&child],
    );
    for operator in ["eq", "contains"] {
        let response = client
            .post(format!("{base}/v1/entities/search"))
            .json(&json!({"blueprint":{"code":"presence_item"},"filters":[{"field":"files","operator":operator,"value":"notes.txt"}]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    server.abort();
}

#[sqlx::test]
async fn presence_traverses_paths_and_preserves_version_and_blueprint_scope(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    let first = create_blueprint(&client, &base, ITEM).await;
    let absent = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Absent")]),
    )
    .await;
    let present = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Present"), scalar("amount", 0)]),
    )
    .await;
    let child = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Child"), relationship("parent", &present)]),
    )
    .await;
    let empty_child = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([
            scalar("name", "Empty child"),
            relationship("parent", &absent)
        ]),
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("parent.amount", true), None).await,
        &[&child],
    );
    assert_ids(
        &search(&client, &base, presence("parent.amount", false), None).await,
        &[&absent, &present, &empty_child],
    );
    let other = create_blueprint(
        &client,
        &base,
        &ITEM.replace("presence_item", "other_family"),
    )
    .await;
    create_entity(&client, &base, &other).await;
    assert_ids(
        &search(&client, &base, presence("amount", false), None).await,
        &[&absent, &child, &empty_child],
    );
    let id = first["blueprint"]["id"].as_str().unwrap();
    let definition = ITEM.replace(
        "code = 'amount'\nvalue_type = 'number'",
        "code = 'amount'\nvalue_type = 'string'",
    ) + "\n[[attributes]]\ncode = 'new_field'\nvalue_type = 'string'\n";
    let draft = expect_status(
        client
            .post(format!("{base}/blueprints/{id}/versions"))
            .json(&json!({"definition":definition}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    let version = draft["blueprint"]["version"].as_i64().unwrap();
    expect_status(
        client
            .post(format!("{base}/blueprints/{id}/versions/{version}/publish"))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    let newer = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([
            scalar("name", "New"),
            scalar("amount", ""),
            scalar("new_field", "present")
        ]),
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("amount", true), None).await,
        &[&present, &newer],
    );
    assert_ids(
        &search(&client, &base, presence("amount", true), Some(1)).await,
        &[&present],
    );
    assert_ids(
        &search(&client, &base, presence("amount", true), Some(version)).await,
        &[&newer],
    );
    assert_ids(
        &search(&client, &base, presence("new_field", false), None).await,
        &[&absent, &present, &child, &empty_child],
    );
    server.abort();
}

#[sqlx::test]
async fn presence_searches_attached_reusable_values_and_round_trips_saved_views(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, ITEM).await;
    let absent = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Unattached")]),
    )
    .await;
    let present = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Attached")]),
    )
    .await;
    let draft=expect_status(client.post(format!("{base}/reusable-attributes")).json(&json!({"definition":"code = 'weight'\nname = 'Weight'\nvalue_type = 'number'\nsearchable = true\ndefault_value = 0"})).send().await.unwrap(),StatusCode::CREATED).await;
    let published = expect_status(
        client
            .post(format!(
                "{base}/reusable-attribute-revisions/{}/publish",
                draft["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .post(format!(
                "{base}/v1/entities/{}/reusable-attributes",
                present["id"].as_str().unwrap()
            ))
            .json(&json!({"reusable_attribute_revision_id":published["id"]}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("default:weight", true), None).await,
        &[&present],
    );
    assert_ids(
        &search(&client, &base, presence("default:weight", false), None).await,
        &[&absent],
    );
    assert_ids(
        &search(
            &client,
            &base,
            json!([
                {"field":"default:weight","operator":"is_set","value":false},
                {"field":"amount","operator":"is_set","value":false}
            ]),
            None,
        )
        .await,
        &[&absent],
    );
    let state = json!({"blueprint":"presence_item","attributeFilters":presence("assignee",false)});
    let view=expect_status(client.post(format!("{base}/saved-views")).json(&json!({"kind":"explorer_search","name":"Unassigned","state":state,"visibility":"workspace"})).send().await.unwrap(),StatusCode::CREATED).await;
    let stored = get_json(
        &client,
        format!("{base}/saved-views/{}", view["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(stored["state"], state);
    let link = expect_status(
        client
            .post(format!("{base}/view-state-links"))
            .json(&json!({"kind":"explorer_search","state":state}))
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    let linked = get_json(
        &client,
        format!("{base}/view-state-links/{}", link["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(linked["state"], state);
    for value in [json!("false"), json!(0)] {
        let bad=client.post(format!("{base}/saved-views")).json(&json!({"kind":"explorer_search","name":"Invalid presence","visibility":"workspace","state":{"blueprint":"presence_item","attributeFilters":[{"field":"assignee","operator":"is_set","value":value}]}})).send().await.unwrap();
        assert_eq!(bad.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    server.abort();
}

#[sqlx::test]
async fn absence_is_scoped_and_paginated_and_requires_no_reachable_value(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let definition = format!(
        "{ITEM}\n[[attributes]]\ncode = 'children'\nvalue_type = 'relationship'\ncardinality = 'many'\ntarget_blueprint = 'presence_item'\n"
    );
    let blueprint = create_blueprint(&client, &base, &definition).await;
    let absent = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Absent")]),
    )
    .await;
    let present = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([scalar("name", "Present"), scalar("amount", 0)]),
    )
    .await;
    let mixed = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([
            scalar("name", "Mixed children"),
            relationship("children", &absent),
            relationship("children", &present)
        ]),
    )
    .await;
    let empty = create_entity_with(
        &client,
        &base,
        "presence_item",
        json!([
            scalar("name", "Empty child"),
            relationship("children", &absent)
        ]),
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("children.amount", true), None).await,
        &[&mixed],
    );
    assert_ids(
        &search(&client, &base, presence("children.amount", false), None).await,
        &[&absent, &present, &empty],
    );
    let mut cursor = Value::Null;
    let mut ids = Vec::new();
    loop {
        assert!(ids.len() < 4, "pagination did not terminate");
        let page=expect_status(client.post(format!("{base}/v1/entities/search"))
            .json(&json!({"blueprint":{"code":"presence_item"},"filters":presence("children.amount",false),"page":{"size":1,"cursor":cursor},"include_total":true}))
            .send().await.unwrap(),StatusCode::OK).await;
        if cursor.is_null() {
            assert_eq!(page["total_count"], 3);
        }
        ids.extend(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_str().unwrap().to_owned()),
        );
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    ids.sort();
    let mut expected = [&absent, &present, &empty]
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .to_vec();
    expected.sort();
    assert_eq!(ids, expected);
    // Exercise repository isolation even when passed another workspace's known blueprint ID.
    let other_workspace = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces(id,slug,name,login_identifier) VALUES($1,'presence-other','Presence other','presence-other.local')")
        .bind(other_workspace).execute(&pool).await.unwrap();
    let system = api::repository::CatalogRepository::system(pool);
    system.initialize_workspace(other_workspace).await.unwrap();
    let other = system.for_workspace(other_workspace).await.unwrap();
    for value in ["true", "false"] {
        let filter = api::repository::EntitySearchFilter {
            field: "amount".into(),
            reusable: false,
            relationship_path: vec![],
            leaf_field: "amount".into(),
            operator: "is_set".into(),
            value_type: "number".into(),
            value: value.into(),
        };
        let ids = other
            .filter_entity_ids(
                Uuid::parse_str(blueprint["blueprint"]["id"].as_str().unwrap()).unwrap(),
                None,
                &[filter],
            )
            .await
            .unwrap();
        assert!(ids.is_empty());
    }
    expect_status(
        client
            .delete(format!(
                "{base}/entities/{}",
                present["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::NO_CONTENT,
    )
    .await;
    assert_ids(
        &search(&client, &base, presence("children.amount", true), None).await,
        &[],
    );
    assert_ids(
        &search(&client, &base, presence("children.amount", false), None).await,
        &[&absent, &mixed, &empty],
    );
    server.abort();
}
