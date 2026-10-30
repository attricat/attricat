use attricat_blueprint::{BlueprintError, GeneratedValue, compile, parse};
use attricat_domain::attribute_options::{ContextEditable, SequenceReset};

/// An invoice blueprint whose attribute list continues with `attributes`,
/// further list items written at the list's indentation.
fn record(attributes: &str) -> String {
    format!(
        r#"
format_version: 1
code: invoice
name: Invoice
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
{attributes}
"#
    )
}

fn rejected(source: &str) -> String {
    match parse(source) {
        Err(BlueprintError::InvalidGeneratedAttribute { code, message }) => {
            assert_eq!(code, "number");
            message
        }
        other => panic!("expected an invalid generated attribute, got {other:?}"),
    }
}

#[test]
fn generated_attributes_compile_read_only_in_the_default_context() {
    let source = record(
        r#"  - code: number
    value_type: string
    generated:
      format: 'INV-{yyyy}-{n:05}'
      reset: yearly
      time_zone: Europe/Warsaw
  - code: sequence
    value_type: integer
    generated: {start: 1000}
"#,
    );
    let compiled = compile(parse(&source).unwrap(), &[], &source).unwrap();
    let number = &compiled.attributes[1];
    assert!(number.readonly);
    assert_eq!(number.context_editable, ContextEditable::Default);
    assert_eq!(
        number.generated,
        Some(GeneratedValue {
            format: Some("INV-{yyyy}-{n:05}".into()),
            start: 1,
            reset: SequenceReset::Yearly,
            time_zone: "Europe/Warsaw".into(),
        })
    );
    let sequence = compiled.attributes[2].generated.as_ref().unwrap();
    assert_eq!((sequence.format.as_deref(), sequence.start), (None, 1000));
    assert!(compiled.attributes[0].generated.is_none());
}

#[test]
fn generated_attributes_reject_conflicting_options() {
    for (attribute, expected) in [
        (
            "value_type: date\ngenerated:\n  format: '{n}'",
            "only string and integer",
        ),
        ("value_type: string\ngenerated: {}", "need a format"),
        (
            "value_type: string\ndefault_value: x\ngenerated:\n  format: '{n}'",
            "default_value",
        ),
        (
            "value_type: string\nvalue_schema: '{\"type\":\"string\",\"enum\":[\"draft\"],\"x-attricat-status\":{\"version\":1,\"options\":[{\"code\":\"draft\",\"label\":\"Draft\"}],\"transitions\":[{\"from\":null,\"to\":\"draft\"}]}}'\ngenerated:\n  format: '{n}'",
            "status",
        ),
        (
            "value_type: string\ngenerated:\n  format: '{n}'\n  reset: yearly",
            "identify the period",
        ),
    ] {
        let attribute = attribute.replace('\n', "\n    ");
        let message = rejected(&record(&format!("  - code: number\n    {attribute}")));
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn mixins_and_extension_types_cannot_generate() {
    let mixin = r#"
format_version: 1
code: numbered
name: Numbered
kind: mixin
attributes:
  - code: number
    value_type: string
    generated: {format: '{n}'}
"#;
    assert!(rejected(mixin).contains("record blueprints"));
    assert!(matches!(
        parse(&record(
            "  - code: number\n    extension_type: 'acme:id@^1'\n    generated: {format: '{n}'}"
        )),
        Err(BlueprintError::InvalidAttributeDeclaration(_))
    ));
}

#[test]
fn unknown_generated_options_are_rejected() {
    assert!(matches!(
        parse(&record(
            "  - code: number\n    value_type: string\n    generated: {format: '{n}', sequence: shared}"
        )),
        Err(BlueprintError::Yaml(_))
    ));
}
