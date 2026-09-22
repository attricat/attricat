use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn presentation_asset_constraints_are_fail_closed(pool: PgPool) {
    let workspace_id = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,'logo','image/png',16,$3,1,1,$4)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind("a".repeat(64))
        .bind("presentation-assets/valid")
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,'logo','image/jpeg',16,$3,1,1,$4)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind("b".repeat(64))
            .bind("presentation-assets/jpeg-logo")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,'illustration','image/svg+xml',16,$3,1,1,$4)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind("c".repeat(64))
            .bind("presentation-assets/svg-dimensions")
            .execute(&pool)
            .await
            .is_err()
    );
    for (width, height, key) in [
        (Some(1_i32), None, "presentation-assets/missing-height"),
        (None, Some(1_i32), "presentation-assets/missing-width"),
    ] {
        assert!(
            sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,'illustration','image/png',16,$3,$4,$5,$6)")
                .bind(Uuid::new_v4())
                .bind(workspace_id)
                .bind("d".repeat(64))
                .bind(width)
                .bind(height)
                .bind(key)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("INSERT INTO presentation_assets (id,workspace_id,purpose,media_type,byte_size,sha256,width,height,object_key) VALUES ($1,$2,'illustration','image/png',2097153,$3,1,1,$4)")
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind("d".repeat(64))
            .bind("presentation-assets/too-large")
            .execute(&pool)
            .await
            .is_err()
    );

    let plan_id = Uuid::new_v4();
    sqlx::query("INSERT INTO solution_pack_plans (id,workspace_id,source_kind,source_metadata,archive_sha256,manifest_version,pack_id,pack_name,pack_version,pack_description,host_api,prefix,blueprint_publication,ready,expires_at) VALUES ($1,$2,'local_archive','{}',$3,1,'attricat.assets','Assets','1.0.0','Assets','^1.0','assets','draft',false,now()+interval '1 hour')")
        .bind(plan_id)
        .bind(workspace_id)
        .bind("e".repeat(64))
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO solution_pack_plan_asset_objects (plan_id,workspace_id,logical_key,target_id,object_key,media_type,byte_size,sha256,state) VALUES ($1,$2,'assets/logo',$3,'presentation-assets/staged','image/svg+xml',10,$4,'staged')")
        .bind(plan_id).bind(workspace_id).bind(Uuid::new_v4()).bind("f".repeat(64))
        .execute(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE solution_pack_plan_asset_objects SET state='deleted' WHERE plan_id=$1")
            .bind(plan_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE solution_pack_plan_asset_objects SET byte_size=2097153 WHERE plan_id=$1"
        )
        .bind(plan_id)
        .execute(&pool)
        .await
        .is_err()
    );
}
