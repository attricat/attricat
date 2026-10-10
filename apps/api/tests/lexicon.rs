mod support;
use support::*;

const BLUEPRINT: &str = r#"
format_version = 1
code = "product"
name = "{{Product}}"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.detail]
type = "tabs"

[[views.detail.tabs]]
label = "{{Overview}}"

[[views.detail.tabs.children]]
type = "field"
field = "title"

[[attributes]]
code = "title"
name = "{{Name}}"
value_type = "string"
"#;

async fn json_response(response: reqwest::Response, status: u16) -> Value {
    assert_eq!(response.status(), status);
    response.json().await.unwrap()
}

async fn viewer_client(pool: &PgPool) -> Client {
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id,email,display_name) VALUES ($1,'lexicon-viewer@example.test','Reader')")
        .bind(user).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO workspace_memberships (id,workspace_id,user_id) VALUES ($1,$2,$3)")
        .bind(membership)
        .bind(workspace)
        .bind(user)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_grants (id,workspace_id,membership_id,role_id,scope_type,scope_target_id) VALUES ($1,$2,$3,'00000000-0000-4000-8000-000000000104','workspace',$2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(membership).execute(pool).await.unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-catalog-user-id", user.to_string().parse().unwrap());
    headers.insert(
        "x-catalog-workspace-id",
        workspace.to_string().parse().unwrap(),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn blueprints_reject_malformed_lexicon_references(pool: PgPool) {
    let (base, _server) = start_server(pool).await;
    let client = authenticated_client();
    let response = client
        .post(format!("{base}/blueprints"))
        .json(&json!({"definition": BLUEPRINT.replace("{{Overview}}", "{{Overview")}))
        .send()
        .await
        .unwrap();
    let body = json_response(response, 422).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid lexicon reference"),
        "{body}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn lexicon_entries_round_trip_and_report_coverage(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base, BLUEPRINT).await;
    let entries = format!("{base}/lexicon/entries");

    let stored = json_response(
        client
            .put(&entries)
            .json(&json!({"key": " Product ", "language": "PL", "plural_category": "one", "text": "Produkt"}))
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    assert_eq!(stored["key"], "Product");
    assert_eq!(stored["language"], "pl");
    assert_eq!(stored["context"], Value::Null);
    assert_eq!(stored["source"], "workspace");
    for invalid in [
        json!({"key": "Product", "language": "en", "plural_category": "few", "text": "x"}),
        json!({"key": "Product", "language": "xx", "text": "x"}),
        json!({"key": "{{Product}}", "language": "pl", "text": "x"}),
        json!({"key": "Product", "language": "pl", "text": "  "}),
    ] {
        assert_eq!(
            client
                .put(&entries)
                .json(&invalid)
                .send()
                .await
                .unwrap()
                .status(),
            422,
            "{invalid}"
        );
    }

    let imported = json_response(
        client
            .post(format!("{base}/lexicon/import"))
            .json(&json!({
                "format_version": 1,
                "language": "pl",
                "entries": [
                    {"key": "Product", "plural_category": "one", "text": "Produkt"},
                    {"key": "Product", "plural_category": "few", "text": "Produkty"},
                    {"key": "Name", "text": "Nazwa"},
                    {"key": "Order", "context": "sorting", "text": "Kolejność"},
                ],
            }))
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    assert_eq!(
        imported,
        json!({"created": 3, "updated": 0, "unchanged": 1, "deleted": 0})
    );

    let listed = json_response(
        client
            .get(&entries)
            .query(&[("language", "pl")])
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    let keys: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            format!(
                "{}/{}",
                entry["key"].as_str().unwrap(),
                entry["plural_category"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        keys,
        ["Name/other", "Order/other", "Product/one", "Product/few"]
    );

    let report = json_response(
        client
            .get(format!("{base}/lexicon/report"))
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    assert_eq!(report["reference_count"], 3);
    let pl = report["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "pl")
        .unwrap();
    assert_eq!(
        pl["untranslated"],
        json!([{"key": "Overview", "context": null}])
    );
    assert_eq!(
        pl["missing_plural_categories"],
        json!([{"key": "Product", "context": null, "missing": ["many", "other"]}])
    );
    let en = report["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "en")
        .unwrap();
    assert_eq!(en["untranslated"], json!([]));
    assert_eq!(
        en["missing_plural_categories"],
        json!([{"key": "Product", "context": null, "missing": ["one", "other"]}])
    );
    assert_eq!(
        report["orphaned"],
        json!([{"key": "Order", "context": "sorting", "languages": ["pl"]}])
    );

    // Replace makes the language match the file exactly.
    let replaced = json_response(
        client
            .post(format!("{base}/lexicon/import"))
            .query(&[("mode", "replace")])
            .json(&json!({
                "format_version": 1,
                "language": "pl",
                "entries": [{"key": "Name", "text": "Nazwa produktu"}],
            }))
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    assert_eq!(
        replaced,
        json!({"created": 0, "updated": 1, "unchanged": 0, "deleted": 3})
    );
    let exported = json_response(
        client
            .get(format!("{base}/lexicon/export"))
            .query(&[("language", "pl")])
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    assert_eq!(
        exported,
        json!({
            "format_version": 1,
            "language": "pl",
            "entries": [{"key": "Name", "text": "Nazwa produktu"}],
        })
    );

    // Readers load translations but cannot change them.
    let viewer = viewer_client(&pool).await;
    assert_eq!(viewer.get(&entries).send().await.unwrap().status(), 200);
    assert_eq!(
        viewer
            .put(&entries)
            .json(&json!({"key": "Name", "language": "pl", "text": "Imię"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    let identity = [("key", "Name"), ("language", "pl")];
    assert_eq!(
        client
            .delete(&entries)
            .query(&identity)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .delete(&entries)
            .query(&identity)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

const STATUS_SCHEMA: &str = r#"{
  "type": "string",
  "enum": ["draft", "live"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "{{Draft|status}}" },
      { "code": "live", "label": "{{Live}}", "tone": "success" }
    ]
  }
}"#;

#[sqlx::test(migrations = "./migrations")]
async fn status_option_labels_are_lexicon_references(pool: PgPool) {
    let (base, _server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = format!(
        "{BLUEPRINT}\n[[attributes]]\ncode = \"status\"\nvalue_type = \"string\"\nvalue_schema = '''{STATUS_SCHEMA}'''\n"
    );
    let response = client
        .post(format!("{base}/blueprints"))
        .json(&json!({"definition": blueprint.replace("{{Live}}", "{{Live")}))
        .send()
        .await
        .unwrap();
    let body = json_response(response, 422).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("status option 'live' label"),
        "{body}"
    );
    create_blueprint(&client, &base, &blueprint).await;

    // Reusable definitions declare the schema as a TOML table.
    let reusable = |draft: &str, live: &str| {
        format!(
            "code = \"review_state\"\nname = \"{{{{Review state}}}}\"\nvalue_type = \"string\"\n\
             [value_schema]\ntype = \"string\"\nenum = [\"draft\", \"live\"]\n\
             [value_schema.x-attricat-status]\nversion = 1\noptions = [\
             {{ code = \"draft\", label = \"{draft}\" }}, {{ code = \"live\", label = \"{live}\" }}]\n"
        )
    };
    for (draft, status) in [("{{|status}}", 422), ("{{Draft|status}}", 201)] {
        let response = client
            .post(format!("{base}/reusable-attributes"))
            .json(&json!({"definition": reusable(draft, "{{Approved}}")}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            status,
            "{}",
            response.text().await.unwrap()
        );
    }

    let report = json_response(
        client
            .get(format!("{base}/lexicon/report"))
            .query(&[("languages", "pl")])
            .send()
            .await
            .unwrap(),
        200,
    )
    .await;
    let pl = report["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "pl")
        .unwrap();
    let untranslated = pl["untranslated"].as_array().unwrap();
    for (key, context) in [
        ("Draft", json!("status")),
        ("Live", Value::Null),
        ("Approved", Value::Null),
        ("Review state", Value::Null),
    ] {
        assert!(
            untranslated.contains(&json!({"key": key, "context": context})),
            "{key}: {untranslated:?}"
        );
    }
}
