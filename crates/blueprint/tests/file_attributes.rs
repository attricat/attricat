use catalog_blueprint::{BlueprintError, compile, parse};

fn source(attribute: &str) -> String {
    format!(
        r#"format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["asset"]

[[attributes]]
{attribute}
"#
    )
}

#[test]
fn compiles_file_policy_with_ordered_many_defaults() {
    let definition = parse(&source(
        r#"code = "asset"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "png"]
max_bytes = 10
purposes = ["product_image"]
image_only = true"#,
    ))
    .unwrap();
    let compiled = compile(definition, &[], "source").unwrap();
    let policy = compiled.attributes[0].file_policy.as_ref().unwrap();
    assert_eq!(policy.cardinality, "many");
    assert!(policy.ordered);
    assert_eq!(policy.allowed_extensions, ["jpg", "png"]);
}

#[test]
fn rejects_file_policy_on_scalar_and_ordered_single_file() {
    let scalar = parse(&source(
        r#"code = "asset"
value_type = "string"
max_bytes = 10"#,
    ));
    assert!(matches!(
        scalar,
        Err(BlueprintError::InvalidAttributeDeclaration(_))
    ));

    let ordered_single = parse(&source(
        r#"code = "asset"
value_type = "file"
cardinality = "one"
ordered = true"#,
    ));
    assert!(matches!(
        ordered_single,
        Err(BlueprintError::InvalidFilePolicy(_))
    ));

    for extension in [".", "..."] {
        let invalid_extension = parse(&source(&format!(
            r#"code = "asset"
value_type = "file"
allowed_extensions = ["{extension}"]"#
        )));
        assert!(matches!(
            invalid_extension,
            Err(BlueprintError::InvalidFilePolicy(_))
        ));
    }
}
