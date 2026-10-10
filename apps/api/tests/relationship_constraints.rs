mod support;

use support::*;

const LOCATION: &str = r#"
format_version = 1
code = "rc_location"
name = "Location"
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
target_blueprint = "rc_location"
tree = true
context_editable = "default"

[[attributes]]
code = "depends_on"
value_type = "relationship"
target_blueprint = "rc_location"
acyclic = true
context_editable = "default"
"#;

async fn record(client: &Client, base_url: &str, blueprint: &str) -> String {
    create_record_with(client, base_url, blueprint, json!([])).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn link(
    client: &Client,
    base_url: &str,
    source: &str,
    field: &str,
    targets: &[&str],
) -> reqwest::Response {
    client
        .post(format!("{base_url}/records/{source}/relationships/replace"))
        .json(&json!({ "relationships": [{ "attribute_code": field, "target_record_ids": targets }] }))
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn hierarchies_reject_writes_that_close_a_cycle(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, LOCATION).await;
    let [a, b, c] = [
        record(&client, &base_url, "rc_location").await,
        record(&client, &base_url, "rc_location").await,
        record(&client, &base_url, "rc_location").await,
    ];

    for (source, target) in [(&a, &b), (&b, &c)] {
        assert_eq!(
            link(&client, &base_url, source, "parent", &[target])
                .await
                .status(),
            StatusCode::CREATED
        );
    }
    let cycle = link(&client, &base_url, &c, "parent", &[&a]).await;
    assert_eq!(cycle.status(), StatusCode::CONFLICT);
    let body: Value = cycle.json().await.unwrap();
    assert_eq!(body["error"]["code"], "relationship_cycle");
    assert_eq!(body["error"]["details"]["attribute"], "parent");
    assert_eq!(body["error"]["details"]["path"], json!([c, a, b, c]));

    let own_parent = link(&client, &base_url, &a, "parent", &[&a]).await;
    assert_eq!(own_parent.status(), StatusCode::CONFLICT);
    assert_eq!(
        own_parent.json::<Value>().await.unwrap()["error"]["details"]["path"],
        json!([a, a])
    );

    // Moving a subtree is fine as long as no cycle forms.
    assert_eq!(
        link(&client, &base_url, &a, "parent", &[&c]).await.status(),
        StatusCode::CREATED
    );

    // Acyclic many-to-many: diamonds are allowed, closing a loop is not.
    let d = record(&client, &base_url, "rc_location").await;
    for (source, targets) in [
        (&a, vec![b.as_str(), c.as_str()]),
        (&b, vec![&d]),
        (&c, vec![&d]),
    ] {
        assert_eq!(
            link(&client, &base_url, source, "depends_on", &targets)
                .await
                .status(),
            StatusCode::CREATED
        );
    }
    let loop_back = link(&client, &base_url, &d, "depends_on", &[&a]).await;
    assert_eq!(loop_back.status(), StatusCode::CONFLICT);
    let path = loop_back.json::<Value>().await.unwrap()["error"]["details"]["path"].clone();
    assert_eq!(path[0], json!(d));
    assert_eq!(path[1], json!(a));
    assert_eq!(path.as_array().unwrap().last(), Some(&json!(d)));

    // Record creation applies the same check.
    let created = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({ "blueprint": { "code": "rc_location" }, "values": [
            {"kind": "relationship", "attribute_code": "parent", "context_id": null, "target_record_id": a},
        ] }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    server.abort();
}

#[sqlx::test]
async fn concurrent_writes_cannot_jointly_create_a_cycle(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url, LOCATION).await;
    for _ in 0..5 {
        let a = record(&client, &base_url, "rc_location").await;
        let b = record(&client, &base_url, "rc_location").await;
        let mut writes = tokio::task::JoinSet::new();
        for (source, target) in [(a.clone(), b.clone()), (b.clone(), a.clone())] {
            let client = client.clone();
            let base_url = base_url.clone();
            writes.spawn(async move {
                link(&client, &base_url, &source, "depends_on", &[&target])
                    .await
                    .status()
            });
        }
        let mut statuses = writes.join_all().await;
        statuses.sort();
        assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CONFLICT]);
    }
    server.abort();
}

#[sqlx::test]
async fn publishing_a_hierarchy_reports_existing_cycles_and_extra_parents(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let unconstrained = LOCATION
        .replace("tree = true\n", "")
        .replace("acyclic = true\n", "");
    let blueprint = create_blueprint(&client, &base_url, &unconstrained).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();
    let [a, b, c] = [
        record(&client, &base_url, "rc_location").await,
        record(&client, &base_url, "rc_location").await,
        record(&client, &base_url, "rc_location").await,
    ];
    link(&client, &base_url, &a, "parent", &[&b, &c])
        .await
        .error_for_status()
        .unwrap();
    for (source, target) in [(&a, &b), (&b, &a)] {
        link(&client, &base_url, source, "depends_on", &[target])
            .await
            .error_for_status()
            .unwrap();
    }

    let publish = |definition: String| {
        let client = client.clone();
        let base_url = base_url.clone();
        let blueprint_id = blueprint_id.clone();
        async move {
            let revision: Value = client
                .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
                .json(&json!({ "definition": definition }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            client
                .post(format!(
                    "{base_url}/blueprints/{blueprint_id}/versions/{}/publish",
                    revision["blueprint"]["version"]
                ))
                .send()
                .await
                .unwrap()
        }
    };

    let tree = publish(LOCATION.replace("acyclic = true\n", "")).await;
    assert_eq!(tree.status(), StatusCode::CONFLICT);
    let body: Value = tree.json().await.unwrap();
    assert_eq!(body["error"]["code"], "relationship_hierarchy_violations");
    assert_eq!(body["error"]["details"]["attribute"], "parent");
    assert_eq!(body["error"]["details"]["multiple_parents"], json!([a]));

    let acyclic = publish(LOCATION.replace("tree = true\n", "")).await;
    assert_eq!(acyclic.status(), StatusCode::CONFLICT);
    let body: Value = acyclic.json().await.unwrap();
    assert_eq!(body["error"]["details"]["attribute"], "depends_on");
    let cycle = body["error"]["details"]["cycles"][0]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(cycle.len(), 3);
    assert_eq!(cycle.first(), cycle.last());

    link(&client, &base_url, &b, "depends_on", &[])
        .await
        .error_for_status()
        .unwrap();
    link(&client, &base_url, &a, "parent", &[&b])
        .await
        .error_for_status()
        .unwrap();
    publish(LOCATION.to_owned())
        .await
        .error_for_status()
        .unwrap();
    // Records pinned to revision 1 now follow the family's hierarchy.
    let pinned = link(&client, &base_url, &b, "parent", &[&a]).await;
    assert_eq!(pinned.status(), StatusCode::CONFLICT);
    let pinned_second_parent = link(&client, &base_url, &c, "parent", &[&a, &b]).await;
    assert_eq!(pinned_second_parent.status(), StatusCode::CONFLICT);

    server.abort();
}

async fn link_in(
    client: &Client,
    base_url: &str,
    source: &str,
    field: &str,
    context_id: &str,
    targets: &[&str],
) -> reqwest::Response {
    client
        .post(format!("{base_url}/records/{source}/relationships/replace"))
        .json(&json!({ "relationships": [{
            "attribute_code": field, "context_id": context_id, "target_record_ids": targets,
        }] }))
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn hierarchies_follow_edges_inherited_from_parent_contexts(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    // Revision 1 lets records set the field per context; revision 2 makes it
    // a hierarchy, which records pinned to revision 1 must still respect.
    let contextual = LOCATION
        .replace("tree = true\n", "")
        .replace("acyclic = true\ncontext_editable = \"default\"\n", "");
    let blueprint = create_blueprint(&client, &base_url, &contextual).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();
    let french: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({ "code": "rc-fr", "data": {} }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let fr = french["id"].as_str().unwrap();
    let [a, b] = [
        record(&client, &base_url, "rc_location").await,
        record(&client, &base_url, "rc_location").await,
    ];
    // a -> b in default, b -> a only in fr: fr inherits a -> b.
    link(&client, &base_url, &a, "depends_on", &[&b])
        .await
        .error_for_status()
        .unwrap();
    link_in(&client, &base_url, &b, "depends_on", fr, &[&a])
        .await
        .error_for_status()
        .unwrap();

    let revision: Value = client
        .post(format!("{base_url}/blueprints/{blueprint_id}/versions"))
        .json(&json!({ "definition": LOCATION.replace("tree = true\n", "") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let publish_url = format!(
        "{base_url}/blueprints/{blueprint_id}/versions/{}/publish",
        revision["blueprint"]["version"]
    );
    let publish = || client.post(&publish_url).send();
    let rejected = publish().await.unwrap();
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let body: Value = rejected.json().await.unwrap();
    assert_eq!(body["error"]["code"], "relationship_hierarchy_violations");
    let cycles = body["error"]["details"]["cycles"].as_array().unwrap();
    assert_eq!(cycles.len(), 1, "the fr cycle is reported once");
    let mut members = cycles[0].as_array().unwrap().clone();
    assert_eq!(members.first(), members.last());
    members.pop();
    members.sort_by_key(|id| id.as_str().unwrap().to_owned());
    let mut expected = vec![json!(a), json!(b)];
    expected.sort_by_key(|id| id.as_str().unwrap().to_owned());
    assert_eq!(members, expected);

    link_in(&client, &base_url, &b, "depends_on", fr, &[])
        .await
        .error_for_status()
        .unwrap();
    publish().await.unwrap().error_for_status().unwrap();

    // b is still pinned to revision 1, so it may write in fr, but the edge
    // would close a cycle with the inherited a -> b.
    let cycle = link_in(&client, &base_url, &b, "depends_on", fr, &[&a]).await;
    assert_eq!(cycle.status(), StatusCode::CONFLICT);
    let body: Value = cycle.json().await.unwrap();
    assert_eq!(body["error"]["code"], "relationship_cycle");
    assert_eq!(body["error"]["details"]["path"], json!([b, a, b]));

    // A local fr edge of a overrides the inherited one, so no cycle forms.
    let c = record(&client, &base_url, "rc_location").await;
    link_in(&client, &base_url, &a, "depends_on", fr, &[&c])
        .await
        .error_for_status()
        .unwrap();
    assert_eq!(
        link_in(&client, &base_url, &b, "depends_on", fr, &[&a])
            .await
            .status(),
        StatusCode::CREATED
    );

    server.abort();
}

#[sqlx::test]
async fn relationships_accept_only_listed_target_blueprints(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    for code in ["rc_product", "rc_material", "rc_supplier"] {
        create_blueprint(
            &client,
            &base_url,
            &format!(
                r#"
format_version = 1
code = "{code}"
name = "{code}"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
            ),
        )
        .await;
    }
    let assessment = create_blueprint(
        &client,
        &base_url,
        r#"
format_version = 1
code = "rc_assessment"
name = "Assessment"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "subject"
value_type = "relationship"
target_blueprints = ["rc_product", "rc_material"]
"#,
    )
    .await;
    let subject = assessment["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attribute| attribute["code"] == "subject")
        .unwrap();
    assert_eq!(subject["target_blueprint_code"], Value::Null);
    assert_eq!(
        subject["target_blueprint_codes"],
        json!(["rc_product", "rc_material"])
    );

    let product = record(&client, &base_url, "rc_product").await;
    let material = record(&client, &base_url, "rc_material").await;
    let supplier = record(&client, &base_url, "rc_supplier").await;
    let source = record(&client, &base_url, "rc_assessment").await;
    assert_eq!(
        link(
            &client,
            &base_url,
            &source,
            "subject",
            &[&product, &material]
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    let rejected = link(&client, &base_url, &source, "subject", &[&supplier]).await;
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        rejected.json::<Value>().await.unwrap()["error"]["code"],
        "relationship_target_type_mismatch"
    );
    let rejected_create = client
        .post(format!("{base_url}/v1/records"))
        .json(&json!({ "blueprint": { "code": "rc_assessment" }, "values": [
            {"kind": "relationship", "attribute_code": "subject", "context_id": null, "target_record_id": supplier},
        ] }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected_create.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Incoming-relationship lists work from every allowed target.
    for target in [&product, &material] {
        let incoming: Value = client
            .post(format!(
                "{base_url}/v1/records/{target}/incoming-relationships"
            ))
            .json(&json!({
                "relationships": [{ "source_blueprint": "rc_assessment", "field": "subject" }],
                "page": { "size": 10 },
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(incoming["items"][0]["id"], json!(source));
    }

    server.abort();
}
