use std::{fs, path::Path};

const CONTRACT: &str = "contracts/blueprint-definition-v1.schema.json";
const UPDATE_CONTRACTS: &str = "UPDATE_CONTRACTS";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
}

fn rendered_schema() -> String {
    let mut rendered =
        serde_json::to_string_pretty(&attricat_blueprint::definition_json_schema()).unwrap();
    rendered.push('\n');
    rendered
}

#[test]
fn definition_schema_contract_is_current() {
    let path = repository_root().join(CONTRACT);
    let rendered = rendered_schema();
    if std::env::var_os(UPDATE_CONTRACTS).is_some() {
        fs::write(&path, rendered).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == rendered,
        "{CONTRACT} is stale; run `just contracts`"
    );
}

/// Extracts raw-string TOML definitions embedded in Rust test sources.
fn embedded_definitions(source: &str) -> Vec<String> {
    let mut definitions = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("r#\"") {
        rest = &rest[start + 3..];
        let Some(end) = rest.find("\"#") else { break };
        if rest[..end].contains("format_version") {
            definitions.push(rest[..end].to_owned());
        }
        rest = &rest[end + 2..];
    }
    definitions
}

fn rust_sources(directory: &Path) -> Vec<String> {
    fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| fs::read_to_string(path).unwrap())
        .collect()
}

/// The schema is advisory and must never reject a definition the parser
/// accepts; cross-field rules stay in `parse`.
#[test]
fn schema_accepts_every_parsed_repository_definition() {
    let root = repository_root();
    let mut definitions: Vec<String> = ["category", "color", "product"]
        .iter()
        .map(|name| {
            fs::read_to_string(root.join(format!("examples/relationships/{name}.toml"))).unwrap()
        })
        .collect();
    for directory in ["crates/blueprint/tests", "apps/api/tests"] {
        for source in rust_sources(&root.join(directory)) {
            definitions.extend(embedded_definitions(&source));
        }
    }

    let schema = attricat_blueprint::definition_json_schema();
    let mut checked = 0;
    for definition in definitions {
        if attricat_blueprint::parse(&definition).is_err() {
            continue;
        }
        let value: toml::Value = toml::from_str(&definition).unwrap();
        let instance = serde_json::to_value(value).unwrap();
        let violations = attricat_validation::validate_json_schema(&schema, &instance).unwrap();
        assert!(
            violations.is_empty(),
            "schema rejected a parsed definition: {violations:?}\n{definition}"
        );
        checked += 1;
    }
    assert!(checked > 20, "only {checked} definitions were checked");
}

#[test]
fn schema_rejects_unknown_keys_and_values() {
    let schema = attricat_blueprint::definition_json_schema();
    for definition in [
        r#"format_version = 1
code = "product"
name = "Product"
kind = "record"
colour = "red"
[[attributes]]
code = "name"
value_type = "string"
"#,
        r#"format_version = 1
code = "product"
name = "Product"
kind = "record"
[[attributes]]
code = "name"
value_type = "text"
"#,
        r#"format_version = 1
code = "product"
name = "Product"
kind = "record"
[views.detail]
type = "stack"
children = [{ type = "field", field = "name", label = "Name" }]
[[attributes]]
code = "name"
value_type = "string"
"#,
    ] {
        assert!(attricat_blueprint::parse(definition).is_err());
        let instance =
            serde_json::to_value(toml::from_str::<toml::Value>(definition).unwrap()).unwrap();
        assert!(
            !attricat_validation::validate_json_schema(&schema, &instance)
                .unwrap()
                .is_empty(),
            "schema accepted {definition}"
        );
    }
}
