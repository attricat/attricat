mod support;
use api::{
    agent_tools::{execute_mutation, execute_read},
    repository::CatalogRepository,
};
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
"#;

const VIEWER_ROLE_ID: &str = "00000000-0000-4000-8000-000000000104";

/// An active member, with the Viewer role when `reader` is set, and a client
/// that authenticates as them.
async fn add_member(pool: &PgPool, email: &str, name: &str, reader: bool) -> (Uuid, Client) {
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    let workspace = bootstrap_workspace_id();
    sqlx::query("INSERT INTO users (id, email, display_name) VALUES ($1, $2, $3)")
        .bind(user)
        .bind(email)
        .bind(name)
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
    if reader {
        sqlx::query("INSERT INTO role_grants (id,workspace_id,membership_id,role_id,scope_type,scope_target_id) VALUES ($1,$2,$3,$4::uuid,'workspace',$2)")
            .bind(Uuid::new_v4())
            .bind(workspace)
            .bind(membership)
            .bind(VIEWER_ROLE_ID)
            .execute(pool)
            .await
            .unwrap();
    }
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-catalog-user-id", user.to_string().parse().unwrap());
    headers.insert(
        "x-catalog-workspace-id",
        workspace.to_string().parse().unwrap(),
    );
    (
        user,
        Client::builder().default_headers(headers).build().unwrap(),
    )
}

async fn inbox(client: &Client, base: &str, query: &str) -> Value {
    client
        .get(format!("{base}/notifications{query}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn kinds(page: &Value) -> Vec<&str> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["kind"].as_str().unwrap())
        .collect()
}

#[sqlx::test]
async fn system_events_reach_the_inbox_of_each_affected_member(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner_client = authenticated_client();
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let (member, member_client) = add_member(&pool, "ada@example.test", "Ada", true).await;
    let (outsider, outsider_client) =
        add_member(&pool, "no-read@example.test", "Outsider", false).await;
    create_blueprint(&owner_client, &base, DEFINITION).await;

    // Direct assignment notifies the assignee, never the person assigning.
    let entity: Value = post_entity(
        &owner_client,
        &base,
        "task",
        json!([
            scalar("title", "Review"),
            scalar("assignee", format!("user:{member}"))
        ]),
    )
    .await
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    let entity_id = entity["id"].as_str().unwrap().to_owned();
    let page = inbox(&member_client, &base, "").await;
    assert_eq!(kinds(&page), ["entity.assigned"]);
    let assigned = &page["items"][0];
    assert_eq!(
        assigned["subject"],
        json!({"kind": "entity", "id": entity_id})
    );
    assert_eq!(assigned["actor_user_id"], json!(owner));
    assert_eq!(assigned["data"]["blueprint_code"], "task");
    assert_eq!(assigned["data"]["attribute_code"], "assignee");
    assert_eq!(assigned["read"], false);
    assert!(
        assigned["title"]
            .as_str()
            .unwrap()
            .ends_with("assigned you to a Task record"),
        "{}",
        assigned["title"]
    );
    assert_eq!(page["unread_count"], 1);
    assert_eq!(inbox(&owner_client, &base, "").await["items"], json!([]));

    // Saving the record again without changing the assignee is silent.
    let current: Value = owner_client
        .get(format!("{base}/v1/entities/{entity_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    owner_client
        .put(format!("{base}/v1/entities/{entity_id}"))
        .json(&json!({
            "expected_updated_at": current["entity"]["updated_at"],
            "values": [scalar("title", "Review again")],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(inbox(&member_client, &base, "").await["unread_count"], 1);

    // Someone who cannot read the record is not told about it.
    post_entity(
        &owner_client,
        &base,
        "task",
        json!([scalar("assignee", format!("user:{outsider}"))]),
    )
    .await
    .error_for_status()
    .unwrap();
    assert_eq!(inbox(&outsider_client, &base, "").await["items"], json!([]));

    // Joining a team is a plain message; assigning the team reaches its
    // members.
    let team: Value = owner_client
        .post(format!("{base}/workspace/teams"))
        .json(&json!({"code": "qa", "name": "Quality", "member_user_ids": [member, owner]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    post_entity(
        &owner_client,
        &base,
        "task",
        json!([scalar(
            "assignee",
            format!("team:{}", team["id"].as_str().unwrap())
        )]),
    )
    .await
    .error_for_status()
    .unwrap();
    let page = inbox(&member_client, &base, "").await;
    assert_eq!(
        kinds(&page),
        ["entity.assigned", "team.member_added", "entity.assigned"]
    );
    assert_eq!(page["items"][1]["subject"], Value::Null);
    assert_eq!(page["items"][1]["data"]["team_name"], "Quality");
    assert_eq!(page["items"][0]["data"]["team_name"], "Quality");

    // Comments reach assignees and earlier commenters, except the author.
    let comments = format!("{base}/v1/entities/{entity_id}/comments");
    owner_client
        .post(&comments)
        .json(&json!({"body": "Please check the **totals**."}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let page = inbox(&member_client, &base, "?unread_only=true").await;
    assert_eq!(page["items"][0]["kind"], "entity.commented");
    assert_eq!(page["items"][0]["body"], "Please check the **totals**.");
    assert_eq!(page["unread_count"], 4);
    member_client
        .post(&comments)
        .json(&json!({"body": "Done."}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        kinds(&inbox(&owner_client, &base, "").await),
        ["entity.commented"]
    );
    assert_eq!(inbox(&member_client, &base, "").await["unread_count"], 4);
}

#[sqlx::test]
async fn comments_notify_current_assignees_not_earlier_ones(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner_client = authenticated_client();
    let (ada, ada_client) = add_member(&pool, "ada@example.test", "Ada", true).await;
    let (bob, bob_client) = add_member(&pool, "bob@example.test", "Bob", true).await;
    create_blueprint(&owner_client, &base, DEFINITION).await;
    let entity: Value = post_entity(
        &owner_client,
        &base,
        "task",
        json!([scalar("assignee", format!("user:{ada}"))]),
    )
    .await
    .error_for_status()
    .unwrap()
    .json()
    .await
    .unwrap();
    let id = entity["id"].as_str().unwrap();
    let current: Value = owner_client
        .get(format!("{base}/v1/entities/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    owner_client
        .put(format!("{base}/v1/entities/{id}"))
        .json(&json!({
            "expected_updated_at": current["entity"]["updated_at"],
            "values": [scalar("assignee", format!("user:{bob}"))],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    owner_client
        .post(format!("{base}/v1/entities/{id}/comments"))
        .json(&json!({"body": "Over to you."}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        kinds(&inbox(&bob_client, &base, "").await),
        ["entity.commented", "entity.assigned"]
    );
    assert_eq!(
        kinds(&inbox(&ada_client, &base, "").await),
        ["entity.assigned"]
    );
}

#[sqlx::test]
async fn members_read_mark_and_delete_only_their_own_notifications(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner_client = authenticated_client();
    let (member, member_client) = add_member(&pool, "ada@example.test", "Ada", true).await;
    let (_, other_client) = add_member(&pool, "bob@example.test", "Bob", true).await;
    create_blueprint(&owner_client, &base, DEFINITION).await;
    for title in ["one", "two", "three"] {
        post_entity(
            &owner_client,
            &base,
            "task",
            json!([
                scalar("title", title),
                scalar("assignee", format!("user:{member}"))
            ]),
        )
        .await
        .error_for_status()
        .unwrap();
    }
    let page = inbox(&member_client, &base, "").await;
    let ids: Vec<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(page["has_more"], false);
    let count = |client: Client| {
        let base = base.clone();
        async move {
            client
                .get(format!("{base}/notifications/unread-count"))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["count"]
                .clone()
        }
    };
    assert_eq!(count(member_client.clone()).await, 3);

    // Another member sees neither the notification nor whether it exists.
    let first = format!("{base}/notifications/{}", ids[0]);
    for request in [
        other_client.get(&first),
        other_client.patch(&first).json(&json!({"read": true})),
        other_client.delete(&first),
    ] {
        assert_eq!(
            request.send().await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    let mark = |read: bool| member_client.patch(&first).json(&json!({"read": read}));
    assert_eq!(
        mark(true).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(count(member_client.clone()).await, 2);
    let read: Value = member_client
        .get(&first)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(read["read"], true);
    assert!(read["read_at"].is_string());
    assert_eq!(
        kinds(&inbox(&member_client, &base, "?unread_only=true").await).len(),
        2
    );
    assert_eq!(
        mark(false).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(count(member_client.clone()).await, 3);

    // Paging follows the keyset cursor of the last item.
    let last = &page["items"][1];
    let next = inbox(
        &member_client,
        &base,
        &format!(
            "?before_time={}&before_id={}",
            urlencoding(last["created_at"].as_str().unwrap()),
            last["id"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(next["items"].as_array().unwrap().len(), 1);
    assert_eq!(next["items"][0]["id"], json!(ids[2]));
    assert_eq!(
        member_client
            .get(format!("{base}/notifications?before_id={}", ids[0]))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Marking all read leaves notifications newer than `up_to` unread.
    let marked: Value = member_client
        .post(format!("{base}/notifications/read-all"))
        .json(&json!({"up_to": "2000-01-01T00:00:00Z"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(marked["updated"], 0);
    let marked: Value = member_client
        .post(format!("{base}/notifications/read-all"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(marked["updated"], 3);
    assert_eq!(count(member_client.clone()).await, 0);

    // Deletion is permanent.
    assert_eq!(
        member_client.delete(&first).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        member_client.get(&first).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_notifications WHERE id = $1")
            .bind(ids[0].parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(
        inbox(&member_client, &base, "").await["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[sqlx::test]
async fn agent_tools_act_on_the_initiating_users_inbox(pool: PgPool) {
    let (base, _server) = start_server(pool.clone()).await;
    let owner_client = authenticated_client();
    let (member, _) = add_member(&pool, "ada@example.test", "Ada", true).await;
    create_blueprint(&owner_client, &base, DEFINITION).await;
    for title in ["one", "two"] {
        post_entity(
            &owner_client,
            &base,
            "task",
            json!([
                scalar("title", title),
                scalar("assignee", format!("user:{member}"))
            ]),
        )
        .await
        .error_for_status()
        .unwrap();
    }
    let workspace = bootstrap_workspace_id();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();

    let listed = execute_read(
        &repository,
        member,
        workspace,
        "list_notifications",
        json!({"limit": 1}),
    )
    .await
    .unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["unread_count"], 2);
    assert!(listed["next_before"].is_object());
    let rest = execute_read(
        &repository,
        member,
        workspace,
        "list_notifications",
        json!({"before": listed["next_before"]}),
    )
    .await
    .unwrap();
    assert_eq!(rest["items"].as_array().unwrap().len(), 1);
    let first = listed["items"][0]["id"].clone();
    let second = rest["items"][0]["id"].clone();

    // The owner's inbox is separate: the member's IDs change nothing there.
    let owner: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let foreign = execute_mutation(
        &repository,
        owner,
        "delete_notifications",
        json!({"notification_ids": [first]}),
    )
    .await
    .unwrap();
    assert_eq!(foreign["deleted"], 0);

    let marked = execute_mutation(
        &repository,
        member,
        "mark_notifications_read",
        json!({"notification_ids": [first]}),
    )
    .await
    .unwrap();
    assert_eq!(marked["updated"], 1);
    let unread = execute_read(
        &repository,
        member,
        workspace,
        "list_notifications",
        json!({"unread_only": true}),
    )
    .await
    .unwrap();
    assert_eq!(unread["items"].as_array().unwrap().len(), 1);
    assert_eq!(unread["items"][0]["id"], second);

    let all = execute_mutation(
        &repository,
        member,
        "mark_all_notifications_read",
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(all["updated"], 1);
    let deleted = execute_mutation(
        &repository,
        member,
        "delete_notifications",
        json!({"notification_ids": [first, second]}),
    )
    .await
    .unwrap();
    assert_eq!(deleted["deleted"], 2);
    let empty = execute_read(
        &repository,
        member,
        workspace,
        "list_notifications",
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(empty["items"], json!([]));
    assert_eq!(empty["unread_count"], 0);
}

fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
