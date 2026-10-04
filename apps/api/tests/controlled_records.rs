mod support;

use std::sync::Arc;

use api::storage::FakeObjectStore;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    multipart::{Form, Part},
};
use support::*;

const WORKSPACE_ID: &str = "00000000-0000-4000-8000-000000000002";
const EDITOR_ROLE_ID: &str = "00000000-0000-4000-8000-000000000103";

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

fn client_for(user: Uuid) -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_str(&user.to_string()).unwrap(),
    );
    headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_static(WORKSPACE_ID),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

/// A workspace member holding `role_id` across the workspace.
async fn member(pool: &PgPool, role_id: Uuid) -> Uuid {
    let workspace = WORKSPACE_ID.parse::<Uuid>().unwrap();
    let (user, membership) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user)
        .bind(format!("{user}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, 'workspace', $2)")
        .bind(Uuid::new_v4())
        .bind(workspace)
        .bind(membership)
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap();
    user
}

async fn reviewer_role(pool: &PgPool) -> Uuid {
    let role = Uuid::new_v4();
    sqlx::query("INSERT INTO roles (id, code, workspace_id) VALUES ($1, 'reviewer', $2)")
        .bind(role)
        .bind(WORKSPACE_ID.parse::<Uuid>().unwrap())
        .execute(pool)
        .await
        .unwrap();
    for permission in ["entities.read", "entities.write"] {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, $2)")
            .bind(role)
            .bind(permission)
            .execute(pool)
            .await
            .unwrap();
    }
    role
}

struct Record {
    url: String,
}

impl Record {
    async fn read(&self, client: &Client) -> Value {
        client
            .get(&self.url)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
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
        self.put(
            client,
            json!([{"kind":"scalar","attribute_code":"status","value":status}]),
        )
        .await
    }

    async fn status(&self, client: &Client) -> Value {
        self.read(client).await["entity"]["projections"]["preview"]["default"]["status"].clone()
    }
}

async fn expect(response: reqwest::Response, status: StatusCode, code: Option<&str>) -> Value {
    let actual = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    assert_eq!(actual, status, "{body}");
    if let Some(code) = code {
        assert_eq!(body["error"]["code"], code, "{body}");
    }
    body
}

async fn setup(pool: &PgPool) -> (String, JoinHandle<()>, Arc<FakeObjectStore>, Record) {
    let store = Arc::new(FakeObjectStore::available());
    let (base, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    create_blueprint(&owner, &base, DEFINITION).await;
    let entity: Value = owner
        .post(format!("{base}/v1/entities"))
        .json(
            &json!({"blueprint":{"code":"controlled_document"},"values":[
                {"kind":"scalar","attribute_code":"status","value":"draft"},
                {"kind":"scalar","attribute_code":"title","value":"Procedure"}
            ]}),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
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
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer_role = reviewer_role(&pool).await;
    let first_reviewer = client_for(member(&pool, reviewer_role).await);
    let second_reviewer = client_for(member(&pool, reviewer_role).await);
    let editor = client_for(member(&pool, EDITOR_ROLE_ID.parse().unwrap()).await);

    expect(
        record.set_status(&first_reviewer, "review").await,
        StatusCode::OK,
        None,
    )
    .await;
    // The owner lacks the reviewer role; the submitter is barred by separation of duties.
    let denied = expect(
        record.set_status(&owner, "approved").await,
        StatusCode::FORBIDDEN,
        Some("status_transition_forbidden"),
    )
    .await;
    assert!(
        denied["error"]["message"]
            .as_str()
            .unwrap()
            .contains("'reviewer'")
    );
    expect(
        record.set_status(&first_reviewer, "approved").await,
        StatusCode::FORBIDDEN,
        Some("status_separation_of_duties"),
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

    expect(
        record.set_status(&second_reviewer, "approved").await,
        StatusCode::OK,
        None,
    )
    .await;
    // Releasing requires entities.publish, which editors do not hold.
    expect(
        record.set_status(&editor, "released").await,
        StatusCode::FORBIDDEN,
        Some("status_transition_forbidden"),
    )
    .await;
    expect(
        record.set_status(&owner, "released").await,
        StatusCode::OK,
        None,
    )
    .await;
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
    let _ = base;
}

#[sqlx::test]
async fn locked_records_reject_writes_until_an_audited_correction(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member(&pool, reviewer_role(&pool).await).await);
    expect(
        upload(&owner, &base, &record, "v1.txt").await,
        StatusCode::CREATED,
        None,
    )
    .await;
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect(
            record.set_status(client, status).await,
            StatusCode::OK,
            None,
        )
        .await;
    }
    let title = |value: &str| json!([{"kind":"scalar","attribute_code":"title","value":value}]);
    expect(
        record.put(&owner, title("Changed")).await,
        StatusCode::CONFLICT,
        Some("record_locked"),
    )
    .await;
    // Re-saving identical values is not a change.
    expect(
        record.put(&owner, title("Procedure")).await,
        StatusCode::OK,
        None,
    )
    .await;
    expect(
        upload(&owner, &base, &record, "v2.txt").await,
        StatusCode::CONFLICT,
        Some("record_locked"),
    )
    .await;
    let entity_id = record.url.rsplit('/').next().unwrap();
    expect(
        owner
            .delete(format!("{base}/entities/{entity_id}"))
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        Some("record_locked"),
    )
    .await;
    // A correction must be a pure status change.
    expect(
        record
            .put(
                &owner,
                json!([
                    {"kind":"scalar","attribute_code":"status","value":"draft"},
                    {"kind":"scalar","attribute_code":"title","value":"Changed"}
                ]),
            )
            .await,
        StatusCode::CONFLICT,
        Some("record_locked"),
    )
    .await;
    expect(
        record.set_status(&reviewer, "draft").await,
        StatusCode::FORBIDDEN,
        Some("status_transition_forbidden"),
    )
    .await;
    expect(
        record.set_status(&owner, "draft").await,
        StatusCode::OK,
        None,
    )
    .await;
    expect(
        record.put(&owner, title("Changed")).await,
        StatusCode::OK,
        None,
    )
    .await;
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
    let reviewer = client_for(member(&pool, reviewer_role(&pool).await).await);
    expect(
        record.set_status(&owner, "review").await,
        StatusCode::OK,
        None,
    )
    .await;
    expect(
        record.set_status(&reviewer, "approved").await,
        StatusCode::OK,
        None,
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
    expect(
        record
            .put(
                &owner,
                json!([{"kind":"scalar","attribute_code":"notes","value":"Typo list"}]),
            )
            .await,
        StatusCode::OK,
        None,
    )
    .await;
    assert_eq!(record.status(&owner).await, "approved");
    // Covered content voids the approval and returns the record to review atomically.
    expect(
        upload(&owner, &base, &record, "late.txt").await,
        StatusCode::CREATED,
        None,
    )
    .await;
    assert_eq!(record.status(&owner).await, "approved");
    expect(
        record
            .put(
                &owner,
                json!([{"kind":"scalar","attribute_code":"title","value":"Procedure v2"}]),
            )
            .await,
        StatusCode::OK,
        None,
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

#[sqlx::test]
async fn released_files_are_held_and_explicit_holds_are_managed(pool: PgPool) {
    let (base, server, _, record) = setup(&pool).await;
    let owner = authenticated_client();
    let reviewer = client_for(member(&pool, reviewer_role(&pool).await).await);
    let editor = client_for(member(&pool, EDITOR_ROLE_ID.parse().unwrap()).await);
    let uploaded = expect(
        upload(&owner, &base, &record, "spec.txt").await,
        StatusCode::CREATED,
        None,
    )
    .await;
    let file_id = uploaded["files"][0]["id"].as_str().unwrap().to_owned();
    for (client, status) in [
        (&owner, "review"),
        (&reviewer, "approved"),
        (&owner, "released"),
    ] {
        expect(
            record.set_status(client, status).await,
            StatusCode::OK,
            None,
        )
        .await;
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
    expect(
        owner
            .post(format!(
                "{base}/files/{file_id}/retention-holds/{}/release",
                hold["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::UNPROCESSABLE_ENTITY,
        Some("invalid_input"),
    )
    .await;

    let place = |client: &Client| {
        client
            .post(format!("{base}/files/{file_id}/retention-holds"))
            .json(&json!({"days": 7, "reason": "Legal hold"}))
    };
    expect(
        place(&editor).send().await.unwrap(),
        StatusCode::FORBIDDEN,
        None,
    )
    .await;
    let explicit = expect(
        place(&owner).send().await.unwrap(),
        StatusCode::CREATED,
        None,
    )
    .await;
    assert_eq!(explicit["source"], "explicit");
    let released = expect(
        owner
            .post(format!(
                "{base}/files/{file_id}/retention-holds/{}/release",
                explicit["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap(),
        StatusCode::OK,
        None,
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
