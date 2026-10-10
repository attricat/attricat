use attricat_blueprint::{BlueprintError, lexicon_references, parse};
use attricat_lexicon::{Reference, ReferenceError};

const SOURCE: &str = r#"
format_version = 1
code = "product"
name = "{{Product}}"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[views.table]
type = "table"
columns = [{ field = "title", label = "{{Title|heading}}" }]

[views.detail]
type = "tabs"

[[views.detail.tabs]]
label = "{{Overview}}"

[[views.detail.tabs.children]]
type = "heading"
text = "{{Not translated}}"

[[views.detail.tabs.children]]
type = "incoming_relationship_list"
label = "SKU \\{{literal}} {{Products in this category}}"
page_size = 10
relationships = [{ source_blueprint = "product", field = "category" }]

[[attributes]]
code = "title"
name = "{{Name}}"
value_type = "string"

[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "live"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "{{Draft|status}}" },
      { "code": "live", "label": "Live" }
    ]
  }
}'''
"#;

fn reference(key: &str, context: Option<&str>) -> Reference {
    Reference {
        key: key.to_owned(),
        context: context.map(str::to_owned),
    }
}

#[test]
fn extracts_references_from_translatable_labels_only() {
    let definition = parse(SOURCE).unwrap();
    let references: Vec<_> = lexicon_references(&definition)
        .into_iter()
        .map(|used| (used.reference, used.counted))
        .collect();
    assert_eq!(
        references,
        [
            (reference("Product", None), true),
            (reference("Name", None), false),
            (reference("Draft", Some("status")), false),
            (reference("Overview", None), false),
            (reference("Products in this category", None), false),
            (reference("Title", Some("heading")), false),
        ]
    );
}

#[test]
fn rejects_malformed_references_in_translatable_labels() {
    for (from, to, expected) in [
        (
            "name = \"{{Product}}\"",
            "name = \"{{Product\"",
            ReferenceError::Unclosed,
        ),
        (
            "name = \"{{Name}}\"",
            "name = \"{{}}\"",
            ReferenceError::EmptyKey,
        ),
        (
            "label = \"{{Overview}}\"",
            "label = \"{{Overview|}}\"",
            ReferenceError::EmptyContext,
        ),
        (
            "label = \"{{Title|heading}}\"",
            "label = \"{{a|b|c}}\"",
            ReferenceError::MultipleContexts,
        ),
        ("{{Draft|status}}", "{{Draft", ReferenceError::Unclosed),
    ] {
        let source = SOURCE.replacen(from, to, 1);
        assert!(
            matches!(
                parse(&source),
                Err(BlueprintError::InvalidLexiconReference { error, .. }) if error == expected
            ),
            "{to}"
        );
    }
    // Free text keeps its braces literally.
    assert!(parse(&SOURCE.replace("{{Not translated}}", "{{oops")).is_ok());
}
