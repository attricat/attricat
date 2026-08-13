use catalog_blueprint::{EffectiveAttribute, ResolvedInclude, compile, parse, raw_hash};

#[test]
fn compiles_only_explicitly_selected_mixin_attributes_in_local_order() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

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
                    context_fallback: "default".to_owned(),
                    context_editable: "all".to_owned(),
                    position: 0,
                },
                EffectiveAttribute {
                    code: "meta_description".to_owned(),
                    value_type: "string".to_owned(),
                    target_blueprint: None,
                    tags: vec![],
                    context_fallback: "default".to_owned(),
                    context_editable: "all".to_owned(),
                    position: 1,
                },
            ],
        }],
        source,
    )
    .unwrap();

    assert_eq!(compiled.attributes.len(), 2);
    assert_eq!(compiled.attributes[0].code, "title");
    assert_eq!(compiled.attributes[0].context_fallback, "default");
    assert_eq!(compiled.attributes[0].context_editable, "all");
    assert_eq!(compiled.attributes[0].position, 0);
    assert_eq!(compiled.attributes[1].code, "meta_title");
    assert_eq!(compiled.attributes[1].position, 1);
    assert_eq!(compiled.raw_definition_hash, raw_hash(source));
}

#[test]
fn supports_context_fallback_on_all_attribute_types() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
context_fallback = "none"

[[attributes]]
code = "related"
value_type = "relationship"
context_fallback = "none"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(
        compiled
            .attributes
            .iter()
            .all(|attribute| attribute.context_fallback == "none")
    );
}

#[test]
fn supports_default_only_context_editing() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "stock"
value_type = "integer"
context_editable = "default"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.attributes[0].context_editable, "all");
    assert_eq!(compiled.attributes[1].context_editable, "default");
}

#[test]
fn rejects_unknown_fields_and_invalid_selections() {
    let unknown_field = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["name"]
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

[display.dropdown_option]
fields = ["name"]

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "category"

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]
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
fn requires_valid_dropdown_option_display_and_preserves_generic_tags() {
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
        "value_type = \"string\"\ntags = [\"searchable\"]\n\n[display.dropdown_option]\nfields = [\"title\"]",
    );
    let compiled = compile(parse(&source).unwrap(), &[], &source).unwrap();
    assert_eq!(compiled.attributes[0].tags, ["searchable"]);
    assert_eq!(compiled.display["dropdown_option"].separator, " · ");

    let relationship_display = source.replace("fields = [\"title\"]", "fields = [\"unknown\"]");
    assert!(
        compile(
            parse(&relationship_display).unwrap(),
            &[],
            &relationship_display
        )
        .is_err()
    );

    let empty_fields = source.replace("fields = [\"title\"]", "fields = []");
    assert!(compile(parse(&empty_fields).unwrap(), &[], &empty_fields).is_err());

    let duplicate_fields =
        source.replace("fields = [\"title\"]", "fields = [\"title\", \"title\"]");
    assert!(compile(parse(&duplicate_fields).unwrap(), &[], &duplicate_fields).is_err());
}
