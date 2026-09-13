mod support;
use support::*;

const DEFINITION: &str = "format_version = 1\ncode = \"tag_new_products\"\nname = \"Tag new products\"\n[[triggers]]\nevent_type = \"entity.created.v1\"\n[[actions]]\ntype = \"system_tags_add\"\ntags = [\"new\"]";

#[sqlx::test]
async fn workflow_lifecycle_keeps_immutable_revisions(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let invalid = client.post(format!("{base_url}/workflows/validate")).json(&json!({"definition":"format_version=1\ncode='x'\nname='x'\nscript='no'\ntriggers=[]\nactions=[]"})).send().await.unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let created: Value = client
        .post(format!("{base_url}/workflows"))
        .json(&json!({"definition":DEFINITION}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["status"], "draft");
    let enabled = client
        .post(format!("{base_url}/workflows/{id}/versions/1/enable"))
        .send()
        .await
        .unwrap();
    assert_eq!(enabled.status(), StatusCode::UNPROCESSABLE_ENTITY);
    client
        .post(format!("{base_url}/workflows/{id}/versions/1/publish"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let enabled: Value = client
        .post(format!("{base_url}/workflows/{id}/versions/1/enable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(enabled["enabled_version"], 1);
    let revised: Value = client
        .post(format!("{base_url}/workflows/{id}/versions"))
        .json(&json!({"definition":DEFINITION.replace("Tag new products", "Tag products") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revised["version"], 2);
    assert_eq!(revised["status"], "draft");
    let disabled: Value = client
        .post(format!("{base_url}/workflows/{id}/disable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(disabled["enabled_version"].is_null());
    server.abort();
}
