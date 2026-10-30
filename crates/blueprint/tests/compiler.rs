use attricat_blueprint::{
    BlueprintError, BrowseBy, EffectiveAttribute, ResolvedInclude, ViewDefinition, compile, parse,
    raw_hash,
};
use attricat_domain::attribute_options::Cardinality;
use attricat_domain::attribute_options::ContextEditable;
use attricat_domain::attribute_options::ContextFallback;
use attricat_domain::value_type::ValueType;

#[test]
fn compiles_only_explicitly_selected_mixin_attributes_in_local_order() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
record_schema: '{"type":"object","required":["meta_title"]}'
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
includes:
  - alias: seo
    code: seo
    version: 2
attributes:
  - code: title
    value_type: string
    tags: [display]
  - code: meta_title
    from: seo.meta_title
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
                    value_type: ValueType::String,
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
                    context_fallback: ContextFallback::Default,
                    context_editable: ContextEditable::All,
                    readonly: false,
                    generated: None,
                    position: 0,
                },
                EffectiveAttribute {
                    code: "meta_description".to_owned(),
                    name: None,
                    value_type: ValueType::String,
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
                    context_fallback: ContextFallback::Default,
                    context_editable: ContextEditable::All,
                    readonly: false,
                    generated: None,
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
    assert_eq!(
        compiled.attributes[0].context_fallback,
        ContextFallback::Default
    );
    assert_eq!(
        compiled.attributes[0].context_editable,
        ContextEditable::All
    );
    assert_eq!(compiled.attributes[0].position, 0);
    assert_eq!(compiled.attributes[1].code, "meta_title");
    assert_eq!(compiled.attributes[1].name.as_deref(), Some("SEO title"));
    assert_eq!(compiled.attributes[1].position, 1);
    assert_eq!(compiled.raw_definition_hash, raw_hash(source));
}

#[test]
fn compiles_optional_attribute_names() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [product_family]
attributes:
  - code: product_family
    name: Family
    value_type: string
  - code: rating
    name: Rating
    extension_type: 'acme:stars@^1'
  - code: sku
    value_type: string
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
format_version: 1
code: product
name: Product
kind: record
attributes:
  - code: title
    name: '  '
    value_type: string
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::EmptyField("attribute name"))
    ));
}

#[test]
fn rejects_names_on_selected_attributes() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
includes:
  - alias: seo
    code: seo
    version: 1
attributes:
  - code: meta_title
    name: Title
    from: seo.meta_title
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::InvalidAttributeDeclaration(code)) if code == "meta_title"
    ));
}

#[test]
fn parses_blueprint_and_attribute_descriptions() {
    let source = r#"
format_version: 1
code: category
name: Category
description: 'Product groupings, such as Basic tools'
kind: record
attributes:
  - code: title
    description: The category name shown to shoppers
    value_type: string
"#;
    let definition = parse(source).unwrap();
    assert_eq!(
        definition.description.as_deref(),
        Some("Product groupings, such as Basic tools")
    );
    let attricat_blueprint::AttributeDeclaration::Local(title) = &definition.attributes[0] else {
        panic!("title is a local attribute");
    };
    assert_eq!(
        title.description.as_deref(),
        Some("The category name shown to shoppers")
    );
    assert!(
        attricat_blueprint::lexicon_texts(&definition)
            .iter()
            .any(|text| text.location == "attribute 'title' description")
    );
}

#[test]
fn rejects_blank_long_and_selected_descriptions() {
    let with = |description: &str| {
        format!(
            "format_version: 1\ncode: product\nname: Product\nkind: record\nattributes:\n  - code: title\n    description: '{description}'\n    value_type: string\n"
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
format_version: 1
code: product
name: Product
kind: record
includes:
  - alias: seo
    code: seo
    version: 1
attributes:
  - code: meta_title
    description: Search title
    from: seo.meta_title
"#;
    assert!(matches!(
        parse(selected),
        Err(BlueprintError::InvalidAttributeDeclaration(code)) if code == "meta_title"
    ));
}

#[test]
fn compiles_scalar_attribute_default_values() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
    default_value: Untitled
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
format_version: 1
code: product
name: Product
kind: record
attributes:
  - code: related
    value_type: relationship
    default_value: '00000000-0000-0000-0000-000000000000'
"#;
    assert!(matches!(
        parse(source),
        Err(BlueprintError::DefaultValueOnNonScalarAttribute { .. })
    ));
}

#[test]
fn supports_context_fallback_on_all_attribute_types() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
    context_fallback: none
  - code: related
    value_type: relationship
    context_fallback: none
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(
        compiled
            .attributes
            .iter()
            .all(|attribute| attribute.context_fallback == ContextFallback::None)
    );
}

#[test]
fn supports_readonly_attributes() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: stock
    value_type: integer
    readonly: true
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(!compiled.attributes[0].readonly);
    assert!(compiled.attributes[1].readonly);
}

#[test]
fn supports_default_only_context_editing() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: stock
    value_type: integer
    context_editable: default
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.attributes[0].context_editable,
        ContextEditable::All
    );
    assert_eq!(
        compiled.attributes[1].context_editable,
        ContextEditable::Default
    );
}

#[test]
fn rejects_unknown_fields_and_invalid_selections() {
    let unknown_field = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
    unexpected: true
attributes:
  - code: title
    value_type: string
"#;
    assert!(parse(unknown_field).is_err());

    let invalid_selection = r#"
format_version: 1
code: product
name: Product
kind: record
attributes:
  - code: title
    from: seo.meta_title
"#;
    assert!(parse(invalid_selection).is_err());
}

#[test]
fn validates_reference_codes_with_the_shared_character_set() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
"#;
    for replacement in ["product-name", "product_name"] {
        assert!(parse(&source.replacen("product", replacement, 1)).is_ok());
    }
    for replacement in ["product name", "product.name", "produit-été", ""] {
        assert!(parse(&source.replacen("product", replacement, 1)).is_err());
    }
    for replacement in ["page-title", "page_title"] {
        assert!(parse(&source.replace("code: title", &format!("code: '{replacement}'"))).is_ok());
    }
    for replacement in ["page title", "page.title", "page.titlé", ""] {
        assert!(parse(&source.replace("code: title", &format!("code: '{replacement}'"))).is_err());
    }
}

#[test]
fn validates_include_aliases_with_the_shared_character_set() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
includes:
  - alias: seo-metadata
    code: seo
    version: 1
attributes:
  - code: meta_title
    from: seo-metadata.meta_title
"#;
    assert!(parse(source).is_ok());
    for alias in ["seo metadata", "seo.metadata", "seo-métadata", ""] {
        assert!(parse(&source.replace("seo-metadata", alias)).is_err());
    }
}

#[test]
fn compiles_relationship_target_blueprint() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
attributes:
  - code: categories
    value_type: relationship
    target_blueprint: category
  - code: name
    value_type: string
    tags: [searchable]
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.attributes[0].target_blueprint.as_deref(),
        Some("category")
    );

    let invalid = source.replace("value_type: relationship", "value_type: string");
    assert!(parse(&invalid).is_err());
}

#[test]
fn requires_valid_dropdown_option_view_and_preserves_generic_tags() {
    let missing_display = r#"
format_version: 1
code: product
name: Product
kind: record
attributes:
  - code: title
    value_type: string
"#;
    assert!(compile(parse(missing_display).unwrap(), &[], missing_display).is_err());

    let source = missing_display.replace(
        "value_type: string",
        "value_type: string\n    tags: [searchable]\nviews:\n  dropdown_option:\n    type: dropdown_option\n    fields: [title]",
    );
    let compiled = compile(parse(&source).unwrap(), &[], &source).unwrap();
    assert_eq!(compiled.attributes[0].tags, ["searchable"]);
    assert!(matches!(
        compiled.views["dropdown_option"],
        ViewDefinition::DropdownOption { ref separator, .. } if separator == " · "
    ));

    let relationship_display = source.replace("fields: [title]", "fields: [unknown]");
    assert!(
        compile(
            parse(&relationship_display).unwrap(),
            &[],
            &relationship_display
        )
        .is_err()
    );

    let empty_fields = source.replace("fields: [title]", "fields: []");
    assert!(compile(parse(&empty_fields).unwrap(), &[], &empty_fields).is_err());

    let duplicate_fields = source.replace("fields: [title]", "fields: [title, title]");
    assert!(compile(parse(&duplicate_fields).unwrap(), &[], &duplicate_fields).is_err());
}

#[test]
fn compiles_optional_recursive_views_with_component_references() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    children:
      - {type: heading, text: Product}
      - type: section
        children:
          - type: grid
            children:
              - type: field
                field: title
                component: {id: attricat.field_display, version: 1}
          - type: tabs
            tabs:
              - label: Details
                children:
                  - {type: text, text: Information}
          - type: accordion
            sections:
              - label: Related
                children:
                  - {type: relationship_list, field: categories}
          - {type: divider}
  index:
    type: table
    columns:
      - {field: title}
attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
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
format_version: 1
code: category
name: Category
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
  detail:
    type: stack
    children:
      - type: incoming_relationship_list
        label: Products in this category
        page_size: 10
        relationships:
          - {source_blueprint: product, field: categories}
        component:
          id: attricat.incoming_relationship_list_display
          version: 1
attributes:
  - code: name
    value_type: string
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("page_size: 10", "page_size: 0"),
        source.replace("label: Products in this category", "label: ' '"),
        source.replace("  detail:\n", "  edit:\n"),
        source.replace(
            "relationships:\n          - {source_blueprint: product, field: categories}",
            "relationships: []",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn rejects_invalid_view_fields_and_component_references() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  index:
    type: table
    columns:
      - {field: categories}
attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_err());

    let invalid_component_id = source.replace(
        "columns: [{field: categories}]",
        "columns:\n  - field: title\ncomponent:\n  id: attricat.bad-id\n  version: 1",
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
        "columns: [{field: categories}]",
        "columns:\n  - field: title\ncomponent:\n  id: attricat.table\n  version: 0",
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
        "type: table\ncolumns:\n  - field: categories",
        "type: stack\nchildren:\n  - type: relationship_list\n    field: title",
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
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    children:
      - type: field
        field: title
        component: {id: attricat.field_display, version: 1}
attributes:
  - code: title
    value_type: string
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("  detail:\n", "  edit:\n"),
        source.replace("attricat.field_display", "attricat.table_display"),
        source.replace("version: 1}", "version: 2}"),
        source.replace(
            "version: 1}",
            "version: 1, props: {label: Title}}",
        ),
        source.replace(
            "    children:\n      - type: field\n        field: title\n        component: {id: attricat.field_display, version: 1}",
            "    component: {id: attricat.field_display, version: 1}\n    children: []",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_phone_components() {
    let source = r#"
format_version: 1
code: contact
name: Contact
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [phone]
  detail:
    type: stack
    children:
      - type: field
        field: phone
        component: {id: attricat.phone_display, version: 1}
  table:
    type: table
    columns:
      - field: phone
        renderer: {id: attricat.phone_display, version: 1}
attributes:
  - code: phone
    value_type: string
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type: string", "value_type: integer"),
        source.replace("attricat.phone_display", "attricat.phone_edit"),
        source.replace("version: 1}", "version: 2}"),
        source.replace("version: 1}", "version: 1, props: {region: PL}}"),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_url_components() {
    let source = r#"
format_version: 1
code: website
name: Website
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [url]
  detail:
    type: stack
    children:
      - type: field
        field: url
        component: {id: attricat.url_display, version: 1}
  table:
    type: table
    columns:
      - field: url
        renderer: {id: attricat.url_display, version: 1}
attributes:
  - code: url
    value_type: string
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type: string", "value_type: number"),
        source.replace("attricat.url_display", "attricat.url_edit"),
        source.replace("version: 1}", "version: 2}"),
        source.replace("version: 1}", "version: 1, props: {unsafe: true}}"),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_color_components() {
    let source = r#"
format_version: 1
code: color
name: Color
kind: record
attributes:
  - code: hex
    value_type: string
views:
  dropdown_option:
    type: dropdown_option
    fields: [hex]
  detail:
    type: stack
    children:
      - type: field
        field: hex
        component: {id: attricat.color_display, version: 1}
  table:
    type: table
    columns:
      - field: hex
        renderer: {id: attricat.color_display, version: 1}
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
    for invalid in [
        source.replace("value_type: string", "value_type: number"),
        source.replace("attricat.color_display", "attricat.color_edit"),
        source.replace("version: 1}", "version: 2}"),
        source.replace("version: 1}", "version: 1, props: {alpha: true}}"),
        source.replace("type: field", "type: relationship_list"),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn compiles_json_schema_contracts() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
record_schema: '{"type":"object","required":["price"]}'
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: price
    value_type: number
    value_schema: '{"type":"number","minimum":0}'
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
format_version: 1
code: product
name: Product
kind: record
record_schema: '__SCHEMA__'
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
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
format_version: 1
code: schedule
name: Schedule
kind: record
record_schema: '{"type":"object","properties":{"cutoff":{"type":"object","required":["time"],"properties":{"time":{"type":"string"}}}}}'
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: cutoff
    value_type: time
"#;

    assert!(compile(parse(source).unwrap(), &[], source).is_ok());
}

#[test]
fn rejects_invalid_json_schema_contracts() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
"#;
    for invalid in [
        source.replace(
            "value_type: string",
            "value_type: string\nvalue_schema: not json",
        ),
        source.replace(
            "kind: record",
            "kind: mixin\nrecord_schema: '{\"type\":\"object\"}'",
        ),
        source.replace(
            "value_type: string",
            "value_type: relationship\nvalue_schema: '{\"type\":\"array\"}'",
        ),
    ] {
        assert!(parse(&invalid).is_err());
    }
}

#[test]
fn validates_record_heading_stack_component() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    component: {id: attricat.record_heading, version: 1}
    children:
      - {type: field, field: title}
      - {type: text, text: SKU}
      - {type: field, field: sku}
attributes:
  - code: title
    value_type: string
  - code: sku
    value_type: string
  - code: category
    value_type: relationship
"#;
    assert!(compile(parse(source).unwrap(), &[], source).is_ok());

    for invalid in [
        source.replace("  detail:\n", "  edit:\n"),
        source.replace(
            "component: {id: attricat.record_heading, version: 1}",
            "component: {id: attricat.field_display, version: 1}",
        ),
        source.replace("{type: field, field: title}", "{type: text, text: Product}"),
        source.replace("field: title", "field: category"),
        source.replace(
            "{type: text, text: SKU}",
            "{type: relationship_list, field: category}",
        ),
        source.replace("{type: text, text: SKU}", "{type: grid, children: []}"),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn validates_self_referential_hierarchy_component() {
    let source = r#"
format_version: 1
code: category
name: Category
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    children:
      - type: relationship_list
        field: parent
        component: {id: attricat.relationship_hierarchy, version: 1}
attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: category
"#;
    compile(parse(source).unwrap(), &[], source).unwrap();

    let invalid = source.replace("target_blueprint: category", "target_blueprint: product");
    assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());

    let blank_parent_field =
        invalid.replace("version: 1}", "version: 1, props: {parent_field: ''}}");
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
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    columns: []
attributes:
  - code: title
    value_type: string
"#;
    assert!(matches!(
        compile(parse(source).unwrap(), &[], source),
        Err(BlueprintError::EmptyTableColumns)
    ));
}

#[test]
fn extension_layout_is_record_only_and_rejects_ambiguous_keys() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  extension_layout:
    type: extension_layout
    version: 1
    outlets:
      record_preview_panel:
        order: ['acme.inventory:summary']
        hidden: ['acme.legacy:panel']
attributes:
  - code: title
    value_type: string
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert!(matches!(
        compiled.views.get("extension_layout"),
        Some(ViewDefinition::ExtensionLayout { version: 1, .. })
    ));

    for invalid in [
        source.replace("kind: record", "kind: mixin"),
        source.replace("    version: 1\n    outlets:", "    version: 2\n    outlets:"),
        source.replace("record_preview_panel", "navigation"),
        source.replace("acme.inventory:summary", "malformed"),
        source.replace(
            "hidden: ['acme.legacy:panel']",
            "hidden: ['acme.inventory:summary']",
        ),
        source.replace(
            "    outlets:\n",
            "    outlets:\n      record_action:\n        order: ['acme.inventory:summary']\n        hidden: []\n",
        ),
    ] {
        assert!(compile(parse(&invalid).unwrap(), &[], &invalid).is_err());
    }
}

#[test]
fn explorer_view_browse_by_requires_a_single_target_relationship_or_path() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  explorer:
    type: explorer
    browse_by: categories
attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
    target_blueprint: category
  - code: related
    value_type: relationship
    target_blueprints: [category, brand]
  - code: model
    value_type: relationship
    target_blueprint: model
    cardinality: one
  - code: tags
    value_type: relationship
    target_blueprint: tag
    cardinality: many
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.views.get("explorer"),
        Some(&ViewDefinition::Explorer {
            browse_by: Some(BrowseBy::Field("categories".to_owned()))
        })
    );
    let hidden = source.replace("browse_by: categories", "browse_by: false");
    assert_eq!(
        compile(parse(&hidden).unwrap(), &[], &hidden)
            .unwrap()
            .views
            .get("explorer"),
        Some(&ViewDefinition::Explorer {
            browse_by: Some(BrowseBy::Hidden(false))
        })
    );

    // Later hops resolve against published targets when the blueprint is saved.
    for path in ["model.brand", "model.brand.group", "tags"] {
        let valid = source.replace("browse_by: categories", &format!("browse_by: '{path}'"));
        assert_eq!(
            compile(parse(&valid).unwrap(), &[], &valid)
                .unwrap()
                .views
                .get("explorer"),
            Some(&ViewDefinition::Explorer {
                browse_by: Some(BrowseBy::Field(path.to_owned()))
            })
        );
    }

    for (invalid, error) in [
        (
            source.replace("browse_by: categories", "browse_by: title"),
            "title",
        ),
        // A path's hops must hold one value each.
        (
            source.replace("browse_by: categories", "browse_by: tags.parent"),
            "tags.parent",
        ),
        (
            source.replace("browse_by: categories", "browse_by: title.brand"),
            "title.brand",
        ),
        (
            source.replace(
                "browse_by: categories",
                "browse_by: model.brand.group.parent",
            ),
            "model.brand.group.parent",
        ),
        (
            source.replace("browse_by: categories", "browse_by: model..brand"),
            "model..brand",
        ),
        (
            source.replace("browse_by: categories", "browse_by: related"),
            "related",
        ),
        (
            source.replace("browse_by: categories", "browse_by: missing"),
            "missing",
        ),
        (
            source.replace("browse_by: categories", "browse_by: true"),
            "true",
        ),
    ] {
        assert!(matches!(
            compile(parse(&invalid).unwrap(), &[], &invalid),
            Err(BlueprintError::InvalidBrowseByField(field)) if field == error
        ));
    }
    let renamed = source.replace("  explorer:\n", "  browse:\n");
    assert!(matches!(
        compile(parse(&renamed).unwrap(), &[], &renamed),
        Err(BlueprintError::InvalidExplorerView)
    ));
}

#[test]
fn page_title_view_joins_scalar_fields_on_record_blueprints() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title, sku]
  page_title:
    type: page_title
    fields: [title]
attributes:
  - code: title
    value_type: string
  - code: sku
    value_type: string
  - code: category
    value_type: relationship
    target_blueprint: category
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(
        compiled.views.get("page_title"),
        Some(&ViewDefinition::PageTitle {
            fields: vec!["title".to_owned()],
            separator: " · ".to_owned(),
        })
    );

    let empty = source.replace("fields: [title]", "fields: []");
    assert!(matches!(
        compile(parse(&empty).unwrap(), &[], &empty),
        Err(BlueprintError::EmptyPageTitleFields)
    ));
    let duplicate = source.replace("fields: [title]", "fields: [title, title]");
    assert!(matches!(
        compile(parse(&duplicate).unwrap(), &[], &duplicate),
        Err(BlueprintError::DuplicatePageTitleField(field)) if field == "title"
    ));
    let relationship = source.replace("fields: [title]", "fields: [category]");
    assert!(matches!(
        compile(parse(&relationship).unwrap(), &[], &relationship),
        Err(BlueprintError::NonScalarViewField { field, .. }) if field == "category"
    ));
    let renamed = source.replace("  page_title:\n", "  heading:\n");
    assert!(matches!(
        compile(parse(&renamed).unwrap(), &[], &renamed),
        Err(BlueprintError::InvalidPageTitleView)
    ));
    let mixin = source.replace("kind: record", "kind: mixin").replace(
        "  dropdown_option:\n    type: dropdown_option\n    fields: [title, sku]\n",
        "",
    );
    assert!(matches!(
        compile(parse(&mixin).unwrap(), &[], &mixin),
        Err(BlueprintError::InvalidPageTitleView)
    ));
}

#[test]
fn table_columns_resolve_relationship_paths_and_reject_invalid_ones() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  table:
    type: table
    columns:
      - field: title
        label: Title
        renderer: {id: attricat.table_display, version: 1}
      - {field: category.name}
attributes:
  - code: title
    value_type: string
  - code: category
    value_type: relationship
    target_blueprint: category
    cardinality: one
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.attributes[1].cardinality, Some(Cardinality::One));
    assert_eq!(
        compiled.attributes[1].target_cardinality,
        Some(Cardinality::Many)
    );
    let legacy = compile(
        parse(&source.replace("cardinality: one", "cardinality: one_to_one")).unwrap(),
        &[],
        source,
    )
    .unwrap();
    assert_eq!(legacy.attributes[1].cardinality, Some(Cardinality::One));
    assert_eq!(
        legacy.attributes[1].target_cardinality,
        Some(Cardinality::One)
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
    // The former `fields` shorthand is no longer part of the table view.
    assert!(
        parse(&source.replace("    columns:\n", "    fields: [title]\n    columns:\n")).is_err()
    );
    assert!(parse(&source.replace("cardinality: one", "cardinality: invalid")).is_err());
    // Modern column tables must validate their table-level component too.
    let invalid_table_component = source.replace(
        "    columns:\n",
        "    component: {id: attricat.table_edit, version: 1}\n    columns:\n",
    );
    assert!(
        compile(
            parse(&invalid_table_component).unwrap(),
            &[],
            &invalid_table_component
        )
        .is_err()
    );
    assert!(parse(&source.replace("version: 1", "version: -1")).is_err());
}

#[test]
fn parses_extension_type_reference_and_json_primitive() {
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [price]
attributes:
  - code: price
    extension_type: 'com.acme.commerce:money@^1'
    extension_configuration: '{"currency":"USD"}'
  - code: metadata
    value_type: json
"#;
    let compiled = compile(parse(source).unwrap(), &[], source).unwrap();
    assert_eq!(compiled.attributes[0].value_type, ValueType::String);
    assert_eq!(
        compiled.attributes[0].extension_type.as_ref().unwrap()["reference"],
        "com.acme.commerce:money@^1"
    );
    assert_eq!(compiled.attributes[1].value_type, ValueType::Json);
}

#[test]
fn edit_views_are_rejected() {
    // Records are edited in place on the detail view.
    let source = r#"
format_version: 1
code: product
name: Product
kind: record
record_schema: '{"type":"object","required":["title","price"]}'
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  edit:
    type: stack
    children:
      - {type: field, field: title}
attributes:
  - code: title
    value_type: string
  - code: price
    value_type: number
"#;
    assert!(matches!(
        compile(parse(source).unwrap(), &[], source),
        Err(BlueprintError::EditViewRemoved)
    ));
}

fn compile_source(source: &str) -> Result<attricat_blueprint::CompiledBlueprint, BlueprintError> {
    compile(parse(source)?, &[], source)
}

const STRUCTURAL_BLUEPRINT: &str = r#"
format_version: 1
code: part
name: Part
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [part_number]
unique_keys:
  - code: manufacturer_part
    attributes: [manufacturer, part_number]
  - code: slug
    attributes: [slug]
    scope: context
    case_sensitive: true
attributes:
  - code: part_number
    value_type: string
  - code: slug
    value_type: string
  - code: manufacturer
    value_type: relationship
    target_blueprint: manufacturer
    cardinality: one
  - code: subject
    value_type: relationship
    target_blueprints: [product, material, part]
  - code: parent
    value_type: relationship
    target_blueprint: part
    tree: true
    context_editable: default
  - code: predecessors
    value_type: relationship
    target_blueprints: [part]
    acyclic: true
    context_editable: default
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
    assert_eq!(parent.cardinality, Some(Cardinality::One));
    let predecessors = attribute("predecessors");
    assert_eq!(predecessors.hierarchy.as_deref(), Some("acyclic"));
    assert_eq!(predecessors.cardinality, Some(Cardinality::Many));
    // A one-element list is the single-target form.
    assert_eq!(predecessors.target_blueprint.as_deref(), Some("part"));
}

#[test]
fn rejects_invalid_structural_constraints() {
    let cases = [
        (
            STRUCTURAL_BLUEPRINT.replace(r#"attributes: [slug]"#, r#"attributes: [missing]"#),
            "unknown attribute 'missing'",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(r#"attributes: [slug]"#, r#"attributes: [slug, slug]"#),
            "listed twice",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(r#"attributes: [slug]"#, r#"attributes: [subject]"#),
            "cardinality: one",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(r#"scope: context"#, r#"scope: channel"#),
            "unsupported scope",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "  - code: slug\n    attributes: [slug]",
                "  - code: manufacturer_part\n    attributes: [slug]",
            ),
            "duplicated",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"target_blueprints: [product, material, part]"#,
                r#"target_blueprints: [product, product]"#,
            ),
            "listed more than once",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                r#"target_blueprints: [product, material, part]"#,
                "target_blueprints: [product]\n    target_blueprint: part",
            ),
            "not both",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "tree: true\n    context_editable: default",
                "tree: true\n    cardinality: many\n    context_editable: default",
            ),
            "at most one target",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "acyclic: true\n    context_editable: default",
                "acyclic: true",
            ),
            "context_editable",
        ),
        (
            STRUCTURAL_BLUEPRINT.replace(
                "target_blueprints: [part]\n    acyclic: true",
                "target_blueprints: [product]\n    acyclic: true",
            ),
            "own blueprint 'part'",
        ),
    ];
    for (source, expected) in cases {
        let error = compile_source(&source).unwrap_err().to_string();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    let scalar_hierarchy = r#"
format_version: 1
code: note
name: Note
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
    acyclic: true
"#;
    assert!(matches!(
        compile_source(scalar_hierarchy),
        Err(BlueprintError::InvalidAttributeDeclaration(_))
    ));

    let mixin_key = r#"
format_version: 1
code: identity
name: Identity
kind: mixin
unique_keys:
  - code: sku
    attributes: [sku]
attributes:
  - code: sku
    value_type: string
"#;
    assert!(
        compile_source(mixin_key)
            .unwrap_err()
            .to_string()
            .contains("only record blueprints")
    );
}
