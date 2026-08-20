use catalog_blueprint::{
    BlueprintError, EffectiveAttribute, ResolvedInclude, compile, parse, raw_hash,
};

#[test]
fn compiles_only_explicitly_selected_mixin_attributes_in_local_order() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '{"type":"object","required":["meta_title"]}'

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
                    value_schema: None,
                    target_blueprint: None,
                    tags: vec![],
                    context_fallback: "default".to_owned(),
                    context_editable: "all".to_owned(),
                    position: 0,
                },
                EffectiveAttribute {
                    code: "meta_description".to_owned(),
                    value_type: "string".to_owned(),
                    value_schema: None,
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
fn validates_reference_codes_with_the_shared_character_set() {
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
"#;
    for replacement in ["product-name", "product_name"] {
        assert!(parse(&source.replacen("product", replacement, 1)).is_ok());
    }
    for replacement in ["product name", "product.name", "produit-été", ""] {
        assert!(parse(&source.replacen("product", replacement, 1)).is_err());
    }
    for replacement in ["page-title", "page_title"] {
        assert!(
            parse(&source.replace("code = \"title\"", &format!("code = \"{replacement}\"")))
                .is_ok()
        );
    }
    for replacement in ["page title", "page.title", "page.titlé", ""] {
        assert!(
            parse(&source.replace("code = \"title\"", &format!("code = \"{replacement}\"")))
                .is_err()
        );
    }
}

#[test]
fn validates_include_aliases_with_the_shared_character_set() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[[includes]]
alias = "seo-metadata"
code = "seo"
version = 1

[[attributes]]
code = "meta_title"
from = "seo-metadata.meta_title"
"#;
    assert!(parse(source).is_ok());
    for alias in ["seo metadata", "seo.metadata", "seo-métadata", ""] {
        assert!(parse(&source.replace("seo-metadata", alias)).is_err());
    }
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

#[test]
fn compiles_optional_recursive_views_with_component_references() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[views.detail]
type = "stack"
children = [
  { type = "heading", text = "Product" },
  { type = "section", children = [
    { type = "grid", children = [{ type = "field", field = "title", component = { id = "catalog.field_display", version = 1 } }] },
    { type = "tabs", tabs = [{ label = "Details", children = [{ type = "text", text = "Information" }] }] },
    { type = "accordion", sections = [{ label = "Related", children = [{ type = "relationship_list", field = "categories" }] }] },
    { type = "divider" }
  ] }
]

[views.index]
type = "table"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.views.len(), 2);
    assert_eq!(
        compiled.views["detail"],
        parse(source).unwrap().views["detail"]
    );
}

#[test]
fn rejects_invalid_view_fields_and_component_references() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[views.index]
type = "table"
fields = ["categories"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_err());

    let invalid_component_id = source.replace(
        "fields = [\"categories\"]",
        "fields = [\"title\"]\ncomponent = { id = \"catalog.bad-id\", version = 1 }",
    );
    assert!(
        compile(
            parse(&invalid_component_id).unwrap(),
            &[],
            &invalid_component_id
        )
        .is_err()
    );

    let invalid_component_version = source.replace(
        "fields = [\"categories\"]",
        "fields = [\"title\"]\ncomponent = { id = \"catalog.table\", version = 0 }",
    );
    assert!(
        compile(
            parse(&invalid_component_version).unwrap(),
            &[],
            &invalid_component_version
        )
        .is_err()
    );

    let invalid_relationship_leaf = source.replace(
        "type = \"table\"\nfields = [\"categories\"]",
        "type = \"stack\"\nchildren = [{ type = \"relationship_list\", field = \"title\" }]",
    );
    assert!(
        compile(
            parse(&invalid_relationship_leaf).unwrap(),
            &[],
            &invalid_relationship_leaf
        )
        .is_err()
    );
}

#[test]
fn validates_component_manifest_applicability() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[views.detail]
type = "stack"
children = [{ type = "field", field = "title", component = { id = "catalog.field_display", version = 1 } }]

[[attributes]]
code = "title"
value_type = "string"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("[views.detail]", "[views.edit]"),
        source.replace("catalog.field_display", "catalog.table_display"),
        source.replace("version = 1 }", "version = 2 }"),
        source.replace(
            "version = 1 }",
            "version = 1, props = { label = \"Title\" } }",
        ),
        source.replace(
            "children = [{ type = \"field\", field = \"title\", component = { id = \"catalog.field_display\", version = 1 } }]",
            "component = { id = \"catalog.field_display\", version = 1 }\nchildren = []",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn compiles_json_schema_contracts() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '{"type":"object","required":["price"]}'

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":0}'
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.entity_schema.unwrap()["required"][0], "price");
    assert_eq!(
        compiled.attributes[1].value_schema.as_ref().unwrap()["minimum"],
        0
    );
}

#[test]
fn rejects_entity_schema_references_to_unknown_attributes() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '__SCHEMA__'

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
"#;

    for (schema, keyword) in [
        (r#"{"type":"object","required":["unknown"]}"#, "required"),
        (
            r#"{"type":"object","properties":{"unknown":{"type":"string"}}}"#,
            "properties",
        ),
        (
            r#"{"type":"object","dependentRequired":{"title":["unknown"]}}"#,
            "dependentRequired",
        ),
        (
            r#"{"type":"object","dependentSchemas":{"unknown":{"type":"object"}}}"#,
            "dependentSchemas",
        ),
    ] {
        let blueprint = source.replace("__SCHEMA__", schema);
        let error = compile(parse(&blueprint).unwrap(), &[], &blueprint).unwrap_err();
        assert!(matches!(
            error,
            BlueprintError::EntitySchemaUnknownAttribute {
                keyword: actual_keyword,
                attribute,
            } if actual_keyword == keyword && attribute == "unknown"
        ));
    }
}

#[test]
fn allows_nested_entity_schema_value_properties() {
    let source = r#"
format_version = 1
code = "schedule"
name = "Schedule"
kind = "entity"
entity_schema = '{"type":"object","properties":{"cutoff":{"type":"object","required":["time"],"properties":{"time":{"type":"string"}}}}}'

[display.dropdown_option]
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "cutoff"
value_type = "time"
"#;

    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
}

#[test]
fn rejects_invalid_json_schema_contracts() {
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
"#;
    for invalid in [
        source.replace(
            "value_type = \"string\"",
            "value_type = \"string\"\nvalue_schema = \"not json\"",
        ),
        source.replace(
            "kind = \"entity\"",
            "kind = \"mixin\"\nentity_schema = '{\"type\":\"object\"}'",
        ),
        source.replace(
            "value_type = \"string\"",
            "value_type = \"relationship\"\nvalue_schema = '{\"type\":\"array\"}'",
        ),
    ] {
        assert!(parse(&invalid).is_err());
    }
}

#[test]
fn validates_entity_heading_stack_component() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[display.dropdown_option]
fields = ["title"]

[views.detail]
type = "stack"
component = { id = "catalog.entity_heading", version = 1 }
children = [
  { type = "field", field = "title" },
  { type = "text", text = "SKU" },
  { type = "field", field = "sku" }
]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("[views.detail]", "[views.edit]"),
        source.replace(
            "component = { id = \"catalog.entity_heading\", version = 1 }",
            "component = { id = \"catalog.field_display\", version = 1 }",
        ),
        source.replace(
            "{ type = \"field\", field = \"title\" }",
            "{ type = \"text\", text = \"Product\" }",
        ),
        source.replace("field = \"title\"", "field = \"category\""),
        source.replace(
            "{ type = \"text\", text = \"SKU\" }",
            "{ type = \"relationship_list\", field = \"category\" }",
        ),
        source.replace(
            "{ type = \"text\", text = \"SKU\" }",
            "{ type = \"grid\", children = [] }",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}
