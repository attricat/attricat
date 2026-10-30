use attricat_blueprint::parse;

const BLUEPRINT: &str = r#"
format_version: 1
code: product
name: Product
kind: record
rules:
  - code: title-required
    name: Title required
    severity: error
    triggers:
      - {type: manual}
    predicate: {type: required, attribute_code: title}
attributes:
  - code: title
    value_type: string
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
"#;

#[test]
fn stored_definitions_with_removed_rules_still_parse() {
    // Revisions saved before rules became standalone keep their `rules`
    // list; they parse, flagged, without being compiled.
    let blueprint = parse(BLUEPRINT).expect("a stored definition parses");
    assert!(blueprint.embedded_rules);
    let rules = BLUEPRINT.find("rules:").unwrap()..BLUEPRINT.find("attributes:").unwrap();
    let mut without = BLUEPRINT.to_owned();
    without.replace_range(rules, "");
    assert!(!parse(&without).expect("parses").embedded_rules);
}

const CHECKED: &str = r#"
format_version: 1
code: contract
name: Contract
kind: record
record_schema: |-
  {"x-attricat-checks": [
    {"code": "range", "predicate": {"type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from"}}
  ]}
attributes:
  - code: owner
    value_type: string
  - code: valid_from
    value_type: date
  - code: valid_until
    value_type: date
  - code: status
    value_type: string
    value_schema: |-
      {"type":"string","enum":["draft","released"],"x-attricat-status":{"version":1,
        "options":[{"code":"draft","label":"Draft"},{"code":"released","label":"Released"}],
        "transitions":[{"from":null,"to":"draft"},{"from":"draft","to":"released","conditions":[
          {"code":"has-start","predicate":{"type":"required","attribute_code":"valid_from"}}]}]}}
views:
  dropdown_option:
    type: dropdown_option
    fields: [owner]
"#;

fn compile(
    source: &str,
) -> Result<attricat_blueprint::CompiledBlueprint, attricat_blueprint::BlueprintError> {
    attricat_blueprint::compile(parse(source)?, &[], source)
}

#[test]
fn type_checks_record_checks_and_conditions() {
    compile(CHECKED).expect("declarative checks compile");
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
    ] {
        assert!(compile(&CHECKED.replace(from, to)).is_err(), "{to}");
    }
}
