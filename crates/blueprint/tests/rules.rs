use catalog_blueprint::{BlueprintError, parse};

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
    assert!(matches!(
        parse(&BLUEPRINT.replace("attribute_code = \"title\"", "attribute_code = \"missing\"")),
        Err(BlueprintError::RuleUnknownAttribute { rule, attribute })
            if rule == "title-required" && attribute == "missing"
    ));
}

const CHECKED: &str = r#"
format_version = 1
code = "contract"
name = "Contract"
kind = "entity"
entity_schema = '''{"x-attricat-checks": [
  {"code": "range", "predicate": {"type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from"}}
]}'''

[[rules]]
code = "released-needs-owner"
name = "Released contracts have an owner"
severity = "error"
[[rules.triggers]]
type = "manual"
[rules.predicate]
type = "required"
attribute_code = "owner"
[rules.enforcement]
[[rules.enforcement.transitions]]
attribute_code = "status"
to = "released"

[[attributes]]
code = "owner"
value_type = "string"

[[attributes]]
code = "valid_from"
value_type = "date"

[[attributes]]
code = "valid_until"
value_type = "date"

[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{"type":"string","enum":["draft","released"],"x-attricat-status":{"version":1,
  "options":[{"code":"draft","label":"Draft"},{"code":"released","label":"Released"}],
  "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"released","conditions":[
    {"code":"has-start","predicate":{"type":"required","attribute_code":"valid_from"}}]}]}}'''

[views.dropdown_option]
type = "dropdown_option"
fields = ["owner"]
"#;

fn compile(
    source: &str,
) -> Result<catalog_blueprint::CompiledBlueprint, catalog_blueprint::BlueprintError> {
    catalog_blueprint::compile(parse(source)?, &[], source)
}

#[test]
fn type_checks_entity_checks_conditions_and_enforcement() {
    let compiled = compile(CHECKED).expect("declarative checks compile");
    assert!(compiled.rules[0].enforcement.is_some());
    for (from, to) in [
        // Ordering a string attribute.
        (
            r#""attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from""#,
            r#""attribute_code": "owner", "op": "gte", "value": "x""#,
        ),
        // Condition on an unknown attribute.
        (
            r#""attribute_code":"valid_from"}}]"#,
            r#""attribute_code":"missing"}}]"#,
        ),
        // Enforcement on a status that does not exist.
        ("to = \"released\"", "to = \"archived\""),
        // Enforcement on a non-status attribute.
        (
            "attribute_code = \"status\"\nto",
            "attribute_code = \"owner\"\nto",
        ),
    ] {
        assert!(compile(&CHECKED.replace(from, to)).is_err(), "{to}");
    }
}
