use catalog_blueprint::{compile, parse};

const SOURCE: &str = r#"
format_version = 1
code = "contact"
name = "Contact"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["email"]
[views.detail]
type = "stack"
children = [{ type = "field", field = "email", component = { id = "catalog.email_display", version = 1 } }]
[[attributes]]
code = "email"
value_type = "string"
value_schema = '{"type":"string","format":"email"}'
"#;

#[test]
fn server_email_format_accepts_control_addresses_and_rejects_invalid_data() {
    let schema = serde_json::json!({"type": "string", "format": "email"});
    for value in [
        "Name+tag@Example.com",
        "a@localhost",
        "o'brien@example.test",
        "a?b#c@example.test",
    ] {
        assert!(
            catalog_validation::validate_json_schema(&schema, &serde_json::json!(value))
                .unwrap()
                .is_empty(),
            "{value}"
        );
    }
    for value in [
        "not an email",
        "a..b@example.test",
        "a@-example.test",
        "a@example.test,b@example.test",
        "a@example.test\r\nBcc:other@example.test",
    ] {
        assert!(
            !catalog_validation::validate_json_schema(&schema, &serde_json::json!(value))
                .unwrap()
                .is_empty(),
            "{value}"
        );
    }
}

#[test]
fn accepts_email_display_component() {
    assert!(compile(parse(SOURCE).unwrap(), &[], SOURCE).is_ok());
}

#[test]
fn rejects_incompatible_email_components() {
    for invalid in [
        // A detail view displays values; edit components pair with it in the UI.
        SOURCE.replace("catalog.email_display", "catalog.email_edit"),
        SOURCE.replace("value_type = \"string\"", "value_type = \"number\""),
        SOURCE.replace("version = 1 }", "version = 2 }"),
        SOURCE.replace(
            "version = 1 }",
            "version = 1, props = { subject = \"Hi\" } }",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn email_display_is_a_string_table_renderer() {
    let renderer = serde_json::from_value(
        serde_json::json!({"id": "catalog.email_display", "version": 1, "props": {}}),
    )
    .unwrap();
    assert!(catalog_blueprint::validate_table_renderer(&renderer, "string").is_ok());
    assert!(catalog_blueprint::validate_table_renderer(&renderer, "number").is_err());
    let editor = serde_json::from_value(
        serde_json::json!({"id": "catalog.email_edit", "version": 1, "props": {}}),
    )
    .unwrap();
    assert!(catalog_blueprint::validate_table_renderer(&editor, "string").is_err());
}
