use attricat_lexicon::{Reference, Segment, parse};
use serde_json::{Value, json};

#[test]
fn reference_parser_matches_shared_conformance_cases() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../contracts/lexicon-references.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let actual = match parse(input) {
            Ok(segments) => json!({
                "segments": segments
                    .into_iter()
                    .map(|segment| match segment {
                        Segment::Literal(literal) => json!({"literal": literal}),
                        Segment::Reference(Reference { key, context: None }) => json!({"key": key}),
                        Segment::Reference(Reference { key, context: Some(context) }) => {
                            json!({"key": key, "context": context})
                        }
                    })
                    .collect::<Vec<_>>()
            }),
            Err(error) => json!({"error": error.code()}),
        };
        let mut expected = case.clone();
        expected.as_object_mut().unwrap().remove("input");
        assert_eq!(actual, expected, "input: {input:?}");
    }
}

const CONTRACT: &str = "contracts/lexicon-v1.schema.json";

#[test]
fn lexicon_schema_contract_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join(CONTRACT);
    let mut rendered =
        serde_json::to_string_pretty(&attricat_lexicon::lexicon_file_json_schema()).unwrap();
    rendered.push('\n');
    if std::env::var_os("UPDATE_CONTRACTS").is_some() {
        std::fs::write(&path, rendered).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == rendered,
        "{CONTRACT} is stale; run `just contracts`"
    );
}
