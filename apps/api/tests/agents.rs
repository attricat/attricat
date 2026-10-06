mod support;

use std::{sync::Arc, time::Duration};

use api::{
    agent_tools::{
        ToolError, change_names, change_summary_named, execute_mutation, execute_read,
        pin_entity_versions,
    },
    agent_worker,
    agents::AgentProviderConfig,
    file_worker::{FileWorker, WorkerConfig},
    repository::{CatalogRepository, RepositoryError},
    storage::FakeObjectStore,
    task_worker::TaskHandler,
};
use axum::{Router, routing::post};
use reqwest::multipart::{Form, Part};
use support::*;
use tokio::time::{sleep, timeout};

#[sqlx::test]
async fn agent_blueprint_reads_list_summaries_and_fetch_one_definition(pool: PgPool) {
    let (_base_url, server) = start_server(pool.clone()).await;
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let definition = |attribute: &str| {
        format!(
            "format_version = 1\ncode = \"agent_desk\"\nname = \"Desk\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"{attribute}\"]\n\n[[attributes]]\ncode = \"{attribute}\"\nvalue_type = \"string\""
        )
    };
    let created = execute_mutation(
        &repository,
        actor,
        "create_blueprint",
        json!({"definition": definition("title")}),
    )
    .await
    .unwrap();
    let id = created["blueprint"]["id"].as_str().unwrap().to_owned();
    let get =
        |arguments: Value| execute_read(&repository, actor, workspace, "get_blueprint", arguments);

    // With no published revision, the latest draft is returned.
    let draft = get(json!({"code":"agent_desk"})).await.unwrap();
    assert_eq!(draft["blueprint"]["status"], "draft");
    assert_eq!(draft["attributes"][0]["code"], "title");

    execute_mutation(
        &repository,
        actor,
        "publish_blueprint",
        json!({"blueprint_id":id,"version":1}),
    )
    .await
    .unwrap();
    execute_mutation(
        &repository,
        actor,
        "create_blueprint_revision",
        json!({"blueprint_id":id,"definition":definition("label")}),
    )
    .await
    .unwrap();

    // The published revision wins over a newer draft; an exact version
    // reaches the draft.
    let published = get(json!({"code":"agent_desk"})).await.unwrap();
    assert_eq!(published["blueprint"]["version"], 1);
    assert!(
        published["blueprint"]["definition"]
            .as_str()
            .unwrap()
            .contains("title")
    );
    let next = get(json!({"code":"agent_desk","version":2})).await.unwrap();
    assert_eq!(next["blueprint"]["status"], "draft");
    assert!(matches!(
        get(json!({"code":"agent_desk","version":3})).await,
        Err(ToolError::Repository(RepositoryError::NotFound(_)))
    ));

    // Listing is a bounded summary without definitions or schemas.
    let listed = execute_read(&repository, actor, workspace, "list_blueprints", json!({}))
        .await
        .unwrap();
    let desk = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|blueprint| blueprint["code"] == "agent_desk")
        .unwrap();
    assert_eq!(desk["version"], 2);
    assert_eq!(desk["status"], "draft");
    assert_eq!(desk["kind"], "entity");
    for omitted in ["definition", "entity_schema", "views", "attributes"] {
        assert!(
            desk.get(omitted).is_none(),
            "{omitted} should not be listed"
        );
    }

    let guide = execute_read(
        &repository,
        actor,
        workspace,
        "blueprint_authoring_guide",
        json!({"topic":"status_control"}),
    )
    .await
    .unwrap();
    assert_eq!(guide["topic"], "status_control");
    assert!(
        guide["markdown"]
            .as_str()
            .unwrap()
            .contains("x-attricat-status")
    );
    let default = execute_read(
        &repository,
        actor,
        workspace,
        "blueprint_authoring_guide",
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(default["topic"], "blueprints");
    assert!(
        execute_read(
            &repository,
            actor,
            workspace,
            "blueprint_authoring_guide",
            json!({"topic":"storage"})
        )
        .await
        .is_err()
    );
    server.abort();
}

#[sqlx::test]
async fn approved_entity_edits_apply_to_the_proposed_state_and_name_their_targets(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"pinned_desk\"\nname = \"Pinned desk\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"\n\n[[attributes]]\ncode = \"related\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"pinned_desk\"").await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap().to_owned();
    let other = create_entity(&client, &base_url, &blueprint).await;
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let set_title = |title: &str| {
        json!({"entity_id": entity_id, "values": [
            {"kind":"scalar","attribute_code":"title","context_id":null,"value":title}
        ]})
    };
    execute_mutation(
        &repository,
        actor,
        "set_entity_values",
        set_title("Oak desk"),
    )
    .await
    .unwrap();

    // A proposal without expected_updated_at is pinned to the current state,
    // and its summary names the entity by its display label.
    let mut proposal = set_title("Walnut desk");
    pin_entity_versions(&repository, "set_entity_values", &mut proposal)
        .await
        .unwrap();
    let pinned = proposal["expected_updated_at"].clone();
    assert!(pinned.is_string());
    let names = change_names(
        &repository,
        actor,
        workspace,
        "set_entity_values",
        &proposal,
    )
    .await
    .unwrap();
    assert_eq!(
        change_summary_named("set_entity_values", &proposal, &names).unwrap(),
        format!("Set attribute values on entity pinned_desk 'Oak desk' ({entity_id}).")
    );

    // An explicit expected_updated_at is kept as the model supplied it.
    let mut explicit =
        json!({"entity_id": entity_id, "expected_updated_at": "2026-01-01T00:00:00Z"});
    pin_entity_versions(&repository, "delete_entity", &mut explicit)
        .await
        .unwrap();
    assert_eq!(explicit["expected_updated_at"], "2026-01-01T00:00:00Z");

    // Someone saves the entity while the proposal waits for approval.
    execute_mutation(
        &repository,
        actor,
        "set_entity_values",
        set_title("Pine desk"),
    )
    .await
    .unwrap();
    assert!(matches!(
        execute_mutation(&repository, actor, "set_entity_values", proposal).await,
        Err(ToolError::Repository(RepositoryError::StaleEntity))
    ));
    let mut deletion = json!({"entity_id": entity_id});
    let mut batch = json!({"operations": [
        {"op":"update","entity_id": entity_id,"values": []},
        {"op":"create","blueprint":{"code":"pinned_desk"}}
    ]});
    pin_entity_versions(&repository, "apply_entity_batch", &mut batch)
        .await
        .unwrap();
    assert!(batch["operations"][0]["expected_updated_at"].is_string());
    assert!(batch["operations"][1].get("expected_updated_at").is_none());
    pin_entity_versions(&repository, "delete_entity", &mut deletion)
        .await
        .unwrap();
    assert_ne!(deletion["expected_updated_at"], pinned);
    execute_mutation(
        &repository,
        actor,
        "update_entity_annotations",
        json!({"entity_id": entity_id, "system_tags": ["reviewed"]}),
    )
    .await
    .unwrap();
    for (name, arguments) in [
        ("delete_entity", deletion.clone()),
        (
            "update_entity_annotations",
            json!({"entity_id": entity_id, "system_tags": [], "expected_updated_at": deletion["expected_updated_at"]}),
        ),
        (
            "remove_entity_values",
            json!({"entity_id": entity_id, "remove_values": [{"attribute_code":"title"}], "expected_updated_at": deletion["expected_updated_at"]}),
        ),
        (
            "replace_entity_relationships",
            json!({"entity_id": entity_id, "relationships": [{"attribute_code":"related","target_entity_ids":[other["id"]]}], "expected_updated_at": deletion["expected_updated_at"]}),
        ),
        (
            "remove_entity_relationships",
            json!({"entity_id": entity_id, "relationships": [{"attribute_code":"related","target_entity_ids":[other["id"]]}], "expected_updated_at": deletion["expected_updated_at"]}),
        ),
        (
            "link_file",
            json!({"entity_id": entity_id, "attribute_code":"title", "file_id": Uuid::new_v4(), "expected_updated_at": deletion["expected_updated_at"]}),
        ),
    ] {
        assert!(
            matches!(
                execute_mutation(&repository, actor, name, arguments).await,
                Err(ToolError::Repository(RepositoryError::StaleEntity))
            ),
            "{name} must refuse a stale proposal"
        );
    }

    // A current pin lets the write through.
    let mut link = json!({"entity_id": entity_id, "relationships": [{"attribute_code":"related","target_entity_ids":[other["id"]]}]});
    pin_entity_versions(&repository, "replace_entity_relationships", &mut link)
        .await
        .unwrap();
    execute_mutation(&repository, actor, "replace_entity_relationships", link)
        .await
        .unwrap();

    // A migration proposed before another edit is refused too.
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap();
    let revision = execute_mutation(&repository, actor, "create_blueprint_revision", json!({
        "blueprint_id": blueprint_id,
        "definition": "format_version = 1\ncode = \"pinned_desk\"\nname = \"Pinned desk\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"\n\n[[attributes]]\ncode = \"related\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"pinned_desk\"\n\n[[attributes]]\ncode = \"finish\"\nvalue_type = \"string\""
    }))
    .await
    .unwrap();
    execute_mutation(
        &repository,
        actor,
        "publish_blueprint",
        json!({"blueprint_id": blueprint_id, "version": revision["blueprint"]["version"]}),
    )
    .await
    .unwrap();
    let mut migration = json!({"entity_id": entity_id});
    pin_entity_versions(&repository, "migrate_entity", &mut migration)
        .await
        .unwrap();
    execute_mutation(
        &repository,
        actor,
        "set_entity_values",
        set_title("Ash desk"),
    )
    .await
    .unwrap();
    assert!(matches!(
        execute_mutation(&repository, actor, "migrate_entity", migration).await,
        Err(ToolError::Repository(RepositoryError::StaleEntity))
    ));
    let mut migration = json!({"entity_id": entity_id});
    pin_entity_versions(&repository, "migrate_entity", &mut migration)
        .await
        .unwrap();
    assert_eq!(
        execute_mutation(&repository, actor, "migrate_entity", migration)
            .await
            .unwrap()["migrated"],
        true
    );

    let mut current = json!({"entity_id": entity_id});
    pin_entity_versions(&repository, "delete_entity", &mut current)
        .await
        .unwrap();
    execute_mutation(&repository, actor, "delete_entity", current)
        .await
        .unwrap();
    server.abort();
}

#[sqlx::test]
async fn agent_reads_incoming_links_hierarchies_labels_and_reusable_attributes(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"linked_part\"\nname = \"Linked part\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"\n\n[[attributes]]\ncode = \"parent\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"linked_part\"\ncardinality = \"one\"").await;
    create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"linked_order\"\nname = \"Linked order\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"\n\n[[attributes]]\ncode = \"parts\"\nvalue_type = \"relationship\"\ntarget_blueprint = \"linked_part\"").await;
    let create = |blueprint: &'static str,
                  title: &'static str,
                  links: Vec<(&'static str, Value)>| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            let mut values = vec![
                json!({"kind":"scalar","attribute_code":"title","context_id":null,"value":title}),
            ];
            values.extend(links.into_iter().map(|(field, target)| {
                json!({"kind":"relationship","attribute_code":field,"context_id":null,"target_entity_id":target})
            }));
            let entity: Value = client
                .post(format!("{base_url}/v1/entities"))
                .json(&json!({"blueprint":{"code":blueprint},"values":values}))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
            entity["id"].clone()
        }
    };
    let root = create("linked_part", "Frame", vec![]).await;
    let wheel = create("linked_part", "Wheel", vec![("parent", root.clone())]).await;
    let spoke = create("linked_part", "Spoke", vec![("parent", wheel.clone())]).await;
    create("linked_order", "Order 1", vec![("parts", root.clone())]).await;
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let read = |name: &'static str, arguments: Value| {
        execute_read(&repository, actor, workspace, name, arguments)
    };

    // Without selectors, every linking field is found and searched.
    let incoming = read("get_incoming_relationships", json!({"entity_id": root}))
        .await
        .unwrap();
    assert_eq!(
        incoming["fields"],
        json!([
            {"source_blueprint":"linked_order","field":"parts","source_count":1},
            {"source_blueprint":"linked_part","field":"parent","source_count":1},
        ])
    );
    assert_eq!(incoming["items"].as_array().unwrap().len(), 2);
    let paged = read(
        "get_incoming_relationships",
        json!({"entity_id": root, "relationships": [{"source_blueprint":"linked_part","field":"parent"}], "page": {"size": 1}}),
    )
    .await
    .unwrap();
    assert_eq!(paged["items"][0]["id"], wheel);
    assert_eq!(paged["items"][0]["display"]["default"], "Wheel");
    assert!(paged["next_cursor"].is_null());
    let unlinked = read("get_incoming_relationships", json!({"entity_id": spoke}))
        .await
        .unwrap();
    assert_eq!(unlinked["fields"], json!([]));
    assert_eq!(unlinked["items"], json!([]));
    assert!(
        read(
            "get_incoming_relationships",
            json!({"entity_id": root, "page": {"size": 51}})
        )
        .await
        .is_err()
    );

    let hierarchy = read(
        "get_entity_hierarchy",
        json!({"entity_id": spoke, "field": "parent"}),
    )
    .await
    .unwrap();
    let path: Vec<_> = hierarchy["paths"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].clone())
        .collect();
    assert!(path.contains(&root) && path.contains(&wheel), "{hierarchy}");
    assert_eq!(hierarchy["cycle_detected"], false);
    assert!(
        read(
            "get_entity_hierarchy",
            json!({"entity_id": spoke, "field": "title"})
        )
        .await
        .is_err()
    );

    let labels = read(
        "get_entity_labels",
        json!({"entity_ids": [root, wheel, Uuid::new_v4()]}),
    )
    .await
    .unwrap();
    assert_eq!(labels["items"].as_array().unwrap().len(), 2);
    assert!(
        read("get_entity_labels", json!({"entity_ids": []}))
            .await
            .is_err()
    );

    let draft: Value = client
        .post(format!("{base_url}/reusable-attributes"))
        .json(&json!({"definition": "code = \"weight\"\nname = \"Weight\"\nvalue_type = \"number\"\n"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let listed = read("list_reusable_attributes", json!({})).await.unwrap();
    assert_eq!(listed["attributes"], json!([]));
    let drafts = read("list_reusable_attributes", json!({"include_drafts": true}))
        .await
        .unwrap();
    assert_eq!(drafts["attributes"][0]["code"], "weight");
    assert!(drafts["attributes"][0].get("definition").is_none());
    let full = read(
        "list_reusable_attributes",
        json!({"code": format!("{}:weight", draft["namespace"].as_str().unwrap()), "include_drafts": true}),
    )
    .await
    .unwrap();
    assert!(
        full["revisions"][0]["definition"]
            .as_str()
            .unwrap()
            .contains("weight")
    );
    assert!(
        read("list_reusable_attributes", json!({"code": "missing"}))
            .await
            .is_err()
    );
    server.abort();
}

#[sqlx::test]
async fn agent_duplicates_comments_validates_and_reviews_catalog_health(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let definition = |extra: &str| {
        format!(
            "format_version = 1\ncode = \"noted_item\"\nname = \"Noted item\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"{extra}"
        )
    };
    let blueprint = create_blueprint(&client, &base_url, &definition("")).await;
    let blueprint_id = blueprint["blueprint"]["id"].as_str().unwrap().to_owned();
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap().to_owned();
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let read = |name: &'static str, arguments: Value| {
        execute_read(&repository, actor, workspace, name, arguments)
    };
    execute_mutation(
        &repository,
        actor,
        "set_entity_values",
        json!({"entity_id": entity_id, "values": [
            {"kind":"scalar","attribute_code":"title","context_id":null,"value":"Lamp"}
        ]}),
    )
    .await
    .unwrap();

    let copy = execute_mutation(
        &repository,
        actor,
        "duplicate_entity",
        json!({"entity_id": entity_id}),
    )
    .await
    .unwrap();
    assert_ne!(copy["id"], entity["id"]);
    let labels = read("get_entity_labels", json!({"entity_ids": [copy["id"]]}))
        .await
        .unwrap();
    assert_eq!(labels["items"][0]["display"]["default"], "Lamp");

    // Comments are written as the initiating user and read back bounded.
    for body in [
        "First note".to_owned(),
        "x".repeat(1_500),
        "Third note".to_owned(),
    ] {
        execute_mutation(
            &repository,
            actor,
            "add_entity_comment",
            json!({"entity_id": entity_id, "body": body}),
        )
        .await
        .unwrap();
    }
    assert!(
        execute_mutation(
            &repository,
            actor,
            "add_entity_comment",
            json!({"entity_id": entity_id, "body": "  "})
        )
        .await
        .is_err()
    );
    let first = read(
        "list_entity_comments",
        json!({"entity_id": entity_id, "limit": 2}),
    )
    .await
    .unwrap();
    assert_eq!(first["items"][0]["body"], "Third note");
    assert_eq!(first["items"][0]["author_user_id"], json!(actor));
    assert_eq!(first["items"][1]["body"].as_str().unwrap().len(), 1_000);
    assert_eq!(first["items"][1]["body_truncated"], true);
    let rest = read(
        "list_entity_comments",
        json!({"entity_id": entity_id, "limit": 2, "before": first["next_before"]}),
    )
    .await
    .unwrap();
    assert_eq!(rest["items"][0]["body"], "First note");
    assert!(rest["next_before"].is_null());

    let rule = "format_version = 1\ncode = \"noted_title\"\nname = \"Noted title\"\nseverity = \"warning\"\n[[triggers]]\ntype = \"manual\"\n[predicate]\ntype = \"required\"\nattribute_code = \"title\"";
    assert_eq!(
        read("validate_rule_definition", json!({"definition": rule}))
            .await
            .unwrap()["valid"],
        true
    );
    assert!(matches!(
        read(
            "validate_rule_definition",
            json!({"definition": "format_version = 1"})
        )
        .await,
        Err(ToolError::Repository(
            RepositoryError::InvalidRuleDefinition(_)
        ))
    ));
    let workflow = "format_version = 2\ncode = \"noted_workflow\"\nname = \"Noted workflow\"\n[[triggers]]\ntype = \"manual\"\n[[actions]]\ntype = \"system_tags_add\"\ntags = [\"checked\"]";
    assert_eq!(
        read(
            "validate_workflow_definition",
            json!({"definition": workflow})
        )
        .await
        .unwrap()["valid"],
        true
    );
    assert!(matches!(
        read(
            "validate_workflow_definition",
            json!({"definition": "code = 'x'"})
        )
        .await,
        Err(ToolError::Repository(
            RepositoryError::InvalidWorkflowDefinition(_)
        ))
    ));

    let created: Value = client
        .post(format!("{base_url}/rules"))
        .json(&json!({"blueprint_id": blueprint_id, "blueprint_version": 1, "context_id": null, "definition": rule}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let finding_id = Uuid::new_v4();
    sqlx::query("INSERT INTO rule_findings (id,workspace_id,rule_id,rule_version,entity_id,evaluation_key,severity,message,evidence) VALUES ($1,$2,$3,1,$4,'noted','warning','Check title','{}')")
        .bind(finding_id)
        .bind(workspace)
        .bind(created["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .bind(entity_id.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let acknowledged = execute_mutation(
        &repository,
        actor,
        "acknowledge_rule_finding",
        json!({"finding_id": finding_id}),
    )
    .await
    .unwrap();
    assert_eq!(acknowledged["state"], "acknowledged");

    // A published revision that removes an attribute reports what a batch
    // migration would drop.
    let revision = execute_mutation(&repository, actor, "create_blueprint_revision", json!({
        "blueprint_id": blueprint_id,
        "definition": "format_version = 1\ncode = \"noted_item\"\nname = \"Noted item\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"label\"]\n\n[[attributes]]\ncode = \"label\"\nvalue_type = \"string\"",
    }))
    .await
    .unwrap();
    let version = revision["blueprint"]["version"].clone();
    execute_mutation(
        &repository,
        actor,
        "publish_blueprint",
        json!({"blueprint_id": blueprint_id, "version": version}),
    )
    .await
    .unwrap();
    let impact = read(
        "preview_blueprint_migration_impact",
        json!({"blueprint_id": blueprint_id, "version": version}),
    )
    .await
    .unwrap();
    assert_eq!(impact["removed_attribute_codes"], json!(["title"]));
    assert_eq!(impact["entities_with_removed_values"], 2);

    let health = read(
        "data_health_details",
        json!({"section": "blueprints", "blueprint": "noted_item"}),
    )
    .await
    .unwrap();
    assert_eq!(health["items"].as_array().unwrap().len(), 1);
    assert_eq!(health["items"][0]["active_entities"], 2);
    for section in ["freshness", "completeness", "contexts", "relationships"] {
        read("data_health_details", json!({"section": section}))
            .await
            .unwrap();
    }
    assert!(
        read("data_health_details", json!({"section": "storage"}))
            .await
            .is_err()
    );
    server.abort();
}

#[sqlx::test]
async fn agent_annotation_and_context_edits_use_catalog_validation(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"annotation_product\"\nname = \"Annotation product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"").await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let updated = execute_mutation(&repository, actor, "update_entity_annotations",
        json!({"entity_id":entity_id,"system_tags":["reviewed"],"system_metadata":{"source":"agent"}})).await.unwrap();
    assert_eq!(updated["system_tags"], json!(["reviewed"]));
    assert_eq!(updated["system_metadata"]["source"], "agent");
    let context = execute_mutation(
        &repository,
        actor,
        "create_context",
        json!({"code":"agent_region","data":{"region":"one"}}),
    )
    .await
    .unwrap();
    let id = context["id"].as_str().unwrap();
    let read = execute_read(
        &repository,
        actor,
        workspace,
        "get_context",
        json!({"context_id":id}),
    )
    .await
    .unwrap();
    assert_eq!(read["data"]["region"], "one");
    let updated = execute_mutation(&repository, actor, "update_context",
        json!({"context_id":id,"parent_id":"00000000-0000-4000-8000-000000000001","data":{"region":"two"}})).await.unwrap();
    assert_eq!(updated["data"]["region"], "two");
    assert_eq!(
        execute_mutation(
            &repository,
            actor,
            "delete_context",
            json!({"context_id":id})
        )
        .await
        .unwrap()["deleted"],
        true
    );
    assert!(
        execute_mutation(
            &repository,
            actor,
            "update_entity_annotations",
            json!({"entity_id":entity_id})
        )
        .await
        .is_err()
    );
    server.abort();
}

#[sqlx::test]
async fn agent_operational_diagnostics_are_bounded_and_omit_internal_payloads(pool: PgPool) {
    CatalogRepository::system(pool.clone())
        .ensure_rule_permissions()
        .await
        .unwrap();
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"diagnostic_product\"\nname = \"Diagnostic product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"").await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    let rule: Value = client.post(format!("{base_url}/rules"))
        .json(&json!({"blueprint_id":blueprint["blueprint"]["id"],"blueprint_version":1,"context_id":null,
            "definition":"format_version = 1\ncode = \"diagnostic_rule\"\nname = \"Diagnostic rule\"\nseverity = \"warning\"\n[[triggers]]\ntype = \"manual\"\n[predicate]\ntype = \"required\"\nattribute_code = \"title\""}))
        .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let rule_id: Uuid = rule["id"].as_str().unwrap().parse().unwrap();
    let workspace: Uuid = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor: Uuid = BOOTSTRAP_OWNER_ID.parse().unwrap();
    for index in 0..2 {
        sqlx::query("INSERT INTO rule_findings (id,workspace_id,rule_id,rule_version,entity_id,evaluation_key,severity,message,evidence) VALUES ($1,$2,$3,1,$4,$5,'warning','Check title','{\"secret\":\"hidden\"}')")
            .bind(Uuid::new_v4()).bind(workspace).bind(rule_id).bind(entity_id.parse::<Uuid>().unwrap())
            .bind(format!("check-{index}")).execute(&pool).await.unwrap();
    }
    client
        .post(format!("{base_url}/rules/{rule_id}/versions/1/publish"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{base_url}/rules/{rule_id}/versions/1/enable"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{base_url}/rules/{rule_id}/run-now"))
        .json(&json!({"entity_id":entity_id,"dry_run":true,"idempotency_key":"diagnostic"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let definition = "format_version = 2\ncode = \"diagnostic_workflow\"\nname = \"Diagnostic workflow\"\n[[triggers]]\ntype = \"manual\"\n[[actions]]\ntype = \"system_tags_add\"\ntags = [\"checked\"]";
    let workflow: Value = client
        .post(format!("{base_url}/workflows"))
        .json(&json!({"definition":definition}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let workflow_id = workflow["id"].as_str().unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/publish"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!(
            "{base_url}/workflows/{workflow_id}/versions/1/enable"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    for index in 0..2 {
        client
            .post(format!("{base_url}/workflows/{workflow_id}/run-now"))
            .json(&json!({"entity_id":entity_id,"idempotency_key":format!("run-{index}")}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let health = execute_read(
        &repository,
        actor,
        workspace,
        "data_health_summary",
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(health["active_entities"], 1);
    let findings = execute_read(
        &repository,
        actor,
        workspace,
        "list_rule_findings",
        json!({"entity_id":entity_id,"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(findings["items"].as_array().unwrap().len(), 1);
    assert_eq!(findings["next_offset"], 1);
    assert!(findings.to_string().contains("Check title"));
    assert!(!findings.to_string().contains("hidden"));
    let workflows = execute_read(
        &repository,
        actor,
        workspace,
        "list_workflow_runs",
        json!({"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(workflows["items"].as_array().unwrap().len(), 1);
    assert_eq!(workflows["next_offset"], 1);
    assert!(workflows["items"][0].get("trigger_event").is_none());
    assert!(workflows["items"][0].get("last_error").is_none());
    let rule_definition = execute_read(
        &repository,
        actor,
        workspace,
        "get_rule_definition",
        json!({"rule_id":rule_id,"version":1}),
    )
    .await
    .unwrap();
    assert!(
        rule_definition["definition"]
            .as_str()
            .unwrap()
            .contains("required")
    );
    assert!(rule_definition.get("compiled_plan").is_none());
    let workflow_definition = execute_read(
        &repository,
        actor,
        workspace,
        "get_workflow_definition",
        json!({"workflow_id":workflow_id,"version":1}),
    )
    .await
    .unwrap();
    assert!(
        workflow_definition["definition"]
            .as_str()
            .unwrap()
            .contains("system_tags_add")
    );
    assert!(workflow_definition.get("compiled_plan").is_none());
    let rule_runs = execute_read(
        &repository,
        actor,
        workspace,
        "list_rule_runs",
        json!({"rule_id":rule_id,"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(rule_runs["items"].as_array().unwrap().len(), 1);
    assert_eq!(rule_runs["items"][0]["rule_id"], rule_id.to_string());
    assert!(rule_runs["items"][0].get("candidate_cursor").is_none());
    let workflow_run = execute_read(
        &repository,
        actor,
        workspace,
        "get_workflow_run",
        json!({"run_id":workflows["items"][0]["id"]}),
    )
    .await
    .unwrap();
    assert_eq!(workflow_run["id"], workflows["items"][0]["id"]);
    assert!(workflow_run.get("trigger_event").is_none());
    server.abort();
}

#[sqlx::test]
async fn agent_history_reads_and_entity_value_edits_use_scoped_catalog_services(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let blueprint = create_blueprint(&client, &base_url,
        "format_version = 1\ncode = \"agent_history_product\"\nname = \"Agent history product\"\nkind = \"entity\"\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]\n\n[[attributes]]\ncode = \"title\"\nvalue_type = \"string\"").await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let entity_id = entity["id"].as_str().unwrap();
    for title in ["first", "second", "third"] {
        client
            .post(format!("{base_url}/entities/{entity_id}/values"))
            .json(&json!({"values":[{"kind":"scalar","attribute_code":"title","value":title}]}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse().unwrap();
    let actor = BOOTSTRAP_OWNER_ID.parse().unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(workspace)
        .await
        .unwrap();
    let changes = execute_read(
        &repository,
        actor,
        workspace,
        "get_entity_changes",
        json!({"entity_id":entity_id,"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(changes["items"].as_array().unwrap().len(), 1);
    assert_eq!(changes["next_offset"], 1);
    let history = execute_read(
        &repository,
        actor,
        workspace,
        "get_value_history",
        json!({"entity_id":entity_id,"limit":1}),
    )
    .await
    .unwrap();
    assert_eq!(history["items"].as_array().unwrap().len(), 1);
    assert_eq!(history["next_offset"], 1);
    let history_id = history["items"][0]["id"].clone();
    execute_mutation(
        &repository,
        actor,
        "remove_entity_values",
        json!({"entity_id":entity_id,"remove_values":[{"attribute_code":"title"}]}),
    )
    .await
    .unwrap();
    let current: Value = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(current.as_array().unwrap().is_empty());
    execute_mutation(
        &repository,
        actor,
        "restore_entity_value",
        json!({"entity_id":entity_id,"history_id":history_id}),
    )
    .await
    .unwrap();
    let current: Value = client
        .get(format!("{base_url}/entities/{entity_id}/values/current"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current[0]["value"], "second");
    server.abort();
}

#[sqlx::test]
async fn agent_conversation_reads_and_persisted_sse_replay(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "nightly catalog review"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"].as_str().unwrap();
    let conversations: Value = client
        .get(format!("{base_url}/agent/conversations"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(conversations[0]["id"], conversation["id"]);

    let message_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
        .bind(file_id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Review this report\"')")
        .bind(message_id).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4()).bind(workspace_id).bind(message_id).bind(file_id).execute(&pool).await.unwrap();
    let messages: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{conversation_id}/messages"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["attachments"][0]["id"], file_id.to_string());
    assert_eq!(messages[0]["attachments"][0]["filename"], "report.pdf");

    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model, finished_at) VALUES ($1, $2, $3, 'manual', 'skipped', 'https://provider.test/v1', 'test', now())")
        .bind(run_id).bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap()).bind(conversation_id.parse::<Uuid>().unwrap()).execute(&pool).await.unwrap();
    let event_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_run_events (id, run_id, sequence, event_type, payload) VALUES ($1, $2, 0, 'terminal', '{\"status\":\"skipped\"}')")
        .bind(event_id).bind(run_id).execute(&pool).await.unwrap();
    let response = client
        .get(format!("{base_url}/agent/runs/{run_id}/events"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let replay = response.text().await.unwrap();
    assert!(replay.contains(&format!("id: {event_id}")));
    assert!(replay.contains("event: terminal"));
    server.abort();
}

#[sqlx::test]
async fn completed_conversation_gets_a_provider_generated_title(pool: PgPool) {
    let (_, server) = start_server(pool.clone()).await;
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_server = tokio::spawn(async move {
        axum::serve(provider_listener, Router::new().route("/v1/chat/completions", post(|| async {
            axum::Json(json!({"choices":[{"message":{"content":"\"Compare product prices\"","tool_calls":[]}}]}))
        }))).await.unwrap();
    });
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(format!("http://{provider_address}/v1")),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let provider = api::agent_provider::OpenAiCompatibleClient::new(&config).unwrap();
    let repository = CatalogRepository::system(pool)
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let conversation = repository
        .create_conversation(
            Some(BOOTSTRAP_OWNER_ID.parse().unwrap()),
            "Compare these items",
        )
        .await
        .unwrap();
    repository
        .append_conversation_message(conversation.id, None, "user", json!("Compare prices"))
        .await
        .unwrap();
    repository
        .append_conversation_message(
            conversation.id,
            None,
            "assistant",
            json!("The second item is cheaper"),
        )
        .await
        .unwrap();
    catalog_agent_runtime::conversation_title::maybe_generate_title(
        &repository,
        &provider,
        conversation.id,
    )
    .await;
    let updated = repository.get_conversation(conversation.id).await.unwrap();
    assert_eq!(updated.title, "Compare product prices");
    assert_eq!(updated.title_source, "generated");
    let draft = repository
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "Smart fill")
        .await
        .unwrap();
    repository
        .append_conversation_message(draft.id, None, "user", json!("Fill from the spec"))
        .await
        .unwrap();
    repository
        .append_conversation_message(
            draft.id,
            None,
            "assistant",
            json!({"draft_proposal":{"fields":{},"explanation":"Suggested a title"}}),
        )
        .await
        .unwrap();
    catalog_agent_runtime::conversation_title::maybe_generate_title(
        &repository,
        &provider,
        draft.id,
    )
    .await;
    assert_eq!(
        repository
            .get_conversation(draft.id)
            .await
            .unwrap()
            .title_source,
        "generated"
    );
    provider_server.abort();
    server.abort();
}

#[sqlx::test]
async fn conversation_search_paginates_and_manual_titles_win(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let repository = CatalogRepository::system(pool)
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let mut ids = Vec::new();
    for n in 0..32 {
        ids.push(
            repository
                .create_conversation(
                    Some(BOOTSTRAP_OWNER_ID.parse().unwrap()),
                    &format!("Review product {n}"),
                )
                .await
                .unwrap()
                .id,
        );
    }
    repository
        .append_conversation_message(ids[0], None, "user", json!("special request about sizing"))
        .await
        .unwrap();
    let client = authenticated_client();
    let first: Value = client
        .get(format!("{base_url}/agent/conversations/search?q=review"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 30);
    let cursor = first["next_cursor"].as_str().unwrap();
    let malformed = client
        .get(format!(
            "{base_url}/agent/conversations/search?cursor=invalid"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let second: Value = client
        .get(format!("{base_url}/agent/conversations/search"))
        .query(&[("q", "review"), ("cursor", cursor)])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert!(second["next_cursor"].is_null());
    let matching: Value = client
        .get(format!("{base_url}/agent/conversations/search?q=sizing"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(matching["items"][0]["id"], ids[0].to_string());
    assert!(
        repository
            .set_generated_conversation_title(ids[0], "Sizing research")
            .await
            .unwrap()
    );
    assert!(
        !repository
            .set_generated_conversation_title(ids[0], "Overwrite")
            .await
            .unwrap()
    );
    let manual = repository
        .update_conversation_title(ids[1], "Human title")
        .await
        .unwrap();
    assert_eq!(manual.title_source, "manual");
    assert!(
        !repository
            .set_generated_conversation_title(ids[1], "Overwrite")
            .await
            .unwrap()
    );
    server.abort();
}

#[sqlx::test]
async fn entity_conversations_validate_their_context_and_entity(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let context = Uuid::new_v4();
    let invalid = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title":"Entity discussion","context_id":context}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let missing = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title":"Entity discussion","entity_id":Uuid::new_v4()}))
        .send()
        .await
        .unwrap();
    assert!(!missing.status().is_success());
    let blueprint = create_blueprint(&client, &base_url,
        "format_version = 1\ncode = 'entity_chat_test'\nname = 'Entity chat test'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'").await;
    let entity = create_entity(&client, &base_url, &blueprint).await;
    let selected_context: Value = client
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code":"entity-chat-context","data":{}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let created: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title":"About this entity","entity_id":entity["id"],"context_id":selected_context["id"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let loaded: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{}",
            created["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(loaded["entity_id"], entity["id"]);
    assert_eq!(loaded["context_id"], selected_context["id"]);
    server.abort();
}

#[sqlx::test]
async fn submitting_a_message_and_enqueuing_its_run_are_atomic(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'Atomic send')",
    )
    .bind(conversation_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'atomic-send@example.test')")
        .bind(BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace_id)
        .await
        .unwrap();
    // The run's actor foreign key rejects the insert after the message insert.
    assert!(
        repository
            .submit_agent_message(
                conversation_id,
                Uuid::new_v4(),
                json!("failed send"),
                &[],
                "https://provider.test/v1",
                "test"
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .conversation_messages(conversation_id)
            .await
            .unwrap()
            .is_empty()
    );

    let run = repository
        .submit_agent_message(
            conversation_id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            json!("good send"),
            &[],
            "https://provider.test/v1",
            "test",
        )
        .await
        .unwrap();
    let messages = repository
        .conversation_messages(conversation_id)
        .await
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, json!("good send"));
    let task_id: Uuid =
        sqlx::query_scalar("SELECT id FROM tasks WHERE subject_id = $1 AND kind = 'agent_run.v1'")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(task_id, Uuid::nil());
}

#[sqlx::test]
async fn reconciliation_retains_conversation_attachments(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let attached_file_id = Uuid::new_v4();
    let orphan_file_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'Attachment retention')",
    )
    .bind(conversation_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"Keep this file\"')")
        .bind(message_id)
        .bind(conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    for file_id in [attached_file_id, orphan_file_id] {
        sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/report.pdf', 'ready')")
            .bind(file_id)
            .bind(workspace_id)
            .bind("0".repeat(64))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(message_id)
        .bind(attached_file_id)
        .execute(&pool)
        .await
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        Arc::new(FakeObjectStore::available()),
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();

    let attached_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(attached_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let orphan_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(orphan_file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(attached_deleted_at.is_none());
    assert!(orphan_deleted_at.is_some());
}

async fn slow_provider() -> axum::http::StatusCode {
    sleep(Duration::from_secs(2)).await;
    axum::http::StatusCode::OK
}

#[sqlx::test]
async fn agent_deltas_are_persisted_before_the_provider_finishes(pool: PgPool) {
    let (_, api_server) = start_server(pool.clone()).await;
    let release = Arc::new(tokio::sync::Notify::new());
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_release = release.clone();
    let provider = tokio::spawn(async move {
        axum::serve(provider_listener, Router::new().route("/v1/chat/completions", post(move || {
            let release = provider_release.clone();
            async move {
                axum::response::Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(axum::body::Body::from_stream(async_stream::stream! {
                        yield Ok::<_, std::convert::Infallible>(bytes::Bytes::from_static(b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n"));
                        release.notified().await;
                        yield Ok(bytes::Bytes::from_static(b"data: [DONE]\n\n"));
                    }))
                    .unwrap()
            }
        }))).await.unwrap();
    });
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(format!("http://{provider_address}/v1")),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let conversation = repository
        .create_conversation(Some(BOOTSTRAP_OWNER_ID.parse().unwrap()), "streamed")
        .await
        .unwrap();
    let run = repository
        .create_agent_run_for_user(
            conversation.id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    let client = api::agent_provider::OpenAiCompatibleClient::new(&config).unwrap();
    let store: Arc<dyn api::storage::ObjectStore> = Arc::new(FakeObjectStore::available());
    let worker_repository = repository.clone();
    let worker = tokio::spawn(async move {
        api::agent_runner::run(&worker_repository, &client, &store, run.id, conversation.id).await
    });
    timeout(Duration::from_secs(3), async {
        loop {
            if repository
                .agent_run_events_after(run.id, -1)
                .await
                .unwrap()
                .iter()
                .any(|event| {
                    event.event_type == "message_delta" && event.payload == json!({"text":"hello"})
                })
            {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("first delta should be visible before provider completion");
    assert!(!worker.is_finished());
    release.notify_one();
    timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    api_server.abort();
    provider.abort();
}

#[sqlx::test]
async fn agent_run_timeout_is_durably_failed_without_provider_details(pool: PgPool) {
    let (base_url, api_server) = start_server(pool.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "timeout test"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_server = tokio::spawn(async move {
        axum::serve(
            provider_listener,
            Router::new().route("/v1/chat/completions", post(slow_provider)),
        )
        .await
        .unwrap();
    });
    let config = AgentProviderConfig::from_values(|name| match name {
        "LLM_API_KEY" => Some("test-key".to_owned()),
        "LLM_BASE_URL" => Some(format!("http://{provider_address}/v1")),
        "LLM_RUN_TIMEOUT_SECONDS" => Some("1".to_owned()),
        _ => None,
    })
    .unwrap()
    .unwrap();
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let user_id = BOOTSTRAP_OWNER_ID.parse::<Uuid>().unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace_id)
        .await
        .unwrap();
    let task_repository =
        CatalogRepository::new(pool, support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap());
    let handler = agent_worker::AgentTaskHandler::new(
        task_repository.clone(),
        config.clone(),
        Arc::new(FakeObjectStore::available()),
    );
    let run = repository
        .create_agent_run_for_user(
            conversation["id"].as_str().unwrap().parse().unwrap(),
            user_id,
            config.base_url.as_str(),
            &config.model,
        )
        .await
        .unwrap();
    let task = task_repository
        .claim_task("agent-timeout-test", Duration::from_secs(30))
        .await
        .unwrap()
        .expect("agent run creation atomically enqueues a task");
    handler.handle(task).await.unwrap();

    let timed_out_run = timeout(Duration::from_secs(5), async {
        loop {
            let run = repository.get_agent_run(run.id).await.unwrap();
            if run.status == "failed" {
                break run;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("run should fail before the test deadline");
    assert_eq!(timed_out_run.error_code.as_deref(), Some("run_timeout"));
    assert_eq!(
        timed_out_run.error_message.as_deref(),
        Some("agent run exceeded configured timeout")
    );
    assert!(timed_out_run.finished_at.is_some());
    let events = repository.agent_run_events_after(run.id, -1).await.unwrap();
    assert!(events.iter().any(|event| {
        event.event_type == "error"
            && event.payload
                == json!({"code":"run_timeout","message":"agent run exceeded configured timeout"})
    }));
    assert!(events.iter().any(|event| {
        event.event_type == "terminal"
            && event.payload == json!({"status":"failed","code":"run_timeout"})
    }));
    assert!(events.iter().all(|event| {
        !event
            .payload
            .to_string()
            .contains(&provider_address.to_string())
    }));
    api_server.abort();
    provider_server.abort();
}

#[sqlx::test]
async fn entity_conversation_mutations_uploads_and_events_require_entity_read(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base_url,
        "format_version = 1\ncode = 'conversation_access'\nname = 'Conversation access'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'",
    )
    .await;
    let entity = create_entity(&owner, &base_url, &blueprint).await;
    let conversation: Value = owner
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"entity_id": entity["id"], "title": "Private discussion"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_runs (id, workspace_id, conversation_id, origin, status, provider_base_url, model) VALUES ($1, $2, $3, 'interactive', 'awaiting_approval', 'https://provider.test/v1', 'test')")
        .bind(run_id).bind(workspace).bind(conversation_id).execute(&pool).await.unwrap();
    let call_id = Uuid::new_v4();
    sqlx::query("INSERT INTO agent_tool_calls (id, run_id, sequence, tool_name, arguments, state) VALUES ($1, $2, 0, 'test', '{}', 'pending_approval')")
        .bind(call_id).bind(run_id).execute(&pool).await.unwrap();

    let actor = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(actor)
        .bind(format!("{actor}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    let membership = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();
    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let role = repository
        .create_workspace_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            "conversation-agent-only",
            &["agents.run".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            role,
            "workspace",
            workspace,
        )
        .await
        .unwrap();
    let client = reqwest::Client::builder()
        .default_headers(reqwest::header::HeaderMap::from_iter([
            (
                "x-catalog-user-id".parse().unwrap(),
                actor.to_string().parse().unwrap(),
            ),
            (
                "x-catalog-workspace-id".parse().unwrap(),
                workspace.to_string().parse().unwrap(),
            ),
        ]))
        .build()
        .unwrap();
    let conversation_url = format!("{base_url}/agent/conversations/{conversation_id}");
    let rename = client
        .patch(&conversation_url)
        .json(&json!({"title": "Changed"}))
        .send()
        .await
        .unwrap();
    assert_eq!(rename.status(), reqwest::StatusCode::FORBIDDEN);
    let delete = client.delete(&conversation_url).send().await.unwrap();
    assert_eq!(delete.status(), reqwest::StatusCode::FORBIDDEN);
    let upload = client
        .post(format!("{conversation_url}/uploads"))
        .multipart(Form::new().part("file", Part::bytes(b"hello".to_vec()).file_name("note.txt")))
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), reqwest::StatusCode::FORBIDDEN);
    let events = client
        .get(format!("{base_url}/agent/runs/{run_id}/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(events.status(), reqwest::StatusCode::FORBIDDEN);
    let approvals: Value = client
        .get(format!("{base_url}/agent/approvals"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(approvals, json!([]));
    let scoped_approvals = client
        .get(format!(
            "{base_url}/agent/approvals?conversation_id={conversation_id}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(scoped_approvals.status(), reqwest::StatusCode::FORBIDDEN);
    let owner_approvals: Value = owner
        .get(format!("{base_url}/agent/approvals"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(owner_approvals[0]["id"], call_id.to_string());
    for action in ["approve", "reject"] {
        let decision = client
            .post(format!("{base_url}/agent/tool-calls/{call_id}/{action}"))
            .send()
            .await
            .unwrap();
        assert_eq!(decision.status(), reqwest::StatusCode::FORBIDDEN);
    }
    let unchanged: Value = owner
        .get(&conversation_url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unchanged["title"], "Private discussion");
    let renamed: Value = owner
        .patch(&conversation_url)
        .json(&json!({"title": "Owner discussion"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(renamed["title"], "Owner discussion");
    assert_eq!(
        owner
            .delete(&conversation_url)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    server.abort();
}

#[sqlx::test]
async fn agent_read_tools_enforce_initiator_permissions_and_scopes(pool: PgPool) {
    let (base_url, server) = start_server(pool.clone()).await;
    let owner = authenticated_client();
    let blueprint = create_blueprint(
        &owner,
        &base_url,
        "format_version = 1\ncode = 'agent_scoped'\nname = 'Agent scoped'\nkind = 'entity'\n\n[views.dropdown_option]\ntype = 'dropdown_option'\nfields = ['title']\n\n[[attributes]]\ncode = 'title'\nvalue_type = 'string'",
    )
    .await;
    let entity = create_entity(&owner, &base_url, &blueprint).await;
    let context: Value = owner
        .post(format!("{base_url}/contexts"))
        .json(&json!({"code": "agent-scoped", "data": {}}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let workspace = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let actor = Uuid::new_v4();
    let membership = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(actor)
        .bind(format!("{actor}@example.test"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(workspace)
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();

    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace)
        .await
        .unwrap();
    let agent_only = repository
        .create_workspace_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            "agent-only",
            &["agents.run".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            agent_only,
            "workspace",
            workspace,
        )
        .await
        .unwrap();

    for (name, arguments) in [
        ("list_blueprints", json!({})),
        ("get_blueprint", json!({"code":"product"})),
        ("list_contexts", json!({})),
        ("get_context", json!({"context_id":context["id"]})),
        ("data_health_summary", json!({})),
        ("list_rule_findings", json!({})),
        ("list_workflow_runs", json!({})),
        ("get_rule_definition", json!({"rule_id":Uuid::new_v4()})),
        (
            "get_workflow_definition",
            json!({"workflow_id":Uuid::new_v4()}),
        ),
        ("list_rule_runs", json!({})),
        ("get_workflow_run", json!({"run_id":Uuid::new_v4()})),
        ("list_extension_operation_runs", json!({})),
        (
            "get_extension_operation_run",
            json!({"run_id":Uuid::new_v4()}),
        ),
        (
            "list_blueprint_connector_jobs",
            json!({"blueprint_id":Uuid::new_v4()}),
        ),
        ("get_entity", json!({"entity_id": entity["id"]})),
        (
            "get_incoming_relationships",
            json!({"entity_id": entity["id"]}),
        ),
        (
            "get_entity_hierarchy",
            json!({"entity_id": entity["id"], "field": "parent"}),
        ),
        ("list_reusable_attributes", json!({})),
        ("list_entity_comments", json!({"entity_id": entity["id"]})),
        (
            "validate_rule_definition",
            json!({"definition": "format_version = 1"}),
        ),
        ("data_health_details", json!({"section": "blueprints"})),
        (
            "get_entity_context_preview",
            json!({"entity_id": entity["id"], "context_id": context["id"]}),
        ),
    ] {
        assert!(matches!(
            execute_read(&repository, actor, workspace, name, arguments).await,
            Err(ToolError::Forbidden)
        ));
    }
    // Labels are filtered per entity, so an unreadable entity is omitted.
    assert_eq!(
        execute_read(
            &repository,
            actor,
            workspace,
            "get_entity_labels",
            json!({"entity_ids": [entity["id"]]}),
        )
        .await
        .unwrap()["items"],
        json!([])
    );

    let scoped_reader = repository
        .create_workspace_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            "agent-scoped-reader",
            &["entities.read".to_owned(), "contexts.read".to_owned()],
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            scoped_reader,
            "entity",
            entity["id"].as_str().unwrap().parse().unwrap(),
        )
        .await
        .unwrap();
    repository
        .grant_workspace_member_role(
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            workspace,
            membership,
            scoped_reader,
            "context_subtree",
            context["id"].as_str().unwrap().parse().unwrap(),
        )
        .await
        .unwrap();

    let loaded_entity = execute_read(
        &repository,
        actor,
        workspace,
        "get_entity",
        json!({"entity_id": entity["id"]}),
    )
    .await
    .unwrap();
    assert_eq!(loaded_entity["id"], entity["id"]);
    assert_eq!(loaded_entity["values"], json!([]));
    assert_eq!(
        execute_read(&repository, actor, workspace, "list_contexts", json!({}))
            .await
            .unwrap(),
        json!([context])
    );
    server.abort();
}

#[sqlx::test]
async fn reconciliation_skips_a_file_locked_by_an_attachment_transaction(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'locked attachment')",
    )
    .bind(conversation_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, attachment_expires_at) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/locked-report.pdf', 'ready', now() - interval '1 second')")
        .bind(file_id)
        .bind(workspace_id)
        .bind("0".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

    // This is the file-locking portion of append_conversation_message_with_attachments.
    // Reconciliation must skip it rather than decide from a stale absence of an attachment.
    let mut attachment = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE")
        .bind(file_id)
        .bind(workspace_id)
        .fetch_one(&mut *attachment)
        .await
        .unwrap();
    sqlx::query("INSERT INTO conversation_messages (id, conversation_id, sequence, role, content) VALUES ($1, $2, 0, 'user', '\"keep it\"')")
        .bind(message_id)
        .bind(conversation_id)
        .execute(&mut *attachment)
        .await
        .unwrap();
    sqlx::query("INSERT INTO conversation_message_attachments (id, workspace_id, message_id, file_id, position) VALUES ($1, $2, $3, $4, 0)")
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(message_id)
        .bind(file_id)
        .execute(&mut *attachment)
        .await
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        Arc::new(FakeObjectStore::available()),
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();
    attachment.commit().await.unwrap();

    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT deleted_at FROM files WHERE id = $1"
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none()
    );
    worker.reconcile().await.unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT deleted_at FROM files WHERE id = $1"
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none()
    );
}

#[sqlx::test]
async fn reconciliation_first_rejects_deleted_and_cross_tenant_attachments(pool: PgPool) {
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let other_workspace_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let other_conversation_id = Uuid::new_v4();
    let deleted_file_id = Uuid::new_v4();
    let live_file_id = Uuid::new_v4();
    sqlx::query("INSERT INTO workspaces (id, slug, name, login_identifier) VALUES ($1, 'attachment-other', 'Attachment other', 'attachment-other.local')")
        .bind(other_workspace_id).execute(&pool).await.unwrap();
    for (id, workspace, title) in [
        (conversation_id, workspace_id, "local"),
        (other_conversation_id, other_workspace_id, "other"),
    ] {
        sqlx::query("INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(workspace)
            .bind(title)
            .execute(&pool)
            .await
            .unwrap();
    }
    for (id, expires_at) in [
        (deleted_file_id, "now() - interval '1 second'"),
        (live_file_id, "now() + interval '1 hour'"),
    ] {
        sqlx::query(&format!("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, attachment_expires_at) VALUES ($1, $2, 'report.pdf', 'report.pdf', 'application/pdf', 7, $3, 'files/{id}', 'ready', {expires_at})"))
            .bind(id).bind(workspace_id).bind("0".repeat(64)).execute(&pool).await.unwrap();
    }
    let worker = FileWorker::new(
        pool.clone(),
        Arc::new(FakeObjectStore::available()),
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();

    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(workspace_id)
        .await
        .unwrap();
    assert!(matches!(
        repository
            .append_conversation_message_with_attachments(
                conversation_id,
                None,
                "user",
                json!("too late"),
                &[deleted_file_id]
            )
            .await,
        Err(RepositoryError::NotFound("file"))
    ));
    let other_repository = CatalogRepository::system(pool.clone())
        .for_workspace(other_workspace_id)
        .await
        .unwrap();
    assert!(matches!(
        other_repository
            .append_conversation_message_with_attachments(
                other_conversation_id,
                None,
                "user",
                json!("not ours"),
                &[live_file_id]
            )
            .await,
        Err(RepositoryError::NotFound("file"))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM conversation_message_attachments WHERE file_id = ANY($1)"
        )
        .bind(&[deleted_file_id, live_file_id][..])
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[sqlx::test]
async fn standalone_conversation_upload_survives_reconciliation_until_attached(pool: PgPool) {
    let store = Arc::new(FakeObjectStore::available());
    let (base_url, server) = start_server_with_object_store(pool.clone(), store.clone()).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "Upload retention"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation_id = conversation["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let upload: Value = client
        .post(format!(
            "{base_url}/agent/conversations/{conversation_id}/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"%PDF-1.4\nattachment".to_vec())
                    .file_name("attachment.pdf")
                    .mime_str("application/pdf")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let file_id = upload["files"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    let worker = FileWorker::new(
        pool.clone(),
        store,
        WorkerConfig {
            worker_id: "test-worker".into(),
            max_pixels: 1,
            max_attempts: 1,
            delete_grace: Duration::from_secs(60),
        },
    );
    worker.reconcile().await.unwrap();
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(deleted_at.is_none());

    let repository = CatalogRepository::system(pool.clone())
        .for_workspace(BOOTSTRAP_WORKSPACE_ID.parse().unwrap())
        .await
        .unwrap();
    let other_conversation = repository
        .create_conversation(None, "Other conversation")
        .await
        .unwrap();
    assert!(
        repository
            .submit_agent_message(
                other_conversation.id,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                json!("Steal attachment"),
                &[file_id],
                "https://provider.test/v1",
                "test"
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .conversation_messages(other_conversation.id)
            .await
            .unwrap()
            .is_empty()
    );
    let unrelated_file = Uuid::new_v4();
    sqlx::query("INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1, $2, 'private.pdf', 'private.pdf', 'application/pdf', 7, $3, 'files/private.pdf', 'ready')")
        .bind(unrelated_file)
        .bind(BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap())
        .bind("0".repeat(64))
        .execute(&pool).await.unwrap();
    assert!(
        repository
            .submit_agent_message(
                conversation_id,
                BOOTSTRAP_OWNER_ID.parse().unwrap(),
                json!("Unrelated file"),
                &[unrelated_file],
                "https://provider.test/v1",
                "test"
            )
            .await
            .is_err()
    );
    let other_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, 'another-agent@example.test')")
        .bind(other_user)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repository
            .submit_agent_message(
                conversation_id,
                other_user,
                json!("Another user's upload"),
                &[file_id],
                "https://provider.test/v1",
                "test"
            )
            .await
            .is_err()
    );
    repository
        .submit_agent_message(
            conversation_id,
            BOOTSTRAP_OWNER_ID.parse().unwrap(),
            json!("Read the attachment"),
            &[file_id],
            "https://provider.test/v1",
            "test",
        )
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT attachment_expires_at FROM files WHERE id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_none()
    );

    assert!(worker.run_once().await.unwrap());
    let messages: Value = client
        .get(format!(
            "{base_url}/agent/conversations/{conversation_id}/messages"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["attachments"][0]["id"], file_id.to_string());
    assert_eq!(messages[0]["attachments"][0]["status"], "ready");

    let never_attached: Value = client
        .post(format!(
            "{base_url}/agent/conversations/{conversation_id}/uploads"
        ))
        .multipart(
            Form::new().part(
                "file",
                Part::bytes(b"%PDF-1.4\nnever attached".to_vec())
                    .file_name("never-attached.pdf")
                    .mime_str("application/pdf")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let never_attached_id = never_attached["files"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query(
        "UPDATE files SET attachment_expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(never_attached_id)
    .execute(&pool)
    .await
    .unwrap();
    worker.reconcile().await.unwrap();
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT deleted_at FROM files WHERE id = $1",
        )
        .bind(never_attached_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .is_some()
    );
    server.abort();
}

#[sqlx::test]
async fn agent_http_limits_accept_the_boundary_and_reject_the_next_value(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let accepted = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "x".repeat(512)}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::CREATED);
    let conversation: Value = accepted.json().await.unwrap();
    let oversized_title = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({"title": "x".repeat(513)}))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_title.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let message_url = format!(
        "{base_url}/agent/conversations/{}/messages",
        conversation["id"].as_str().unwrap()
    );
    let message_at_limit = client
        .post(&message_url)
        .json(&json!({"content": "x".repeat(32 * 1024)}))
        .send()
        .await
        .unwrap();
    assert_eq!(message_at_limit.status(), StatusCode::SERVICE_UNAVAILABLE);
    let oversized_message = client
        .post(&message_url)
        .json(&json!({"content": "x".repeat(32 * 1024 + 1)}))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized_message.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let attachments_at_limit = client
        .post(&message_url)
        .json(&json!({
            "content": "",
            "attachment_ids": (0..16).map(|_| Uuid::new_v4()).collect::<Vec<_>>(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        attachments_at_limit.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let oversized_attachments = client
        .post(&message_url)
        .json(&json!({
            "content": "",
            "attachment_ids": (0..17).map(|_| Uuid::new_v4()).collect::<Vec<_>>(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        oversized_attachments.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    server.abort();
}

#[sqlx::test]
async fn agent_message_requires_a_configured_provider(pool: PgPool) {
    let (base_url, server) = start_server(pool).await;
    let client = authenticated_client();
    let conversation: Value = client
        .post(format!("{base_url}/agent/conversations"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = client
        .post(format!(
            "{base_url}/agent/conversations/{}/messages",
            conversation["id"].as_str().unwrap()
        ))
        .json(&json!({"content":"hello"}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let response: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{response}");
    assert_eq!(response["error"]["code"], "service_unavailable");
    server.abort();
}

#[sqlx::test]
async fn startup_recovery_only_interrupts_expired_agent_tasks(pool: PgPool) {
    use api::task_queue::{TaskInsert, TaskKind};

    let repository = CatalogRepository::new(
        pool.clone(),
        support::BOOTSTRAP_WORKSPACE_ID.parse().unwrap(),
    );
    let workspace_id = BOOTSTRAP_WORKSPACE_ID.parse::<Uuid>().unwrap();
    let conversation_id = Uuid::new_v4();
    sqlx::query("INSERT INTO conversations (id, workspace_id, title) VALUES ($1, $2, 'recovery')")
        .bind(conversation_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    for expired in [false, true] {
        let run_id = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_runs (id,workspace_id,conversation_id,origin,status,provider_base_url,model,started_at) VALUES ($1,$2,$3,'interactive','running','https://provider.test','test',now())")
            .bind(run_id).bind(workspace_id).bind(conversation_id).execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        repository
            .enqueue_task(
                &mut tx,
                TaskInsert {
                    workspace_id,
                    kind: TaskKind::AgentRunV1,
                    subject_id: run_id,
                    generation: 0,
                    payload: json!({"agent_run_id": run_id.to_string()}),
                    correlation_id: None,
                    causation_id: None,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let task = repository
            .claim_task("agent-worker", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        if expired {
            sqlx::query("UPDATE tasks SET lease_until=now()-interval '1 second' WHERE id=$1")
                .bind(task.id)
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    repository.recover_interrupted_agent_runs().await.unwrap();
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM agent_runs ORDER BY created_at")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(statuses.contains(&"running".to_owned()));
    assert!(statuses.contains(&"failed".to_owned()));
    let recovered: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM tasks WHERE kind='agent_run.v1' AND status='succeeded'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recovered, 1);
}
