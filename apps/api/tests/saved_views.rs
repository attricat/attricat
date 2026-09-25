mod support;
use support::*;

#[sqlx::test]
async fn saved_views_are_scoped_and_link_snapshots_are_reusable(pool: PgPool) {
    let (base_url, _server) = start_server(pool).await;
    let client = authenticated_client();
    let state = json!({"blueprint":"asset","query":"laptop","attributeFilters":[{"field":"name","operator":"contains","value":"think"}]});
    let payload =
        json!({"kind":"explorer_search","name":"Laptops","visibility":"private","state":state});
    let created = client
        .post(format!("{base_url}/saved-views"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let view: Value = created.json().await.unwrap();
    let id = view["id"].as_str().unwrap();
    let listed: Value = client
        .get(format!("{base_url}/saved-views"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["state"], state);
    let loaded: Value = client
        .get(format!("{base_url}/saved-views/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(loaded["name"], "Laptops");
    let changed: Value = client.put(format!("{base_url}/saved-views/{id}")).json(&json!({"kind":"explorer_search","name":"Hardware","visibility":"workspace","state":state})).send().await.unwrap().json().await.unwrap();
    assert_eq!(changed["name"], "Hardware");
    let link_payload = json!({"kind":"explorer_search","state":state});
    let first: Value = client
        .post(format!("{base_url}/view-state-links"))
        .json(&link_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: Value = client
        .post(format!("{base_url}/view-state-links"))
        .json(&link_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["id"], second["id"]);
    let with_defaults = json!({"kind":"explorer_search","state":{"blueprint":"asset","query":"laptop","context":"default","allVersions":false,"attributeFilters":[{"field":"name","operator":"contains","value":"think"}],"relationshipFacets":[]}});
    let normalized: Value = client
        .post(format!("{base_url}/view-state-links"))
        .json(&with_defaults)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["id"], normalized["id"]);
    let link_id = first["id"].as_str().unwrap();
    let linked: Value = client
        .get(format!("{base_url}/view-state-links/{link_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(linked["state"], state);
    assert_eq!(
        reqwest::Client::new()
            .get(format!("{base_url}/view-state-links/{link_id}"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!(
                "{base_url}/saved-views/{}",
                first["id"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{base_url}/view-state-links/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .delete(format!("{base_url}/saved-views/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(format!("{base_url}/saved-views/{id}"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(client.post(format!("{base_url}/saved-views")).json(&json!({"kind":"explorer_search","name":"Bad","visibility":"private","state":{"blueprint":"asset","attributeFilters":[{"field":"","operator":"eq","value":1}]}})).send().await.unwrap().status(), 422);
    assert_eq!(
        client
            .post(format!("{base_url}/saved-views"))
            .json(&json!({"kind":"dashboard","name":"Bad","visibility":"private","state":state}))
            .send()
            .await
            .unwrap()
            .status(),
        422
    );
}
