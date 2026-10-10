use catalog_blueprint::{
    BlueprintError, EffectiveAttribute, ResolvedInclude, ViewDefinition, compile, parse, raw_hash,
};

#[test]
fn compiles_only_explicitly_selected_mixin_attributes_in_local_order() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"
record_schema = '{"type":"object","required":["meta_title"]}'

[views.dropdown_option]
type = "dropdown_option"
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
                    name: Some("SEO title".to_owned()),
                    value_type: "string".to_owned(),
                    value_schema: None,
                    extension_type: None,
                    default_value: None,
                    file_policy: None,
                    target_blueprint: None,
                    target_blueprints: vec![],
                    cardinality: None,
                    target_cardinality: None,
                    hierarchy: None,
                    tags: vec![],
                    context_fallback: "default".to_owned(),
                    context_editable: "all".to_owned(),
                    readonly: false,
                    position: 0,
                },
                EffectiveAttribute {
                    code: "meta_description".to_owned(),
                    name: None,
                    value_type: "string".to_owned(),
                    value_schema: None,
                    extension_type: None,
                    default_value: None,
                    file_policy: None,
                    target_blueprint: None,
                    target_blueprints: vec![],
                    cardinality: None,
                    target_cardinality: None,
                    hierarchy: None,
                    tags: vec![],
                    context_fallback: "default".to_owned(),
                    context_editable: "all".to_owned(),
                    readonly: false,
                    position: 1,
                },
            ],
        }],
        source,
    )
    .unwrap();

    assert_eq!(compiled.attributes.len(), 2);
    assert_eq!(compiled.attributes[0].code, "title");
    assert_eq!(compiled.attributes[0].name, None);
    assert_eq!(compiled.attributes[0].context_fallback, "default");
    assert_eq!(compiled.attributes[0].context_editable, "all");
    assert_eq!(compiled.attributes[0].position, 0);
    assert_eq!(compiled.attributes[1].code, "meta_title");
    assert_eq!(compiled.attributes[1].name.as_deref(), Some("SEO title"));
    assert_eq!(compiled.attributes[1].position, 1);
    assert_eq!(compiled.raw_definition_hash, raw_hash(source));
}

#[test]
fn compiles_optional_attribute_names() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["product_family"]

[[attributes]]
code = "product_family"
name = "Family"
value_type = "string"

[[attributes]]
code = "rating"
name = "Rating"
extension_type = "acme:stars@^1"

[[attributes]]
code = "sku"
value_type = "string"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    let names: Vec<_> = compiled
        .attributes
        .iter()
        .map(|attribute| attribute.name.as_deref())
        .collect();
    assert_eq!(names, [Some("Family"), Some("Rating"), None]);
}

#[test]
fn rejects_blank_attribute_names() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[attributes]]
code = "title"
name = "  "
value_type = "string"
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::EmptyField("attribute name"))
    ));
}

#[test]
fn rejects_names_on_selected_attributes() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "meta_title"
name = "Title"
from = "seo.meta_title"
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::InvalidAttributeDeclaration(code)) if code == "meta_title"
    ));
}

#[test]
fn parses_blueprint_and_attribute_descriptions() {
    let source = r#"
format_version = 1
code = "category"
name = "Category"
description = "Product groupings, such as Basic tools"
kind = "record"

[[attributes]]
code = "title"
description = "The category name shown to shoppers"
value_type = "string"
"#;
    let definition = parse(source).unwrap();
    assert_eq!(
        definition.description.as_deref(),
        Some("Product groupings, such as Basic tools")
    );
    let catalog_blueprint::AttributeDeclaration::Local(title) = &definition.attributes[0] else {
        panic!("title is a local attribute");
    };
    assert_eq!(
        title.description.as_deref(),
        Some("The category name shown to shoppers")
    );
    assert!(
        catalog_blueprint::lexicon_texts(&definition)
            .iter()
            .any(|text| text.location == "attribute 'title' description")
    );
}

#[test]
fn rejects_blank_long_and_selected_descriptions() {
    let with = |description: &str| {
        format!(
            "format_version = 1\ncode = \"product\"\nname = \"Product\"\nkind = \"record\"\n\n[[attributes]]\ncode = \"title\"\ndescription = \"{description}\"\nvalue_type = \"string\"\n"
        )
    };
    assert!(matches!(
        parse(&with("  ")),
        Err(BlueprintError::EmptyField("attribute description"))
    ));
    assert!(matches!(
        parse(&with(&"x".repeat(501))),
        Err(BlueprintError::FieldTooLong {
            field: "attribute description",
            max: 500
        })
    ));
    let selected = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[includes]]
alias = "seo"
code = "seo"
version = 1

[[attributes]]
code = "meta_title"
description = "Search title"
from = "seo.meta_title"
"#;
    assert!(matches!(
        parse(selected),
        Err(BlueprintError::InvalidAttributeDeclaration(code)) if code == "meta_title"
    ));
}

#[test]
fn compiles_scalar_attribute_default_values() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
default_value = "Untitled"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.attributes[0].default_value,
        Some(serde_json::json!("Untitled"))
    );
}

#[test]
fn rejects_default_values_on_non_scalar_attributes() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[attributes]]
code = "related"
value_type = "relationship"
default_value = "00000000-0000-0000-0000-000000000000"
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::DefaultValueOnNonScalarAttribute { .. })
    ));
}

#[test]
fn supports_context_fallback_on_all_attribute_types() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
fn supports_readonly_attributes() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "stock"
value_type = "integer"
readonly = true
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(!compiled.attributes[0].readonly);
    assert!(compiled.attributes[1].readonly);
}

#[test]
fn supports_default_only_context_editing() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
kind = "record"

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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
kind = "record"

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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
fn requires_valid_dropdown_option_view_and_preserves_generic_tags() {
    let missing_display = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[[attributes]]
code = "title"
value_type = "string"
"#;
    assert!(compile(parse(missing_display).unwrap(), &[], missing_display).is_err());

    let source = missing_display.replace(
        "value_type = \"string\"",
        "value_type = \"string\"\ntags = [\"searchable\"]\n\n[views.dropdown_option]\ntype = \"dropdown_option\"\nfields = [\"title\"]",
    );
    let compiled = compile(parse(&source).unwrap(), &[], &source).unwrap();
    assert_eq!(compiled.attributes[0].tags, ["searchable"]);
    assert!(matches!(
        compiled.views["dropdown_option"],
        ViewDefinition::DropdownOption { ref separator, .. } if separator == " · "
    ));

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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
    assert_eq!(compiled.views.len(), 3);
    assert_eq!(
        compiled.views["detail"],
        parse(source).unwrap().views["detail"]
    );
}

#[test]
fn compiles_incoming_relationship_lists() {
    let source = r#"
format_version = 1
code = "category"
name = "Category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[views.detail]
type = "stack"
children = [{ type = "incoming_relationship_list", label = "Products in this category", page_size = 10, relationships = [{ source_blueprint = "product", field = "categories" }], component = { id = "catalog.incoming_relationship_list_display", version = 1 } }]

[[attributes]]
code = "name"
value_type = "string"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("page_size = 10", "page_size = 0"),
        source.replace("label = \"Products in this category\"", "label = \" \""),
        source.replace("[views.detail]", "[views.edit]"),
        source.replace(
            "relationships = [{ source_blueprint = \"product\", field = \"categories\" }]",
            "relationships = []",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn rejects_invalid_view_fields_and_component_references() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
fn validates_phone_components() {
    let source = r#"
format_version = 1
code = "contact"
name = "Contact"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["phone"]

[views.detail]
type = "stack"
children = [{ type = "field", field = "phone", component = { id = "catalog.phone_display", version = 1 } }]

[views.edit]
type = "stack"
children = [{ type = "field", field = "phone", component = { id = "catalog.phone_edit", version = 1 } }]

[views.table]
type = "table"
columns = [{ field = "phone", renderer = { id = "catalog.phone_display", version = 1 } }]

[[attributes]]
code = "phone"
value_type = "string"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type = \"string\"", "value_type = \"integer\""),
        source.replace("catalog.phone_edit", "catalog.phone_display"),
        source.replace("catalog.phone_display", "catalog.phone_edit"),
        source.replace("version = 1 }", "version = 2 }"),
        source.replace(
            "version = 1 }",
            "version = 1, props = { region = \"PL\" } }",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_url_components() {
    let source = r#"
format_version = 1
code = "website"
name = "Website"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["url"]
[views.detail]
type = "stack"
children = [{ type = "field", field = "url", component = { id = "catalog.url_display", version = 1 } }]
[views.edit]
type = "stack"
children = [{ type = "field", field = "url", component = { id = "catalog.url_edit", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "url", renderer = { id = "catalog.url_display", version = 1 } }]
[[attributes]]
code = "url"
value_type = "string"
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type = \"string\"", "value_type = \"number\""),
        source.replace("catalog.url_display", "catalog.url_edit"),
        source.replace("catalog.url_edit", "catalog.url_display"),
        source.replace("version = 1 }", "version = 2 }"),
        source.replace("version = 1 }", "version = 1, props = { unsafe = true } }"),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_color_components() {
    let source = r#"
format_version = 1
code = "color"
name = "Color"
kind = "record"
attributes = [{ code = "hex", value_type = "string" }]
[views.dropdown_option]
type = "dropdown_option"
fields = ["hex"]
[views.detail]
type = "stack"
children = [{ type = "field", field = "hex", component = { id = "catalog.color_display", version = 1 } }]
[views.edit]
type = "stack"
children = [{ type = "field", field = "hex", component = { id = "catalog.color_edit", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "hex", renderer = { id = "catalog.color_display", version = 1 } }]
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type = \"string\"", "value_type = \"number\""),
        source.replace("catalog.color_display", "catalog.color_edit"),
        source.replace("catalog.color_edit", "catalog.color_display"),
        source.replace("version = 1 }", "version = 2 }"),
        source.replace("version = 1 }", "version = 1, props = { alpha = true } }"),
        source.replace("type = \"field\"", "type = \"relationship_list\""),
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
kind = "record"
record_schema = '{"type":"object","required":["price"]}'

[views.dropdown_option]
type = "dropdown_option"
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
    assert_eq!(compiled.record_schema.unwrap()["required"][0], "price");
    assert_eq!(
        compiled.attributes[1].value_schema.as_ref().unwrap()["minimum"],
        0
    );
}

#[test]
fn rejects_record_schema_references_to_unknown_attributes() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"
record_schema = '__SCHEMA__'

[views.dropdown_option]
type = "dropdown_option"
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
            BlueprintError::RecordSchemaUnknownAttribute {
                keyword: actual_keyword,
                attribute,
            } if actual_keyword == keyword && attribute == "unknown"
        ));
    }
}

#[test]
fn allows_nested_record_schema_value_properties() {
    let source = r#"
format_version = 1
code = "schedule"
name = "Schedule"
kind = "record"
record_schema = '{"type":"object","properties":{"cutoff":{"type":"object","required":["time"],"properties":{"time":{"type":"string"}}}}}'

[views.dropdown_option]
type = "dropdown_option"
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
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
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
            "kind = \"record\"",
            "kind = \"mixin\"\nrecord_schema = '{\"type\":\"object\"}'",
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
fn validates_record_heading_stack_component() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.detail]
type = "stack"
component = { id = "catalog.record_heading", version = 1 }
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
            "component = { id = \"catalog.record_heading\", version = 1 }",
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

#[test]
fn validates_self_referential_hierarchy_component() {
    let source = r#"
format_version = 1
code = "category"
name = "Category"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.detail]
type = "stack"
children = [
  { type = "relationship_list", field = "parent", component = { id = "catalog.relationship_hierarchy", version = 1 } }
]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "category"
"#;
    compile(parse(source).unwrap(), &[], source).unwrap();

    let invalid = source.replace(
        "target_blueprint = \"category\"",
        "target_blueprint = \"product\"",
    );
    assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());

    let blank_parent_field = invalid.replace(
        "version = 1 }",
        "version = 1, props = { parent_field = \"\" } }",
    );
    assert!(
        compile(
            parse(&blank_parent_field).unwrap(),
            &[],
            &blank_parent_field
        )
        .is_err()
    );
}

#[test]
fn rejects_empty_legacy_table_fields() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
fields = []

[[attributes]]
code = "title"
value_type = "string"
"#;
    assert!(matches!(
        compile(parse(source).unwrap(), &[], source),
        Err(BlueprintError::EmptyTableColumns)
    ));
}

#[test]
fn extension_layout_is_record_only_and_rejects_ambiguous_keys() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.record_preview_panel]
order = ["acme.inventory:summary"]
hidden = ["acme.legacy:panel"]

[[attributes]]
code = "title"
value_type = "string"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(matches!(
        compiled.views.get("extension_layout"),
        Some(ViewDefinition::ExtensionLayout { version: 1, .. })
    ));

    for invalid in [
        source.replace("kind = \"record\"", "kind = \"mixin\""),
        source.replace(
            "version = 1\n\n[views.extension_layout.outlets",
            "version = 2\n\n[views.extension_layout.outlets",
        ),
        source.replace("record_preview_panel", "navigation"),
        source.replace("acme.inventory:summary", "malformed"),
        source.replace(
            "hidden = [\"acme.legacy:panel\"]",
            "hidden = [\"acme.inventory:summary\"]",
        ),
        format!(
            "{source}\n[views.extension_layout.outlets.record_action]\norder = [\"acme.inventory:summary\"]\nhidden = []\n"
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn table_columns_preserve_legacy_fields_and_reject_invalid_paths() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [
  { field = "title", label = "Title", renderer = { id = "catalog.table_display", version = 1 } },
  { field = "category.name" },
]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "category"
cardinality = "one"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.attributes[1].cardinality.as_deref(), Some("one"));
    assert_eq!(
        compiled.attributes[1].target_cardinality.as_deref(),
        Some("many")
    );
    let legacy = compile(
        parse(&source.replace("cardinality = \"one\"", "cardinality = \"one_to_one\"")).unwrap(),
        &[],
        source,
    )
    .unwrap();
    assert_eq!(legacy.attributes[1].cardinality.as_deref(), Some("one"));
    assert_eq!(
        legacy.attributes[1].target_cardinality.as_deref(),
        Some("one")
    );
    assert!(
        compile(
            parse(&source.replace("category.name", "category.parent.parent.parent.name",),)
                .unwrap(),
            &[],
            source,
        )
        .is_err()
    );
    assert!(
        compile(
            parse(&source.replace("columns = [", "fields = [\"title\"]\ncolumns = [")).unwrap(),
            &[],
            source
        )
        .is_err()
    );
    assert!(parse(&source.replace("cardinality = \"one\"", "cardinality = \"invalid\"")).is_err());
    // Modern column tables must validate their table-level component too.
    let invalid_table_component = source.replace(
        "columns = [",
        "component = { id = \"catalog.table_edit\", version = 1 }\ncolumns = [",
    );
    assert!(
        compile(
            parse(&invalid_table_component).unwrap(),
            &[],
            &invalid_table_component
        )
        .is_err()
    );
    assert!(parse(&source.replace("version = 1", "version = -1")).is_err());
}

#[test]
fn parses_extension_type_reference_and_json_primitive() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["price"]

[[attributes]]
code = "price"
extension_type = "com.acme.commerce:money@^1"
extension_configuration = '{"currency":"USD"}'

[[attributes]]
code = "metadata"
value_type = "json"
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.attributes[0].value_type, "string");
    assert_eq!(
        compiled.attributes[0].extension_type.as_ref().unwrap()["reference"],
        "com.acme.commerce:money@^1"
    );
    assert_eq!(compiled.attributes[1].value_type, "json");
}

#[test]
fn deprecated_edit_view_has_no_placement_requirements() {
    // Records are edited in place on the detail view; a legacy `views.edit`
    // is accepted for compatibility but no longer has to place required
    // attributes.
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "record"
record_schema = '{"type":"object","required":["title","price"]}'

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.edit]
type = "stack"
children = [{ type = "field", field = "title" }]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"
"#;
    compile(parse(source).unwrap(), &[], source).unwrap();
}

fn compile_source(source: &str) -> Result<catalog_blueprint::CompiledBlueprint, BlueprintError> {
    compile(parse(source)?, &[], source)
}

const STRUCTURAL_BLUEPRINT: &str = r#"
format_version = 1
code = "part"
name = "Part"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["part_number"]

[[unique_keys]]
code = "manufacturer_part"
attributes = ["manufacturer", "part_number"]

[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
case_sensitive = true

[[attributes]]
code = "part_number"
value_type = "string"

[[attributes]]
code = "slug"
value_type = "string"

[[attributes]]
code = "manufacturer"
value_type = "relationship"
target_blueprint = "manufacturer"
cardinality = "one"

[[attributes]]
code = "subject"
value_type = "relationship"
target_blueprints = ["product", "material", "part"]

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "part"
tree = true
context_editable = "default"

[[attributes]]
code = "predecessors"
value_type = "relationship"
target_blueprints = ["part"]
acyclic = true
context_editable = "default"
"#;

#[test]
fn compiles_unique_keys_target_sets_and_hierarchies() {
    let compiled = compile_source(STRUCTURAL_BLUEPRINT).unwrap();
    assert_eq!(compiled.unique_keys.len(), 2);
    assert_eq!(compiled.unique_keys[0].scope, "workspace");
    assert!(!compiled.unique_keys[0].case_sensitive);
    assert_eq!(compiled.unique_keys[1].scope, "context");
    assert!(compiled.unique_keys[1].case_sensitive);
    let attribute = |code: &str| {
        compiled
            .attributes
            .iter()
            .find(|attribute| attribute.code == code)
            .unwrap()
            .clone()
    };
    let subject = attribute("subject");
    assert_eq!(subject.target_blueprint, None);
    assert_eq!(subject.target_blueprints, ["product", "material", "part"]);
    let manufacturer = attribute("manufacturer");
    assert_eq!(
        manufacturer.target_blueprint.as_deref(),
        Some("manufacturer")
    );
    assert_eq!(manufacturer.target_blueprints, ["manufacturer"]);
    let parent = attribute("parent");
    assert_eq!(parent.hierarchy.as_deref(), Some("tree"));
    assert_eq!(parent.cardinality.as_deref(), Some("one"));
    let predecessors = attribute("predecessors");
    assert_eq!(predecessors.hierarchy.as_deref(), Some("acyclic"));
    assert_eq!(predecessors.cardinality.as_deref(), Some("many"));
    // A one-element list is the single-target form.
    assert_eq!(predecessors.target_blueprint.as_deref(), Some("part"));
}

#[test]
fn rejects_invalid_structural_constraints() {
    let cases = [
        (
            STRUCTURAL_BLUEPRINT.replace(r#"attributes = ["slug"]"#, r#"attributes = ["missing"]"#),
            "unknown attribute 'missing'",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"attributes = ["slug"]"#,
                r#"attributes = ["slug", "slug"]"#,
            ),
            "listed twice",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(r#"attributes = ["slug"]"#, r#"attributes = ["subject"]"#),
            "cardinality = \"one\"",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(r#"scope = "context""#, r#"scope = "channel""#),
            "unsupported scope",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"code = "slug"
attributes"#,
                r#"code = "manufacturer_part"
attributes"#,
            ),
            "duplicated",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"target_blueprints = ["product", "material", "part"]"#,
                r#"target_blueprints = ["product", "product"]"#,
            ),
            "listed more than once",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"target_blueprints = ["product", "material", "part"]"#,
                r#"target_blueprints = ["product"]
target_blueprint = "part""#,
            ),
            "not both",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "tree = true\ncontext_editable = \"default\"",
                "tree = true\ncardinality = \"many\"\ncontext_editable = \"default\"",
            ),
            "at most one target",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "acyclic = true\ncontext_editable = \"default\"",
                "acyclic = true",
            ),
            "context_editable",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"target_blueprints = ["part"]
acyclic = true"#,
                r#"target_blueprints = ["product"]
acyclic = true"#,
            ),
            "own blueprint 'part'",
        ),
    ];
    for (source, expected) in cases {
        let error = compile_source(&source).unwrap_err().to_string();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    let scalar_hierarchy = r#"
format_version = 1
code = "note"
name = "Note"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
acyclic = true
"#;
    assert!(matches!(
        compile_source(scalar_hierarchy),
        Err(BlueprintError::InvalidAttributeDeclaration(_))
    ));

    let mixin_key = r#"
format_version = 1
code = "identity"
name = "Identity"
kind = "mixin"

[[unique_keys]]
code = "sku"
attributes = ["sku"]

[[attributes]]
code = "sku"
value_type = "string"
"#;
    assert!(
        compile_source(mixin_key)
            .unwrap_err()
            .to_string()
            .contains("only record blueprints")
    );
}
