use std::{fs, path::PathBuf};

use clap::Parser;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use crate::cli::{Cli, Command, PageArgs, SourceInput};
use crate::commands::data_health::data_health_path;
use crate::commands::extension::connector_job::{ConnectorJobCommand, connector_job_command};
use crate::commands::extension::operation::{
    ExtensionOperationCommand, extension_operation_command, operation_source,
};
use crate::commands::extension::schedule::{ExtensionScheduleCommand, extension_schedule_command};
use crate::commands::lexicon::lexicon_request;
use crate::commands::notification::notification_request;
use crate::commands::record::migrate_records;
use crate::commands::rule::{RuleCommand, rule_command};
use crate::commands::saved_view::changed_saved_view;
use crate::commands::solution_pack::{
    BlueprintPublicationArgument, SolutionPackApplicationsCommand, SolutionPackChecksCommand,
    SolutionPackCommand, SolutionPackPlanCommand, blueprint_mapping_value, parse_pack_mappings,
    solution_pack_command,
};
use crate::commands::team::team_request;
use crate::commands::token::{GENERATOR_TOKEN_LABEL, GENERATOR_TOKEN_PERMISSIONS};
use crate::commands::workspace::{AgentSettingsCommand, WorkspaceCommand, changed_agent_settings};
use crate::error::CliError;
use crate::input::{
    ContextFile, RelationshipFile, ValueFile, form_update_body, json_array_input, json_input,
    json_object_input, json_string_array_input, parse_stdin_lines, read_source, relationships_body,
    toml_to_json, values_body,
};
use crate::run;
use crate::session::SessionFile;
use crate::transport::{
    Api, MAX_RESPONSE_BYTES, endpoint, is_secure_credential_transport, raw_response, segment,
};

#[test]
fn blueprint_mapping_accepts_planner_generated_stable_codes() {
    for code in [
        "seed_environmental-declaration",
        "material_specification",
        "a-",
        "a_",
    ] {
        let requested = vec![format!("blueprints/environmental-declaration={code}")];
        let parts = parse_pack_mappings(
            &requested,
            "--map",
            "blueprints/",
            "EXISTING_CODE",
            blueprint_mapping_value,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&parts[0]).unwrap(),
            json!({"key":"blueprints/environmental-declaration","code":code})
        );
    }
    assert!(blueprint_mapping_value(&"a".repeat(128)).is_some());
}

#[test]
fn blueprint_mapping_rejects_non_stable_codes_and_duplicate_keys() {
    for code in [
        "",
        "Uppercase",
        "1code",
        "-code",
        "has space",
        "a/b",
        "é",
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    ] {
        assert!(blueprint_mapping_value(code).is_none(), "{code}");
    }
    assert!(blueprint_mapping_value(&"a".repeat(129)).is_none());
    let requested = vec!["blueprints/item=one".into(), "blueprints/item=two".into()];
    assert!(
        parse_pack_mappings(
            &requested,
            "--map",
            "blueprints/",
            "EXISTING_CODE",
            blueprint_mapping_value
        )
        .is_err()
    );
    assert!(
        parse_pack_mappings(
            &["contexts/item=one".into()],
            "--map",
            "blueprints/",
            "EXISTING_CODE",
            blueprint_mapping_value
        )
        .is_err()
    );
}

#[test]
fn auth_preferences_require_exactly_one_time_zone_choice() {
    assert!(
        Cli::try_parse_from([
            "acli",
            "auth",
            "preferences",
            "--time-zone",
            "Europe/Warsaw"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "auth", "preferences", "--clear-time-zone"]).is_ok());
    assert!(Cli::try_parse_from(["acli", "auth", "preferences"]).is_err());
    assert!(
        Cli::try_parse_from([
            "acli",
            "auth",
            "preferences",
            "--time-zone",
            "UTC",
            "--clear-time-zone"
        ])
        .is_err()
    );
}

#[test]
fn notification_commands_build_requests() {
    let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
        Command::Notification { command } => notification_request(command),
        _ => unreachable!(),
    };
    let id = "6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60";
    assert_eq!(
        parse(&["acli", "notification", "list"]),
        (Method::GET, "/notifications".to_owned(), None)
    );
    assert_eq!(
        parse(&[
            "acli",
            "notification",
            "list",
            "--unread",
            "--before-time",
            "2026-10-08T10:00:00Z",
            "--before-id",
            id
        ])
        .1,
        format!(
            "/notifications?unread_only=true&before_time=2026-10-08T10%3A00%3A00Z&before_id={id}"
        )
    );
    assert!(Cli::try_parse_from(["acli", "notification", "list", "--before-id", id]).is_err());
    assert_eq!(
        parse(&["acli", "notification", "read", id]),
        (
            Method::PATCH,
            format!("/notifications/{id}"),
            Some(json!({"read": true}))
        )
    );
    assert_eq!(
        parse(&["acli", "notification", "unread", id]).2,
        Some(json!({"read": false}))
    );
    assert_eq!(
        parse(&["acli", "notification", "read-all"]).1,
        "/notifications/read-all"
    );
    assert_eq!(
        parse(&["acli", "notification", "delete", id]),
        (Method::DELETE, format!("/notifications/{id}"), None)
    );
    assert_eq!(
        parse(&["acli", "notification", "count"]).1,
        "/notifications/unread-count"
    );
}

#[test]
fn agent_settings_changes_keep_unchanged_settings() {
    let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
        Command::Workspace {
            command:
                WorkspaceCommand::Agent {
                    command:
                        AgentSettingsCommand::Set {
                            bulk_edits,
                            bulk_edit_max_records,
                        },
                },
        } => changed_agent_settings(
            json!({"bulk_edits": "per_proposal", "bulk_edit_max_records": 500}),
            bulk_edits,
            bulk_edit_max_records,
        )
        .unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(
        parse(&[
            "acli",
            "workspace",
            "agent",
            "set",
            "--bulk-edits",
            "per-record"
        ]),
        json!({"bulk_edits": "per_record", "bulk_edit_max_records": 500})
    );
    assert_eq!(
        parse(&[
            "acli",
            "workspace",
            "agent",
            "set",
            "--bulk-edit-max-records",
            "50"
        ]),
        json!({"bulk_edits": "per_proposal", "bulk_edit_max_records": 50})
    );
    for invalid in [
        &["acli", "workspace", "agent", "set"][..],
        &[
            "acli",
            "workspace",
            "agent",
            "set",
            "--bulk-edit-max-records",
            "0",
        ],
        &[
            "acli",
            "workspace",
            "agent",
            "set",
            "--bulk-edit-max-records",
            "5001",
        ],
        &[
            "acli",
            "workspace",
            "agent",
            "set",
            "--bulk-edits",
            "sometimes",
        ],
    ] {
        assert!(Cli::try_parse_from(invalid).is_err(), "{invalid:?}");
    }
    assert!(Cli::try_parse_from(["acli", "workspace", "agent", "get"]).is_ok());
}

#[test]
fn team_commands_build_requests() {
    let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
        Command::Team { command } => team_request(command),
        _ => unreachable!(),
    };
    let member = "6a1f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f60";
    let team = "8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62";
    assert_eq!(
        parse(&[
            "acli", "team", "create", "--code", "qa", "--name", "QA", "--member", member
        ]),
        (
            Method::POST,
            "/workspace/teams".to_owned(),
            Some(json!({"code": "qa", "name": "QA", "member_user_ids": [member]}))
        )
    );
    assert_eq!(
        parse(&["acli", "team", "update", team, "--name", "Quality"]),
        (
            Method::PATCH,
            format!("/workspace/teams/{}", segment(team)),
            Some(json!({"name": "Quality"}))
        )
    );
    assert_eq!(
        parse(&["acli", "team", "update", team, "--clear-members"]).2,
        Some(json!({"member_user_ids": []}))
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "team",
            "update",
            team,
            "--clear-members",
            "--member",
            member
        ])
        .is_err()
    );
}

#[test]
fn lexicon_commands_build_requests_and_read_toml_files() {
    let parse = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
        Command::Lexicon { command } => lexicon_request(command).unwrap(),
        _ => unreachable!(),
    };
    let (method, path, _) = parse(&[
        "acli",
        "lexicon",
        "delete",
        "--key",
        "Order",
        "--context",
        "sorting & paging",
        "--language",
        "pl",
    ]);
    assert_eq!(method, Method::DELETE);
    assert_eq!(
        path,
        "/lexicon/entries?key=Order&context=sorting+%26+paging&language=pl&plural_category=other"
    );
    let (_, path, _) = parse(&[
        "acli",
        "lexicon",
        "report",
        "--language",
        "pl",
        "--language",
        "de",
    ]);
    assert_eq!(path, "/lexicon/report?languages=pl%2Cde");
    let (_, _, body) = parse(&[
        "acli",
        "lexicon",
        "set",
        "--key",
        "Product",
        "--language",
        "pl",
        "--plural-category",
        "few",
        "--text",
        "Produkty",
    ]);
    assert_eq!(body.unwrap()["plural_category"], "few");

    let file = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
    fs::write(
        file.path(),
        "format_version = 1\nlanguage = \"pl\"\n[[entries]]\nkey = \"Product\"\ntext = \"Produkt\"\n",
    )
    .unwrap();
    let path = file.path().to_str().unwrap();
    let (_, request_path, body) =
        parse(&["acli", "lexicon", "import", "--file", path, "--replace"]);
    assert_eq!(request_path, "/lexicon/import?mode=replace");
    assert_eq!(
        body.unwrap(),
        json!({"format_version": 1, "language": "pl", "entries": [{"key": "Product", "text": "Produkt"}]})
    );
}

#[test]
fn saved_view_commands_accept_json_state_and_reject_bad_identifiers() {
    assert!(
        Cli::try_parse_from([
            "acli",
            "saved-view",
            "create",
            "--name",
            "Assets",
            "--state",
            "{\"blueprint\":\"asset\"}"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "saved-view", "link", "--state", "state.json"]).is_ok());
    assert!(Cli::try_parse_from(["acli", "saved-view", "get", "not-a-uuid"]).is_err());
    assert_eq!(
        json_object_input("{\"blueprint\":\"asset\"}", "--state").unwrap(),
        json!({"blueprint":"asset"})
    );
}

#[test]
fn only_allows_plaintext_credential_transport_to_loopback() {
    for url in [
        "https://attricat.example.test",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(
            is_secure_credential_transport(&Url::parse(url).unwrap()),
            "{url}"
        );
    }
    assert!(!is_secure_credential_transport(
        &Url::parse("http://attricat.example.test").unwrap()
    ));
}

#[test]
fn converts_value_file_to_api_shape() {
    let source = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
value = "Blue shirt"
"#;
    let input: ValueFile = toml::from_str(source).unwrap();
    assert_eq!(
        values_body(input, None).unwrap(),
        json!({
            "values": [{
                "kind": "scalar",
                "attribute_id": "00000000-0000-0000-0000-000000000001",
                "context_id": null,
                "value": "Blue shirt"
            }]
        })
    );
}

#[test]
fn converts_attribute_code_to_api_shape() {
    let source = r#"
[[values]]
kind = "relationship"
attribute_code = "related_products"
target_record_id = "00000000-0000-0000-0000-000000000002"
"#;
    let input: ValueFile = toml::from_str(source).unwrap();
    assert_eq!(
        values_body(input, None).unwrap(),
        json!({
            "values": [{
                "kind": "relationship",
                "attribute_code": "related_products",
                "context_id": null,
                "target_record_id": "00000000-0000-0000-0000-000000000002"
            }]
        })
    );
}

#[test]
fn converts_toml_temporal_literals_to_iso_strings() {
    let input: ValueFile = toml::from_str(
        r#"
[[values]]
kind = "scalar"
attribute_code = "available_on"
value = 2026-08-12

[[values]]
kind = "scalar"
attribute_code = "released_at"
value = 2026-08-12T14:30:00Z
"#,
    )
    .unwrap();
    assert_eq!(
        values_body(input, None).unwrap()["values"],
        json!([
            { "kind": "scalar", "attribute_code": "available_on", "context_id": null, "value": "2026-08-12" },
            { "kind": "scalar", "attribute_code": "released_at", "context_id": null, "value": "2026-08-12T14:30:00Z" }
        ])
    );
}

#[test]
fn converts_relationship_replacement_file_to_api_shape() {
    let source = r#"
[[relationships]]
attribute_code = "categories"
target_record_ids = [
  "00000000-0000-0000-0000-000000000002",
  "00000000-0000-0000-0000-000000000003",
]
"#;
    let input: RelationshipFile = toml::from_str(source).unwrap();
    assert_eq!(
        relationships_body(input, None).unwrap(),
        json!({
            "relationships": [{
                "attribute_code": "categories",
                "context_id": null,
                "target_record_ids": [
                    "00000000-0000-0000-0000-000000000002",
                    "00000000-0000-0000-0000-000000000003"
                ]
            }]
        })
    );
}

#[test]
fn combines_record_update_files_into_the_form_contract() {
    let values = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        values.path(),
        "[[values]]\nkind = \"scalar\"\nattribute_code = \"title\"\nvalue = \"Updated shirt\"\n",
    )
    .unwrap();
    let removals = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        removals.path(),
        "[[remove_values]]\nattribute_code = \"subtitle\"\n",
    )
    .unwrap();
    let context_id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();

    assert_eq!(
        form_update_body(
            Some(&values.path().to_path_buf()),
            None,
            Some(&removals.path().to_path_buf()),
            Some(context_id),
        )
        .unwrap(),
        json!({
            "values": [{
                "kind": "scalar",
                "attribute_code": "title",
                "context_id": context_id,
                "value": "Updated shirt"
            }],
            "relationships": [],
            "remove_values": [{ "attribute_code": "subtitle", "context_id": context_id }]
        })
    );
}

#[test]
fn rejects_incompatible_value_fields_and_unknown_toml_keys() {
    let relationship_with_value = r#"
[[values]]
kind = "relationship"
attribute_id = "00000000-0000-0000-0000-000000000001"
target_record_id = "00000000-0000-0000-0000-000000000002"
value = "ignored before this validation"
"#;
    let input: ValueFile = toml::from_str(relationship_with_value).unwrap();
    assert!(matches!(values_body(input, None), Err(CliError::Input(_))));

    let conflicting_selectors = r#"
[[values]]
kind = "scalar"
attribute_id = "00000000-0000-0000-0000-000000000001"
attribute_code = "title"
value = "Blue shirt"
"#;
    let input: ValueFile = toml::from_str(conflicting_selectors).unwrap();
    assert!(matches!(values_body(input, None), Err(CliError::Input(_))));
}

#[test]
fn preserves_base_path_and_encodes_dynamic_segments() {
    let server = Url::parse("https://example.test/attricat-api/").unwrap();
    let url = endpoint(&server, &format!("/contexts/{}", segment("en/GB?#"))).unwrap();
    assert_eq!(
        url.as_str(),
        "https://example.test/attricat-api/contexts/en%2FGB%3F%23"
    );
    // A path segment keeps `+` literal, so spaces must not become `+`.
    assert_eq!(segment("a b+c~d"), "a%20b%2Bc~d");
}

#[test]
fn saved_view_update_keeps_unchanged_fields() {
    let current = json!({
        "id": Uuid::from_u128(1), "kind": "explorer_search", "name": "Assets",
        "description": "Shared", "visibility": "workspace", "state": {"blueprint": "asset"},
    });
    assert_eq!(
        changed_saved_view(&current, Some("Renamed".into()), None, None, None).unwrap(),
        json!({
            "kind": "explorer_search", "name": "Renamed", "description": "Shared",
            "visibility": "workspace", "state": {"blueprint": "asset"},
        })
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "saved-view",
            "update",
            &Uuid::from_u128(1).to_string()
        ])
        .is_err()
    );
}

#[test]
fn json_input_errors_name_the_flag() {
    let error = json_object_input("[]", "--state").unwrap_err().to_string();
    assert_eq!(error, "--state must be a JSON object");
    let error = json_string_array_input("[1]", "--system-tags")
        .unwrap_err()
        .to_string();
    assert_eq!(error, "--system-tags must be a JSON array of strings");
}

#[test]
fn accepts_only_a_json_string_array_or_file_for_permissions() {
    assert_eq!(
        json_string_array_input("[\"records.read\"]", "--permissions").unwrap(),
        json!(["records.read"])
    );
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), "[\"members.manage\"]").unwrap();
    assert_eq!(
        json_string_array_input(file.path().to_str().unwrap(), "--permissions").unwrap(),
        json!(["members.manage"])
    );
    assert!(json_string_array_input("not-json", "--permissions").is_err());
    assert!(
        json_string_array_input("{\"permission\": \"records.read\"}", "--permissions").is_err()
    );
    assert!(json_string_array_input("[\"records.read\", 1]", "--permissions").is_err());
}

#[test]
fn generator_token_preset_requires_no_manual_fields_and_uses_least_privilege_permissions() {
    assert!(Cli::try_parse_from(["acli", "token", "create", "--generator"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "token",
            "create",
            "--label",
            "manual",
            "--permissions",
            "[\"records.read\"]"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "token", "create", "--label", "manual"]).is_err());
    assert!(
        Cli::try_parse_from([
            "acli",
            "token",
            "create",
            "--permissions",
            "[\"records.read\"]"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "token",
            "create",
            "--generator",
            "--label",
            "manual"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "token",
            "create",
            "--generator",
            "--permissions",
            "[\"records.read\"]"
        ])
        .is_err()
    );
    assert_eq!(GENERATOR_TOKEN_LABEL, "catalog-generator");
    assert_eq!(
        GENERATOR_TOKEN_PERMISSIONS,
        [
            "blueprints.read",
            "blueprints.write",
            "blueprints.publish",
            "contexts.read",
            "contexts.write",
            "records.read",
            "records.write",
            "records.publish",
        ]
    );
}

#[test]
fn context_creation_defaults_the_parent_and_role_duplication_requires_a_code() {
    assert!(
        Cli::try_parse_from([
            "acli", "context", "create", "--code", "en-GB", "--data", "{}",
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "workspace",
            "role",
            "duplicate",
            "00000000-0000-0000-0000-000000000001",
        ])
        .is_err()
    );
}

#[test]
fn context_file_can_omit_parent_id() {
    let input: ContextFile =
        toml::from_str("code = \"en-GB\"\ndata = { language = \"en-GB\" }").unwrap();
    let body = json!({ "code": input.code, "data": toml_to_json(input.data).unwrap(), "parent_id": input.parent_id });
    assert_eq!(body["parent_id"], Value::Null);
}

#[test]
fn maps_api_errors_to_exit_code_four() {
    let error = CliError::Api {
        status: reqwest::StatusCode::UNPROCESSABLE_ENTITY.as_u16(),
        code: "invalid_input".to_owned(),
        message: "bad value".to_owned(),
        details: None,
    };
    assert_eq!(error.exit_code(), 4);
    assert_eq!(error.json()["error"]["status"], 422);
    assert!(error.json()["error"].get("details").is_none());
    let conflict = CliError::Api {
        status: reqwest::StatusCode::CONFLICT.as_u16(),
        code: "unique_key_conflict".to_owned(),
        message: "taken".to_owned(),
        details: Some(json!({ "conflicting_record_id": "e" })),
    };
    assert_eq!(
        conflict.json()["error"]["details"]["conflicting_record_id"],
        "e"
    );
}

#[test]
fn json_or_file_inputs_and_generated_help_cover_new_commands() {
    assert_eq!(
        json_array_input("[\"one\"]", "--filters").unwrap(),
        json!(["one"])
    );
    assert!(json_array_input("{\"field\":\"title\"}", "--filters").is_err());
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), "{\"operation\":\"get\",\"key\":\"sync\"}").unwrap();
    assert_eq!(
        json_input(file.path().to_str().unwrap(), "--body").unwrap()["operation"],
        "get"
    );
    let help = Cli::try_parse_from(["acli", "--help"]);
    assert!(matches!(
        help,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp
    ));
    assert!(Cli::try_parse_from(["acli", "workflow", "validate", "--stdin"]).is_ok());
    assert!(
        Cli::try_parse_from(["acli", "solution-pack", "inspect", "--file", "pack.tar.zst"]).is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "solution-pack", "inspect"]).is_err());
    assert!(Cli::try_parse_from(["acli", "--no-env", "health"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "file",
            "download-original",
            "00000000-0000-0000-0000-000000000001",
            "--output",
            "x.bin"
        ])
        .is_ok()
    );
}

#[test]
fn rule_commands_require_definition_metadata_and_sources() {
    let id = "00000000-0000-4000-8000-000000000001";
    assert!(
        Cli::try_parse_from([
            "acli",
            "rule",
            "validate",
            "--blueprint-id",
            id,
            "--blueprint-version",
            "1",
            "--stdin"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "rule",
            "create",
            "--blueprint-id",
            id,
            "--blueprint-version",
            "1",
            "--file",
            "rule.toml"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "rule", "validate", "--stdin"]).is_err());
    assert!(Cli::try_parse_from(["acli", "blueprint", "validate", "--file", "b.toml"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "blueprint",
            "validate",
            "--blueprint-id",
            id,
            "--stdin"
        ])
        .is_ok()
    );
    assert!(
        read_source(SourceInput {
            file: None,
            stdin: false
        })
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "rule",
            "run-now",
            id,
            "--idempotency-key",
            "check",
            "--dry-run"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "rule",
            "enable",
            id,
            "2",
            "--accept-existing-violations"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "data-health", "background-processing"]).is_ok());
}

#[tokio::test]
async fn rule_routes_forward_definition_and_run_options() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/rules/validate",
            axum::routing::post(
                |axum::Json(body): axum::Json<Value>| async move { axum::Json(body) },
            ),
        )
        .route(
            "/rules/{rule_id}/run-now",
            axum::routing::post(
                |axum::Json(body): axum::Json<Value>| async move { axum::Json(body) },
            ),
        )
        .route(
            "/rules/{rule_id}/versions/{version}/enable",
            axum::routing::post(
                |axum::Json(body): axum::Json<Value>| async move { axum::Json(body) },
            ),
        )
        .route(
            "/rule-findings",
            axum::routing::get(|uri: axum::http::Uri| async move {
                axum::Json(json!({"query":uri.query()}))
            }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let api = Api::new(
        Client::new(),
        Url::parse(&format!("http://{address}")).unwrap(),
    );
    let id = Uuid::from_u128(1);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), "format_version = 1\n").unwrap();
    let result = rule_command(
        &api,
        RuleCommand::Validate {
            blueprint_id: id,
            blueprint_version: 2,
            context_id: None,
            source: SourceInput {
                file: Some(file.path().to_owned()),
                stdin: false,
            },
        },
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        body,
        json!({"blueprint_id":id,"blueprint_version":2,"context_id":null,"definition":"format_version = 1\n"})
    );
    let result = rule_command(
        &api,
        RuleCommand::RunNow {
            rule_id: id,
            idempotency_key: "check".into(),
            record_id: Some(id),
            dry_run: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap(),
        json!({"idempotency_key":"check","record_id":id,"dry_run":true})
    );
    let result = rule_command(
        &api,
        RuleCommand::Enable {
            rule_id: id,
            version: 3,
            accept_existing_violations: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap(),
        json!({"accept_existing_violations":true})
    );
    let result = rule_command(
        &api,
        RuleCommand::Findings {
            record_id: Some(id),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap()["query"],
        format!("record_id={id}")
    );
    server.abort();
}

#[test]
fn extension_run_and_annotation_commands_validate_arguments() {
    let id = "00000000-0000-4000-8000-000000000001";
    for args in [
        vec![
            "acli",
            "extension",
            "run",
            "list",
            "--extension-id",
            "acme.docs",
        ],
        vec!["acli", "extension", "run", "get", id],
        vec!["acli", "extension", "run", "cancel", id],
        vec![
            "acli",
            "extension",
            "run",
            "download",
            id,
            id,
            "--output",
            "out.pdf",
        ],
        vec!["acli", "extension", "annotations", "adopt", "acme.docs"],
        vec![
            "acli",
            "extension",
            "annotations",
            "repair",
            "acme.docs",
            id,
            "--patch",
            "{}",
        ],
    ] {
        assert!(Cli::try_parse_from(args.clone()).is_ok(), "{args:?}");
    }
    assert!(Cli::try_parse_from(["acli", "extension", "run", "get", "not-a-uuid"]).is_err());
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "annotations",
            "repair",
            "acme.docs",
            id
        ])
        .is_err()
    );
}

#[test]
fn operation_commands_validate_arguments() {
    let id = "00000000-0000-4000-8000-000000000001";
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "operation",
            "start",
            "example",
            "--operation-id",
            "export",
            "--input",
            "{}",
            "--idempotency-key",
            "once",
            "--input-file-id",
            id
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "operation",
            "download",
            id,
            id,
            "--output",
            "export.csv"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "operation",
            "start",
            "example",
            "--input",
            "{}"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "schedule",
            "create",
            "example",
            "--operation-id",
            "export",
            "--input",
            "{}",
            "--interval-seconds",
            "59"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "schedule",
            "update",
            id,
            "--enabled",
            "false",
            "--interval-seconds",
            "60"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "extension",
            "connector-job",
            "run",
            id,
            "--idempotency-key",
            "once"
        ])
        .is_ok()
    );
    assert!(json_object_input("[]", "--patch").is_err());
    assert_eq!(
        operation_source(Some(Uuid::from_u128(1))),
        json!({"input_file_id":Uuid::from_u128(1)})
    );
}

#[tokio::test]
async fn operation_commands_forward_bounded_requests() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/extensions/{extension_id}/operations",
            axum::routing::post(
                |axum::extract::Path(id): axum::extract::Path<String>,
                 axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(json!({"id":id,"body":body}))
                },
            ),
        )
        .route(
            "/extensions/{extension_id}/operation-schedules",
            axum::routing::post(
                |axum::extract::Path(id): axum::extract::Path<String>,
                 axum::Json(body): axum::Json<Value>| async move {
                    axum::Json(json!({"id":id,"body":body}))
                },
            ),
        )
        .route(
            "/extension-operation-schedules/{id}",
            axum::routing::patch(
                |axum::Json(body): axum::Json<Value>| async move { axum::Json(body) },
            ),
        )
        .route(
            "/blueprint-connector-jobs/{id}/run",
            axum::routing::post(
                |axum::Json(body): axum::Json<Value>| async move { axum::Json(body) },
            ),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let api = Api::new(
        Client::new(),
        Url::parse(&format!("http://{address}")).unwrap(),
    );
    let id = Uuid::from_u128(1);
    let start: Value = serde_json::from_str(
        &extension_operation_command(
            &api,
            ExtensionOperationCommand::Start {
                extension_id: "example".into(),
                operation_id: "export".into(),
                input: "{\"limit\":1}".into(),
                idempotency_key: "once".into(),
                input_file_id: Some(id),
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        start["body"],
        json!({"operation_id":"export","input":{"limit":1},"idempotency_key":"once","source_reference":{"input_file_id":id},"destination_reference":{}})
    );
    let schedule: Value = serde_json::from_str(
        &extension_schedule_command(
            &api,
            ExtensionScheduleCommand::Create {
                extension_id: "example".into(),
                operation_id: "export".into(),
                input: "{}".into(),
                input_file_id: None,
                interval_seconds: 60,
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(schedule["body"]["interval_seconds"], 60);
    let updated: Value = serde_json::from_str(
        &extension_schedule_command(
            &api,
            ExtensionScheduleCommand::Update {
                schedule_id: id,
                enabled: false,
                interval_seconds: 120,
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(updated, json!({"enabled":false,"interval_seconds":120}));
    let run: Value = serde_json::from_str(
        &connector_job_command(
            &api,
            ConnectorJobCommand::Run {
                job_id: id,
                idempotency_key: "once".into(),
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(run, json!({"idempotency_key":"once"}));
    server.abort();
}

#[test]
fn data_health_and_audit_query_paths_are_encoded() {
    assert_eq!(
        data_health_path("summary", Some(7)),
        "/data-health/summary?stale_after_days=7"
    );
    assert_eq!(data_health_path("storage", None), "/data-health/storage");
    assert_eq!(segment("owner/repository"), "owner%2Frepository");
}

#[test]
fn auth_commands_require_stdin_secrets_and_support_a_session_file() {
    assert!(
        Cli::try_parse_from([
            "acli",
            "--session-file",
            "session.json",
            "auth",
            "login",
            "workspace",
            "--email",
            "user@example.test",
            "--password-stdin",
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "auth",
            "password-reset-confirm",
            "--reset-token-stdin",
            "--password-stdin",
        ])
        .is_ok()
    );

    let file = tempfile::NamedTempFile::new().unwrap();
    let path = file.path().to_path_buf();
    SessionFile::save(&path, "session-secret".to_owned(), "csrf-secret".to_owned()).unwrap();
    let loaded = SessionFile::load(Some(&path)).unwrap().unwrap();
    assert_eq!(loaded.session, "session-secret");
    assert_eq!(loaded.csrf, "csrf-secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn raw_responses_are_bounded_before_buffering() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/large",
            axum::routing::get(|| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    "x".repeat(MAX_RESPONSE_BYTES + 1),
                )
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let response = Client::new()
        .get(format!("http://{address}/large"))
        .send()
        .await
        .unwrap();
    assert!(matches!(
        raw_response(response).await,
        Err(CliError::ResponseTooLarge)
    ));
    server.abort();
}

#[tokio::test]
async fn failed_download_leaves_the_existing_output_unchanged() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/download",
            axum::routing::get(|| async {
                axum::response::Response::new(axum::body::Body::from_stream(
                    futures_util::stream::iter(vec![
                        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"partial")),
                        Err(std::io::Error::other("interrupted transfer")),
                    ]),
                ))
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("artifact.bin");
    fs::write(&output, "previous").unwrap();
    let result = Api::new(
        Client::new(),
        Url::parse(&format!("http://{address}")).unwrap(),
    )
    .download("/download", &output, None)
    .await;
    assert!(matches!(result, Err(CliError::Transport(_))));
    assert_eq!(fs::read_to_string(output).unwrap(), "previous");
    server.abort();
}

#[tokio::test]
async fn migrate_bulk_classifies_needs_input_without_submitting_a_migration() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let migrations = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn({
        let migrations = migrations.clone();
        async move {
            let app = axum::Router::new()
                .route(
                    "/v1/records/search",
                    axum::routing::post(|| async {
                        axum::Json(json!({
                            "items": [{ "id": "00000000-0000-0000-0000-000000000001" }],
                            "next_cursor": null,
                        }))
                    }),
                )
                .route(
                    "/v1/records/{record_id}/blueprint-migration/preview",
                    axum::routing::post(|| async {
                        axum::Json(
                            json!({ "status": "needs_input", "issues": [{ "code": "required" }] }),
                        )
                    }),
                )
                .route(
                    "/v1/records/{record_id}/blueprint-migration",
                    axum::routing::post(move || {
                        let migrations = migrations.clone();
                        async move {
                            migrations.fetch_add(1, Ordering::SeqCst);
                            axum::Json(json!({ "status": "migrated" }))
                        }
                    }),
                );
            axum::serve(listener, app).await.unwrap();
        }
    });
    let client = Client::builder().build().unwrap();
    let body = migrate_records(
        &Api::new(client, Url::parse(&format!("http://{address}")).unwrap()),
        "product",
        1,
        25,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["needs_input"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(migrations.load(Ordering::SeqCst), 0);
    server.abort();
}

#[tokio::test]
async fn migrate_bulk_rejects_a_repeated_next_cursor() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn({
        let requests = requests.clone();
        async move {
            let app = axum::Router::new().route(
                "/v1/records/search",
                axum::routing::post(move || {
                    let requests = requests.clone();
                    async move {
                        requests.fetch_add(1, Ordering::SeqCst);
                        axum::Json(json!({ "items": [], "next_cursor": "repeated" }))
                    }
                }),
            );
            axum::serve(listener, app).await.unwrap();
        }
    });
    let result = migrate_records(
        &Api::new(
            Client::new(),
            Url::parse(&format!("http://{address}")).unwrap(),
        ),
        "product",
        1,
        25,
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidResponse)));
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    server.abort();
}

#[tokio::test]
async fn migrate_bulk_rejects_a_malformed_next_cursor() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/v1/records/search",
            axum::routing::post(|| async {
                axum::Json(json!({ "items": [], "next_cursor": true }))
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::new();
    let result = migrate_records(
        &Api::new(client, Url::parse(&format!("http://{address}")).unwrap()),
        "product",
        1,
        25,
        false,
    )
    .await;
    assert!(matches!(result, Err(CliError::InvalidResponse)));
    server.abort();
}

#[tokio::test]
async fn solution_pack_inspect_streams_the_archive_to_the_server() {
    use std::sync::{Arc, Mutex};

    let received = Arc::new(Mutex::new(None));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn({
        let received = received.clone();
        async move {
            let app = axum::Router::new().route(
                "/solution-packs/inspect",
                axum::routing::post(
                    move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                        let received = received.clone();
                        async move {
                            *received.lock().unwrap() = Some((
                                headers
                                    .get(axum::http::header::CONTENT_TYPE)
                                    .unwrap()
                                    .to_str()
                                    .unwrap()
                                    .to_owned(),
                                body.to_vec(),
                            ));
                            axum::Json(json!({"archive_sha256": "server-validated"}))
                        }
                    },
                ),
            );
            axum::serve(listener, app).await.unwrap();
        }
    });
    let archive = tempfile::NamedTempFile::new().unwrap();
    fs::write(archive.path(), b"opaque archive bytes").unwrap();

    let body = run(Cli {
        server: Url::parse(&format!("http://{address}")).unwrap(),
        token: None,
        token_stdin: false,
        session_file: None,
        no_env: false,
        command: Command::SolutionPack {
            command: SolutionPackCommand::Inspect {
                file: archive.path().to_path_buf(),
            },
        },
    })
    .await
    .unwrap();

    assert_eq!(body, r#"{"archive_sha256":"server-validated"}"#);
    assert_eq!(
        received.lock().unwrap().take().unwrap(),
        (
            "application/zstd".to_owned(),
            b"opaque archive bytes".to_vec()
        )
    );
    server.abort();
}

#[test]
fn presentation_asset_parser_supports_bounded_discovery_and_download() {
    assert!(
        Cli::try_parse_from([
            "acli",
            "presentation-asset",
            "list",
            "--limit",
            "100",
            "--offset",
            "10000"
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "presentation-asset", "list", "--limit", "0"]).is_err());
    assert!(
        Cli::try_parse_from(["acli", "presentation-asset", "list", "--offset", "10001"]).is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "presentation-asset",
            "get",
            "00000000-0000-4000-8000-000000000001",
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "presentation-asset",
            "download",
            "00000000-0000-4000-8000-000000000001",
            "--output",
            "asset.svg",
        ])
        .is_ok()
    );
}

#[test]
fn solution_pack_plan_parser_supports_create_and_show_shapes() {
    let create = Cli::try_parse_from([
        "acli",
        "solution-pack",
        "plan",
        "create",
        "--file",
        "pack.tar.zst",
        "--prefix",
        "ecom",
        "--blueprint-publication",
        "publish",
        "--include-sample-data",
    ]);
    assert!(create.is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "create",
            "--file",
            "pack.tar.zst",
            "--prefix",
            "ecom",
            "--blueprint-publication",
            "publish",
            "--from-application",
            "00000000-0000-4000-8000-000000000001",
        ])
        .is_err()
    );
    let show = Cli::try_parse_from([
        "acli",
        "solution-pack",
        "plan",
        "get",
        "00000000-0000-4000-8000-000000000001",
    ]);
    assert!(show.is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "apply",
            "00000000-0000-4000-8000-000000000001",
        ])
        .is_ok()
    );
    assert!(Cli::try_parse_from(["acli", "solution-pack", "apply", "not-a-uuid"]).is_err());
    assert!(Cli::try_parse_from(["acli", "solution-pack", "applications", "list"]).is_ok());
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "applications",
            "get",
            "00000000-0000-4000-8000-000000000002",
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "checks",
            "rerun",
            "00000000-0000-4000-8000-000000000002"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "checks",
            "list",
            "00000000-0000-4000-8000-000000000002",
            "--limit",
            "100",
            "--offset",
            "10000"
        ])
        .is_ok()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "checks",
            "get",
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003"
        ])
        .is_ok()
    );
}

#[tokio::test]
async fn solution_pack_apply_and_history_use_uuid_only_json_routes() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let application_id = Uuid::from_u128(2);
    let app =
        axum::Router::new()
            .route(
                "/solution-packs/plans/{plan_id}/apply",
                axum::routing::post(
                    move |axum::extract::Path(plan_id): axum::extract::Path<Uuid>| async move {
                        axum::Json(json!({"plan_id": plan_id}))
                    },
                ),
            )
            .route(
                "/solution-packs/applications",
                axum::routing::get(|uri: axum::http::Uri| async move {
                    axum::Json(json!({"query": uri.query()}))
                }),
            )
            .route(
                "/solution-packs/applications/{application_id}",
                axum::routing::get(
                    move |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                        axum::Json(json!({"id": id}))
                    },
                ),
            )
            .route(
                "/solution-packs/applications/{application_id}/abandon",
                axum::routing::post(
                    |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                        axum::Json(json!({"abandoned": id}))
                    },
                ),
            )
            .route(
                "/solution-packs/applications/{application_id}/checks",
                axum::routing::get(
                    |axum::extract::Path(id): axum::extract::Path<Uuid>,
                     uri: axum::http::Uri| async move {
                        axum::Json(json!({"id":id,"query":uri.query()}))
                    },
                )
                .post(
                    |axum::extract::Path(id): axum::extract::Path<Uuid>| async move {
                        axum::Json(json!({"rerun":id}))
                    },
                ),
            )
            .route(
                "/solution-packs/applications/{application_id}/checks/{run_id}",
                axum::routing::get(
                    |axum::extract::Path((application_id, run_id)): axum::extract::Path<(
                        Uuid,
                        Uuid,
                    )>| async move {
                        axum::Json(json!({"application_id":application_id,"run_id":run_id}))
                    },
                ),
            );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let api = Api::new(
        Client::new(),
        Url::parse(&format!("http://{address}")).unwrap(),
    );
    let plan_id = Uuid::from_u128(1);
    assert_eq!(
        solution_pack_command(&api, SolutionPackCommand::Apply { plan_id })
            .await
            .unwrap(),
        format!(r#"{{"plan_id":"{plan_id}"}}"#)
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Applications {
                command: SolutionPackApplicationsCommand::List {
                    page: PageArgs {
                        limit: Some(25),
                        offset: Some(0),
                    },
                },
            }
        )
        .await
        .unwrap(),
        r#"{"query":"limit=25&offset=0"}"#
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Applications {
                command: SolutionPackApplicationsCommand::List {
                    page: PageArgs {
                        limit: Some(100),
                        offset: Some(10_000),
                    },
                },
            }
        )
        .await
        .unwrap(),
        r#"{"query":"limit=100&offset=10000"}"#
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Applications {
                command: SolutionPackApplicationsCommand::Get { application_id },
            }
        )
        .await
        .unwrap(),
        format!(r#"{{"id":"{application_id}"}}"#)
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Applications {
                command: SolutionPackApplicationsCommand::Abandon { application_id },
            }
        )
        .await
        .unwrap(),
        format!(r#"{{"abandoned":"{application_id}"}}"#)
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Checks {
                command: SolutionPackChecksCommand::Rerun { application_id },
            }
        )
        .await
        .unwrap(),
        format!(r#"{{"rerun":"{application_id}"}}"#)
    );
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Checks {
                command: SolutionPackChecksCommand::List {
                    application_id,
                    page: PageArgs {
                        limit: Some(25),
                        offset: Some(0),
                    },
                },
            }
        )
        .await
        .unwrap(),
        format!(r#"{{"id":"{application_id}","query":"limit=25&offset=0"}}"#)
    );
    let run_id = Uuid::from_u128(3);
    assert_eq!(
        solution_pack_command(
            &api,
            SolutionPackCommand::Checks {
                command: SolutionPackChecksCommand::Get {
                    application_id,
                    run_id
                },
            }
        )
        .await
        .unwrap(),
        format!(r#"{{"application_id":"{application_id}","run_id":"{run_id}"}}"#)
    );
    server.abort();
}

#[tokio::test]
async fn solution_pack_plan_rejects_incomplete_and_invalid_forms() {
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "create",
            "--file",
            "pack.tar.zst",
            "--prefix",
            "ecom",
            "--blueprint-publication",
            "invalid",
        ])
        .is_err()
    );
    assert!(Cli::try_parse_from(["acli", "solution-pack", "plan", "get", "not-a-uuid",]).is_err());

    let api = Api::new(Client::new(), Url::parse("http://127.0.0.1:1").unwrap());
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "create",
            "--file",
            "pack.tar.zst",
            "--prefix",
            "ecom",
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "acli",
            "solution-pack",
            "plan",
            "get",
            "00000000-0000-4000-8000-000000000001",
            "--file",
            "pack.tar.zst",
        ])
        .is_err()
    );

    let mut command = <Cli as clap::CommandFactory>::command();
    let help = command
        .find_subcommand_mut("solution-pack")
        .unwrap()
        .find_subcommand_mut("plan")
        .unwrap()
        .find_subcommand_mut("create")
        .unwrap()
        .render_long_help()
        .to_string();
    assert!(help.contains("--map <LOGICAL_KEY=EXISTING_CODE>"));

    for mapping in [
        "missing_equals",
        "=missing_key",
        "blueprints/product=Unsafe-code",
        "contexts/default=valid_code",
    ] {
        let invalid_mapping = solution_pack_command(
            &api,
            SolutionPackCommand::Plan {
                command: SolutionPackPlanCommand::Create {
                    file: PathBuf::from("pack.tar.zst"),
                    prefix: "ecom".to_owned(),
                    blueprint_publication: BlueprintPublicationArgument::Draft,
                    blueprint_maps: vec![mapping.to_owned()],
                    asset_maps: Vec::new(),
                    context_maps: Vec::new(),
                    include_sample_data: false,
                },
            },
        )
        .await;
        assert!(matches!(invalid_mapping, Err(CliError::Input(_))));
    }
}

#[tokio::test]
async fn solution_pack_plan_streams_parameters_and_show_fetches_by_id() {
    use std::sync::{Arc, Mutex};

    let received = Arc::new(Mutex::new(None));
    let plan_id = Uuid::from_u128(0x00000000000040008000000000000166);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn({
        let received = received.clone();
        async move {
            let app = axum::Router::new()
                .route(
                    "/solution-packs/plans",
                    axum::routing::post(
                        move |uri: axum::http::Uri,
                              headers: axum::http::HeaderMap,
                              body: axum::body::Bytes| {
                            let received = received.clone();
                            async move {
                                *received.lock().unwrap() = Some((
                                    uri.query().unwrap().to_owned(),
                                    headers
                                        .get(axum::http::header::CONTENT_TYPE)
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .to_owned(),
                                    body.to_vec(),
                                ));
                                axum::Json(json!({
                                    "id":"created",
                                    "actions":[{
                                        "resource_kind":"workspace_setting",
                                        "logical_key":"workspace/explore-navigation",
                                        "action":"append"
                                    }]
                                }))
                            }
                        },
                    ),
                )
                .route(
                    "/solution-packs/plans/{plan_id}",
                    axum::routing::get(
                        |axum::extract::Path(plan_id): axum::extract::Path<Uuid>| async move {
                            axum::Json(json!({"id": plan_id}))
                        },
                    ),
                );
            axum::serve(listener, app).await.unwrap();
        }
    });
    let archive = tempfile::NamedTempFile::new().unwrap();
    fs::write(archive.path(), b"opaque archive bytes").unwrap();
    let api = Api::new(
        Client::new(),
        Url::parse(&format!("http://{address}")).unwrap(),
    );

    let created = solution_pack_command(
        &api,
        SolutionPackCommand::Plan {
            command: SolutionPackPlanCommand::Create {
                file: archive.path().to_path_buf(),
                prefix: "shop prefix".to_owned(),
                blueprint_publication: BlueprintPublicationArgument::Publish,
                blueprint_maps: Vec::new(),
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                include_sample_data: false,
            },
        },
    )
    .await
    .unwrap();
    assert!(created.contains(r#""resource_kind":"workspace_setting""#));
    assert!(created.contains(r#""logical_key":"workspace/explore-navigation""#));
    assert_eq!(
        received.lock().unwrap().take().unwrap(),
        (
            "prefix=shop+prefix&blueprint_publication=publish".to_owned(),
            "application/zstd".to_owned(),
            b"opaque archive bytes".to_vec()
        )
    );

    solution_pack_command(
        &api,
        SolutionPackCommand::Plan {
            command: SolutionPackPlanCommand::Create {
                file: archive.path().to_path_buf(),
                prefix: "shop".to_owned(),
                blueprint_publication: BlueprintPublicationArgument::Publish,
                blueprint_maps: vec![
                    "blueprints/product=shared_product".to_owned(),
                    "blueprints/category=shared_category".to_owned(),
                ],
                asset_maps: vec![
                    "assets/brand-logo=00000000-0000-4000-8000-000000000999".to_owned(),
                ],
                context_maps: vec!["contexts/poland=PL".to_owned()],
                include_sample_data: false,
            },
        },
    )
    .await
    .unwrap();
    let (query, content_type, body) = received.lock().unwrap().take().unwrap();
    assert_eq!(query, "prefix=shop&blueprint_publication=publish");
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8(body).unwrap();
    assert!(body.contains("name=\"archive\""));
    assert!(body.contains("application/zstd"));
    assert!(body.contains("opaque archive bytes"));
    assert_eq!(body.matches("name=\"blueprint_map\"").count(), 2);
    assert_eq!(body.matches("name=\"asset_map\"").count(), 1);
    assert!(body.contains("assets/brand-logo"));
    assert!(body.contains("00000000-0000-4000-8000-000000000999"));
    assert!(body.contains("blueprints/product"));
    assert!(body.contains("shared_product"));
    assert_eq!(body.matches("name=\"context_map\"").count(), 1);
    assert!(body.contains(r#"{"code":"PL","key":"contexts/poland"}"#));

    let duplicate = solution_pack_command(
        &api,
        SolutionPackCommand::Plan {
            command: SolutionPackPlanCommand::Create {
                file: archive.path().to_path_buf(),
                prefix: "shop".to_owned(),
                blueprint_publication: BlueprintPublicationArgument::Publish,
                blueprint_maps: vec![
                    "blueprints/product=shared_product".to_owned(),
                    "blueprints/product=other_product".to_owned(),
                ],
                asset_maps: Vec::new(),
                context_maps: Vec::new(),
                include_sample_data: false,
            },
        },
    )
    .await;
    assert!(matches!(duplicate, Err(CliError::Input(_))));

    let shown = solution_pack_command(
        &api,
        SolutionPackCommand::Plan {
            command: SolutionPackPlanCommand::Get { plan_id },
        },
    )
    .await
    .unwrap();
    assert!(shown.contains(&plan_id.to_string()));
    server.abort();
}

#[tokio::test]
async fn bearer_token_skips_a_saved_browser_session() {
    let session_file = tempfile::NamedTempFile::new().unwrap();
    fs::write(session_file.path(), "not JSON").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route(
                "/health",
                axum::routing::get(|| async { axum::Json(json!({ "status": "ok" })) }),
            ),
        )
        .await
        .unwrap()
    });

    let body = run(Cli {
        server: Url::parse(&format!("http://{address}")).unwrap(),
        token: Some("personal-token".to_owned()),
        token_stdin: false,
        session_file: Some(session_file.path().to_path_buf()),
        no_env: false,
        command: Command::Health,
    })
    .await
    .unwrap();
    assert_eq!(body, r#"{"status":"ok"}"#);
    server.abort();
}

#[tokio::test]
async fn forwards_http_json_without_reformatting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route(
                "/health",
                axum::routing::get(|| async { axum::Json(json!({ "status": "ok" })) }),
            ),
        )
        .await
        .unwrap()
    });

    let body = run(Cli {
        server: Url::parse(&format!("http://{address}")).unwrap(),
        token: None,
        token_stdin: false,
        session_file: None,
        no_env: false,
        command: Command::Health,
    })
    .await
    .unwrap();
    assert_eq!(body, r#"{"status":"ok"}"#);

    server.abort();
}

#[test]
fn stdin_secrets_need_exactly_one_non_empty_line_each() {
    assert_eq!(
        parse_stdin_lines("token\nsecret\n", ["reset token", "password"]).unwrap(),
        ["token".to_owned(), "secret".to_owned()]
    );
    let error = parse_stdin_lines("token\n\n", ["reset token", "password"]).unwrap_err();
    assert_eq!(error.to_string(), "password must be stdin line 2");
    assert!(parse_stdin_lines("a\nb\nc\n", ["reset token", "password"]).is_err());
}

/// Runs an `acli` command line against a server that answers every request
/// with `{method, path, query, body}` and returns that echo.
async fn echo(args: &[&str]) -> Value {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new().fallback(
        |method: axum::http::Method, uri: axum::http::Uri, body: axum::body::Bytes| async move {
            let body = serde_json::from_slice::<Value>(&body)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&body).into_owned()));
            axum::Json(json!({
                "method": method.as_str(),
                "path": uri.path(),
                "query": uri.query(),
                "body": body,
            }))
        },
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!("http://{address}");
    let mut command = vec!["acli", "--server", &url];
    command.extend_from_slice(args);
    let output = run(Cli::try_parse_from(command).unwrap()).await.unwrap();
    server.abort();
    serde_json::from_str(&output).unwrap()
}

#[tokio::test]
async fn record_status_comment_and_history_commands_build_requests() {
    let id = "00000000-0000-4000-8000-000000000001";
    let comment = "00000000-0000-4000-8000-000000000002";
    assert_eq!(
        echo(&[
            "record",
            "transition",
            id,
            "--attribute",
            "status",
            "--to",
            "approved"
        ])
        .await,
        json!({
            "method": "PUT",
            "path": format!("/v1/records/{id}"),
            "query": null,
            "body": {
                "expected_updated_at": null,
                "values": [{
                    "kind": "scalar",
                    "attribute_code": "status",
                    "context_id": null,
                    "value": "approved",
                }],
            },
        })
    );
    let unlock = echo(&[
        "record",
        "unlock",
        id,
        "--attribute",
        "status",
        "--to",
        "draft",
        "--expected-updated-at",
        "2026-10-01T00:00:00Z",
    ])
    .await;
    assert_eq!(unlock["path"], format!("/v1/records/{id}/status-unlock"));
    assert_eq!(
        unlock["body"]["expected_updated_at"],
        "2026-10-01T00:00:00Z"
    );
    let restore = echo(&[
        "record",
        "restore-value",
        id,
        comment,
        "--expected-updated-at",
        "2026-10-01T00:00:00Z",
    ])
    .await;
    assert_eq!(
        restore["query"],
        "expected_updated_at=2026-10-01T00%3A00%3A00Z"
    );
    let history = echo(&[
        "record",
        "value-history",
        id,
        "--limit",
        "10",
        "--offset",
        "20",
    ])
    .await;
    assert_eq!(history["query"], "limit=10&offset=20");
    let edit = echo(&[
        "record",
        "comment",
        "edit",
        id,
        comment,
        "--body",
        "Fixed",
        "--revision",
        "2",
    ])
    .await;
    assert_eq!(edit["method"], "PATCH");
    assert_eq!(edit["path"], format!("/v1/records/{id}/comments/{comment}"));
    assert_eq!(edit["body"], json!({ "body": "Fixed", "revision": 2 }));
    let labels = echo(&[
        "record",
        "labels",
        "--record-id",
        id,
        "--record-id",
        comment,
    ])
    .await;
    assert_eq!(labels["body"], json!({ "record_ids": [id, comment] }));
    assert!(Cli::try_parse_from(["acli", "record", "labels"]).is_err());
    assert!(
        Cli::try_parse_from(["acli", "record", "approvals", id, "--before-time", "x"]).is_err()
    );
}
