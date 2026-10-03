mod support;
use support::*;

const DEFINITION: &str = r#"format_version = 1
code = 'task'
name = 'Task'
kind = 'entity'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['title']
[[attributes]]
code = 'title'
value_type = 'string'
[[attributes]]
code = 'assignee'
value_type = 'string'
value_schema = '''{"type":"string","x-attricat-principal":{"version":1,"kinds":["user","team"]}}'''
[[attributes]]
code = 'owner'
value_type = 'string'
value_schema = '''{"type":"string","x-attricat-principal":{"version":1,"kinds":["user"]}}'''
"#;

async fn add_member(pool: &PgPool, email: &str, state: &str) -> Uuid {
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, display_name) VALUES ($1, $2, 'Member')")
        .bind(user)
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id, state) VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(user)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();
    user
}

fn scalar(code: &str, value: impl Into<Value>) -> Value {
    json!({"kind": "scalar", "attribute_code": code, "value": value.into()})
}

#[sqlx::test]
async fn assignment_attributes_reference_members_and_teams(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let member = add_member(&pool, "member@example.test", "active").await;
    let former = add_member(&pool, "former@example.test", "inactive").await;

    // Teams: member managers create them from workspace members only.
    let teams = format!("{base}/workspace/teams");
    let invalid = client
        .post(&teams)
        .json(&json!({"code": "qa", "name": "QA", "member_user_ids": [Uuid::new_v4()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let team: Value = client
        .post(&teams)
        .json(&json!({"code": "qa", "name": " Quality ", "member_user_ids": [owner]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(team["name"], "Quality");
    assert_eq!(team["member_user_ids"], json!([owner]));
    let team_id = team["id"].as_str().unwrap().to_owned();
    let duplicate = client
        .post(&teams)
        .json(&json!({"code": "qa", "name": "Again"}))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let empty: Value = client
        .post(&teams)
        .json(&json!({"code": "ops", "name": "Operations"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let renamed: Value = client
        .patch(format!("{teams}/{}", empty["id"].as_str().unwrap()))
        .json(&json!({"name": "Ops", "member_user_ids": [member]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(renamed["name"], "Ops");
    assert_eq!(renamed["member_user_ids"], json!([member]));

    let directory: Value = client
        .get(format!("{base}/directory"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let former_entry = directory["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["id"] == json!(former))
        .unwrap();
    assert_eq!(former_entry["active"], false);
    assert_eq!(directory["teams"].as_array().unwrap().len(), 2);

    create_blueprint(&client, &base, DEFINITION).await;
    let create = |values: Vec<Value>| {
        client
            .post(format!("{base}/v1/entities"))
            .json(&json!({"blueprint": {"code": "task"}, "values": values}))
            .send()
    };
    for (code, value) in [
        ("assignee", format!("user:{}", Uuid::new_v4())),
        ("assignee", format!("user:{former}")),
        ("assignee", format!("team:{}", Uuid::new_v4())),
        ("assignee", "alice".to_owned()),
        ("owner", format!("team:{team_id}")),
    ] {
        let response = create(vec![scalar(code, value.clone())]).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{code} = {value}"
        );
    }
    let mut ids = Vec::new();
    for (title, assignee) in [
        ("mine", format!("user:{owner}")),
        ("my team", format!("team:{team_id}")),
        ("theirs", format!("user:{member}")),
    ] {
        let entity: Value = create(vec![
            scalar("title", title),
            scalar("assignee", assignee.clone()),
        ])
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
        ids.push(entity["id"].as_str().unwrap().to_owned());
    }

    let search = |value: &str| {
        client
            .post(format!("{base}/v1/entities/search"))
            .json(&json!({
                "blueprint": {"code": "task"},
                "filters": [{"field": "assignee", "operator": "eq", "value": value}],
            }))
            .send()
    };
    let found = |body: Value| {
        let mut ids: Vec<String> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    };
    let mine: Value = search("@me")
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut expected = vec![ids[0].clone(), ids[1].clone()];
    expected.sort();
    assert_eq!(found(mine), expected);
    let exact: Value = search(&format!("user:{member}"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(found(exact), vec![ids[2].clone()]);

    // A deleted team stays readable on existing records, which can still be
    // edited, but cannot be newly assigned.
    assert_eq!(
        client
            .delete(format!("{teams}/{team_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let entity_url = format!("{base}/v1/entities/{}", ids[1]);
    let entity: Value = client
        .get(&entity_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let edited = client
        .put(&entity_url)
        .json(&json!({
            "expected_updated_at": entity["entity"]["updated_at"],
            "values": [scalar("title", "renamed")],
        }))
        .send()
        .await
        .unwrap();
    assert!(
        edited.status().is_success(),
        "{}",
        edited.text().await.unwrap()
    );
    let reassigned = create(vec![scalar("assignee", format!("team:{team_id}"))])
        .await
        .unwrap();
    assert_eq!(reassigned.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn assignment_definitions_are_validated(pool: PgPool) {
    let (base, _server) = start_server(pool).await;
    let client = authenticated_client();
    for (from, to) in [
        (r#""kinds":["user","team"]"#, r#""kinds":["group"]"#),
        (
            r#"{"type":"string","x-attricat-principal""#,
            r#"{"type":"string","enum":["a"],"x-attricat-principal""#,
        ),
        (
            "code = 'assignee'\nvalue_type = 'string'",
            "code = 'assignee'\nvalue_type = 'json'",
        ),
        (
            "code = 'owner'\nvalue_type = 'string'",
            "code = 'owner'\nvalue_type = 'string'\ndefault_value = 'user:00000000-0000-4000-8000-000000000201'",
        ),
    ] {
        let definition = DEFINITION.replacen(from, to, 1);
        assert_ne!(definition, DEFINITION, "{from}");
        let response = client
            .post(format!("{base}/blueprints"))
            .json(&json!({"definition": definition}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{to}: {}",
            response.text().await.unwrap()
        );
    }
}
