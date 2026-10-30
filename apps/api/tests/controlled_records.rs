use crate::support;

use std::sync::Arc;

use api::storage::FakeObjectStore;
use reqwest::multipart::{Form, Part};
use support::*;

const DEFINITION: &str = r#"format_version: 1
code: controlled_document
name: Controlled document
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: notes
    value_type: string
  - code: document
    value_type: file
    allowed_mime_groups: [text/plain]
    allowed_extensions: [txt]
    max_bytes: 1024
  - code: status
    value_type: string
    value_schema: |-
      {"type":"string","enum":["draft","review","approved","released"],"x-attricat-status":{"version":1,
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
          {"from":"approved","to":"released","code":"release","permission":"records.publish"},
          {"from":"released","to":"draft","code":"correct","roles":["owner"]}
        ]}}
"#;

async fn reviewer_role(pool: &PgPool) -> Uuid {
    create_role(pool, "reviewer", &["records.read", "records.write"]).await
}

struct RecordClient {
    url: String,
}

impl RecordClient {
    async fn read(&self, client: &Client) -> Value {
        get_json(client, &self.url).await
    }

    async fn put(&self, client: &Client, values: Value) -> reqwest::Response {
        let current = self.read(client).await;
        client
            .put(&self.url)
            .json(
                &json!({"expected_updated_at": current["record"]["updated_at"], "values": values}),
            )
            .send()
            .await
            .unwrap()
    }

    async fn set_status(&self, client: &Client, status: &str) -> reqwest::Response {
        self.put(client, json!([scalar("status", status)])).await
    }

    async fn status(&self, client: &Client) -> Value {
        self.read(client).await["record"]["projections"]["preview"]["default"]["status"].clone()
    }
}

async fn setup(pool: &PgPool) -> (String, JoinHandle<()>, Arc<FakeObjectStore>, RecordClient) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    create_blueprint(&owner, &base, DEFINITION).await;
    let record = create_record_with(
        &owner,
        &base,
        "controlled_document",
        json!([scalar("status", "draft"), scalar("title", "Procedure")]),
    )
    .await;
    let url = format!("{base}/v1/records/{}", record["id"].as_str().unwrap());
    (base, server, store, RecordClient { url })
}

async fn upload(
    client: &Client,
    base: &str,
    record: &RecordClient,
    name: &str,
) -> reqwest::Response {
    let record_id = record.url.rsplit('/').next().unwrap();
    client
        .post(format!(
            "{base}/records/{record_id}/file-attributes/document/uploads"
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    // Releasing requires records.publish, which editors do not hold.
    expect_error(
        record.set_status(&editor, "released").await,
        StatusCode::FORBIDDEN,
        "status_transition_forbidden",
    )
    .await;
    expect_status(record.set_status(&owner, "released").await, StatusCode::OK).await;
    assert_eq!(record.status(&owner).await, "released");
    let actors: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT edge_code, to_status FROM record_status_transitions ORDER BY occurred_at, to_status",
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

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn owners_may_approve_their_own_submission(pool: PgPool) {
    let (_, server, _, record) = setup(&pool).await;
    // A sole owner who also holds the edge's role: nobody else could approve.
    let (user, membership) = add_workspace_user(&pool).await;
    grant_role(&pool, membership, OWNER_ROLE_ID, GrantScope::Workspace).await;
    grant_role(
        &pool,
        membership,
        reviewer_role(&pool).await,
        GrantScope::Workspace,
    )
    .await;
    let owner = client_for(user);

    expect_status(record.set_status(&owner, "review").await, StatusCode::OK).await;
    let access: Value = get_json(&owner, &format!("{}/status-transitions", record.url)).await;
    let approve = access["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| edge["to"] == "approved")
        .unwrap();
    assert_eq!(approve["allowed"], true);
    assert_eq!(approve["waives_separation"], true);

    expect_status(record.set_status(&owner, "approved").await, StatusCode::OK).await;
    let rows: Vec<(Option<String>, bool)> = sqlx::query_as(
        "SELECT edge_code, separation_waived FROM record_status_transitions ORDER BY occurred_at, to_status",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        [
            (None, false),
            (Some("submit".to_owned()), false),
            (Some("approve".to_owned()), true)
        ]
    );
    let audit: Value = sqlx::query_scalar(
        "SELECT metadata->'record_control' FROM audit_events WHERE action = 'record.separation.waive'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit["transition_code"], "approve");
    assert_eq!(audit["separate_from"], "submit");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    let record_id = record.url.rsplit('/').next().unwrap();
    expect_error(
        owner
            .delete(format!("{base}/records/{record_id}"))
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
        "SELECT count(*) FROM audit_events WHERE action = 'record.unlock' AND target->>'record_id' = $1",
    )
    .bind(record_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unlocks, 1);
    let unlocked: bool = sqlx::query_scalar(
        "SELECT unlocked FROM record_status_transitions WHERE edge_code = 'correct'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(unlocked);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn owners_unlock_any_locking_status_outside_the_declared_transitions(pool: PgPool) {
    let (_, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    let unlock = |client: &Client, to: &str| {
        let client = client.clone();
        let url = format!("{}/status-unlock", record.url);
        let to = to.to_owned();
        let record = &record;
        async move {
            let current = record.read(&client).await;
            client
                .post(url)
                .json(&json!({
                    "attribute_code": "status",
                    "to": to,
                    "expected_updated_at": current["record"]["updated_at"],
                }))
                .send()
                .await
                .unwrap()
        }
    };
    expect_error(
        unlock(&owner, "review").await,
        StatusCode::CONFLICT,
        "status_not_locked",
    )
    .await;
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect_status(record.set_status(client, status).await, StatusCode::OK).await;
    }
    expect_error(
        unlock(&reviewer, "review").await,
        StatusCode::FORBIDDEN,
        "forbidden",
    )
    .await;
    expect_error(
        unlock(&owner, "retired").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "attribute_value_schema_mismatch",
    )
    .await;
    // `released` declares no edge to `review`, and the unlock skips the
    // reviewer role the approval edges require.
    expect_status(unlock(&owner, "review").await, StatusCode::OK).await;
    assert_eq!(record.status(&owner).await, "review");
    expect_status(
        record
            .put(&owner, json!([scalar("title", "Changed")]))
            .await,
        StatusCode::OK,
    )
    .await;
    let (kind, edge, unlocked): (String, Option<String>, bool) = sqlx::query_as(
        "SELECT kind, edge_code, unlocked FROM record_status_transitions WHERE from_status = 'released'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((kind.as_str(), edge, unlocked), ("override", None, true));
    let audited: Value =
        sqlx::query_scalar("SELECT metadata FROM audit_events WHERE action = 'record.unlock'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audited["record_control"]["override"], true);
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn an_unlock_into_an_approval_status_records_its_override(pool: PgPool) {
    let (_, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect_status(record.set_status(client, status).await, StatusCode::OK).await;
    }
    let current = record.read(&owner).await;
    expect_status(
        owner
            .post(format!("{}/status-unlock", record.url))
            .json(&json!({
                "attribute_code": "status",
                "to": "approved",
                "expected_updated_at": current["record"]["updated_at"],
            }))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
    )
    .await;
    let approvals: Value = get_json(&owner, &format!("{}/approvals", record.url)).await;
    assert_eq!(
        approvals["items"][0]["cause"],
        json!({
            "from": "released",
            "transition_code": null,
            "kind": "override",
            "operation": "edit",
            "channel": "person",
            "separation_waived": false,
        })
    );
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    // The approval explains the status change that entered it.
    assert_eq!(
        recorded[0]["cause"],
        json!({
            "from": "review",
            "transition_code": "approve",
            "kind": "transition",
            "operation": "edit",
            "channel": "person",
            "separation_waived": false,
        })
    );
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
        "SELECT action FROM audit_events WHERE action LIKE 'record.approval.%' ORDER BY occurred_at",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions, ["record.approval.record", "record.approval.void"]);

    // Older superseded copies fill more than one page.
    sqlx::query(
        "INSERT INTO record_approvals (id, workspace_id, record_id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest, approved_at, ended_at, end_reason) SELECT gen_random_uuid(), workspace_id, record_id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest, approved_at - make_interval(mins => n), approved_at, 'superseded' FROM record_approvals, generate_series(1, 20) AS n",
    )
    .execute(&pool)
    .await
    .unwrap();
    let page = |query: Vec<(&'static str, String)>| {
        let request = owner.get(format!("{}/approvals", record.url)).query(&query);
        async move { request.send().await.unwrap() }
    };
    let first: Value = page(vec![]).await.json().await.unwrap();
    let items = first["items"].as_array().unwrap();
    assert_eq!(items.len(), 20);
    assert_eq!(first["has_more"], true);
    assert_eq!(items[0]["id"], voided[0]["id"]);
    let last = &items[19];
    let second: Value = page(vec![
        (
            "before_time",
            last["approved_at"].as_str().unwrap().to_owned(),
        ),
        ("before_id", last["id"].as_str().unwrap().to_owned()),
    ])
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["has_more"], false);
    // Approvals recorded without a transition have no cause.
    assert_eq!(last["cause"], Value::Null);
    expect_error(
        page(vec![("before_id", last["id"].as_str().unwrap().to_owned())]).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_input",
    )
    .await;

    // Restoring the approved status from history approves again, along the
    // same edge, as a history restoration.
    let history_id: Uuid = sqlx::query_scalar(
        "SELECT h.id FROM attribute_value_history h JOIN attributes a ON a.id = h.attribute_id WHERE a.code = 'status' AND h.value_text = 'approved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let record_id = record.url.rsplit('/').next().unwrap();
    let current = record.read(&reviewer).await;
    expect_status(
        reviewer
            .post(format!(
                "{base}/records/{record_id}/values/history/{history_id}/restore"
            ))
            .query(&[(
                "expected_updated_at",
                current["record"]["updated_at"].as_str().unwrap(),
            )])
            .send()
            .await
            .unwrap(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(record.status(&owner).await, "approved");
    let restored = approvals(owner.clone()).await;
    assert_eq!(restored[0]["cause"]["from"], "review");
    assert_eq!(restored[0]["cause"]["transition_code"], "approve");
    assert_eq!(restored[0]["cause"]["operation"], "history_restore");
    assert_eq!(restored[0]["cause"]["channel"], "person");
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn approval_pages_keep_one_write_together(pool: PgPool) {
    let (_, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member_with_role(&pool, reviewer_role(&pool).await).await);
    expect_status(record.set_status(&owner, "review").await, StatusCode::OK).await;
    expect_status(
        record.set_status(&reviewer, "approved").await,
        StatusCode::OK,
    )
    .await;
    // 24 older copies recorded at one time, as one write across contexts
    // records them, then 5 older still.
    sqlx::query(
        "INSERT INTO record_approvals (id, workspace_id, record_id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest, approved_at, ended_at, end_reason) SELECT gen_random_uuid(), workspace_id, record_id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest, approved_at - CASE WHEN n <= 24 THEN interval '1 hour' ELSE make_interval(hours => n) END, approved_at, 'superseded' FROM record_approvals, generate_series(1, 29) AS n",
    )
    .execute(&pool)
    .await
    .unwrap();
    let url = format!("{}/approvals", record.url);
    let first: Value = get_json(&owner, &url).await;
    let items = first["items"].as_array().unwrap();
    // The 20th falls inside the batch, so the page runs to its end.
    assert_eq!(items.len(), 25);
    assert_eq!(first["has_more"], true);
    assert_eq!(items[1]["approved_at"], items[24]["approved_at"]);
    let last = &items[24];
    let second: Value = owner
        .get(&url)
        .query(&[
            ("before_time", last["approved_at"].as_str().unwrap()),
            ("before_id", last["id"].as_str().unwrap()),
        ])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 5);
    assert_eq!(second["has_more"], false);
    server.abort();
}

const VOIDABLE: &str = r#"format_version: 1
code: voidable_document
name: Voidable document
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: reason
    value_type: string
  - code: status
    value_type: string
    value_schema: |-
      {"type":"string","enum":["draft","approved"],"x-attricat-status":{"version":1,
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
        ]}}
"#;

#[sqlx::test(migrator = "api::MIGRATOR")]
async fn approval_voids_are_not_guarded_by_transition_conditions(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    create_blueprint(&owner, &base, VOIDABLE).await;
    let record = create_record_with(
        &owner,
        &base,
        "voidable_document",
        json!([scalar("status", "draft"), scalar("title", "Procedure")]),
    )
    .await;
    let record = RecordClient {
        url: format!("{base}/v1/records/{}", record["id"].as_str().unwrap()),
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

#[sqlx::test(migrator = "api::MIGRATOR")]
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
    // The record's holds continue, newest first, before the last one shown.
    let record_holds = |query: Vec<(&'static str, String)>| {
        let request = editor
            .get(format!("{}/retention-holds", record.url))
            .query(&query);
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };
    let first = record_holds(vec![]).await;
    assert_eq!(first["has_more"], false);
    let newest = &first["items"][0];
    let older = record_holds(vec![
        (
            "before_time",
            newest["created_at"].as_str().unwrap().to_owned(),
        ),
        ("before_id", newest["id"].as_str().unwrap().to_owned()),
    ])
    .await;
    assert_eq!(
        older["items"].as_array().unwrap().len(),
        first["items"].as_array().unwrap().len() - 1
    );
    assert_eq!(older["items"][0]["id"], first["items"][1]["id"]);
    // FileRead routes defer resource authorization to the handler. Merely
    // having an active membership must not expose another record's holds.
    let (unrelated, membership) = add_workspace_user(&pool).await;
    grant_role(
        &pool,
        membership,
        VIEWER_ROLE_ID,
        GrantScope::Record(Uuid::new_v4()),
    )
    .await;
    expect_status(
        client_for(unrelated)
            .get(format!("{base}/files/{file_id}/retention-holds"))
            .send()
            .await
            .unwrap(),
        StatusCode::FORBIDDEN,
    )
    .await;
    server.abort();
}

#[sqlx::test(migrator = "api::MIGRATOR")]
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
            "SELECT count(*) FROM record_approvals WHERE context_id = $1 AND ended_at IS NULL",
        )
        .bind(c_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let transitions = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM record_status_transitions")
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
