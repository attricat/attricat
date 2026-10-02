use std::{fs, path::Path};

use catalog_blueprint::{compile, parse};

#[test]
fn every_editor_template_compiles_without_workspace_dependencies() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/catalog-web/src/features/blueprints/templates");
    let schema = catalog_blueprint::definition_json_schema();
    let mut checked = 0;

    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let definition = parse(&source)
            .unwrap_or_else(|error| panic!("{} failed to parse: {error}", path.display()));
        compile(definition, &[], &source)
            .unwrap_or_else(|error| panic!("{} failed to compile: {error}", path.display()));

        let instance =
            serde_json::to_value(toml::from_str::<toml::Value>(&source).unwrap()).unwrap();
        let violations = catalog_validation::validate_json_schema(&schema, &instance).unwrap();
        assert!(
            violations.is_empty(),
            "{} violates the editor schema: {violations:?}",
            path.display()
        );
        checked += 1;
    }

    assert!(
        checked >= 3,
        "expected at least the three starter templates"
    );
}
