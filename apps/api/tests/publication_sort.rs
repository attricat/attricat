mod support;

use support::*;

#[sqlx::test]
async fn publication_sort_uses_selected_channel_and_keyset_pages(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(
        &client,
        &base_url,
        r#"format_version = 1
code = "publication_sort_product"
name = "Publication sort product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "title"
value_type = "string""#,
    )
    .await;
    let first = create_entity(&client, &base_url, &blueprint).await;
    let second = create_entity(&client, &base_url, &blueprint).await;
    let third = create_entity(&client, &base_url, &blueprint).await;
    let web: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code":"pub_sort_web","data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let app: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code":"pub_sort_app","data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for context in [&web, &app] {
        client
            .put(format!(
                "{base_url}/publication-channels/{}",
                context["id"].as_str().unwrap()
            ))
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    for entity in [&first, &third] {
        client
            .post(format!(
                "{base_url}/v1/entities/{}/publications",
                entity["id"].as_str().unwrap()
            ))
            .json(&json!({"context_id":web["id"]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    client
        .post(format!(
            "{base_url}/v1/entities/{}/publications",
            second["id"].as_str().unwrap()
        ))
        .json(&json!({"context_id":app["id"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    for (channel, direction) in [
        ("pub_sort_web", "asc"),
        ("pub_sort_app", "asc"),
        ("pub_sort_web", "desc"),
    ] {
        let mut cursor: Option<String> = None;
        let mut found = Vec::new();
        loop {
            let response: Value = client.post(format!("{base_url}/v1/entities/search"))
                .json(&json!({"blueprint":{"code":"publication_sort_product"},
                    "sort":{"field":"publication_status","direction":direction,"context_code":channel},
                    "page":{"size":1,"cursor":cursor}}))
                .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
            found.push(response["items"][0]["id"].clone());
            cursor = response["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(found.len(), 3);
        if channel == "pub_sort_web" && direction == "asc" {
            assert_eq!(found[0], second["id"]);
        } else {
            assert_eq!(found[2], second["id"]);
            assert!(found[..2].contains(&first["id"]));
            assert!(found[..2].contains(&third["id"]));
        }
    }
    let first_page: Value = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"publication_sort_product"},
            "sort":{"field":"publication_status","direction":"asc","context_code":"pub_sort_web"},
            "page":{"size":1}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mismatch = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"publication_sort_product"},
            "sort":{"field":"publication_status","direction":"asc","context_code":"pub_sort_app"},
            "page":{"size":1,"cursor":first_page["next_cursor"]}}))
        .send()
        .await
        .unwrap();
    assert_eq!(mismatch.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let missing = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"publication_sort_product"},
            "sort":{"field":"publication_status","direction":"asc"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNPROCESSABLE_ENTITY);
    client
        .put(format!(
            "{base_url}/publication-channels/{}",
            app["id"].as_str().unwrap()
        ))
        .json(&json!({"enabled":false}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let disabled = client
        .post(format!("{base_url}/v1/entities/search"))
        .json(&json!({"blueprint":{"code":"publication_sort_product"},
            "sort":{"field":"publication_status","direction":"asc","context_code":"pub_sort_app"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::UNPROCESSABLE_ENTITY);
    server.abort();
}
