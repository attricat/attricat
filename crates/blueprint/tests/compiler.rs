use catalog_blueprint::{EffectiveAttribute, ResolvedInclude, compile, parse, raw_hash};

#[test]
fn compiles_only_explicitly_selected_mixin_attributes_in_local_order() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[includes]]
alias = "seo"
code = "seo"
version = 2

[[attributes]]
code = "title"
value_type = "string"
tags = ["display"]

[[attributes]]
code = "meta_title"
from = "seo.meta_title"
"#;
    let definition = parse(source).unwrap();
    let compiled = compile(
        definition,
        &[ResolvedInclude {
            alias: "seo".to_owned(),
            code: "seo".to_owned(),
            version: 2,
            attributes: vec![
                EffectiveAttribute {
                    code: "meta_title".to_owned(),
                    value_type: "string".to_owned(),
                    target_blueprint: None,
                    tags: vec![],
                    position: 0,
                },
                EffectiveAttribute {
                    code: "meta_description".to_owned(),
                    value_type: "string".to_owned(),
                    target_blueprint: None,
                    tags: vec![],
                    position: 1,
                },
            ],
        }],
        source,
    )
    .unwrap();

    assert_eq!(compiled.attributes.len(), 2);
    assert_eq!(compiled.attributes[0].code, "title");
    assert_eq!(compiled.attributes[0].position, 0);
    assert_eq!(compiled.attributes[1].code, "meta_title");
    assert_eq!(compiled.attributes[1].position, 1);
    assert_eq!(compiled.raw_definition_hash, raw_hash(source));
}

#[test]
fn rejects_unknown_fields_and_invalid_selections() {
    let unknown_field = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
unexpected = true

[[attributes]]
code = "title"
value_type = "string"
"#;
    assert!(parse(unknown_field).is_err());

    let invalid_selection = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "title"
from = "seo.meta_title"
"#;
    assert!(parse(invalid_selection).is_err());
}

#[test]
fn compiles_relationship_target_blueprint() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "name"
value_type = "string"
tags = ["display"]
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.attributes[0].target_blueprint.as_deref(),
        Some("category")
    );

    let invalid = source.replace("value_type = \"relationship\"", "value_type = \"string\"");
    assert!(parse(&invalid).is_err());
}

#[test]
fn requires_and_preserves_entity_display_tags() {
    let missing_display = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[attributes]]
code = "title"
value_type = "string"
"#;
    assert!(compile(parse(missing_display).unwrap(), &[], missing_display).is_err());

    let source = missing_display.replace(
        "value_type = \"string\"",
        "value_type = \"string\"\ntags = [\"display\", \"searchable\"]",
    );
    let compiled = compile(parse(&source).unwrap(), &[], &source).unwrap();
    assert_eq!(compiled.attributes[0].tags, ["display", "searchable"]);
}
