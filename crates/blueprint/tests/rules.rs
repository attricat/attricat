use catalog_blueprint::parse;

const BLUEPRINT: &str = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[rules]]
code = "title-required"
name = "Title required"
severity = "error"
[[rules.triggers]]
type = "manual"
[rules.predicate]
type = "required"
attribute_code = "title"

[[attributes]]
code = "title"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
"#;

#[test]
fn parses_embedded_rule_and_rejects_unknown_attribute() {
    let blueprint = parse(BLUEPRINT).expect("embedded rule parses");
    assert_eq!(blueprint.rules.len(), 1);
    assert_eq!(blueprint.rules[0].code, "title-required");
    assert!(
        parse(&BLUEPRINT.replace("attribute_code = \"title\"", "attribute_code = \"missing\""))
            .is_err()
    );
}
