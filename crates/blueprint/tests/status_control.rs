use catalog_blueprint::parse;

fn definition(value_type: &str, schema: serde_json::Value) -> String {
    format!(
        r#"format_version = 1
code = "status_item"
name = "Status item"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["status"]
[[attributes]]
code = "status"
value_type = "{value_type}"
value_schema = '''{schema}'''
"#
    )
}

#[test]
fn status_configuration_is_checked_when_parsing_blueprints() {
    let schema = serde_json::json!({"type":"string", "enum":["draft","live"], "x-attricat-status":{
        "version":1,"options":[{"code":"draft","label":"Draft"},{"code":"live","label":"Live"}],
        "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"live"}]
    }});
    assert!(parse(&definition("string", schema.clone())).is_ok());
    assert!(parse(&definition("number", schema.clone())).is_err());
    let mut invalid = schema.clone();
    invalid["x-attricat-status"]["options"][0]["code"] = serde_json::json!("live");
    assert!(parse(&definition("string", invalid)).is_err());
    let mut invalid = schema;
    invalid["x-attricat-status"]["transitions"][0]["to"] = serde_json::json!("unknown");
    assert!(parse(&definition("string", invalid)).is_err());
}
