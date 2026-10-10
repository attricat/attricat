mod support;

use std::{io::Cursor, sync::Arc, time::Duration};

use api::{
    file_worker::{FileWorker, WorkerConfig},
    storage::FakeObjectStore,
};
use image::{DynamicImage, ImageFormat};
use reqwest::multipart::{Form, Part};
use support::*;

fn worker(pool: PgPool, store: Arc<FakeObjectStore>) -> FileWorker {
    FileWorker::new(
        pool,
        store,
        WorkerConfig {
            worker_id: "avatar-integration-test".into(),
            max_pixels: 40_000_000,
            max_attempts: 3,
            delete_grace: Duration::ZERO,
        },
    )
}

fn image(format: ImageFormat) -> Vec<u8> {
    let image =
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(640, 480, image::Rgb([9, 8, 7])));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

fn avatar_form(bytes: Vec<u8>, name: &str, mime: &str) -> Form {
    Form::new().part(
        "file",
        Part::bytes(bytes)
            .file_name(name.to_owned())
            .mime_str(mime)
            .unwrap(),
    )
}

/// A workspace member without any role grant.
async fn plain_member(pool: &PgPool) -> Client {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'avatar-viewer@example.test')")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::new_v4())
    .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-attricat-user-id",
        reqwest::header::HeaderValue::from_str(&user_id.to_string()).unwrap(),
    );
    headers.insert(
        "x-attricat-workspace-id",
        reqwest::header::HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

async fn get_status(client: &Client, url: String) -> StatusCode {
    client.get(url).send().await.unwrap().status()
}

#[sqlx::test]
async fn avatars_are_processed_shared_with_members_and_reclaimed(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let owner = authenticated_client();
    let viewer = plain_member(&pool).await;
    let upload = |form: Form| {
        owner
            .put(format!("{base_url}/auth/avatar"))
            .multipart(form)
            .send()
    };

    // Only PNG and JPEG are accepted, judged by content rather than name.
    for (bytes, name, mime) in [
        (image(ImageFormat::WebP), "avatar.webp", "image/webp"),
        (image(ImageFormat::WebP), "avatar.png", "image/png"),
        (b"not an image".to_vec(), "avatar.txt", "text/plain"),
    ] {
        assert_eq!(
            upload(avatar_form(bytes, name, mime))
                .await
                .unwrap()
                .status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{name}"
        );
    }
    let two_files = avatar_form(image(ImageFormat::Png), "a.png", "image/png").part(
        "file",
        Part::bytes(image(ImageFormat::Png))
            .file_name("b.png")
            .mime_str("image/png")
            .unwrap(),
    );
    assert_eq!(
        upload(two_files).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let uploaded = upload(avatar_form(
        image(ImageFormat::Jpeg),
        "me.jpg",
        "image/jpeg",
    ))
    .await
    .unwrap();
    assert_eq!(uploaded.status(), StatusCode::CREATED);
    let uploaded: Value = uploaded.json().await.unwrap();
    assert_eq!(uploaded["status"], "queued");
    let first_id = uploaded["file_id"].as_str().unwrap().to_owned();
    let session: Value = owner
        .get(format!("{base_url}/auth/session"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(session["avatar"]["file_id"], first_id.as_str());
    assert_eq!(session["avatar"]["status"], "queued");

    // Pending avatars are not shown to other people.
    let members: Value = owner
        .get(format!("{base_url}/workspace/members"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let owner_member = |members: &Value| {
        members
            .as_array()
            .unwrap()
            .iter()
            .find(|member| member["user_id"] == BOOTSTRAP_OWNER_ID)
            .unwrap()
            .clone()
    };
    assert_eq!(owner_member(&members)["avatar_file_id"], Value::Null);

    assert!(
        worker(pool.clone(), store.clone())
            .run_once()
            .await
            .unwrap()
    );
    let variants: Vec<(String, i32, i32)> =
        sqlx::query_as("SELECT kind, width, height FROM file_variants WHERE file_id = $1")
            .bind(first_id.parse::<Uuid>().unwrap())
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(variants, vec![("avatar".into(), 256, 256)]);

    let members: Value = owner
        .get(format!("{base_url}/workspace/members"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(owner_member(&members)["avatar_file_id"], first_id.as_str());
    let audit: Value = owner
        .get(format!("{base_url}/audit-events?limit=5"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        audit["events"][0]["actor_avatar_file_id"],
        first_id.as_str()
    );

    // Any member may read the processed avatar, never the original upload.
    let avatar_url = |id: &str| format!("{base_url}/files/{id}/variants/avatar/download");
    let response = viewer.get(avatar_url(&first_id)).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/webp");
    for path in [
        format!("{base_url}/files/{first_id}"),
        format!("{base_url}/files/{first_id}/download"),
        format!("{base_url}/files/{first_id}/variants/thumbnail/download"),
    ] {
        assert_eq!(get_status(&viewer, path).await, StatusCode::FORBIDDEN);
    }

    // Replacing the avatar orphans the old file, which reconciliation reclaims.
    let replaced: Value = upload(avatar_form(image(ImageFormat::Png), "me.png", "image/png"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let second_id = replaced["file_id"].as_str().unwrap().to_owned();
    assert_ne!(first_id, second_id);
    assert_eq!(
        get_status(&viewer, avatar_url(&first_id)).await,
        StatusCode::FORBIDDEN
    );
    let file_worker = worker(pool.clone(), store.clone());
    while file_worker.run_once().await.unwrap() {}
    let statuses: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, status FROM files WHERE purpose = 'avatar' ORDER BY created_at")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        statuses,
        vec![
            (first_id.parse().unwrap(), "deleted".into()),
            (second_id.parse().unwrap(), "ready".into()),
        ]
    );
    assert_eq!(
        get_status(&viewer, avatar_url(&second_id)).await,
        StatusCode::OK
    );

    let removed = owner
        .delete(format!("{base_url}/auth/avatar"))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
    let session: Value = owner
        .get(format!("{base_url}/auth/session"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(session["avatar"], Value::Null);
    assert_eq!(
        get_status(&viewer, avatar_url(&second_id)).await,
        StatusCode::FORBIDDEN
    );
    server.abort();
}
