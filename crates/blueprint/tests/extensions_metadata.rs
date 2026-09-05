use catalog_blueprint::parse;

#[test]
fn accepts_namespaced_extension_metadata_without_relaxing_core_fields() {
    let source = r#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["price_gross"]

[extensions.attricat-extension-example.formulas]
price_gross = "price_net * (1 + vat_rate)"

[[attributes]]
code = "price_net"
value_type = "number"

[[attributes]]
code = "vat_rate"
value_type = "number"

[[attributes]]
code = "price_gross"
value_type = "number"
"#;
    assert!(parse(source).is_ok());
}
