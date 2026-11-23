use attricat_blueprint::{BalancePolicy, BlueprintError, TargetScope, compile, parse};
use attricat_domain::attribute_options::{ContextEditable, MovementDirection};
use rust_decimal::Decimal;

fn record(code: &str, body: &str) -> String {
    format!(
        r#"
format_version: 1
code: {code}
name: {code}
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [number]
{body}"#
    )
}

const STOCK_ITEM: &str = r#"unique_keys:
  - code: product
    attributes: [product]
attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0}
"#;

/// A goods issue whose attribute list ends with `extra` and whose movement
/// is `movement`, both written at top-level indentation.
fn goods_issue(movement: &str, extra: &str) -> String {
    record(
        "goods_issue",
        &format!(
            r#"attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    context_editable: default
    target_scope: {{attribute: product, equals: product}}
  - code: stock_item
    value_type: relationship
    target_blueprint: stock_item
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: goods_issue
    cardinality: one
    context_editable: default
{extra}movements:
{movement}"#
        ),
    )
}

const ISSUE: &str = r#"  - code: stock
    direction: out
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: stock_item
      balance: quantity
      key: product
      values: {product: product}
    reversal: {relationship: reverses}
"#;

fn compiled(source: &str) -> Result<attricat_blueprint::CompiledBlueprint, BlueprintError> {
    compile(parse(source)?, &[], source)
}

fn movement_error(source: &str) -> String {
    match compiled(source) {
        Err(BlueprintError::InvalidMovement { movement, message }) => {
            assert_eq!(movement, "stock");
            message
        }
        other => panic!("expected an invalid movement, got {other:?}"),
    }
}

#[test]
fn balances_compile_read_only_in_the_default_context() {
    let source = record("stock_item", STOCK_ITEM);
    let compiled = compiled(&source).unwrap();
    let quantity = &compiled.attributes[2];
    assert!(quantity.readonly);
    assert_eq!(quantity.context_editable, ContextEditable::Default);
    assert_eq!(
        quantity.balance,
        Some(BalancePolicy {
            min: Some(Decimal::ZERO)
        })
    );
    assert_eq!(
        serde_json::to_value(quantity.balance.as_ref().unwrap()).unwrap(),
        serde_json::json!({"min": "0"})
    );
    let unbounded = record(
        "stock_item",
        "attributes:\n  - code: number\n    value_type: string\n  - code: level\n    value_type: integer\n    balance: {}\n",
    );
    assert_eq!(
        compiled_attribute_balance(&unbounded, "level"),
        Some(BalancePolicy { min: None })
    );
}

fn compiled_attribute_balance(source: &str, code: &str) -> Option<BalancePolicy> {
    compiled(source)
        .unwrap()
        .attributes
        .into_iter()
        .find(|attribute| attribute.code == code)
        .unwrap()
        .balance
}

#[test]
fn balances_reject_conflicting_options() {
    for (attribute, expected) in [
        (
            "value_type: string\n    balance: {}",
            "only number and integer",
        ),
        (
            "value_type: number\n    default_value: 3\n    balance: {}",
            "default_value",
        ),
        (
            "value_type: integer\n    balance: {min: 0.5}",
            "whole-number",
        ),
        (
            "value_type: integer\n    generated: {}\n    balance: {}",
            "cannot also be generated",
        ),
    ] {
        let source = record(
            "stock_item",
            &format!(
                "attributes:\n  - code: number\n    value_type: string\n  - code: level\n    {attribute}\n"
            ),
        );
        match parse(&source) {
            Err(BlueprintError::InvalidBalanceAttribute { code, message }) => {
                assert_eq!(code, "level");
                assert!(message.contains(expected), "{message} lacks {expected}");
            }
            other => panic!("expected an invalid balance for {attribute}, got {other:?}"),
        }
    }
    let mixin = r#"
format_version: 1
code: stock_fields
name: Stock fields
kind: mixin
attributes:
  - code: level
    value_type: number
    balance: {}
"#;
    assert!(matches!(
        parse(mixin),
        Err(BlueprintError::InvalidBalanceAttribute { .. })
    ));
}

#[test]
fn movements_compile_and_protect_the_attributes_core_writes() {
    let compiled = compiled(&goods_issue(ISSUE, "")).unwrap();
    let movement = &compiled.movements[0];
    assert_eq!(movement.direction, MovementDirection::Out);
    assert!(!movement.target.create_missing);
    assert_eq!(movement.target.values["product"], "product");
    for code in ["stock_item", "base_quantity"] {
        let attribute = compiled
            .attributes
            .iter()
            .find(|attribute| attribute.code == code)
            .unwrap();
        assert!(attribute.readonly, "{code} is read-only");
        assert_eq!(attribute.context_editable, ContextEditable::Default);
    }
    let quantity = &compiled.attributes[2];
    assert!(!quantity.readonly);
    assert_eq!(
        compiled.attributes[3].target_scope,
        Some(TargetScope {
            attribute: "product".into(),
            equals: "product".into(),
        })
    );
}

#[test]
fn movements_reject_invalid_attribute_roles() {
    for (from, to, expected) in [
        (
            "quantity: quantity",
            "quantity: number",
            "must be a number or integer",
        ),
        (
            "quantity: quantity",
            "quantity: missing",
            "unknown attribute 'missing'",
        ),
        (
            "relationship: stock_item",
            "relationship: other",
            "must target exactly blueprint 'stock_item'",
        ),
        (
            "result: base_quantity",
            "result: quantity",
            "written by core and cannot be used twice",
        ),
        (
            "reversal: {relationship: reverses}",
            "reversal: {relationship: product}",
            "must target exactly this blueprint",
        ),
        (
            "factor: {relationship: conversion, attribute: factor}",
            "factor: {relationship: stock_item, attribute: factor}",
            "written by core",
        ),
    ] {
        let movement = ISSUE.replace(from, to);
        assert_ne!(movement, ISSUE, "{from} is in the movement");
        let extra = "  - code: other\n    value_type: relationship\n    target_blueprint: product\n    cardinality: one\n";
        let message = match compiled(&goods_issue(&movement, extra)) {
            Err(BlueprintError::InvalidMovement { message, .. }) => message,
            other => panic!("expected an invalid movement for {to}, got {other:?}"),
        };
        assert!(message.contains(expected), "{message} lacks {expected}");
    }
}

#[test]
fn movements_read_inputs_from_the_default_context() {
    let source = goods_issue(ISSUE, "").replacen(
        "  - code: quantity\n    value_type: number\n    context_editable: default\n",
        "  - code: quantity\n    value_type: number\n",
        1,
    );
    assert!(movement_error(&source).contains("set context_editable: default"));
}

#[test]
fn movements_belong_to_record_blueprints_and_have_unique_codes() {
    let twice = format!(
        "{ISSUE}{}",
        ISSUE.replace("    result: base_quantity\n", "")
    );
    let message = match compiled(&goods_issue(&twice, "")) {
        Err(BlueprintError::InvalidMovement { message, .. }) => message,
        other => panic!("expected an invalid movement, got {other:?}"),
    };
    assert!(message.contains("duplicated"), "{message}");

    let integer_result = goods_issue(ISSUE, "").replacen(
        "  - code: base_quantity\n    value_type: number\n",
        "  - code: base_quantity\n    value_type: integer\n",
        1,
    );
    assert!(movement_error(&integer_result).contains("plain number attribute"));

    let reversal_without_result =
        goods_issue(&ISSUE.replace("    result: base_quantity\n", ""), "");
    assert!(movement_error(&reversal_without_result).contains("must declare result"));
}

#[test]
fn target_scopes_compare_single_target_relationships() {
    let scoped = |scope: &str| {
        record(
            "goods_issue",
            &format!(
                r#"attributes:
  - code: number
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
  - code: note
    value_type: string
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    target_scope: {scope}
"#
            ),
        )
    };
    assert!(compiled(&scoped("{attribute: product, equals: product}")).is_ok());
    for scope in [
        "{attribute: product, equals: note}",
        "{attribute: product, equals: missing}",
        "{attribute: product, equals: conversion}",
    ] {
        assert!(
            matches!(
                compiled(&scoped(scope)),
                Err(BlueprintError::InvalidTargetScope { .. })
            ),
            "{scope} is rejected"
        );
    }
    let on_string = record(
        "goods_issue",
        "attributes:\n  - code: number\n    value_type: string\n    target_scope: {attribute: a, equals: b}\n",
    );
    assert!(parse(&on_string).is_err());
}
