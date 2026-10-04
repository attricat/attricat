mod support;

use std::sync::Arc;

use api::storage::FakeObjectStore;
use reqwest::multipart::{Form, Part};
use support::*;

const DEFINITION: &str = r#"format_version = 1
code = 'controlled_document'
name = 'Controlled document'
kind = 'entity'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['title']
[[attributes]]
code = 'title'
value_type = 'string'
[[attributes]]
code = 'notes'
value_type = 'string'
[[attributes]]
code = 'document'
value_type = 'file'
allowed_mime_groups = ['text/plain']
allowed_extensions = ['txt']
max_bytes = 1024
[[attributes]]
code = 'status'
value_type = 'string'
value_schema = '''{"type":"string","enum":["draft","review","approved","released"],"x-attricat-status":{"version":1,
  "options":[
    {"code":"draft","label":"Draft"},
    {"code":"review","label":"In review"},
    {"code":"approved","label":"Approved","approval":{"covers":["title","document"],"void_to":"review"}},
    {"code":"released","label":"Released","lock":"all","retention_days":30}
  ],
  "transitions":[
    {"from":null,"to":"draft"},
    {"from":"draft","to":"review","code":"submit"},
    {"from":"review","to":"approved","code":"approve","roles":["reviewer"],"separate_from":["submit"]},
    {"from":"approved","to":"released","code":"release","permission":"entities.publish"},
    {"from":"released","to":"draft","code":"correct","roles":["owner"]}
  ]}}'''
"#;

async fn reviewer_role(pool: &PgPool) -> Uuid {
    create_role(pool, "reviewer", &["entities.read", "entities.write"]).await
}

struct Record {
    url: String,
}

impl Record {
    async fn read(&self, client: &Client) -> Value {
        get_json(client, &self.url).await
    }

    async fn put(&self, client: &Client, values: Value) -> reqwest::Response {
        let current = self.read(client).await;
        client
            .put(&self.url)
            .json(
                &json!({"expected_updated_at": current["entity"]["updated_at"], "values": values}),
            )
            .send()
            .await
            .unwrap()
    }

    async fn set_status(&self, client: &Client, status: &str) -> reqwest::Response {
        self.put(client, json!([scalar("status", status)])).await
    }

    async fn status(&self, client: &Client) -> Value {
        self.read(client).await["entity"]["projections"]["preview"]["default"]["status"].clone()
    }
}

async fn setup(pool: &PgPool) -> (String, JoinHandle<()>, Arc<FakeObjectStore>, Record) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    create_blueprint(&owner, &base, DEFINITION).await;
    let entity = create_entity_with(
        &owner,
        &base,
        "controlled_document",
        json!([scalar("status", "draft"), scalar("title", "Procedure")]),
    )
    .await;
    let url = format!("{base}/v1/entities/{}", entity["id"].as_str().unwrap());
    (base, server, store, Record { url })
}

async fn upload(client: &Client, base: &str, record: &Record, name: &str) -> reqwest::Response {
    let entity_id = record.url.rsplit('/').next().unwrap();
    client
        .post(format!(
            "{base}/entities/{entity_id}/file-attributes/document/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"controlled text".to_vec())
                    .file_name(name.to_owned())
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
}

#[sqlx::test]
async fn transitions_enforce_roles_permissions_and_separation_of_duties(pool: PgPool) {
    let (_, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer_role = reviewer_role(&pool).await;
    let first_reviewer = client_for(member_with_role(&pool, reviewer_role).await);
    let second_reviewer = client_for(member_with_role(&pool, reviewer_role).await);
    let editor = client_for(member_with_role(&pool, EDITOR_ROLE_ID).await);

    expect_status(
        record.set_status(&first_reviewer, "review").await,
        StatusCode::OK,
    )
    .await;
    // The owner lacks the reviewer role; the submitter is barred by separation of duties.
    let denied = expect_error(
        record.set_status(&owner, "approved").await,
        StatusCode::FORBIDDEN,
        "status_transition_forbidden",
    )
    .await;
    assert!(
        denied["error"]["message"]
            .as_str()
            .unwrap()
            .contains("'reviewer'")
    );
    expect_error(
        record.set_status(&first_reviewer, "approved").await,
        StatusCode::FORBIDDEN,
        "status_separation_of_duties",
    )
    .await;

    // The status control learns the same outcome before the user saves.
    let access: Value = first_reviewer
        .get(format!("{}/status-transitions", record.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let approve = access["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["to"] == "approved")
        .unwrap();
    assert_eq!(approve["allowed"], false);
    assert_eq!(approve["code"], "approve");
    assert_eq!(approve["denial_code"], "status_separation_of_duties");

    expect_status(
        record.set_status(&second_reviewer, "approved").await,
        StatusCode::OK,
    )
    .await;
    // Releasing requires entities.publish, which editors do not hold.
    expect_error(
        record.set_status(&editor, "released").await,
        StatusCode::FORBIDDEN,
        "status_transition_forbidden",
    )
    .await;
    expect_status(record.set_status(&owner, "released").await, StatusCode::OK).await;
    assert_eq!(record.status(&owner).await, "released");
    let actors: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT edge_code, to_status FROM entity_status_transitions ORDER BY occurred_at, to_status",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        actors
            .iter()
            .filter_map(|(edge, _)| edge.as_deref())
            .collect::<Vec<_>>(),
        ["submit", "approve", "release"]
    );
    server.abort();
}

#[sqlx::test]
async fn locked_records_reject_writes_until_an_audited_correction(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    expect_status(
        upload(&owner, &base, &record, "v1.txt").await,
        StatusCode::CREATED,
    )
    .await;
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect_status(record.set_status(client, status).await, StatusCode::OK).await;
    }
    let title = |value: &str| json!([scalar("title", value)]);
    expect_error(
        record.put(&owner, title("Changed")).await,
        StatusCode::CONFLICT,
        "record_locked",
    )
    .await;
    // Re-saving identical values is not a change.
    expect_status(record.put(&owner, title("Procedure")).await, StatusCode::OK).await;
    expect_error(
        upload(&owner, &base, &record, "v2.txt").await,
        StatusCode::CONFLICT,
        "record_locked",
    )
    .await;
    let entity_id = record.url.rsplit('/').next().unwrap();
    expect_error(
        owner
            .delete(format!("{base}/entities/{entity_id}"))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        "record_locked",
    )
    .await;
    // A correction must be a pure status change.
    expect_error(
        record
            .put(
                &owner,
                json!([scalar("status", "draft"), scalar("title", "Changed")]),
            )
            .await,
        StatusCode::CONFLICT,
        "record_locked",
    )
    .await;
    expect_error(
        record.set_status(&reviewer, "draft").await,
        StatusCode::FORBIDDEN,
        "status_transition_forbidden",
    )
    .await;
    expect_status(record.set_status(&owner, "draft").await, StatusCode::OK).await;
    expect_status(record.put(&owner, title("Changed")).await, StatusCode::OK).await;
    let unlocks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE action = 'entity.record.unlock' AND target->>'entity_id' = $1",
    )
    .bind(entity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unlocks, 1);
    let unlocked: bool = sqlx::query_scalar(
        "SELECT unlocked FROM entity_status_transitions WHERE edge_code = 'correct'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(unlocked);
    server.abort();
}

#[sqlx::test]
async fn approvals_bind_to_content_and_void_on_change(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    expect_status(record.set_status(&owner, "review").await, StatusCode::OK).await;
    expect_status(
        record.set_status(&reviewer, "approved").await,
        StatusCode::OK,
    )
    .await;
    let approvals = |client: Client| {
        let url = format!("{}/approvals", record.url);
        async move {
            client
                .get(url)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["items"]
                .clone()
        }
    };
    let recorded = approvals(owner.clone()).await;
    assert_eq!(recorded.as_array().unwrap().len(), 1);
    assert_eq!(recorded[0]["status"], "approved");
    assert_eq!(recorded[0]["end_reason"], Value::Null);
    assert_eq!(recorded[0]["content_digest"].as_str().unwrap().len(), 64);
    // Content outside the approval's coverage does not void it.
    expect_status(
        record
            .put(&owner, json!([scalar("notes", "Typo list")]))
            .await,
        StatusCode::OK,
    )
    .await;
    assert_eq!(record.status(&owner).await, "approved");
    // Covered content, here a file upload, voids the approval and returns the
    // record to review atomically, although no approved -> review edge is
    // declared: the void is a system transition, not validated or guarded
    // like the write's own.
    expect_status(
        upload(&owner, &base, &record, "late.txt").await,
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(record.status(&owner).await, "review");
    expect_status(
        record
            .put(&owner, json!([scalar("title", "Procedure v2")]))
            .await,
        StatusCode::OK,
    )
    .await;
    assert_eq!(record.status(&owner).await, "review");
    let voided = approvals(owner.clone()).await;
    assert_eq!(voided[0]["end_reason"], "content_changed");
    assert_eq!(voided[0]["void_status"], "review");
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_events WHERE action LIKE 'entity.approval.%' ORDER BY occurred_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions, ["entity.approval.record", "entity.approval.void"]);
    server.abort();
}

const VOIDABLE: &str = r#"format_version = 1
code = 'voidable_document'
name = 'Voidable document'
kind = 'entity'
[views.dropdown_option]
type = 'dropdown_option'
fields = ['title']
[[attributes]]
code = 'title'
value_type = 'string'
[[attributes]]
code = 'reason'
value_type = 'string'
[[attributes]]
code = 'status'
value_type = 'string'
value_schema = '''{"type":"string","enum":["draft","approved"],"x-attricat-status":{"version":1,
  "options":[
    {"code":"draft","label":"Draft"},
    {"code":"approved","label":"Approved","approval":{"covers":["title"],"void_to":"draft"}}
  ],
  "transitions":[
    {"from":null,"to":"draft"},
    {"from":"draft","to":"approved"},
    {"from":"approved","to":"draft","conditions":[
      {"code":"reason","predicate":{"type":"required","attribute_code":"reason"}}
    ]}
  ]}}'''
"#;

#[sqlx::test]
async fn approval_voids_are_not_guarded_by_transition_conditions(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    create_blueprint(&owner, &base, VOIDABLE).await;
    let entity = create_entity_with(
        &owner,
        &base,
        "voidable_document",
        json!([scalar("status", "draft"), scalar("title", "Procedure")]),
    )
    .await;
    let record = Record {
        url: format!("{base}/v1/entities/{}", entity["id"].as_str().unwrap()),
    };
    expect_status(record.set_status(&owner, "approved").await, StatusCode::OK).await;
    // A user moving approved -> draft must give a reason ...
    expect_error(
        record.set_status(&owner, "draft").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "transition_conditions_unmet",
    )
    .await;
    // ... but the system void of a covered edit is not that user transition.
    expect_status(
        record
            .put(&owner, json!([scalar("title", "Procedure v2")]))
            .await,
        StatusCode::OK,
    )
    .await;
    assert_eq!(record.status(&owner).await, "draft");
    server.abort();
}

#[sqlx::test]
async fn released_files_are_held_and_explicit_holds_are_managed(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    let editor = client_for(member_with_role(&pool, EDITOR_ROLE_ID).await);
    let uploaded = expect_status(
        upload(&owner, &base, &record, "spec.txt").await,
        StatusCode::CREATED,
    )
    .await;
    let file_id = uploaded["files"][0]["id"].as_str().unwrap().to_owned();
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect_status(record.set_status(client, status).await, StatusCode::OK).await;
    }
    let holds: Value = owner
        .get(format!("{}/retention-holds", record.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hold = &holds["items"][0];
    assert_eq!(hold["source"], "status");
    assert_eq!(hold["status"], "released");
    assert_eq!(hold["file_id"], file_id);
    assert_eq!(hold["active"], true);
    expect_error(
        owner
            .post(format!(
                "{base}/files/{file_id}/retention-holds/{}/release",
                hold["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;

    let place = |client: &Client| {
        client
            .post(format!("{base}/files/{file_id}/retention-holds"))
            .json(&json!({"days": 7, "reason": "Legal hold"}))
    };
    expect_status(place(&editor).send().await.unwrap(), StatusCode::FORBIDDEN).await;
    let explicit = expect_status(place(&owner).send().await.unwrap(), StatusCode::CREATED).await;
    assert_eq!(explicit["source"], "explicit");
    let released = expect_status(
        owner
            .post(format!(
                "{base}/files/{file_id}/retention-holds/{}/release",
                explicit["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    assert_eq!(released["active"], false);
    let listed: Value = editor
        .get(format!("{base}/files/{file_id}/retention-holds"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 2);
    server.abort();
}

#[sqlx::test]
async fn context_reparenting_revalidates_without_status_effects(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    let context = |code: &str, parent_id: Option<Value>| {
        let owner = owner.clone();
        let url = format!("{base}/contexts");
        let body = json!({"code": code, "data": {}, "parent_id": parent_id});
        async move {
            owner
                .post(url)
                .json(&body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["id"]
                .clone()
        }
    };
    let a = context("region_a", None).await;
    let b = context("region_b", None).await;
    let c = context("region_c", Some(a.clone())).await;
    expect_status(record.set_status(&owner, "review").await, StatusCode::OK).await;
    expect_status(record
            .put(
                &owner,
                json!([{"kind":"scalar","attribute_code":"title","context_id":b,"value":"Procedure B"}]),
            )
            .await, StatusCode::OK)
    .await;
    // Approval in the default context is inherited, and recorded, in every
    // context, each with that context's covered content.
    expect_status(
        record.set_status(&reviewer, "approved").await,
        StatusCode::OK,
    )
    .await;
    let c_id: Uuid = c.as_str().unwrap().parse().unwrap();
    let active_in_c = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM entity_approvals WHERE context_id = $1 AND ended_at IS NULL",
        )
        .bind(c_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let transitions = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM entity_status_transitions")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    assert_eq!(active_in_c().await, 1);
    let recorded = transitions().await;

    // Moving c under b changes its inherited title. That is not an edit of
    // the record: the approval is neither voided nor is any transition
    // recorded or enforced for the operator who reparents.
    let moved = owner
        .put(format!("{base}/contexts/id/{c_id}"))
        .json(&json!({"parent_id": b, "data": {}}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        moved.status(),
        StatusCode::OK,
        "{}",
        moved.text().await.unwrap()
    );
    assert_eq!(active_in_c().await, 1);
    assert_eq!(transitions().await, recorded);
    server.abort();
}
