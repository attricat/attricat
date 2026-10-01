mod support;
use support::*;

const BLUEPRINT: &str = r#"
format_version = 1
code = "comment_entity"
name = "Comment entity"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

#[sqlx::test(migrations = "./migrations")]
async fn readers_can_comment_but_only_authors_can_edit(pool: PgPool) {
    let (base, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(&owner, &base, BLUEPRINT).await;
    let entity = create_entity(&owner, &base, &blueprint).await;
    let id = entity["id"].as_str().unwrap();
    let path = format!("{base}/v1/entities/{id}/comments");
    let user = Uuid::new_v4();
    let membership = Uuid::new_v4();
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO users (id,email,display_name) VALUES ($1,'comment-viewer@example.test','Reader')")
        .bind(user).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO workspace_memberships (id,workspace_id,user_id) VALUES ($1,$2,$3)")
        .bind(membership)
        .bind(workspace)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO role_grants (id,workspace_id,membership_id,role_id,scope_type,scope_target_id) VALUES ($1,$2,$3,'00000000-0000-4000-8000-000000000104','workspace',$2)")
        .bind(Uuid::new_v4()).bind(workspace).bind(membership).execute(&pool).await.unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-catalog-user-id", user.to_string().parse().unwrap());
    headers.insert(
        "x-catalog-workspace-id",
        workspace.to_string().parse().unwrap(),
    );
    let viewer = Client::builder().default_headers(headers).build().unwrap();

    assert_eq!(
        viewer
            .post(&path)
            .json(&json!({"body":"**Hello** 🦀"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let page: Value = viewer
        .get(&path)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page["items"][0]["author_user_id"], user.to_string());
    assert_eq!(page["items"][0]["author_display_name"], "Reader");
    assert_eq!(page["items"][0]["body"], "**Hello** 🦀");
    let comment = page["items"][0]["id"].as_str().unwrap();
    let edit = format!("{path}/{comment}");
    assert_eq!(
        owner
            .patch(&edit)
            .json(&json!({"body":"not mine","revision":1}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        viewer
            .patch(&edit)
            .json(&json!({"body":"Updated","revision":1}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        viewer
            .patch(&edit)
            .json(&json!({"body":"Stale","revision":1}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    for body in [" \n".to_owned(), "🦀".repeat(10001), "\0".to_owned()] {
        assert_eq!(
            viewer
                .post(&path)
                .json(&json!({"body":body}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        viewer
            .post(&path)
            .json(&json!({"body":"spoof","author_user_id":BOOTSTRAP_OWNER_ID}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let other = create_entity(&owner, &base, &blueprint).await;
    assert_eq!(
        viewer
            .patch(format!(
                "{base}/v1/entities/{}/comments/{comment}",
                other["id"].as_str().unwrap()
            ))
            .json(&json!({"body":"wrong entity","revision":2}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_events WHERE actor_user_id=$1 AND outcome='success'",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2);
    // Revoking membership removes both read and comment access.
    sqlx::query("UPDATE workspace_memberships SET state='inactive' WHERE id=$1")
        .bind(membership)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        viewer
            .post(&path)
            .json(&json!({"body":"revoked"}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    owner
        .delete(format!("{base}/entities/{id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert_eq!(
        owner.get(&path).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    server.abort();
}

#[sqlx::test(migrations = "./migrations")]
async fn comments_have_stable_bounded_pagination(pool: PgPool) {
    let (base, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base, BLUEPRINT).await;
    let entity = create_entity(&client, &base, &blueprint).await;
    let path = format!(
        "{base}/v1/entities/{}/comments",
        entity["id"].as_str().unwrap()
    );
    for index in 0..31 {
        client
            .post(&path)
            .json(&json!({"body":format!("comment {index}")}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let first: Value = client
        .get(&path)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 30);
    assert_eq!(first["has_more"], true);
    let last = &first["items"][29];
    let second: Value = client
        .get(&path)
        .query(&[
            ("before_time", last["created_at"].as_str().unwrap()),
            ("before_id", last["id"].as_str().unwrap()),
        ])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["has_more"], false);
    assert_ne!(second["items"][0]["id"], last["id"]);
    assert_eq!(
        client
            .get(&path)
            .query(&[("before_id", last["id"].as_str().unwrap())])
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    server.abort();
}
