use attricat_blueprint::{BlueprintError, compile, parse};

/// An order line whose price defaults from its item.
fn order_line(price: &str) -> String {
    format!(
        r#"
format_version: 1
code: order_line
name: Order line
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [note]
attributes:
  - code: item
    value_type: relationship
    target_blueprints: [product, service]
    cardinality: one
  - code: lines
    value_type: relationship
    target_blueprint: product
  - code: note
    value_type: string
{price}
"#
    )
}

fn compile_line(price: &str) -> Result<attricat_blueprint::CompiledBlueprint, BlueprintError> {
    let source = order_line(price);
    compile(parse(&source)?, &[], &source)
}

fn rejection(price: &str) -> String {
    match compile_line(price) {
        Err(BlueprintError::InvalidDefaultFrom { message, .. }) => message,
        other => panic!("expected an invalid default_from, got {other:?}"),
    }
}

#[test]
fn compiles_a_path_through_a_single_valued_multi_target_relationship() {
    let compiled = compile_line(
        "  - code: unit_price_net\n    value_type: number\n    default_from: item.sale_price_net",
    )
    .unwrap();
    let price = compiled
        .attributes
        .iter()
        .find(|attribute| attribute.code == "unit_price_net")
        .unwrap();
    assert_eq!(price.default_from.as_deref(), Some("item.sale_price_net"));
    // Longer paths are resolved against the published targets on save.
    assert!(
        compile_line(
            "  - code: vat\n    value_type: number\n    default_from: item.tax_class.rate"
        )
        .is_ok()
    );
}

#[test]
fn rejects_malformed_paths() {
    for path in ["item", "item..rate", "item.a.b.c.d", "item.sale-price!"] {
        let message = rejection(&format!(
            "  - code: unit_price_net\n    value_type: number\n    default_from: '{path}'"
        ));
        assert!(
            message.contains("relationships and an attribute"),
            "{path}: {message}"
        );
    }
}

#[test]
fn the_first_hop_must_be_a_single_valued_relationship_of_the_blueprint() {
    let missing = rejection(
        "  - code: unit_price_net\n    value_type: number\n    default_from: product.sale_price_net",
    );
    assert!(
        missing.contains("'product' is not an attribute"),
        "{missing}"
    );
    let scalar = rejection(
        "  - code: unit_price_net\n    value_type: number\n    default_from: note.sale_price_net",
    );
    assert!(scalar.contains("must be a relationship"), "{scalar}");
    let many = rejection(
        "  - code: unit_price_net\n    value_type: number\n    default_from: lines.sale_price_net",
    );
    assert!(many.contains("cardinality: one"), "{many}");
}

#[test]
fn rejects_non_scalar_and_conflicting_attributes() {
    let cases = [
        (
            "  - code: copy\n    value_type: relationship\n    cardinality: one\n    default_from: item.supplier",
            "only scalar attributes",
        ),
        (
            "  - code: copy\n    value_type: file\n    default_from: item.image",
            "only scalar attributes",
        ),
        (
            "  - code: copy\n    value_type: number\n    default_value: 0\n    default_from: item.sale_price_net",
            "default_value",
        ),
        (
            "  - code: copy\n    value_type: integer\n    generated: {}\n    default_from: item.number",
            "generated value or balance",
        ),
        (
            "  - code: copy\n    value_type: string\n    value_schema: '{\"type\":\"string\",\"enum\":[\"open\"],\"x-attricat-status\":{\"version\":1,\"options\":[{\"code\":\"open\",\"label\":\"Open\"}],\"transitions\":[{\"from\":null,\"to\":\"open\"}]}}'\n    default_from: item.state",
            "status or an assignment",
        ),
    ];
    for (attribute, expected) in cases {
        let message = rejection(attribute);
        assert!(message.contains(expected), "{attribute}: {message}");
    }
}

#[test]
fn mixins_and_selections_cannot_default_from_relationships() {
    let mixin = r#"
format_version: 1
code: pricing
name: Pricing
kind: mixin
attributes:
  - code: unit_price_net
    value_type: number
    default_from: item.sale_price_net
"#;
    assert!(matches!(
        parse(mixin),
        Err(BlueprintError::InvalidDefaultFrom { .. })
    ));
    let selection = r#"
format_version: 1
code: order_line
name: Order line
kind: record
includes:
  - alias: pricing
    code: pricing
    version: 1
attributes:
  - code: unit_price_net
    from: pricing.unit_price_net
    default_from: item.sale_price_net
"#;
    assert!(matches!(
        parse(selection),
        Err(BlueprintError::InvalidAttributeDeclaration(_))
    ));
}
