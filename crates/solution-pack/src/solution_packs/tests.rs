use std::io::Write;

use serde_json::json;
use tar::{Builder, EntryType, Header};

use image::ImageFormat;

use super::{
    guidance::{validate_configuration_template_value, validate_markdown},
    plan::normalized_blueprint_payload,
    presentation_assets::sanitize_svg,
    *,
};
use crate::solution_pack_seeds::SeedWorkspaceSnapshot;

const PRODUCT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "blueprints/category"
"#;
const CATEGORY_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "category"
name = "Category"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[views.detail]
type = "stack"

[[views.detail.children]]
type = "tabs"

[[views.detail.children.tabs]]
label = "Relationships"

[[views.detail.children.tabs.children]]
type = "grid"

[[views.detail.children.tabs.children.children]]
type = "incoming_relationship_list"
label = "Products"
relationships = [{ source_blueprint = "blueprints/product", field = "categories" }]
page_size = 10

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "products"
value_type = "relationship"
target_blueprint = "blueprints/product"
"#;
const EXPLORE_NAVIGATION: &[u8] = br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","visible_to_role_codes":["viewer","editor"]}]}"#;

fn digest(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

fn resource(key: &str, path: &str, bytes: &[u8]) -> Value {
    json!({"key":key,"path":path,"required":true,"sha256":digest(bytes)})
}

fn manifest_value() -> Value {
    json!({
        "manifest_version": 1,
        "id": "attricat.ecommerce",
        "name": "Ecommerce",
        "version": "1.2.0",
        "description": "Starter catalog",
        "catalog": {"host_api": ">=1.0.0, <2.0.0"},
        "resources": {
            "blueprints": [
                resource("blueprints/product", "blueprints/product.toml", PRODUCT_BLUEPRINT),
                resource("blueprints/category", "blueprints/category.toml", CATEGORY_BLUEPRINT)
            ]
        }
    })
}

fn valid_files() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("blueprints/product.toml", PRODUCT_BLUEPRINT),
        ("blueprints/category.toml", CATEGORY_BLUEPRINT),
    ]
}

fn archive_with_explore_navigation(required: bool) -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["resources"]["workspace_settings"] = json!([{
        "key": "workspace/explore-navigation",
        "path": "workspace/explore-navigation.json",
        "required": required,
        "sha256": digest(EXPLORE_NAVIGATION),
    }]);
    let mut files = valid_files();
    files.push(("workspace/explore-navigation.json", EXPLORE_NAVIGATION));
    archive(&manifest, &files)
}

const LEXICON: &[u8] = br#"{"format_version":1,"kind":"lexicon","languages":[{"language":"pl","entries":[{"key":"Product","plural_category":"one","text":"Produkt"},{"key":"Order","context":"sorting","text":"Kolejno\u015b\u0107"}]},{"language":"en","entries":[{"key":"Product","plural_category":"other","text":"Products"}]}]}"#;

fn archive_with_lexicon(lexicon: &[u8]) -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["resources"]["workspace_settings"] = json!([resource(
        "workspace/lexicon",
        "workspace/lexicon.json",
        lexicon
    )]);
    let mut files = valid_files();
    files.push(("workspace/lexicon.json", lexicon));
    archive(&manifest, &files)
}

fn archive_with_extension_layout(layout: &[u8], requirement_required: bool) -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["resources"]["workspace_settings"] = json!([{
        "key": "workspace/extension-layout",
        "path": "workspace/extension-layout.json",
        "required": true,
        "sha256": digest(layout),
    }]);
    manifest["extensions"] = json!([{
        "key": "extensions/shop",
        "id": "acme.shop",
        "version": "^1.0",
        "required": requirement_required,
    }]);
    let mut files = valid_files();
    files.push(("workspace/extension-layout.json", layout));
    archive(&manifest, &files)
}

fn archive_with_configuration_template(template: &[u8]) -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["extensions"] = json!([{
        "key": "extensions/shopify",
        "id": "acme.shopify",
        "version": ">=2.1.0 <3.0.0",
        "required": true,
        "configuration_template": {
            "path": "extensions/shopify.json",
            "sha256": digest(template)
        }
    }]);
    let mut files = valid_files();
    files.push(("extensions/shopify.json", template));
    archive(&manifest, &files)
}

fn archive(manifest: &Value, files: &[(&str, &[u8])]) -> Vec<u8> {
    let manifest = serde_json::to_vec(manifest).unwrap();
    let mut tar_bytes = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_bytes);
        append_file(&mut builder, SOLUTION_PACK_MANIFEST_PATH, &manifest);
        for (path, bytes) in files {
            append_file(&mut builder, path, bytes);
        }
        builder.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn custom_archive(entries: &[(&[u8], EntryType, &[u8])]) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_bytes);
        for (path, kind, bytes) in entries {
            let mut header = Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(*kind);
            header.as_mut_bytes()[..path.len()].copy_from_slice(path);
            header.set_cksum();
            builder.append(&header, *bytes).unwrap();
        }
        builder.finish().unwrap();
    }
    zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap()
}

fn append_file(builder: &mut Builder<&mut Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut header = Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, path, bytes).unwrap();
}

fn assert_invalid(archive: &[u8], expected: &str) {
    let error = ValidatedSolutionPack::from_tar_zst(archive).unwrap_err();
    assert!(
        error.to_string().contains(expected),
        "expected '{expected}' in '{error}'"
    );
}

fn asset_manifest(bytes: &[u8], purpose: &str, media_type: &str) -> Value {
    json!({
        "manifest_version": 1,
        "id": "attricat.brand",
        "name": "Brand",
        "version": "1.0.0",
        "description": "Brand assets",
        "catalog": {"host_api": ">=1.0.0, <2.0.0"},
        "resources": {"presentation_assets": [{
            "key": "assets/brand-logo",
            "path": "assets/brand-logo.svg",
            "required": true,
            "purpose": purpose,
            "media_type": media_type,
            "sha256": digest(bytes)
        }]}
    })
}

#[test]
fn validates_and_deterministically_normalizes_static_svg_assets() {
    let svg = br##"<svg height="10" xmlns="http://www.w3.org/2000/svg" width="20"><defs><linearGradient id="paint"><stop offset="0" stop-color="#fff"/></linearGradient></defs><rect height="10" fill="#fff" width="20"/></svg>"##;
    let manifest = asset_manifest(svg, "logo", "image/svg+xml");
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &[("assets/brand-logo.svg", svg)]))
            .unwrap();
    let asset = pack.presentation_asset("assets/brand-logo").unwrap();
    assert_eq!(asset.source_sha256, digest(svg));
    assert_eq!(asset.stored_sha256, digest(&asset.stored_bytes));
    assert_eq!(asset.width, None);
    assert_eq!(
        std::str::from_utf8(&asset.stored_bytes).unwrap(),
        r##"<svg height="10" width="20" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint"><stop offset="0" stop-color="#fff"/></linearGradient></defs><rect fill="#fff" height="10" width="20"/></svg>"##
    );
    assert_eq!(
        sanitize_svg(&asset.stored_bytes, "assets/brand-logo").unwrap(),
        asset.stored_bytes
    );
}

#[test]
fn svg_assets_reject_active_remote_and_scriptable_content() {
    for svg in [
        br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#.as_slice(),
        br#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"/>"#,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject/></svg>"#,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><use href="https://example.test/a.svg#x"/></svg>"#,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(https://example.test/x)"/></svg>"#,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><animate attributeName="x"/></svg>"#,
        br#"<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><svg xmlns="http://www.w3.org/2000/svg"/>"#,
    ] {
        let manifest = asset_manifest(svg, "logo", "image/svg+xml");
        assert_invalid(
            &archive(&manifest, &[("assets/brand-logo.svg", svg)]),
            "SVG",
        );
    }
}

#[test]
fn svg_url_capable_attributes_reject_css_escapes() {
    let attributes = [
        "fill",
        "stroke",
        "stop-color",
        "clip-path",
        "mask",
        "href",
        "xlink:href",
    ];
    let escaped_values = [
        r"\75\72\6c(\68\74\74\70\73\3a\2f\2f example.test/x)",
        r"\68\74\74\70\73\3a\2f\2f example.test/x",
        r"\64\61\74\61\3a image/svg+xml,x",
    ];
    for attribute in attributes {
        for value in escaped_values {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use {attribute}="{value}"/></svg>"#
            );
            let manifest = asset_manifest(svg.as_bytes(), "logo", "image/svg+xml");
            assert_invalid(
                &archive(&manifest, &[("assets/brand-logo.svg", svg.as_bytes())]),
                "SVG",
            );
        }
    }
}

#[test]
fn raster_presentation_assets_reject_oversized_dimensions_before_decode() {
    let image = image::DynamicImage::new_luma8(MAX_SOLUTION_PACK_ASSET_DIMENSION + 1, 1);
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Png).unwrap();
    assert!(encoded.get_ref().len() < MAX_SOLUTION_PACK_PRESENTATION_ASSET_BYTES);
    let error =
        validate_presentation_asset_bytes("logo", "image/png", encoded.get_ref(), "oversized")
            .unwrap_err();
    assert!(error.to_string().contains("dimensions exceed"));
}

#[test]
fn raster_presentation_assets_require_exact_magic_decode_and_dimensions() {
    for (media_type, format) in [
        ("image/png", ImageFormat::Png),
        ("image/jpeg", ImageFormat::Jpeg),
        ("image/webp", ImageFormat::WebP),
    ] {
        let image = image::DynamicImage::new_rgb8(2, 3);
        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, format).unwrap();
        let purpose = if media_type == "image/jpeg" {
            "illustration"
        } else {
            "logo"
        };
        let normalized =
            validate_presentation_asset_bytes(purpose, media_type, encoded.get_ref(), "test")
                .unwrap();
        assert_eq!((normalized.width, normalized.height), (Some(2), Some(3)));
        assert_eq!(normalized.bytes, encoded.into_inner());
    }
    assert!(validate_presentation_asset_bytes("logo", "image/png", b"not a png", "test").is_err());
    assert!(
        validate_presentation_asset_bytes("logo", "image/jpeg", &[0xff, 0xd8, 0xff, 0xd9], "test")
            .is_err()
    );
}

#[test]
fn presentation_asset_manifest_enforces_media_purpose_and_digest() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
    let manifest = asset_manifest(svg, "logo", "image/jpeg");
    assert_invalid(
        &archive(&manifest, &[("assets/brand-logo.svg", svg)]),
        "media type is not allowed",
    );

    let mut manifest = asset_manifest(svg, "logo", "image/svg+xml");
    manifest["resources"]["presentation_assets"][0]["sha256"] = json!("A".repeat(64));
    assert_invalid(
        &archive(&manifest, &[("assets/brand-logo.svg", svg)]),
        "lowercase hexadecimal",
    );
}

#[test]
fn presentation_asset_path_runtime_and_schema_share_strict_ascii_grammar() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-manifest-v1.schema.json"
    ))
    .unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
    for path in [
        "assets/.logo.svg",
        "assets/brand logo.svg",
        "assets/bränd-logo.svg",
    ] {
        let mut manifest = asset_manifest(svg, "logo", "image/svg+xml");
        manifest["resources"]["presentation_assets"][0]["path"] = json!(path);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &manifest)
                .unwrap()
                .is_empty(),
            "schema accepted {path}"
        );
        assert_invalid(&archive(&manifest, &[(path, svg)]), "path");
    }
}

#[test]
fn validates_a_complete_archive_and_exposes_validated_content() {
    let archive_bytes = archive(&manifest_value(), &valid_files());
    let pack = ValidatedSolutionPack::from_tar_zst(&archive_bytes).unwrap();

    assert_eq!(pack.manifest().id, "attricat.ecommerce");
    assert_eq!(pack.manifest().version, "1.2.0");
    assert_eq!(pack.archive_sha256(), digest(&archive_bytes));
    assert_eq!(pack.files().count(), 2);
    assert_eq!(
        pack.blueprint("blueprints/product").unwrap().code(),
        "product"
    );

    let mut illustrative_range = manifest_value();
    illustrative_range["catalog"]["host_api"] = json!(">=1.0.0 <2.0.0");
    ValidatedSolutionPack::from_tar_zst(&archive(&illustrative_range, &valid_files())).unwrap();
}

#[test]
fn validates_and_normalizes_public_guidance_and_informational_checks() {
    let readme = b"# Setup\r\n\r\nSee [details](#details).";
    let checklist = br#"{"format_version":1,"items":[{"key":"checklist/publish","title":"Publish product","markdown":"Publish the product.","check":"checks/product-published"}]}"#;
    let checks = br#"{"format_version":1,"checks":[{"key":"checks/product-published","title":"Product is published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}]}"#;
    let mut manifest = manifest_value();
    manifest["documentation"] = json!({
        "readme":{"path":"README.md","sha256":digest(readme)},
        "setup_checklist":{"path":"setup/checklist.json","sha256":digest(checklist)}
    });
    manifest["checks"] = json!({"path":"checks/checks.json","sha256":digest(checks)});
    let mut files = valid_files();
    files.extend([
        ("README.md", readme.as_slice()),
        ("setup/checklist.json", checklist.as_slice()),
        ("checks/checks.json", checks.as_slice()),
    ]);
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    assert_eq!(
        pack.guidance().readme_markdown.as_deref(),
        Some("# Setup\n\nSee [details](#details).")
    );
    assert_eq!(
        pack.guidance()
            .setup_checklist
            .as_ref()
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        pack.checks()[0].predicate.predicate_type(),
        "blueprint_published"
    );

    let unsafe_readme = b"![remote](https://example.test/image.png)";
    manifest["documentation"]["readme"]["sha256"] = json!(digest(unsafe_readme));
    files[2] = ("README.md", unsafe_readme);
    assert_invalid(&archive(&manifest, &files), "unsafe Markdown");
}

#[test]
fn guidance_text_bounds_count_utf8_bytes() {
    assert!(validate_bounded_text(&"é".repeat(100), "title", 200).is_ok());
    assert!(validate_bounded_text(&"é".repeat(101), "title", 200).is_err());
    assert!(validate_markdown(&"é".repeat(2048), 4096, "markdown").is_ok());
    assert!(validate_markdown(&"é".repeat(2049), 4096, "markdown").is_err());
}

#[test]
fn check_predicates_are_a_closed_host_defined_catalogue() {
    for (value, expected) in [
        (
            json!({"type":"blueprint_published","blueprint":"blueprints/product"}),
            "blueprint_published",
        ),
        (
            json!({"type":"extension_installed","extension":"extensions/shop"}),
            "extension_installed",
        ),
        (
            json!({"type":"extension_enabled","extension":"extensions/shop"}),
            "extension_enabled",
        ),
        (
            json!({"type":"extension_configuration_matches","extension":"extensions/shop"}),
            "extension_configuration_matches",
        ),
        (
            json!({"type":"explore_navigation_entry_present","blueprint":"blueprints/product"}),
            "explore_navigation_entry_present",
        ),
        (
            json!({"type":"workspace_extension_layout_placement_present","contribution":"acme.shop:nav"}),
            "workspace_extension_layout_placement_present",
        ),
    ] {
        let predicate: SolutionPackCheckPredicate = serde_json::from_value(value).unwrap();
        assert_eq!(predicate.predicate_type(), expected);
    }
    assert!(
        serde_json::from_value::<SolutionPackCheckPredicate>(
            json!({"type":"query","sql":"select 1"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<SolutionPackCheckPredicate>(
            json!({"type":"blueprint_published","blueprint":"blueprints/product","required":true})
        )
        .is_err()
    );
}

#[test]
fn rejects_unknown_or_unresolvable_guidance_and_check_fields() {
    let checklist = br#"{"format_version":1,"items":[{"key":"checklist/setup","title":"Setup","markdown":"Do setup.","check":"checks/missing"}]}"#;
    let checks = br#"{"format_version":1,"checks":[{"key":"checks/missing","title":"Missing","predicate":{"type":"blueprint_published","blueprint":"blueprints/missing"}}]}"#;
    let mut manifest = manifest_value();
    manifest["documentation"] =
        json!({"setup_checklist":{"path":"setup.json","sha256":digest(checklist)}});
    manifest["checks"] = json!({"path":"checks.json","sha256":digest(checks)});
    let mut files = valid_files();
    files.extend([
        ("setup.json", checklist.as_slice()),
        ("checks.json", checks.as_slice()),
    ]);
    assert_invalid(&archive(&manifest, &files), "undeclared blueprint");

    let context_reference = br#"{"format_version":1,"checks":[{"key":"checks/context","title":"Context","predicate":{"type":"blueprint_published","blueprint":"contexts/web"}}]}"#;
    manifest["checks"]["sha256"] = json!(digest(context_reference));
    files[3] = ("checks.json", context_reference);
    assert_invalid(
        &archive(&manifest, &files),
        "undeclared blueprint 'contexts/web'",
    );

    let unknown = br#"{"format_version":1,"checks":[],"query":"select *"}"#;
    manifest["checks"]["sha256"] = json!(digest(unknown));
    files[3] = ("checks.json", unknown);
    assert_invalid(&archive(&manifest, &files), "not valid strict JSON");
}

#[test]
fn validates_strict_explore_navigation_contract() {
    let pack = ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(true)).unwrap();
    let navigation = pack.explore_navigation().unwrap();
    assert_eq!(navigation.entries.len(), 1);
    assert_eq!(
        navigation.entries[0].visible_to_role_codes,
        ["editor", "viewer"]
    );

    for (invalid, expected) in [
        (
            br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","visible_to_role_codes":["editor","editor"]}]}"#.as_slice(),
            "duplicate role codes",
        ),
        (
            br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product","unknown":true}]}"#.as_slice(),
            "not valid strict JSON",
        ),
    ] {
        let mut manifest = manifest_value();
        manifest["resources"]["workspace_settings"] = json!([resource(
            "workspace/explore-navigation",
            "workspace/explore-navigation.json",
            invalid,
        )]);
        let mut files = valid_files();
        files.push(("workspace/explore-navigation.json", invalid));
        assert_invalid(&archive(&manifest, &files), expected);
    }

    for (navigation, expected) in [
        (
            json!({
                "format_version": 1,
                "kind": "explore_navigation",
                "entries": (0..=MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ENTRIES)
                    .map(|index| json!({"blueprint": format!("blueprints/product_{index}")}))
                    .collect::<Vec<_>>(),
            }),
            "must contain 1-64 entries",
        ),
        (
            json!({
                "format_version": 1,
                "kind": "explore_navigation",
                "entries": [{
                    "blueprint": "blueprints/product",
                    "visible_to_role_codes": (0..=MAX_SOLUTION_PACK_EXPLORE_NAVIGATION_ROLES)
                        .map(|index| format!("role_{index}"))
                        .collect::<Vec<_>>(),
                }],
            }),
            "too many role codes",
        ),
    ] {
        let navigation = serde_json::to_vec(&navigation).unwrap();
        let mut manifest = manifest_value();
        manifest["resources"]["workspace_settings"] = json!([resource(
            "workspace/explore-navigation",
            "workspace/explore-navigation.json",
            &navigation,
        )]);
        let mut files: Vec<(&str, &[u8])> = valid_files();
        files.push(("workspace/explore-navigation.json", navigation.as_slice()));
        assert_invalid(&archive(&manifest, &files), expected);
    }
}

#[test]
fn validates_and_plans_extension_layout_item_level_merge() {
    let layout = br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","promoted":true,"required":true}]}"#;
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(layout, true)).unwrap();
    assert_eq!(pack.extension_layout().unwrap().entries.len(), 1);

    let installed = InstalledExtensionSnapshot {
        installed_release_id: uuid::Uuid::from_u128(7),
        version: "1.2.0".into(),
        state: "disabled".into(),
        configuration: json!({}),
        policy_compatible: true,
        contributions: BTreeMap::from([("acme.shop:nav".to_owned(), "navigation".to_owned())]),
        pending_install: false,
    };
    let workspace = |extension_layout| PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::nil(),
        physical_codes: BTreeSet::from(["default".to_owned()]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::from([("acme.shop".to_owned(), installed.clone())]),
        explore_navigation: Vec::new(),
        explore_navigation_valid: true,
        extension_layout,
        extension_layout_valid: true,
        role_codes: BTreeSet::new(),
        published_entity_codes: BTreeSet::new(),
        seed: Default::default(),
    };
    let action_for = |current| {
        build_solution_pack_plan(
            &pack,
            "shop",
            BlueprintPublication::Draft,
            &workspace(current),
        )
        .unwrap()
        .actions
        .into_iter()
        .find(|action| action.logical_key == "workspace/extension-layout")
        .unwrap()
    };
    assert_eq!(
        action_for(json!({"version":1,"outlets":{}})).action,
        PlanActionKind::Append
    );
    assert_eq!(
        action_for(json!({"version":1,"outlets":{"navigation":{"order":["acme.shop:nav"],"hidden":[],"promoted":["acme.shop:nav"]}}})).action,
        PlanActionKind::Satisfied
    );
    assert_eq!(
        action_for(json!({"version":1,"outlets":{"navigation":{"order":[],"hidden":["acme.shop:nav"],"promoted":[]}}})).action,
        PlanActionKind::Conflict
    );
    assert_eq!(
        action_for(json!({"version":1,"outlets":{"navigation":{"order":[],"hidden":[],"promoted":["acme.shop:nav"]}}})).action,
        PlanActionKind::Conflict
    );
    assert_eq!(
        action_for(json!({"version":1,"outlets":{
            "navigation":{"order":[],"hidden":[],"promoted":["acme.shop:nav"]},
            "entity_action":{"order":["acme.shop:nav"],"hidden":[]}
        }}))
        .action,
        PlanActionKind::Conflict
    );

    let unavailable_action = |installed: Option<InstalledExtensionSnapshot>| {
        let plan = build_solution_pack_plan(
            &pack,
            "shop",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                existing_presentation_assets: BTreeMap::new(),
                installed_extensions: installed
                    .map(|installed| BTreeMap::from([("acme.shop".to_owned(), installed)]))
                    .unwrap_or_default(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
                seed: Default::default(),
            },
        )
        .unwrap();
        let action = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "workspace/extension-layout")
            .unwrap();
        (action.action, action.reason_code)
    };
    let mut missing_contribution = installed.clone();
    missing_contribution.contributions.clear();
    let mut wrong_outlet = installed.clone();
    wrong_outlet
        .contributions
        .insert("acme.shop:nav".to_owned(), "entity_action".to_owned());
    let mut quarantined = installed.clone();
    quarantined.state = "quarantined".to_owned();
    let mut policy_incompatible = installed.clone();
    policy_incompatible.policy_compatible = false;
    let mut incompatible_version = installed.clone();
    incompatible_version.version = "2.0.0".to_owned();
    for (candidate, reason) in [
        (None, "missing"),
        (Some(missing_contribution), "contribution_missing"),
        (Some(wrong_outlet), "outlet_mismatch"),
        (Some(quarantined), "quarantined"),
        (Some(policy_incompatible), "policy_incompatible"),
        (Some(incompatible_version), "incompatible_version"),
    ] {
        assert_eq!(
            unavailable_action(candidate),
            (PlanActionKind::Blocked, reason)
        );
    }

    let optional_layout = br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":false}]}"#;
    let optional_pack =
        ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(optional_layout, false))
            .unwrap();
    let optional_plan = build_solution_pack_plan(
        &optional_pack,
        "shop",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(
        optional_plan
            .actions
            .iter()
            .find(|action| action.logical_key == "workspace/extension-layout")
            .unwrap()
            .action,
        PlanActionKind::Skip
    );
}

#[test]
fn blueprint_layout_skips_optional_unavailable_contributions_and_blocks_required() {
    let blueprint = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.extension_layout]
type = "extension_layout"
version = 1
[views.extension_layout.outlets.entity_action]
order = ["acme.shop:action"]
hidden = []
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    for (required, expected_action) in [
        (false, PlanActionKind::Create),
        (true, PlanActionKind::Blocked),
    ] {
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&blueprint));
        manifest["extensions"] = json!([{
            "key": "extensions/shop",
            "id": "acme.shop",
            "version": "^1.0",
            "required": required,
        }]);
        let mut files = valid_files();
        files[0] = (files[0].0, &blueprint);
        let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "shop",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                existing_presentation_assets: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::new(),
                seed: Default::default(),
            },
        )
        .unwrap();
        let action = plan
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/product")
            .unwrap();
        assert_eq!(action.action, expected_action);
        if !required {
            assert!(
                !action.normalized_payload.as_ref().unwrap()["definition"]
                    .as_str()
                    .unwrap()
                    .contains("acme.shop:action")
            );
            assert_eq!(action.summary["extension_layout"][0]["outcome"], "skip");
        } else {
            for contributions in [
                BTreeMap::new(),
                BTreeMap::from([(
                    "acme.shop:action".to_owned(),
                    "entity_preview_panel".to_owned(),
                )]),
            ] {
                let mapped = build_solution_pack_plan(
                    &pack,
                    "shop",
                    BlueprintPublication::Draft,
                    &PlanningWorkspaceSnapshot {
                        workspace_id: uuid::Uuid::nil(),
                        physical_codes: BTreeSet::from(["shop_product".to_owned()]),
                        existing_presentation_assets: BTreeMap::new(),
                        existing_blueprints: BTreeMap::from([(
                            "blueprints/product".to_owned(),
                            ExistingBlueprintSnapshot {
                                id: uuid::Uuid::from_u128(100),
                                code: "shop_product".to_owned(),
                                version: 1,
                                kind: "entity".to_owned(),
                                canonical_definition_hash: "0".repeat(64),
                                definition_hash: "0".repeat(64),
                            },
                        )]),
                        installed_extensions: BTreeMap::from([(
                            "acme.shop".to_owned(),
                            InstalledExtensionSnapshot {
                                installed_release_id: uuid::Uuid::from_u128(101),
                                version: "1.0.0".to_owned(),
                                state: "enabled".to_owned(),
                                configuration: json!({}),
                                policy_compatible: true,
                                contributions,
                                pending_install: false,
                            },
                        )]),
                        explore_navigation: Vec::new(),
                        explore_navigation_valid: true,
                        extension_layout: json!({"version":1,"outlets":{}}),
                        extension_layout_valid: true,
                        role_codes: BTreeSet::new(),
                        published_entity_codes: BTreeSet::from(["shop_product".to_owned()]),
                        seed: Default::default(),
                    },
                )
                .unwrap();
                let mapped_product = mapped
                    .actions
                    .iter()
                    .find(|action| action.logical_key == "blueprints/product")
                    .unwrap();
                assert_eq!(
                    (mapped_product.action, mapped_product.reason_code),
                    (
                        PlanActionKind::Blocked,
                        "extension_contribution_unavailable"
                    )
                );
                assert_eq!(mapped.extension_requirements[0].status, "satisfied");
                assert!(!mapped.ready);
            }
        }
    }
}

#[test]
fn workspace_layout_accepts_every_manifest_outlet_and_both_primary_lists() {
    for outlet in [
        "navigation",
        "entity_preview_panel",
        "blueprint_attribute_configuration",
        "entity_attribute_decoration",
        "entity_action",
        "explorer_row_action",
        "explorer_table_cell",
        "blueprint_detail_panel",
        "explorer_action",
        "explorer_bulk_action",
        "entity_header_action",
        "entity_attribute_panel",
        "blueprint_panel",
        "blueprint_publish_check",
        "file_panel",
        "audit_event_panel",
        "data_health_card",
    ] {
        for hidden in [false, true] {
            let layout = serde_json::to_vec(&json!({
                "format_version": 1,
                "kind": "extension_layout",
                "entries": [{
                    "contribution": "acme.shop:item",
                    "outlet": outlet,
                    "hidden": hidden,
                    "required": true,
                }],
            }))
            .unwrap();
            ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(&layout, true))
                .unwrap();
        }
    }
}

#[test]
fn blueprint_layout_contribution_evidence_order_is_deterministic() {
    let blueprint = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.extension_layout]
type = "extension_layout"
version = 1
[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.shop:preview"]
hidden = []
[views.extension_layout.outlets.entity_action]
order = ["acme.shop:z_action"]
hidden = ["acme.shop:a_action"]
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&blueprint));
    manifest["extensions"] = json!([{
        "key": "extensions/shop",
        "id": "acme.shop",
        "version": "^1.0",
        "required": true,
    }]);
    let mut files = valid_files();
    files[0] = (files[0].0, &blueprint);
    let archive = archive(&manifest, &files);
    let expected = vec![
        ("entity_action", "acme.shop:a_action"),
        ("entity_action", "acme.shop:z_action"),
        ("entity_preview_panel", "acme.shop:preview"),
    ];
    for _ in 0..20 {
        let pack = ValidatedSolutionPack::from_tar_zst(&archive).unwrap();
        let actual = pack
            .blueprint("blueprints/product")
            .unwrap()
            .extension_layout
            .iter()
            .map(|entry| (entry.outlet.as_str(), entry.contribution.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}

#[test]
fn rejects_invalid_extension_layout_contract() {
    for invalid in [
        br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"entity_action","promoted":true,"required":true}]}"#.as_slice(),
        br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","hidden":true,"promoted":true,"required":true}]}"#.as_slice(),
        br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":true,"unknown":true}]}"#.as_slice(),
        br#"{"format_version":1,"kind":"extension_layout","entries":[{"contribution":"acme.shop:nav","outlet":"navigation","required":true},{"contribution":"acme.shop:nav","outlet":"navigation","required":true}]}"#.as_slice(),
    ] {
        assert!(ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(invalid, true)).is_err());
    }
    let too_many = serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "extension_layout",
        "entries": (0..=MAX_SOLUTION_PACK_EXTENSION_LAYOUT_ENTRIES)
            .map(|index| json!({
                "contribution": format!("acme.shop:item_{index}"),
                "outlet": "entity_action",
                "required": true,
            }))
            .collect::<Vec<_>>(),
    }))
    .unwrap();
    assert!(
        ValidatedSolutionPack::from_tar_zst(&archive_with_extension_layout(&too_many, true))
            .is_err()
    );
}

#[test]
fn planner_appends_satisfies_and_conflicts_explore_navigation() {
    let pack = ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(true)).unwrap();
    let workspace = |navigation| PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::nil(),
        physical_codes: BTreeSet::from(["default".to_owned()]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::new(),
        explore_navigation: navigation,
        explore_navigation_valid: true,
        extension_layout: serde_json::json!({"version":1,"outlets":{}}),
        extension_layout_valid: true,
        role_codes: BTreeSet::from(["editor".to_owned(), "viewer".to_owned()]),
        published_entity_codes: BTreeSet::from(["ecom_product".to_owned()]),
        seed: Default::default(),
    };
    let appended = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &workspace(Vec::new()),
    )
    .unwrap();
    let action = appended.actions.last().unwrap();
    assert_eq!(
        (action.action, action.reason_code),
        (PlanActionKind::Append, "target_absent")
    );
    assert!(appended.ready);

    let exact = PlanningExploreNavigationEntry {
        blueprint_code: "ecom_product".to_owned(),
        visible_to_role_codes: vec!["viewer".to_owned(), "editor".to_owned()],
    };
    let satisfied = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &workspace(vec![exact]),
    )
    .unwrap();
    assert_eq!(
        satisfied.actions.last().unwrap().action,
        PlanActionKind::Satisfied
    );

    let conflicting = PlanningExploreNavigationEntry {
        blueprint_code: "ecom_product".to_owned(),
        visible_to_role_codes: vec!["viewer".to_owned()],
    };
    let conflicted = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &workspace(vec![conflicting]),
    )
    .unwrap();
    assert_eq!(
        conflicted.actions.last().unwrap().action,
        PlanActionKind::Conflict
    );
    assert!(!conflicted.ready);
}

#[test]
fn validates_lexicon_and_plans_an_idempotent_append() {
    let pack = ValidatedSolutionPack::from_tar_zst(&archive_with_lexicon(LEXICON)).unwrap();
    let entries = pack.lexicon().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].context.as_deref(), Some("sorting"));
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(plan.ready);
    let action = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "workspace/lexicon")
        .unwrap();
    assert_eq!(action.action, PlanActionKind::Append);
    assert_eq!(action.summary["languages"], json!(["en", "pl"]));
    assert_eq!(action.summary["entry_count"], 3);
    assert_eq!(
        action.normalized_payload.as_ref().unwrap()["entries"][0]["key"],
        "Product"
    );

    for (invalid, expected) in [
        (
            br#"{"format_version":1,"kind":"lexicon","languages":[]}"#.as_slice(),
            "at least one language",
        ),
        (
            br#"{"format_version":1,"kind":"lexicon","languages":[{"language":"xx","entries":[{"key":"A","text":"B"}]}]}"#.as_slice(),
            "unsupported",
        ),
        (
            br#"{"format_version":1,"kind":"lexicon","languages":[{"language":"pl","entries":[{"key":"A","text":"B"}]},{"language":"PL","entries":[{"key":"A","text":"B"}]}]}"#.as_slice(),
            "duplicated",
        ),
        (
            br#"{"format_version":1,"kind":"lexicon","languages":[{"language":"en","entries":[{"key":"A","plural_category":"few","text":"B"}]}]}"#.as_slice(),
            "plural category",
        ),
        (
            br#"{"format_version":1,"kind":"lexicon","languages":[{"language":"en","entries":[{"key":"A","text":"B","unknown":1}]}]}"#.as_slice(),
            "not valid strict JSON",
        ),
    ] {
        assert_invalid(&archive_with_lexicon(invalid), expected);
    }
}

#[test]
fn planner_blocks_required_navigation_and_skips_optional_unmet_navigation() {
    for (required, expected) in [
        (true, PlanActionKind::Blocked),
        (false, PlanActionKind::Skip),
    ] {
        let pack = ValidatedSolutionPack::from_tar_zst(&archive_with_explore_navigation(required))
            .unwrap();
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["default".to_owned()]),
                existing_blueprints: BTreeMap::new(),
                existing_presentation_assets: BTreeMap::new(),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: serde_json::json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::from(["editor".to_owned(), "viewer".to_owned()]),
                published_entity_codes: BTreeSet::new(),
                seed: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(plan.actions.last().unwrap().action, expected);
    }

    let navigation = br#"{"format_version":1,"kind":"explore_navigation","entries":[{"blueprint":"blueprints/product"},{"blueprint":"blueprints/category","visible_to_role_codes":["missing_role"]}]}"#;
    let mut manifest = manifest_value();
    manifest["resources"]["workspace_settings"] = json!([{
        "key": "workspace/explore-navigation",
        "path": "workspace/explore-navigation.json",
        "required": false,
        "sha256": digest(navigation),
    }]);
    let files = [
        ("blueprints/product.toml", PRODUCT_BLUEPRINT),
        ("blueprints/category.toml", CATEGORY_BLUEPRINT),
        ("workspace/explore-navigation.json", navigation.as_slice()),
    ];
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    let action = plan.actions.last().unwrap();
    assert_eq!(action.action, PlanActionKind::Append);
    assert_eq!(action.summary["entries"][1]["outcome"], "skip");
    assert_eq!(
        action.normalized_payload.as_ref().unwrap()["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn validates_bounded_non_secret_extension_configuration_templates() {
    const TEMPLATE: &[u8] = br#"{"endpoint":"https://example.test","features":{"sync":true}}"#;
    let pack = ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(TEMPLATE))
        .unwrap();
    assert_eq!(pack.manifest().extensions.len(), 1);
    assert_eq!(
        pack.configuration_template("extensions/shopify"),
        Some(&json!({"endpoint":"https://example.test","features":{"sync":true}}))
    );

    for template in [
        br#"[]"#.as_slice(),
        br#"{"api_token":"public-looking-but-forbidden"}"#,
        br#"{"nested":{"PASSWORD":"forbidden"}}"#,
        br#"{"unterminated":true"#,
    ] {
        assert!(
            ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(template))
                .is_err()
        );
    }

    let exact_size = format!(
        "{{}}{}",
        " ".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_BYTES - 2)
    );
    ValidatedSolutionPack::from_tar_zst(&archive_with_configuration_template(
        exact_size.as_bytes(),
    ))
    .unwrap();
    assert_invalid(
        &archive_with_configuration_template(format!("{exact_size} ").as_bytes()),
        "size limit",
    );

    fn validate_value(value: &Value) -> Result<(), SolutionPackError> {
        let mut items = 0;
        validate_configuration_template_value(value, 1, &mut items, "extensions/test")
    }

    let mut maximum_depth = json!(true);
    for _ in 0..(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_DEPTH - 1) {
        maximum_depth = json!({"level": maximum_depth});
    }
    validate_value(&maximum_depth).unwrap();
    assert!(validate_value(&json!({"level": maximum_depth})).is_err());

    let maximum_items = Value::Object(
        (0..MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS)
            .map(|index| (format!("field_{index}"), Value::Null))
            .collect(),
    );
    validate_value(&maximum_items).unwrap();
    let excessive_items = Value::Object(
        (0..=MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_ITEMS)
            .map(|index| (format!("field_{index}"), Value::Null))
            .collect(),
    );
    assert!(validate_value(&excessive_items).is_err());

    validate_value(&json!({
        "k".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES): true
    }))
    .unwrap();
    assert!(
        validate_value(&json!({
            "k".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_KEY_BYTES + 1): true
        }))
        .is_err()
    );
    validate_value(&json!({
        "value": "v".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES)
    }))
    .unwrap();
    assert!(
        validate_value(&json!({
            "value": "v".repeat(MAX_SOLUTION_PACK_CONFIGURATION_TEMPLATE_STRING_BYTES + 1)
        }))
        .is_err()
    );
}

#[test]
fn extension_requirement_evaluation_is_deterministic_and_does_not_require_enablement() {
    let requirement = SolutionPackExtensionRequirement {
        key: "extensions/shopify".into(),
        id: "acme.shopify".into(),
        version: ">=2.1.0 <3.0.0".into(),
        required: true,
        configuration_template: None,
    };
    let installed = InstalledExtensionSnapshot {
        installed_release_id: uuid::Uuid::nil(),
        version: "2.2.0".into(),
        state: "disabled".into(),
        configuration: json!({"endpoint":"https://example.test","unrelated_secret":"not exposed"}),
        policy_compatible: true,
        contributions: BTreeMap::new(),
        pending_install: false,
    };
    let template = json!({"endpoint":"https://example.test"});
    let satisfied = evaluate_extension_requirement(&requirement, Some(&template), Some(&installed));
    assert_eq!(satisfied.status, "satisfied");
    assert_eq!(satisfied.reason_code, "satisfied");

    let mismatch = evaluate_extension_requirement(
        &requirement,
        Some(&json!({"endpoint":"https://other.test"})),
        Some(&installed),
    );
    assert_eq!(mismatch.status, "blocked");
    assert_eq!(mismatch.reason_code, "configuration_mismatch");

    let incompatible = InstalledExtensionSnapshot {
        version: "3.0.0".into(),
        ..installed.clone()
    };
    let blocked = evaluate_extension_requirement(&requirement, None, Some(&incompatible));
    assert_eq!(blocked.status, "blocked");
    assert_eq!(blocked.reason_code, "incompatible_version");

    let mut optional = requirement.clone();
    optional.required = false;
    let missing = evaluate_extension_requirement(&optional, None, None);
    assert_eq!(missing.status, "skipped");
    assert_eq!(missing.reason_code, "missing");
    let incompatible = evaluate_extension_requirement(&optional, None, Some(&incompatible));
    assert_eq!(incompatible.status, "skipped");
    assert_eq!(incompatible.reason_code, "incompatible_version");
    let optional_mismatch = evaluate_extension_requirement(
        &optional,
        Some(&json!({"endpoint":"https://other.test"})),
        Some(&installed),
    );
    assert_eq!(optional_mismatch.status, "skipped");
    assert_eq!(optional_mismatch.reason_code, "configuration_mismatch");
    let optional_satisfied =
        evaluate_extension_requirement(&optional, Some(&template), Some(&installed));
    assert_eq!(optional_satisfied.status, "satisfied");

    let quarantined = InstalledExtensionSnapshot {
        state: "quarantined".into(),
        ..installed
    };
    let blocked = evaluate_extension_requirement(&requirement, None, Some(&quarantined));
    assert_eq!(blocked.reason_code, "quarantined");
    let skipped = evaluate_extension_requirement(&optional, None, Some(&quarantined));
    assert_eq!(skipped.status, "skipped");
    assert_eq!(skipped.reason_code, "quarantined");
}

#[test]
fn pending_official_release_is_planned_as_an_install_without_installed_state() {
    let requirement = SolutionPackExtensionRequirement {
        key: "extensions/shopify".into(),
        id: "acme.shopify".into(),
        version: "^2.1".into(),
        required: true,
        configuration_template: None,
    };
    let pending = InstalledExtensionSnapshot {
        installed_release_id: uuid::Uuid::from_u128(7),
        version: "2.3.0".into(),
        state: "disabled".into(),
        configuration: json!({}),
        policy_compatible: true,
        contributions: BTreeMap::new(),
        pending_install: true,
    };
    let planned = evaluate_extension_requirement(&requirement, None, Some(&pending));
    assert_eq!(planned.status, "install");
    assert_eq!(planned.reason_code, "install");
    assert_eq!(planned.installed_release_id, None);
    assert_eq!(planned.installed_version, None);
    assert_eq!(planned.installed_state, None);
    assert_eq!(planned.configuration_matches, None);

    let denied = InstalledExtensionSnapshot {
        policy_compatible: false,
        ..pending
    };
    let blocked = evaluate_extension_requirement(&requirement, None, Some(&denied));
    assert_eq!(blocked.status, "blocked");
    assert_eq!(blocked.reason_code, "policy_incompatible");
    assert_eq!(blocked.installed_release_id, None);
}

#[test]
fn configuration_matching_uses_recursive_object_containment_and_exact_arrays() {
    assert!(json_deep_contains(
        &json!({"nested":{"enabled":true,"extra":1},"list":[1,2],"extra":true}),
        &json!({"nested":{"enabled":true},"list":[1,2]})
    ));
    assert!(!json_deep_contains(
        &json!({"list":[1,2,3]}),
        &json!({"list":[1,2]})
    ));
    assert!(!json_deep_contains(
        &json!({"value":"1"}),
        &json!({"value":1})
    ));
}

#[test]
fn accepts_safe_directory_entries() {
    let manifest = serde_json::to_vec(&manifest_value()).unwrap();
    let files = valid_files();
    let mut entries = vec![(
        b"blueprints/".as_slice(),
        EntryType::Directory,
        b"".as_slice(),
    )];
    entries.push((b"solution-pack.json", EntryType::Regular, &manifest));
    entries.extend(
        files
            .iter()
            .map(|(path, bytes)| (path.as_bytes(), EntryType::Regular, *bytes)),
    );
    ValidatedSolutionPack::from_tar_zst(&custom_archive(&entries)).unwrap();
}

#[test]
fn published_sample_schema_allows_only_the_exact_native_time_object() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-sample-data-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&schema).unwrap();
    let sample = |value: Value| {
        json!({
            "format_version":1,
            "kind":"solution_pack_sample_data",
            "classification":"synthetic",
            "entities":[{
                "key":"sample-entities/item",
                "blueprint":"blueprints/product",
                "facts":[{"attribute":"blueprints/product/attributes/available_at","value":value}],
                "relationships":[]
            }]
        })
    };
    assert!(
        catalog_validation::validate_json_schema(
            &schema,
            &sample(json!({"time":"12:34:56","time_zone":"UTC"})),
        )
        .unwrap()
        .is_empty()
    );
    for invalid in [
        json!({"time":"12:34:56"}),
        json!({"time":"12:34:56","time_zone":"UTC","extra":true}),
        json!({"copied":true}),
    ] {
        assert!(
            !catalog_validation::validate_json_schema(&schema, &sample(invalid))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn published_manifest_schema_accepts_the_v1_fixture_and_is_strict() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-manifest-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&schema).unwrap();
    assert!(
        catalog_validation::validate_json_schema(&schema, &manifest_value())
            .unwrap()
            .is_empty()
    );
    for contexts in [
        json!([{"key": "contexts/web"}]),
        json!([{"key": "contexts/web", "path": "contexts/web.toml", "required": true, "sha256": "0".repeat(64)}]),
    ] {
        let mut with_contexts = manifest_value();
        with_contexts["resources"]["contexts"] = contexts;
        assert!(
            !catalog_validation::validate_json_schema(&schema, &with_contexts)
                .unwrap()
                .is_empty()
        );
    }
    let mut with_navigation = manifest_value();
    with_navigation["resources"]["workspace_settings"] = json!([{
        "key": "workspace/explore-navigation",
        "path": "workspace/explore-navigation.json",
        "required": true,
        "sha256": "0".repeat(64),
    }]);
    assert!(
        catalog_validation::validate_json_schema(&schema, &with_navigation)
            .unwrap()
            .is_empty()
    );
    let mut with_asset = manifest_value();
    with_asset["resources"]["presentation_assets"] = json!([{
        "key": "assets/brand-logo",
        "path": "assets/brand-logo.svg",
        "required": true,
        "purpose": "logo",
        "media_type": "image/svg+xml",
        "sha256": "0".repeat(64),
    }]);
    assert!(
        catalog_validation::validate_json_schema(&schema, &with_asset)
            .unwrap()
            .is_empty()
    );
    with_asset["resources"]["presentation_assets"][0]["media_type"] = json!("image/jpeg");
    assert!(
        !catalog_validation::validate_json_schema(&schema, &with_asset)
            .unwrap()
            .is_empty()
    );
    let mut duplicate_navigation = with_navigation.clone();
    duplicate_navigation["resources"]["workspace_settings"] = json!([
        with_navigation["resources"]["workspace_settings"][0].clone(),
        with_navigation["resources"]["workspace_settings"][0].clone(),
    ]);
    assert!(
        !catalog_validation::validate_json_schema(&schema, &duplicate_navigation)
            .unwrap()
            .is_empty()
    );
    let checklist_schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-setup-checklist-v1.schema.json"
    ))
    .unwrap();
    let checks_schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-checks-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&checklist_schema).unwrap();
    catalog_validation::validate_json_schema_definition(&checks_schema).unwrap();
    let checklist = json!({"format_version":1,"items":[{"key":"checklist/publish","title":"Publish","markdown":"Publish it.","check":"checks/published"}]});
    let checks = json!({"format_version":1,"checks":[{"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}]});
    assert!(
        catalog_validation::validate_json_schema(&checklist_schema, &checklist)
            .unwrap()
            .is_empty()
    );
    assert!(
        catalog_validation::validate_json_schema(&checks_schema, &checks)
            .unwrap()
            .is_empty()
    );
    let duplicate_checks = json!({"format_version":1,"checks":[
        {"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}},
        {"key":"checks/published","title":"Published","predicate":{"type":"blueprint_published","blueprint":"blueprints/product"}}
    ]});
    assert!(
        !catalog_validation::validate_json_schema(&checks_schema, &duplicate_checks)
            .unwrap()
            .is_empty()
    );
    let contribution_256 = format!("a:{}", "b".repeat(254));
    let contribution_257 = format!("a:{}", "b".repeat(255));
    for (contribution, valid) in [(contribution_256, true), (contribution_257, false)] {
        assert_eq!(valid_contribution_key(&contribution), valid);
        let value = json!({"format_version":1,"checks":[{"key":"checks/layout","title":"Layout","predicate":{"type":"workspace_extension_layout_placement_present","contribution":contribution}}]});
        assert_eq!(
            catalog_validation::validate_json_schema(&checks_schema, &value)
                .unwrap()
                .is_empty(),
            valid
        );
    }
    let mut unknown_check = checks;
    unknown_check["checks"][0]["predicate"]["query"] = json!("select 1");
    assert!(
        !catalog_validation::validate_json_schema(&checks_schema, &unknown_check)
            .unwrap()
            .is_empty()
    );

    let duplicate_checklist = json!({"format_version":1,"items":[
        {"key":"checklist/publish","title":"Publish","markdown":"Publish it."},
        {"key":"checklist/publish","title":"Publish","markdown":"Publish it."}
    ]});
    assert!(
        !catalog_validation::validate_json_schema(&checklist_schema, &duplicate_checklist)
            .unwrap()
            .is_empty()
    );
    let mut manifest_with_guidance = manifest_value();
    manifest_with_guidance["documentation"] =
        json!({"readme":{"path":"README.md","sha256":"0".repeat(64)}});
    assert!(
        catalog_validation::validate_json_schema(&schema, &manifest_with_guidance)
            .unwrap()
            .is_empty()
    );
    for unsafe_path in [
        "/README.md",
        "../README.md",
        "docs/../README.md",
        "C:README.md",
        "docs\\README.md",
        "docs/README\n.md",
        "docs/README\u{85}.md",
    ] {
        manifest_with_guidance["documentation"]["readme"]["path"] = json!(unsafe_path);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &manifest_with_guidance)
                .unwrap()
                .is_empty(),
            "schema accepted unsafe path {unsafe_path}"
        );
    }

    let navigation_schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-explore-navigation-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&navigation_schema).unwrap();
    let navigation: Value = serde_json::from_slice(EXPLORE_NAVIGATION).unwrap();
    assert!(
        catalog_validation::validate_json_schema(&navigation_schema, &navigation)
            .unwrap()
            .is_empty()
    );
    let mut unknown_navigation = navigation;
    unknown_navigation["unknown"] = json!(true);
    assert!(
        !catalog_validation::validate_json_schema(&navigation_schema, &unknown_navigation)
            .unwrap()
            .is_empty()
    );

    let lexicon_schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-lexicon-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&lexicon_schema).unwrap();
    let lexicon: Value = serde_json::from_slice(LEXICON).unwrap();
    assert!(
        catalog_validation::validate_json_schema(&lexicon_schema, &lexicon)
            .unwrap()
            .is_empty()
    );
    let mut with_lexicon = manifest_value();
    with_lexicon["resources"]["workspace_settings"] = json!([{
        "key": "workspace/lexicon",
        "path": "workspace/lexicon.json",
        "required": false,
        "sha256": "0".repeat(64),
    }]);
    assert!(
        catalog_validation::validate_json_schema(&schema, &with_lexicon)
            .unwrap()
            .is_empty()
    );

    let layout_schema: Value = serde_json::from_str(include_str!(
        "../../../../contracts/solution-pack-extension-layout-v1.schema.json"
    ))
    .unwrap();
    catalog_validation::validate_json_schema_definition(&layout_schema).unwrap();
    let layout_entry = json!({
        "contribution": "acme.shop:nav",
        "outlet": "navigation",
        "required": true,
    });
    let valid_layout = json!({
        "format_version": 1,
        "kind": "extension_layout",
        "entries": [layout_entry.clone()],
    });
    assert!(
        catalog_validation::validate_json_schema(&layout_schema, &valid_layout)
            .unwrap()
            .is_empty()
    );
    let promoted_layout = json!({
        "format_version": 1,
        "kind": "extension_layout",
        "entries": [{
            "contribution": "acme.shop:nav",
            "outlet": "navigation",
            "hidden": false,
            "promoted": true,
            "required": true,
        }],
    });
    assert!(
        catalog_validation::validate_json_schema(&layout_schema, &promoted_layout)
            .unwrap()
            .is_empty()
    );
    for invalid_layout in [
        json!({
            "format_version": 1,
            "kind": "extension_layout",
            "entries": [layout_entry.clone(), layout_entry.clone()],
        }),
        json!({
            "format_version": 1,
            "kind": "extension_layout",
            "entries": [{
                "contribution": "acme.shop:nav",
                "outlet": "navigation",
                "hidden": true,
                "promoted": true,
                "required": true,
            }],
        }),
    ] {
        assert!(
            !catalog_validation::validate_json_schema(&layout_schema, &invalid_layout)
                .unwrap()
                .is_empty()
        );
    }

    let mut unknown = manifest_value();
    unknown["unknown"] = json!(true);
    assert!(
        !catalog_validation::validate_json_schema(&schema, &unknown)
            .unwrap()
            .is_empty()
    );

    let mut with_extension = manifest_value();
    with_extension["extensions"] = json!([{
        "key":"extensions/shopify",
        "id":"acme.shopify",
        "version":">=2.1.0 <3.0.0",
        "required":true,
        "configuration_template": {
            "path":"extensions/shopify.json",
            "sha256":"0".repeat(64)
        }
    }]);
    assert!(
        catalog_validation::validate_json_schema(&schema, &with_extension)
            .unwrap()
            .is_empty()
    );
    with_extension["extensions"][0]["unknown"] = json!(true);
    assert!(
        !catalog_validation::validate_json_schema(&schema, &with_extension)
            .unwrap()
            .is_empty()
    );

    for (pointer, invalid_value, runtime_error) in [
        ("/extensions/0/version", "not-a-range", "SemVer range"),
        (
            "/extensions/0/configuration_template/path",
            "extensions/../template.json",
            "configuration template path is invalid",
        ),
        (
            "/extensions/0/key",
            "extensions/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "extension requirement key",
        ),
    ] {
        let mut invalid = manifest_value();
        invalid["extensions"] = json!([{
            "key":"extensions/shopify",
            "id":"acme.shopify",
            "version":"^1.0",
            "required":true,
            "configuration_template": {
                "path":"extensions/shopify.json",
                "sha256":"0".repeat(64)
            }
        }]);
        *invalid.pointer_mut(pointer).unwrap() = json!(invalid_value);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &invalid)
                .unwrap()
                .is_empty(),
            "schema accepted invalid extension value '{invalid_value}'"
        );
        assert_invalid(&archive(&invalid, &valid_files()), runtime_error);
    }

    for (field, invalid_value, runtime_error) in [
        ("id", "attricat.foo-", "reverse-DNS-style"),
        ("version", "1.0.0-01", "version must be SemVer"),
    ] {
        let mut invalid = manifest_value();
        invalid[field] = json!(invalid_value);
        assert!(
            !catalog_validation::validate_json_schema(&schema, &invalid)
                .unwrap()
                .is_empty(),
            "schema accepted invalid {field} '{invalid_value}'"
        );
        assert_invalid(&archive(&invalid, &valid_files()), runtime_error);
    }
}

#[test]
fn rejects_unknown_fields_at_each_manifest_contract_level() {
    for pointer in ["", "/catalog", "/resources", "/resources/blueprints/0"] {
        let mut manifest = manifest_value();
        manifest
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert_invalid(&archive(&manifest, &valid_files()), "strict v1 manifest");
    }
}

#[test]
fn rejects_invalid_container_manifest_and_package_confusion() {
    assert!(ValidatedSolutionPack::from_tar_zst(b"not zstd").is_err());

    let only_extension_manifest =
        custom_archive(&[(b"manifest.json", EntryType::Regular, br#"{}"#)]);
    assert_invalid(&only_extension_manifest, "must contain solution-pack.json");

    let nested = custom_archive(&[(b"nested/solution-pack.json", EntryType::Regular, br#"{}"#)]);
    assert_invalid(&nested, "must contain solution-pack.json");

    let manifest = serde_json::to_vec(&manifest_value()).unwrap();
    let duplicate = custom_archive(&[
        (b"solution-pack.json", EntryType::Regular, &manifest),
        (b"solution-pack.json", EntryType::Regular, &manifest),
    ]);
    assert_invalid(&duplicate, "duplicate entry paths");
}

#[test]
fn rejects_unsafe_non_utf8_and_non_file_entries() {
    let manifest = serde_json::to_vec(&manifest_value()).unwrap();
    for path in [
        b"../solution-pack.json".as_slice(),
        b"/solution-pack.json",
        b"C:/solution-pack.json",
        b"dir\\solution-pack.json",
        b"dir/solution-pack\n.json",
        b"dir/solution-pack\x7f.json",
    ] {
        let archive = custom_archive(&[(path, EntryType::Regular, &manifest)]);
        assert_invalid(&archive, "unsafe entry path");
    }
    let archive = custom_archive(&[(&[0xff], EntryType::Regular, b"x")]);
    assert_invalid(&archive, "paths must be UTF-8");

    for kind in [EntryType::Symlink, EntryType::Link, EntryType::Char] {
        let archive = custom_archive(&[(b"solution-pack.json", kind, b"")]);
        assert_invalid(&archive, "non-file entry");
    }
}

#[test]
fn enforces_compressed_expanded_per_file_manifest_and_entry_limits() {
    let valid = archive(&manifest_value(), &valid_files());
    let limits = ArchiveLimits {
        compressed_bytes: valid.len() - 1,
        ..ArchiveLimits::default()
    };
    assert!(ValidatedSolutionPack::from_tar_zst_with_limits(&valid, limits).is_err());

    let limits = ArchiveLimits {
        file_bytes: PRODUCT_BLUEPRINT.len() - 1,
        ..ArchiveLimits::default()
    };
    assert_invalid_with_limits(&valid, limits, "per-file size limit");

    let limits = ArchiveLimits {
        manifest_bytes: 10,
        ..ArchiveLimits::default()
    };
    assert_invalid_with_limits(&valid, limits, "manifest exceeds");

    let limits = ArchiveLimits {
        entries: 2,
        ..ArchiveLimits::default()
    };
    assert_invalid_with_limits(&valid, limits, "too many entries");

    let mut tar_bytes = Vec::new();
    {
        let mut builder = Builder::new(&mut tar_bytes);
        append_file(&mut builder, SOLUTION_PACK_MANIFEST_PATH, b"{}");
        builder.finish().unwrap();
    }
    tar_bytes.write_all(&vec![0; 4096]).unwrap();
    let trailing = zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap();
    let limits = ArchiveLimits {
        expanded_bytes: 2048,
        file_bytes: 1024,
        ..ArchiveLimits::default()
    };
    assert_invalid_with_limits(&trailing, limits, "expanded size limit");
}

#[test]
fn rejects_non_zero_data_after_the_tar_end_marker() {
    let valid = archive(&manifest_value(), &valid_files());
    let mut tar_bytes = zstd::stream::decode_all(Cursor::new(valid)).unwrap();
    tar_bytes.extend_from_slice(b"hidden payload");
    let archive = zstd::stream::encode_all(Cursor::new(tar_bytes), 0).unwrap();

    assert_invalid(&archive, "data after the tar end marker");
}

fn assert_invalid_with_limits(archive: &[u8], limits: ArchiveLimits, expected: &str) {
    let error = ValidatedSolutionPack::from_tar_zst_with_limits(archive, limits).unwrap_err();
    assert!(
        error.to_string().contains(expected),
        "expected '{expected}' in '{error}'"
    );
}

#[test]
fn rejects_missing_undeclared_and_digest_mismatched_files() {
    let files = valid_files();
    assert_invalid(&archive(&manifest_value(), &files[..1]), "is missing");

    let mut extra = files.clone();
    extra.push(("README.md", b"undeclared"));
    assert_invalid(&archive(&manifest_value(), &extra), "is not declared");

    let mut changed = files;
    changed[0] = (changed[0].0, b"changed");
    assert_invalid(&archive(&manifest_value(), &changed), "sha256 digest");
}

#[test]
fn rejects_invalid_manifest_versions_ids_keys_paths_and_digests() {
    let cases = [
        ("/manifest_version", json!(2), "manifest_version 2"),
        ("/id", json!("Ecommerce"), "reverse-DNS"),
        ("/version", json!("latest"), "SemVer"),
        ("/catalog/host_api", json!(">=2"), "incompatible"),
        (
            "/resources/blueprints/0/key",
            json!("contexts/product"),
            "blueprints/ namespace",
        ),
        (
            "/resources/blueprints/0/path",
            json!("contexts/product.toml"),
            "resource path",
        ),
        (
            "/resources/blueprints/0/sha256",
            json!("ABC"),
            "64 lowercase",
        ),
    ];
    for (pointer, replacement, expected) in cases {
        let mut manifest = manifest_value();
        *manifest.pointer_mut(pointer).unwrap() = replacement;
        assert_invalid(&archive(&manifest, &valid_files()), expected);
    }

    let mut duplicate_key = manifest_value();
    duplicate_key["resources"]["blueprints"][1]["key"] = json!("blueprints/product");
    assert_invalid(
        &archive(&duplicate_key, &valid_files()),
        "duplicate resource key",
    );

    let mut duplicate_path = manifest_value();
    duplicate_path["resources"]["blueprints"][1]["path"] = json!("blueprints/product.toml");
    assert_invalid(
        &archive(&duplicate_path, &valid_files()),
        "duplicate resource path",
    );
}

#[test]
fn validates_blueprint_content_and_logical_reference_closure() {
    let mut files = valid_files();
    let invalid = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
unknown = true
[[attributes]]
code = "name"
value_type = "string"
"#;
    files[0] = (files[0].0, invalid);
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(invalid));
    assert_invalid(
        &archive(&manifest, &files),
        "blueprint 'blueprints/product' is invalid",
    );

    let missing_target = CATEGORY_BLUEPRINT.replace_ascii(b"product", b"missing");
    let mut files = valid_files();
    files[1] = (files[1].0, &missing_target);
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&missing_target));
    assert_invalid(
        &archive(&manifest, &files),
        "undeclared relationship target",
    );
}

#[test]
fn rejects_blueprints_that_fail_ordinary_compiler_validation() {
    let missing_dropdown = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
[[attributes]]
code = "name"
value_type = "string"
"#;
    assert_blueprint_error(missing_dropdown, "must define views.dropdown_option");

    let unknown_view_field =
        PRODUCT_BLUEPRINT.replace_ascii(b"fields = [\"name\"]", b"fields = [\"missing\"]");
    assert_blueprint_error(&unknown_view_field, "unknown attribute 'missing'");

    let malformed_component = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.table]
type = "table"
fields = ["name"]
component = { id = "INVALID", version = 1 }
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    assert_blueprint_error(&malformed_component, "component id");

    let product = br#"
format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{ alias = "base", key = "blueprints/category" }]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
from = "base.name"
"#;
    let mixin = br#"
format_version = 1
code = "category"
name = "Category fields"
kind = "mixin"
[[attributes]]
code = "title"
value_type = "string"
"#;
    let files = vec![
        ("blueprints/product.toml", product.as_slice()),
        ("blueprints/category.toml", mixin.as_slice()),
    ];
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(product));
    manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(mixin));
    assert_invalid(&archive(&manifest, &files), "not exposed by include 'base'");
}

#[test]
fn rejects_manifests_that_exceed_resource_count_limits() {
    let mut manifest: SolutionPackManifest = serde_json::from_value(manifest_value()).unwrap();
    let blueprint = manifest.resources.blueprints[0].clone();
    manifest.resources.blueprints = vec![blueprint; MAX_SOLUTION_PACK_BLUEPRINTS + 1];
    assert!(
        validate_manifest(&manifest)
            .unwrap_err()
            .to_string()
            .contains("too many blueprints")
    );

    let mut manifest: SolutionPackManifest = serde_json::from_value(manifest_value()).unwrap();
    let requirement: SolutionPackExtensionRequirement = serde_json::from_value(json!({
        "key":"extensions/example",
        "id":"acme.example",
        "version":"^1.0",
        "required":false
    }))
    .unwrap();
    manifest.extensions = vec![requirement; MAX_SOLUTION_PACK_EXTENSION_REQUIREMENTS + 1];
    assert!(
        validate_manifest(&manifest)
            .unwrap_err()
            .to_string()
            .contains("too many extension requirements")
    );
}

#[test]
fn rejects_blueprints_that_exceed_structural_complexity_limits() {
    let includes = (0..=MAX_SOLUTION_PACK_BLUEPRINT_INCLUDES)
        .map(|index| format!("{{ alias = \"m{index}\", key = \"blueprints/category\" }}"))
        .collect::<Vec<_>>()
        .join(", ");
    let excessive_includes = format!(
        r#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{includes}]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
    )
    .into_bytes();
    assert_blueprint_error(&excessive_includes, "exceeds the include limit");

    let mut excessive_attributes = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["field_0"]
"#
    .to_vec();
    for index in 0..=MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
        excessive_attributes.extend_from_slice(
            format!("[[attributes]]\ncode = \"field_{index}\"\nvalue_type = \"string\"\n")
                .as_bytes(),
        );
    }
    assert_blueprint_error(&excessive_attributes, "exceeds the attribute limit");

    let includes = (0..5)
        .map(|index| format!("{{ alias = \"m{index}\", key = \"blueprints/category\" }}"))
        .collect::<Vec<_>>()
        .join(", ");
    let product = format!(
        r#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{includes}]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
    )
    .into_bytes();
    let mut mixin = br#"format_version = 1
code = "category"
name = "Category fields"
kind = "mixin"
"#
    .to_vec();
    for index in 0..MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
        mixin.extend_from_slice(
            format!("[[attributes]]\ncode = \"field_{index}\"\nvalue_type = \"string\"\n")
                .as_bytes(),
        );
    }
    assert_blueprints_error(
        &product,
        &mixin,
        "exceeds the resolved include attribute limit",
    );

    let mut owned_files = Vec::new();
    let mut resources = Vec::new();
    for blueprint_index in 0..17 {
        let code = format!("mixin_{blueprint_index}");
        let path = format!("blueprints/{code}.toml");
        let mut source = format!(
            "format_version = 1\ncode = \"{code}\"\nname = \"Mixin {blueprint_index}\"\nkind = \"mixin\"\n"
        )
        .into_bytes();
        for attribute_index in 0..MAX_SOLUTION_PACK_BLUEPRINT_ATTRIBUTES {
            source.extend_from_slice(
                format!(
                    "[[attributes]]\ncode = \"field_{attribute_index}\"\nvalue_type = \"string\"\n"
                )
                .as_bytes(),
            );
        }
        resources.push(resource(&format!("blueprints/{code}"), &path, &source));
        owned_files.push((path, source));
    }
    let file_refs = owned_files
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_slice()))
        .collect::<Vec<_>>();
    let manifest = json!({
        "manifest_version": 1,
        "id": "attricat.complex",
        "name": "Complex",
        "version": "1.0.0",
        "description": "Complex pack",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": resources}
    });
    assert_invalid(
        &archive(&manifest, &file_refs),
        "complexity exceeds the total limit",
    );
}

#[test]
fn validates_incoming_relationship_selectors_across_blueprints() {
    let missing =
        CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"missing\"");
    assert_blueprints_error(PRODUCT_BLUEPRINT, &missing, "has no field 'missing'");

    let scalar = CATEGORY_BLUEPRINT.replace_ascii(b"field = \"categories\"", b"field = \"name\"");
    assert_blueprints_error(
        PRODUCT_BLUEPRINT,
        &scalar,
        "field 'name' must be a relationship",
    );

    let wrong_target = PRODUCT_BLUEPRINT.replace_ascii(
        b"target_blueprint = \"blueprints/category\"",
        b"target_blueprint = \"blueprints/product\"",
    );
    assert_blueprints_error(
        &wrong_target,
        CATEGORY_BLUEPRINT,
        "field 'categories' must target 'category'",
    );
}

fn assert_blueprints_error(product: &[u8], category: &[u8], expected: &str) {
    let files = vec![
        ("blueprints/product.toml", product),
        ("blueprints/category.toml", category),
    ];
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(product));
    manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(category));
    assert_invalid(&archive(&manifest, &files), expected);
}

fn assert_blueprint_error(blueprint: &[u8], expected: &str) {
    let mut files = valid_files();
    files[0] = (files[0].0, blueprint);
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(blueprint));
    assert_invalid(&archive(&manifest, &files), expected);
}

#[test]
fn requires_portable_include_keys_instead_of_native_revisions() {
    let native_include = PRODUCT_BLUEPRINT.replace_ascii(
        b"kind = \"entity\"",
        b"kind = \"entity\"\nincludes = [{ alias = \"base\", code = \"category\", version = 1 }]",
    );
    assert_blueprint_error(&native_include, "native code or revision fields");

    let mixed_include = PRODUCT_BLUEPRINT.replace_ascii(
        b"kind = \"entity\"",
        b"kind = \"entity\"\nincludes = [{ alias = \"base\", key = \"blueprints/category\", version = 1 }]",
    );
    assert_blueprint_error(&mixed_include, "native code or revision fields");

    let portable = blueprint_with_include("product", "category");
    let mixin = blueprint_with_include_without_dependencies("category");
    let files = vec![
        ("blueprints/product.toml", portable.as_slice()),
        ("blueprints/category.toml", mixin.as_slice()),
    ];
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&portable));
    manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&mixin));
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let include = &pack.blueprint("blueprints/product").unwrap().includes()[0];
    assert_eq!(include.alias(), "base");
    assert_eq!(include.key(), "blueprints/category");
}

#[test]
fn rejects_blueprint_include_cycles_and_extension_metadata() {
    let first = blueprint_with_include("product", "category");
    let second = blueprint_with_include("category", "product");
    let files = vec![
        ("blueprints/product.toml", first.as_slice()),
        ("blueprints/category.toml", second.as_slice()),
    ];
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&first));
    manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(&second));
    assert_invalid(
        &archive(&manifest, &files),
        "include references contain a cycle",
    );

    let extension_metadata = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            b"\n[extensions.\"acme.test\"]\nenabled = true\n"
                .iter()
                .copied(),
        )
        .collect::<Vec<_>>();
    let mut files = valid_files();
    files[0] = (files[0].0, &extension_metadata);
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&extension_metadata));
    assert_invalid(
        &archive(&manifest, &files),
        "cannot contain extension metadata",
    );
}

fn blueprint_with_include(code: &str, include: &str) -> Vec<u8> {
    format!(
        r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "mixin"
includes = [{{ alias = "base", key = "blueprints/{include}" }}]
[[attributes]]
code = "name"
value_type = "string"
"#
    )
    .into_bytes()
}

fn blueprint_with_include_without_dependencies(code: &str) -> Vec<u8> {
    format!(
        r#"format_version = 1
code = "{code}"
name = "{code}"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#
    )
    .into_bytes()
}

#[test]
fn planner_rewrites_portable_references_and_orders_dependencies() {
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files())).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();

    assert!(plan.ready);
    assert_eq!(
        plan.actions
            .iter()
            .map(|action| action.logical_key.as_str())
            .collect::<Vec<_>>(),
        ["blueprints/category", "blueprints/product"]
    );
    assert!(
        plan.actions
            .iter()
            .all(|action| action.action == PlanActionKind::Create)
    );
    let product = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    let definition = product.normalized_payload.as_ref().unwrap()["definition"]
        .as_str()
        .unwrap();
    assert!(definition.contains("code = \"ecom_product\""));
    assert!(definition.contains("target_blueprint = \"ecom_category\""));
    assert_eq!(
        product.normalized_payload.as_ref().unwrap()["publication"],
        "publish"
    );
    let category_definition = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/category")
        .unwrap()
        .normalized_payload
        .as_ref()
        .unwrap()["definition"]
        .as_str()
        .unwrap();
    assert!(category_definition.contains("source_blueprint = \"ecom_product\""));
    assert!(category_definition.contains("target_blueprint = \"ecom_product\""));
}

#[test]
fn planner_rewrites_relationship_target_lists() {
    let product = PRODUCT_BLUEPRINT.replace_ascii(
        b"target_blueprint = \"blueprints/category\"",
        b"target_blueprints = [\"blueprints/category\", \"blueprints/product\"]",
    );
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&product));
    let mut files = valid_files();
    files[0] = (files[0].0, &product);
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(plan.ready);
    let definition = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap()
        .normalized_payload
        .as_ref()
        .unwrap()["definition"]
        .as_str()
        .unwrap();
    assert!(
        definition.contains("target_blueprints = [\"ecom_category\", \"ecom_product\"]"),
        "{definition}"
    );
}

#[test]
fn planner_rewrites_portable_includes_to_mapped_revision_one() {
    let entity = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
includes = [{ alias = "base", key = "blueprints/base" }]
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
from = "base.name"
"#;
    let mixin = br#"format_version = 1
code = "base"
name = "Base"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#;
    let manifest = json!({
        "manifest_version": 1,
        "id": "attricat.includes",
        "name": "Includes",
        "version": "1.0.0",
        "description": "Include rewrite",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": [
            resource("blueprints/product", "blueprints/product.toml", entity),
            resource("blueprints/base", "blueprints/base.toml", mixin)
        ]}
    });
    let files = [
        ("blueprints/product.toml", entity.as_slice()),
        ("blueprints/base.toml", mixin.as_slice()),
    ];
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "mapped",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(plan.actions[0].logical_key, "blueprints/base");
    let definition = plan.actions[1].normalized_payload.as_ref().unwrap()["definition"]
        .as_str()
        .unwrap();
    assert!(definition.contains("code = \"mapped_base\""));
    assert!(definition.contains("version = 1"));
    assert!(!definition.contains("key = \"blueprints/base\""));

    let base_definition = plan.actions[0].normalized_payload.as_ref().unwrap()["definition"]
        .as_str()
        .unwrap();
    let existing_id = uuid::Uuid::from_u128(42);
    let reused = build_solution_pack_plan(
        &pack,
        "mapped",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned(), "mapped_base".to_owned()]),
            existing_presentation_assets: BTreeMap::new(),
            existing_blueprints: BTreeMap::from([(
                "blueprints/base".to_owned(),
                ExistingBlueprintSnapshot {
                    id: existing_id,
                    code: "mapped_base".to_owned(),
                    version: 7,
                    kind: "mixin".to_owned(),
                    canonical_definition_hash: catalog_blueprint::raw_hash(base_definition),
                    definition_hash: catalog_blueprint::raw_hash(base_definition),
                },
            )]),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(reused.ready);
    assert_eq!(reused.actions[0].action, PlanActionKind::Map);
    assert_eq!(reused.actions[0].reason_code, "exact_blueprint_match");
    assert!(reused.actions[0].normalized_payload.is_none());
    assert_eq!(reused.mappings[0].mapping_kind, MappingKind::Existing);
    assert_eq!(reused.mappings[0].target_id, existing_id);
    assert_eq!(reused.mappings[0].target_version, Some(7));
    let dependent = reused.actions[1].normalized_payload.as_ref().unwrap()["definition"]
        .as_str()
        .unwrap();
    assert!(dependent.contains("version = 7"));

    let incompatible = build_solution_pack_plan(
        &pack,
        "mapped",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["mapped_base".to_owned()]),
            existing_presentation_assets: BTreeMap::new(),
            existing_blueprints: BTreeMap::from([(
                "blueprints/base".to_owned(),
                ExistingBlueprintSnapshot {
                    id: existing_id,
                    code: "mapped_base".to_owned(),
                    version: 7,
                    kind: "mixin".to_owned(),
                    canonical_definition_hash: "0".repeat(64),
                    definition_hash: "0".repeat(64),
                },
            )]),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(!incompatible.ready);
    assert_eq!(incompatible.actions[0].action, PlanActionKind::Conflict);
    assert_eq!(
        incompatible.actions[0].reason_code,
        "existing_blueprint_incompatible"
    );
}

#[test]
fn rejects_invalid_explicit_blueprint_mapping_requests() {
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files())).unwrap();
    assert!(
        validate_blueprint_mapping_requests(
            &pack,
            &[
                BlueprintMappingRequest {
                    key: "blueprints/product".to_owned(),
                    code: "shared".to_owned(),
                },
                BlueprintMappingRequest {
                    key: "blueprints/product".to_owned(),
                    code: "other".to_owned(),
                },
            ],
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate")
    );
    assert!(
        validate_blueprint_mapping_requests(
            &pack,
            &vec![
                BlueprintMappingRequest {
                    key: "blueprints/product".to_owned(),
                    code: "shared".to_owned(),
                };
                MAX_SOLUTION_PACK_BLUEPRINTS + 1
            ],
        )
        .unwrap_err()
        .to_string()
        .contains("too many")
    );
    for request in [
        BlueprintMappingRequest {
            key: "blueprints/unknown".to_owned(),
            code: "shared".to_owned(),
        },
        BlueprintMappingRequest {
            key: "blueprints/product".to_owned(),
            code: "Unsafe-code".to_owned(),
        },
    ] {
        assert!(validate_blueprint_mapping_requests(&pack, &[request]).is_err());
    }
}

#[test]
fn fully_mapped_resource_does_not_validate_an_unused_generated_code() {
    let suffix = "a".repeat(117);
    let key = format!("blueprints/{suffix}");
    assert_eq!(key.len(), MAX_IDENTIFIER_BYTES);
    let blueprint = format!(
        r#"format_version = 1
code = "{suffix}"
name = "Portable"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
    )
    .into_bytes();
    let manifest = json!({
        "manifest_version": 1,
        "id": "attricat.long-key",
        "name": "Long key",
        "version": "1.0.0",
        "description": "Mapped long logical key",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": [resource(&key, "blueprints/long.toml", &blueprint)]}
    });
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(
        &manifest,
        &[("blueprints/long.toml", blueprint.as_slice())],
    ))
    .unwrap();
    let mut canonical: toml::Value =
        toml::from_str(std::str::from_utf8(&blueprint).unwrap()).unwrap();
    canonical
        .as_table_mut()
        .unwrap()
        .insert("code".to_owned(), toml::Value::String("shared".to_owned()));
    let definition = toml::to_string(&canonical).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "a2345678901234567890123456789012",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["shared".to_owned()]),
            existing_presentation_assets: BTreeMap::new(),
            existing_blueprints: BTreeMap::from([(
                key.clone(),
                ExistingBlueprintSnapshot {
                    id: uuid::Uuid::from_u128(102),
                    code: "shared".to_owned(),
                    version: 3,
                    kind: "entity".to_owned(),
                    canonical_definition_hash: catalog_blueprint::raw_hash(&definition),
                    definition_hash: catalog_blueprint::raw_hash(&definition),
                },
            )]),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::from(["shared".to_owned()]),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(plan.ready);
    assert_eq!(plan.actions[0].action, PlanActionKind::Map);
}

#[test]
fn planner_rejects_unsafe_prefixes_and_reports_collisions() {
    for prefix in [
        "",
        "Bad",
        "bad-",
        "bad_",
        "a23456789012345678901234567890123",
    ] {
        assert!(validate_plan_prefix(prefix).is_err(), "accepted {prefix:?}");
    }
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files())).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned(), "ecom_product".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    assert!(!plan.ready);
    let product = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    assert_eq!(product.action, PlanActionKind::Conflict);
    assert_eq!(product.reason_code, "target_code_exists");
}

#[test]
fn planner_blocks_required_resources_when_optional_dependencies_are_skipped() {
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][1]["required"] = json!(false);
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &valid_files())).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["default".to_owned()]),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: serde_json::json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    let category = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/category")
        .unwrap();
    let product = plan
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    assert_eq!(
        (category.action, category.reason_code),
        (PlanActionKind::Skip, "optional_not_selected")
    );
    assert_eq!(
        (product.action, product.reason_code),
        (PlanActionKind::Blocked, "dependency_not_creatable")
    );
    assert!(!plan.ready);

    let required_pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files())).unwrap();
    let baseline = build_solution_pack_plan(
        &required_pack,
        "ecom",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::new(),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        },
    )
    .unwrap();
    let product_definition = baseline
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap()
        .normalized_payload
        .as_ref()
        .unwrap()["definition"]
        .as_str()
        .unwrap();
    let mapped = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Draft,
        &PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::from(["ecom_product".to_owned()]),
            existing_presentation_assets: BTreeMap::new(),
            existing_blueprints: BTreeMap::from([(
                "blueprints/product".to_owned(),
                ExistingBlueprintSnapshot {
                    id: uuid::Uuid::from_u128(99),
                    code: "ecom_product".to_owned(),
                    version: 1,
                    kind: "entity".to_owned(),
                    canonical_definition_hash: catalog_blueprint::raw_hash(product_definition),
                    definition_hash: catalog_blueprint::raw_hash(product_definition),
                },
            )]),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::from(["ecom_product".to_owned()]),
            seed: Default::default(),
        },
    )
    .unwrap();
    let mapped_product = mapped
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    assert_eq!(
        (mapped_product.action, mapped_product.reason_code),
        (PlanActionKind::Blocked, "dependency_not_creatable")
    );
    assert!(!mapped.ready);
}

#[test]
fn mapped_blueprints_are_blocked_by_skipped_include_relationship_and_view_dependencies() {
    let cases = [
        (
            "include",
            br#"format_version = 1
code = "main"
name = "Main"
kind = "mixin"
includes = [{ alias = "base", key = "blueprints/dep" }]
[[attributes]]
code = "name"
from = "base.name"
"#
            .as_slice(),
            br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "mixin"
[[attributes]]
code = "name"
value_type = "string"
"#
            .as_slice(),
            "mixin",
        ),
        (
            "relationship",
            br#"format_version = 1
code = "main"
name = "Main"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "dep"
value_type = "relationship"
target_blueprint = "blueprints/dep"
"#
            .as_slice(),
            br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#
            .as_slice(),
            "entity",
        ),
        (
            "view",
            br#"format_version = 1
code = "main"
name = "Main"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.detail]
type = "stack"
[[views.detail.children]]
type = "tabs"
[[views.detail.children.tabs]]
label = "Relationships"
[[views.detail.children.tabs.children]]
type = "grid"
[[views.detail.children.tabs.children.children]]
type = "incoming_relationship_list"
label = "Dependencies"
relationships = [{ source_blueprint = "blueprints/dep", field = "main_ref" }]
page_size = 10
[[attributes]]
code = "name"
value_type = "string"
"#
            .as_slice(),
            br#"format_version = 1
code = "dep"
name = "Dependency"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "main_ref"
value_type = "relationship"
target_blueprint = "blueprints/main"
"#
            .as_slice(),
            "entity",
        ),
    ];
    for (case, main, dep, main_kind) in cases {
        let manifest_for = |dep_required| {
            let mut dep_resource = resource("blueprints/dep", "blueprints/dep.toml", dep);
            dep_resource["required"] = json!(dep_required);
            json!({
                "manifest_version": 1,
                "id": format!("attricat.dependency-{case}"),
                "name": "Dependency",
                "version": "1.0.0",
                "description": "Mapped dependency closure",
                "catalog": {"host_api": "^1.0"},
                "resources": {"blueprints": [
                    resource("blueprints/main", "blueprints/main.toml", main),
                    dep_resource
                ]}
            })
        };
        let files = [("blueprints/main.toml", main), ("blueprints/dep.toml", dep)];
        let required_pack =
            ValidatedSolutionPack::from_tar_zst(&archive(&manifest_for(true), &files))
                .unwrap_or_else(|error| panic!("{case}: {error}"));
        let empty_workspace = PlanningWorkspaceSnapshot {
            workspace_id: uuid::Uuid::nil(),
            physical_codes: BTreeSet::new(),
            existing_blueprints: BTreeMap::new(),
            existing_presentation_assets: BTreeMap::new(),
            installed_extensions: BTreeMap::new(),
            explore_navigation: Vec::new(),
            explore_navigation_valid: true,
            extension_layout: json!({"version":1,"outlets":{}}),
            extension_layout_valid: true,
            role_codes: BTreeSet::new(),
            published_entity_codes: BTreeSet::new(),
            seed: Default::default(),
        };
        let baseline = build_solution_pack_plan(
            &required_pack,
            "deps",
            BlueprintPublication::Draft,
            &empty_workspace,
        )
        .unwrap();
        let main_definition = baseline
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/main")
            .unwrap()
            .normalized_payload
            .as_ref()
            .unwrap()["definition"]
            .as_str()
            .unwrap();
        let optional_pack =
            ValidatedSolutionPack::from_tar_zst(&archive(&manifest_for(false), &files)).unwrap();
        let mapped = build_solution_pack_plan(
            &optional_pack,
            "deps",
            BlueprintPublication::Draft,
            &PlanningWorkspaceSnapshot {
                workspace_id: uuid::Uuid::nil(),
                physical_codes: BTreeSet::from(["deps_main".to_owned()]),
                existing_presentation_assets: BTreeMap::new(),
                existing_blueprints: BTreeMap::from([(
                    "blueprints/main".to_owned(),
                    ExistingBlueprintSnapshot {
                        id: uuid::Uuid::new_v4(),
                        code: "deps_main".to_owned(),
                        version: 1,
                        kind: main_kind.to_owned(),
                        canonical_definition_hash: catalog_blueprint::raw_hash(main_definition),
                        definition_hash: catalog_blueprint::raw_hash(main_definition),
                    },
                )]),
                installed_extensions: BTreeMap::new(),
                explore_navigation: Vec::new(),
                explore_navigation_valid: true,
                extension_layout: json!({"version":1,"outlets":{}}),
                extension_layout_valid: true,
                role_codes: BTreeSet::new(),
                published_entity_codes: BTreeSet::from(["deps_main".to_owned()]),
                seed: Default::default(),
            },
        )
        .unwrap();
        let main_action = mapped
            .actions
            .iter()
            .find(|action| action.logical_key == "blueprints/main")
            .unwrap();
        assert_eq!(
            (main_action.action, main_action.reason_code),
            (PlanActionKind::Blocked, "dependency_not_creatable"),
            "{case}"
        );
        assert!(!mapped.ready, "{case}");
    }
}

#[test]
fn planner_blocks_draft_relationship_paths_and_orders_publish_targets() {
    let product = br#"format_version = 1
code = "product"
name = "Product"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "category.name" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "blueprints/category"
"#;
    let category = br#"format_version = 1
code = "category"
name = "Category"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[[attributes]]
code = "name"
value_type = "string"
"#;
    let manifest = json!({
        "manifest_version": 1,
        "id": "attricat.paths",
        "name": "Paths",
        "version": "1.0.0",
        "description": "Relationship table paths",
        "catalog": {"host_api": "^1.0"},
        "resources": {"blueprints": [
            resource("blueprints/product", "blueprints/product.toml", product),
            resource("blueprints/category", "blueprints/category.toml", category)
        ]}
    });
    let files = [
        ("blueprints/product.toml", product.as_slice()),
        ("blueprints/category.toml", category.as_slice()),
    ];
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let workspace = PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::nil(),
        physical_codes: BTreeSet::from(["default".to_owned()]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::new(),
        explore_navigation: Vec::new(),
        explore_navigation_valid: true,
        extension_layout: serde_json::json!({"version":1,"outlets":{}}),
        extension_layout_valid: true,
        role_codes: BTreeSet::new(),
        published_entity_codes: BTreeSet::new(),
        seed: Default::default(),
    };

    let draft =
        build_solution_pack_plan(&pack, "paths", BlueprintPublication::Draft, &workspace).unwrap();
    let product_action = draft
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    assert_eq!(
        (product_action.action, product_action.reason_code),
        (
            PlanActionKind::Blocked,
            "draft_table_path_target_unpublished"
        )
    );
    assert!(!draft.ready);

    let publish =
        build_solution_pack_plan(&pack, "paths", BlueprintPublication::Publish, &workspace)
            .unwrap();
    assert!(publish.ready);
    assert_eq!(
        publish
            .actions
            .iter()
            .map(|action| action.logical_key.as_str())
            .collect::<Vec<_>>(),
        ["blueprints/category", "blueprints/product"]
    );

    let cyclic_category = br#"format_version = 1
code = "category"
name = "Category"
kind = "entity"
[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
[views.table]
type = "table"
columns = [{ field = "product.name" }]
[[attributes]]
code = "name"
value_type = "string"
[[attributes]]
code = "product"
value_type = "relationship"
target_blueprint = "blueprints/product"
"#;
    let mut cyclic_manifest = manifest;
    cyclic_manifest["resources"]["blueprints"][1]["sha256"] = json!(digest(cyclic_category));
    let cyclic_files = [
        ("blueprints/product.toml", product.as_slice()),
        ("blueprints/category.toml", cyclic_category.as_slice()),
    ];
    let cyclic_pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&cyclic_manifest, &cyclic_files)).unwrap();
    let error = build_solution_pack_plan(
        &cyclic_pack,
        "paths",
        BlueprintPublication::Publish,
        &workspace,
    )
    .unwrap_err();
    assert!(error.to_string().contains("dependencies contain a cycle"));

    let mut mapping_graph = BTreeMap::new();
    for (key, code, version, id) in [
        ("blueprints/category", "existing_category", 4, 103_u128),
        ("blueprints/product", "existing_product", 6, 104_u128),
    ] {
        mapping_graph.insert(
            key.to_owned(),
            PlannedMapping {
                resource_kind: PlanResourceKind::Blueprint,
                logical_key: key.to_owned(),
                target_id: uuid::Uuid::from_u128(id),
                target_code: code.to_owned(),
                target_version: Some(version),
                mapping_kind: MappingKind::Existing,
                snapshot: json!({}),
            },
        );
    }
    let mut mapped_workspace = PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::nil(),
        physical_codes: BTreeSet::from([
            "existing_category".to_owned(),
            "existing_product".to_owned(),
        ]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::new(),
        explore_navigation: Vec::new(),
        explore_navigation_valid: true,
        extension_layout: json!({"version":1,"outlets":{}}),
        extension_layout_valid: true,
        role_codes: BTreeSet::new(),
        published_entity_codes: BTreeSet::from([
            "existing_category".to_owned(),
            "existing_product".to_owned(),
        ]),
        seed: Default::default(),
    };
    for (key, code, version, id) in [
        ("blueprints/category", "existing_category", 4, 103_u128),
        ("blueprints/product", "existing_product", 6, 104_u128),
    ] {
        let payload = normalized_blueprint_payload(
            cyclic_pack.blueprint(key).unwrap(),
            &mapping_graph,
            BlueprintPublication::Publish,
            &HashSet::new(),
            &mapped_workspace,
            cyclic_pack.manifest(),
        )
        .unwrap();
        mapped_workspace.existing_blueprints.insert(
            key.to_owned(),
            ExistingBlueprintSnapshot {
                id: uuid::Uuid::from_u128(id),
                code: code.to_owned(),
                version,
                kind: "entity".to_owned(),
                canonical_definition_hash: catalog_blueprint::raw_hash(
                    payload["definition"].as_str().unwrap(),
                ),
                definition_hash: catalog_blueprint::raw_hash(
                    payload["definition"].as_str().unwrap(),
                ),
            },
        );
    }
    let mapped = build_solution_pack_plan(
        &cyclic_pack,
        "paths",
        BlueprintPublication::Publish,
        &mapped_workspace,
    )
    .unwrap();
    assert!(mapped.ready);
    assert!(
        mapped
            .actions
            .iter()
            .all(|action| action.action == PlanActionKind::Map)
    );
}

#[test]
fn planner_uses_workspace_wide_codes_and_deterministic_target_ids() {
    let pack =
        ValidatedSolutionPack::from_tar_zst(&archive(&manifest_value(), &valid_files())).unwrap();
    let workspace = PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::from_u128(1),
        physical_codes: BTreeSet::from(["default".to_owned(), "ecom_product".to_owned()]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::new(),
        explore_navigation: Vec::new(),
        explore_navigation_valid: true,
        extension_layout: serde_json::json!({"version":1,"outlets":{}}),
        extension_layout_valid: true,
        role_codes: BTreeSet::new(),
        published_entity_codes: BTreeSet::new(),
        seed: Default::default(),
    };
    let first =
        build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace).unwrap();
    let second =
        build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace).unwrap();

    assert_eq!(
        first
            .mappings
            .iter()
            .map(|mapping| mapping.target_id)
            .collect::<Vec<_>>(),
        second
            .mappings
            .iter()
            .map(|mapping| mapping.target_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        first
            .actions
            .iter()
            .map(|action| &action.normalized_payload)
            .collect::<Vec<_>>(),
        second
            .actions
            .iter()
            .map(|action| &action.normalized_payload)
            .collect::<Vec<_>>()
    );
    let product = first
        .actions
        .iter()
        .find(|action| action.logical_key == "blueprints/product")
        .unwrap();
    assert_eq!(
        (product.action, product.reason_code),
        (PlanActionKind::Conflict, "target_code_exists")
    );
}

#[test]
fn rejects_unsafe_workspace_dependent_blueprint_constructs_and_allows_layouts() {
    let role = PRODUCT_BLUEPRINT.replace_ascii(
        b"kind = \"entity\"",
        b"kind = \"entity\"\n[publication]\nretain_on_edit_roles = [\"editor\"]",
    );
    assert_blueprint_error(&role, "cannot declare workspace roles");

    let renderer = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.table]
type = "table"
columns = [{ field = "name", renderer = { id = "acme.custom", version = 1 } }]
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    assert_blueprint_error(&renderer, "cannot declare extension table renderers");

    let layout = PRODUCT_BLUEPRINT
        .iter()
        .copied()
        .chain(
            br#"
[views.extension_layout]
type = "extension_layout"
version = 1
outlets = {}
"#
            .iter()
            .copied(),
        )
        .collect::<Vec<_>>();
    let mut files = valid_files();
    files[0] = (files[0].0, &layout);
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"][0]["sha256"] = json!(digest(&layout));
    ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
}

trait ReplaceAscii {
    fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
}

impl ReplaceAscii for [u8] {
    fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let text = std::str::from_utf8(self).unwrap();
        text.replace(
            std::str::from_utf8(from).unwrap(),
            std::str::from_utf8(to).unwrap(),
        )
        .into_bytes()
    }
}

const DOCUMENT_BLUEPRINT: &[u8] = br#"
format_version = 1
code = "document"
name = "Document"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "summary"
value_type = "string"
context_editable = "all"

[[attributes]]
code = "internal_note"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "scan"
value_type = "file"
cardinality = "one"
image_only = true

[[attributes]]
code = "evidence"
value_type = "file"
cardinality = "many"
"#;
const EU_CONTEXT: &[u8] =
    br#"{"format_version":1,"kind":"solution_pack_context","data":{"region":"eu"}}"#;
const PL_CONTEXT: &[u8] = br#"{"format_version":1,"kind":"solution_pack_context","data":{"language":"pl"},"parent":"contexts/eu","publication_channel":{"enabled":true}}"#;
const NAME_RULE: &[u8] = br#"
format_version = 1
code = "name-required"
name = "Products have a name"
severity = "error"
blueprint = "blueprints/product"
context = "contexts/pl"
enabled = true

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "name"
"#;
const REVIEW_WORKFLOW: &[u8] = br#"
format_version = 2
code = "mark-reviewed"
name = "Mark reviewed"
enabled = false

[[triggers]]
type = "manual"

[[actions]]
type = "system_tags_add"
tags = ["reviewed"]
"#;
const UNNAMED_SEARCH: &[u8] = br#"{"format_version":1,"kind":"solution_pack_saved_search","name":"Unnamed products","description":"Products without a name","state":{"blueprint":"blueprints/product","context":"contexts/pl","sort":{"field":"categories.name","direction":"asc"},"attributeFilters":[{"field":"name","operator":"eq","value":""}],"relationshipFacets":[{"field":"categories","targetBlueprint":"blueprints/category"}],"locked":false}}"#;

/// The name rule with its predicate (and anything after it) replaced.
fn rule_with_predicate(predicate: &str) -> Vec<u8> {
    String::from_utf8(NAME_RULE.to_vec())
        .unwrap()
        .replace("type = \"required\"\nattribute_code = \"name\"", predicate)
        .into_bytes()
}

fn replace_seed_file(
    manifest: &mut Value,
    files: &mut [(&'static str, &'static [u8])],
    kind: &str,
    path: &'static str,
    bytes: &'static [u8],
) {
    let entry = manifest["resources"][kind]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|resource| resource["path"] == path)
        .unwrap();
    entry["sha256"] = json!(digest(bytes));
    files
        .iter_mut()
        .find(|(candidate, _)| *candidate == path)
        .unwrap()
        .1 = bytes;
}

#[test]
fn predicates_resolve_and_rewrite_pack_blueprint_codes() {
    let (mut manifest, mut files) = seed_manifest();
    // A nested `linked` code belongs to the linked blueprint, not to the
    // rule's own; a `referenced_by` source is a pack-local blueprint code.
    let rule = rule_with_predicate(
        "type = \"all_of\"\n\n[[predicate.predicates]]\ntype = \"linked\"\nrelationship_code = \"categories\"\n\n[predicate.predicates.predicate]\ntype = \"required\"\nattribute_code = \"products\"\n\n[[predicate.predicates]]\ntype = \"referenced_by\"\nblueprint_code = \"category\"\nrelationship_code = \"products\"\nmin = 1",
    );
    replace_seed_file(
        &mut manifest,
        &mut files,
        "rules",
        "rules/name-required.toml",
        Box::leak(rule.into_boxed_slice()),
    );
    let product = String::from_utf8(PRODUCT_BLUEPRINT.to_vec()).unwrap().replace(
        "kind = \"entity\"\n",
        "kind = \"entity\"\nentity_schema = '{\"type\":\"object\",\"x-attricat-checks\":[{\"code\":\"in_category\",\"predicate\":{\"type\":\"referenced_by\",\"blueprint_code\":\"category\",\"relationship_code\":\"products\",\"min\":1}}]}'\n",
    );
    let product: &'static [u8] = Box::leak(product.into_bytes().into_boxed_slice());
    replace_seed_file(
        &mut manifest,
        &mut files,
        "blueprints",
        "blueprints/product.toml",
        product,
    );
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &seed_workspace(SeedWorkspaceSnapshot::default()),
    )
    .unwrap();
    assert!(plan.ready, "{:?}", plan.actions);
    let definition = action(&plan, "rules/name-required")
        .normalized_payload
        .as_ref()
        .unwrap()["definition"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        definition.contains("blueprint_code = \"ecom_category\""),
        "{definition}"
    );
    assert!(catalog_rules::compile(&definition).is_ok());
    let blueprint = action(&plan, "blueprints/product")
        .normalized_payload
        .as_ref()
        .unwrap()["definition"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(blueprint.contains("ecom_category"), "{blueprint}");

    let unknown = String::from_utf8(product.to_vec()).unwrap().replace(
        "\"blueprint_code\":\"category\"",
        "\"blueprint_code\":\"missing\"",
    );
    replace_seed_file(
        &mut manifest,
        &mut files,
        "blueprints",
        "blueprints/product.toml",
        Box::leak(unknown.into_bytes().into_boxed_slice()),
    );
    assert_invalid(&archive(&manifest, &files), "the pack does not declare");
}

#[test]
fn channels_require_rules_by_physical_code_and_enforcing_rules_on_mapped_blueprints_wait() {
    let (mut manifest, mut files) = seed_manifest();
    let context = String::from_utf8(PL_CONTEXT.to_vec()).unwrap().replace(
        "{\"enabled\":true}",
        "{\"enabled\":true,\"required_rules\":[\"rules/name-required\"],\"require_valid_entity\":true}",
    );
    replace_seed_file(
        &mut manifest,
        &mut files,
        "contexts",
        "contexts/pl.json",
        Box::leak(context.into_bytes().into_boxed_slice()),
    );
    let rule = rule_with_predicate(
        "type = \"required\"\nattribute_code = \"name\"\n\n[enforcement]\non_save = true",
    );
    replace_seed_file(
        &mut manifest,
        &mut files,
        "rules",
        "rules/name-required.toml",
        Box::leak(rule.into_boxed_slice()),
    );
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &seed_workspace(SeedWorkspaceSnapshot::default()),
    )
    .unwrap();
    let channel = action(&plan, "channels/pl")
        .normalized_payload
        .clone()
        .unwrap();
    assert_eq!(
        channel["required_rule_codes"],
        json!(["ecom_name-required"])
    );
    assert_eq!(channel["require_valid_entity"], json!(true));
    // A newly created blueprint has no entities, so the rule is enabled.
    let rule = action(&plan, "rules/name-required");
    assert_eq!(
        rule.normalized_payload.as_ref().unwrap()["enabled"],
        json!(true)
    );
    assert_eq!(rule.summary["enable_deferred_reason"], Value::Null);

    let existing = |hash: String| ExistingBlueprintSnapshot {
        id: uuid::Uuid::from_u128(9),
        code: "shared_product".into(),
        version: 2,
        kind: "entity".into(),
        canonical_definition_hash: hash,
        definition_hash: "0".repeat(64),
    };
    let mut workspace = seed_workspace(SeedWorkspaceSnapshot::default());
    workspace.physical_codes.insert("shared_product".to_owned());
    workspace.existing_blueprints =
        BTreeMap::from([("blueprints/product".to_owned(), existing("0".repeat(64)))]);
    let probe =
        build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace).unwrap();
    let hash = probe.blueprint_canonical_definition_hashes["blueprints/product"].clone();
    workspace.existing_blueprints =
        BTreeMap::from([("blueprints/product".to_owned(), existing(hash))]);
    let plan =
        build_solution_pack_plan(&pack, "ecom", BlueprintPublication::Publish, &workspace).unwrap();
    assert_eq!(
        action(&plan, "blueprints/product").action,
        PlanActionKind::Map
    );
    let rule = action(&plan, "rules/name-required");
    assert_eq!(rule.action, PlanActionKind::Create);
    assert_eq!(
        rule.normalized_payload.as_ref().unwrap()["enabled"],
        json!(false)
    );
    assert_eq!(rule.summary["requested_enabled"], json!(true));
    assert_eq!(
        rule.summary["enable_deferred_reason"],
        json!("enforcing_rule_requires_dry_run")
    );
}

fn seed_manifest() -> (Value, Vec<(&'static str, &'static [u8])>) {
    let mut manifest = manifest_value();
    manifest["resources"]["contexts"] = json!([
        resource("contexts/pl", "contexts/pl.json", PL_CONTEXT),
        resource("contexts/eu", "contexts/eu.json", EU_CONTEXT),
    ]);
    manifest["resources"]["rules"] = json!([resource(
        "rules/name-required",
        "rules/name-required.toml",
        NAME_RULE
    )]);
    manifest["resources"]["workflows"] = json!([resource(
        "workflows/mark-reviewed",
        "workflows/mark-reviewed.toml",
        REVIEW_WORKFLOW
    )]);
    manifest["resources"]["saved_searches"] = json!([resource(
        "saved-searches/unnamed",
        "saved-searches/unnamed.json",
        UNNAMED_SEARCH
    )]);
    let mut files = valid_files();
    files.extend([
        ("contexts/pl.json", PL_CONTEXT),
        ("contexts/eu.json", EU_CONTEXT),
        ("rules/name-required.toml", NAME_RULE),
        ("workflows/mark-reviewed.toml", REVIEW_WORKFLOW),
        ("saved-searches/unnamed.json", UNNAMED_SEARCH),
    ]);
    (manifest, files)
}

fn seed_workspace(seed: SeedWorkspaceSnapshot) -> PlanningWorkspaceSnapshot {
    PlanningWorkspaceSnapshot {
        workspace_id: uuid::Uuid::nil(),
        physical_codes: BTreeSet::from(["default".to_owned()]),
        existing_blueprints: BTreeMap::new(),
        existing_presentation_assets: BTreeMap::new(),
        installed_extensions: BTreeMap::new(),
        explore_navigation: Vec::new(),
        explore_navigation_valid: true,
        extension_layout: json!({"version":1,"outlets":{}}),
        extension_layout_valid: true,
        role_codes: BTreeSet::new(),
        published_entity_codes: BTreeSet::new(),
        seed,
    }
}

fn action<'a>(plan: &'a SolutionPackPlanDraft, key: &str) -> &'a PlannedAction {
    plan.actions
        .iter()
        .find(|action| action.logical_key == key)
        .unwrap_or_else(|| panic!("missing action {key}"))
}

fn mapping<'a>(plan: &'a SolutionPackPlanDraft, key: &str) -> &'a PlannedMapping {
    plan.mappings
        .iter()
        .find(|mapping| mapping.logical_key == key)
        .unwrap_or_else(|| panic!("missing mapping {key}"))
}

#[test]
fn seed_resources_plan_with_physical_references_in_dependency_order() {
    let (manifest, files) = seed_manifest();
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    assert!(pack.rule("rules/name-required").unwrap().enabled);
    let plan = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &seed_workspace(SeedWorkspaceSnapshot::default()),
    )
    .unwrap();
    assert!(plan.ready, "{:?}", plan.actions);
    assert_eq!(plan.mappings.len(), plan.actions.len());
    let order = plan
        .actions
        .iter()
        .map(|action| action.logical_key.as_str())
        .collect::<Vec<_>>();
    let position = |key: &str| {
        order
            .iter()
            .position(|candidate| *candidate == key)
            .unwrap()
    };
    assert!(position("contexts/eu") < position("contexts/pl"));
    assert!(position("contexts/pl") < position("blueprints/product"));
    assert!(position("blueprints/product") < position("rules/name-required"));
    // Channels follow the rules they can require.
    assert!(position("rules/name-required") < position("channels/pl"));

    let eu = mapping(&plan, "contexts/eu");
    let pl = mapping(&plan, "contexts/pl");
    let product = mapping(&plan, "blueprints/product");
    assert_eq!(pl.target_code, "ecom_pl");
    let pl_action = action(&plan, "contexts/pl");
    assert_eq!(pl_action.action, PlanActionKind::Create);
    assert_eq!(
        pl_action.normalized_payload.as_ref().unwrap()["parent_id"],
        json!(eu.target_id)
    );
    assert_eq!(
        action(&plan, "channels/pl").normalized_payload,
        Some(json!({
            "context_id": pl.target_id,
            "context_code": "ecom_pl",
            "enabled": true,
            "required_rule_codes": [],
            "require_valid_entity": false,
        }))
    );

    let rule = action(&plan, "rules/name-required");
    assert_eq!(rule.action, PlanActionKind::Create);
    let payload = rule.normalized_payload.as_ref().unwrap();
    assert_eq!(payload["blueprint_id"], json!(product.target_id));
    assert_eq!(payload["blueprint_version"], json!(1));
    assert_eq!(payload["context_id"], json!(pl.target_id));
    assert_eq!(payload["enabled"], json!(true));
    let definition = payload["definition"].as_str().unwrap();
    assert!(definition.contains("code = \"ecom_name-required\""));
    assert!(!definition.contains("blueprint"));
    assert!(catalog_rules::compile(definition).is_ok());
    assert_eq!(
        mapping(&plan, "rules/name-required").target_code,
        "ecom_name-required"
    );

    let workflow = action(&plan, "workflows/mark-reviewed");
    let payload = workflow.normalized_payload.as_ref().unwrap();
    assert_eq!(payload["enabled"], json!(false));
    assert!(
        catalog_workflow::compile(payload["definition"].as_str().unwrap())
            .unwrap()
            .code
            == "ecom_mark-reviewed"
    );

    let search = action(&plan, "saved-searches/unnamed");
    assert_eq!(
        search.normalized_payload.as_ref().unwrap()["state"],
        json!({
            "blueprint": "ecom_product",
            "context": "ecom_pl",
            "sort": {"field": "categories.name", "direction": "asc"},
            "attributeFilters": [{"field": "name", "operator": "eq", "value": ""}],
            "relationshipFacets": [{"field": "categories", "targetBlueprint": "ecom_category"}],
        })
    );
    assert_eq!(
        search.normalized_payload.as_ref().unwrap()["visibility"],
        "workspace"
    );
}

#[test]
fn seed_resources_respect_publication_mapping_and_existing_codes() {
    let (manifest, files) = seed_manifest();
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let draft = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Draft,
        &seed_workspace(SeedWorkspaceSnapshot::default()),
    )
    .unwrap();
    assert!(!draft.ready);
    let rule = action(&draft, "rules/name-required");
    assert_eq!(
        (rule.action, rule.reason_code),
        (PlanActionKind::Blocked, "blueprint_not_published")
    );
    assert_eq!(
        action(&draft, "saved-searches/unnamed").action,
        PlanActionKind::Create
    );

    let existing_id = uuid::Uuid::from_u128(7);
    for (enabled, expected) in [
        (Some(true), (PlanActionKind::Satisfied, "exact_match")),
        (None, (PlanActionKind::Create, "target_absent")),
        (
            Some(false),
            (PlanActionKind::Conflict, "publication_channel_mismatch"),
        ),
    ] {
        let plan = build_solution_pack_plan(
            &pack,
            "ecom",
            BlueprintPublication::Publish,
            &seed_workspace(SeedWorkspaceSnapshot {
                existing_contexts: BTreeMap::from([(
                    "contexts/pl".to_owned(),
                    crate::solution_pack_seeds::ExistingContextSnapshot {
                        id: existing_id,
                        code: "PL".to_owned(),
                        publication_channel: enabled.map(|enabled| {
                            crate::solution_pack_seeds::ExistingPublicationChannel {
                                enabled,
                                required_rule_codes: Vec::new(),
                                require_valid_entity: false,
                            }
                        }),
                    },
                )]),
                rule_codes: BTreeSet::from(["ecom_name-required".to_owned()]),
                ..Default::default()
            }),
        )
        .unwrap();
        let context = action(&plan, "contexts/pl");
        assert_eq!(
            (context.action, context.reason_code),
            (PlanActionKind::Map, "existing_context_selected")
        );
        let channel = action(&plan, "channels/pl");
        assert_eq!((channel.action, channel.reason_code), expected);
        let rule = action(&plan, "rules/name-required");
        assert_eq!(
            (rule.action, rule.reason_code),
            (PlanActionKind::Conflict, "target_code_exists")
        );
        assert_eq!(
            action(&plan, "saved-searches/unnamed")
                .normalized_payload
                .as_ref()
                .unwrap()["state"]["context"],
            "PL"
        );
    }

    let conflict = build_solution_pack_plan(
        &pack,
        "ecom",
        BlueprintPublication::Publish,
        &PlanningWorkspaceSnapshot {
            physical_codes: BTreeSet::from(["ecom_eu".to_owned()]),
            ..seed_workspace(SeedWorkspaceSnapshot::default())
        },
    )
    .unwrap();
    assert_eq!(
        action(&conflict, "contexts/eu").action,
        PlanActionKind::Conflict
    );
    let child = action(&conflict, "contexts/pl");
    assert_eq!(
        (child.action, child.reason_code),
        (PlanActionKind::Blocked, "dependency_not_creatable")
    );
    assert_eq!(
        action(&conflict, "rules/name-required").action,
        PlanActionKind::Blocked
    );
    assert_eq!(
        action(&conflict, "saved-searches/unnamed").action,
        PlanActionKind::Blocked
    );
}

#[test]
fn invalid_seed_resources_are_rejected_offline() {
    let cases: Vec<(&str, &str, Vec<u8>, &str)> = vec![
        (
            "rules",
            "rules/name-required.toml",
            String::from_utf8(NAME_RULE.to_vec())
                .unwrap()
                .replace("attribute_code = \"name\"", "attribute_code = \"missing\"")
                .into_bytes(),
            "unknown attribute 'missing'",
        ),
        (
            "rules",
            "rules/name-required.toml",
            String::from_utf8(NAME_RULE.to_vec())
                .unwrap()
                .replace("blueprints/product", "blueprints/missing")
                .into_bytes(),
            "undeclared blueprint",
        ),
        (
            "rules",
            "rules/name-required.toml",
            String::from_utf8(NAME_RULE.to_vec())
                .unwrap()
                .replace("enabled = true\n", "")
                .into_bytes(),
            "must declare boolean 'enabled'",
        ),
        (
            "workflows",
            "workflows/mark-reviewed.toml",
            String::from_utf8(REVIEW_WORKFLOW.to_vec())
                .unwrap()
                .replace(
                    "type = \"manual\"",
                    "type = \"schedule\"\ncron = \"0 0 * * * *\"\ntimezone = \"UTC\"\ntarget_entity_id = \"00000000-0000-0000-0000-000000000001\"",
                )
                .into_bytes(),
            "schedule trigger",
        ),
        (
            "saved_searches",
            "saved-searches/unnamed.json",
            String::from_utf8(UNNAMED_SEARCH.to_vec())
                .unwrap()
                .replace("\"locked\":false", "\"version\":1")
                .into_bytes(),
            "cannot be seeded",
        ),
        (
            "saved_searches",
            "saved-searches/unnamed.json",
            String::from_utf8(UNNAMED_SEARCH.to_vec())
                .unwrap()
                .replace(
                    "\"targetBlueprint\":\"blueprints/category\"",
                    "\"selectedIds\":[\"00000000-0000-0000-0000-000000000001\"]",
                )
                .into_bytes(),
            "selected entity IDs",
        ),
        (
            "saved_searches",
            "saved-searches/unnamed.json",
            String::from_utf8(UNNAMED_SEARCH.to_vec())
                .unwrap()
                .replace("categories.name", "categories.missing")
                .into_bytes(),
            "unknown attribute 'missing'",
        ),
        (
            "rules",
            "rules/name-required.toml",
            rule_with_predicate(
                "type = \"referenced_by\"\nblueprint_code = \"missing\"\nrelationship_code = \"products\"\nmin = 1",
            ),
            "not a pack entity blueprint",
        ),
        (
            "rules",
            "rules/name-required.toml",
            rule_with_predicate(
                "type = \"referenced_by\"\nblueprint_code = \"category\"\nrelationship_code = \"name\"\nmin = 1",
            ),
            "unknown relationship 'name'",
        ),
        (
            "rules",
            "rules/name-required.toml",
            rule_with_predicate(
                "type = \"compare\"\nattribute_code = \"name\"\nop = \"eq\"\nother_attribute_code = \"missing\"",
            ),
            "unknown attribute 'missing'",
        ),
        (
            "rules",
            "rules/name-required.toml",
            rule_with_predicate(
                "type = \"required\"\nattribute_code = \"name\"\n\n[enforcement]\non_save = false\n\n[[enforcement.transitions]]\nattribute_code = \"missing\"\nto = \"done\"",
            ),
            "unknown enforcement attribute 'missing'",
        ),
        (
            "workflows",
            "workflows/mark-reviewed.toml",
            String::from_utf8(REVIEW_WORKFLOW.to_vec())
                .unwrap()
                .replace(
                    "[[actions]]\ntype = \"system_tags_add\"\ntags = [\"reviewed\"]",
                    "[[actions]]\ntype = \"referencing_entities_update\"\nrelationship_attribute = \"categories\"\nmax_targets = 5\n[[actions.actions]]\ntype = \"attribute_write\"\nattribute_code = \"products\"\nfixed = \"x\"",
                )
                .into_bytes(),
            "writes attribute 'products'",
        ),
        (
            "workflows",
            "workflows/mark-reviewed.toml",
            String::from_utf8(REVIEW_WORKFLOW.to_vec())
                .unwrap()
                .replace(
                    "[[actions]]\ntype = \"system_tags_add\"\ntags = [\"reviewed\"]",
                    "[[actions]]\ntype = \"referencing_entities_update\"\nrelationship_attribute = \"name\"\nmax_targets = 5\n[[actions.actions]]\ntype = \"system_tags_add\"\ntags = [\"x\"]",
                )
                .into_bytes(),
            "follows relationship 'name'",
        ),
        (
            "saved_searches",
            "saved-searches/unnamed.json",
            String::from_utf8(UNNAMED_SEARCH.to_vec())
                .unwrap()
                .replace(
                    "categories.name",
                    "categories.products.categories.products.name",
                )
                .into_bytes(),
            "more than 3 relationship hops",
        ),
        (
            "saved_searches",
            "saved-searches/unnamed.json",
            String::from_utf8(UNNAMED_SEARCH.to_vec())
                .unwrap()
                .replace(
                    "\"targetBlueprint\":\"blueprints/category\"",
                    "\"targetBlueprint\":\"blueprints/product\"",
                )
                .into_bytes(),
            "cannot target 'blueprints/product'",
        ),
        (
            "contexts",
            "contexts/pl.json",
            String::from_utf8(PL_CONTEXT.to_vec())
                .unwrap()
                .replace(
                    "{\"enabled\":true}",
                    "{\"enabled\":true,\"required_rules\":[\"rules/missing\"]}",
                )
                .into_bytes(),
            "requires undeclared rule 'rules/missing'",
        ),
        (
            "contexts",
            "contexts/eu.json",
            br#"{"format_version":1,"kind":"solution_pack_context","parent":"contexts/pl"}"#
                .to_vec(),
            "cycle",
        ),
        (
            "contexts",
            "contexts/eu.json",
            br#"{"format_version":1,"code":"eu","data":{}}"#.to_vec(),
            "not strict JSON",
        ),
    ];
    for (kind, path, bytes, expected) in cases {
        let (mut manifest, files) = seed_manifest();
        let resources = manifest["resources"][kind].as_array_mut().unwrap();
        let entry = resources
            .iter_mut()
            .find(|resource| resource["path"] == path)
            .unwrap();
        entry["sha256"] = json!(digest(&bytes));
        let files = files
            .into_iter()
            .map(|(candidate, content)| {
                if candidate == path {
                    (candidate, bytes.as_slice())
                } else {
                    (candidate, content)
                }
            })
            .collect::<Vec<_>>();
        assert_invalid(&archive(&manifest, &files), expected);
    }

    let (mut manifest, files) = seed_manifest();
    manifest["resources"]["rules"][0]["reuse"] =
        json!({"prerequisite": "prerequisites/base", "blueprint": "blueprints/product"});
    assert_invalid(
        &archive(&manifest, &files),
        "cannot declare blueprint reuse",
    );
}

fn prerequisite_archive() -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["prerequisites"] =
        json!([{"key": "prerequisites/base", "id": "attricat.base", "version": "^1.2"}]);
    manifest["resources"]["blueprints"][1]["reuse"] =
        json!({"prerequisite": "prerequisites/base", "blueprint": "blueprints/category"});
    archive(&manifest, &valid_files())
}

#[test]
fn prerequisites_are_declared_and_reused_blueprints_must_match_exactly() {
    let mut manifest = manifest_value();
    manifest["prerequisites"] =
        json!([{"key": "prerequisites/base", "id": "attricat.base", "version": "^1.2"}]);
    assert_invalid(&archive(&manifest, &valid_files()), "is not reused");
    manifest["resources"]["blueprints"][1]["reuse"] =
        json!({"prerequisite": "prerequisites/other", "blueprint": "blueprints/category"});
    assert_invalid(
        &archive(&manifest, &valid_files()),
        "undeclared prerequisite",
    );
    manifest["prerequisites"][0]["id"] = json!("attricat.ecommerce");
    manifest["resources"]["blueprints"][1]["reuse"]["prerequisite"] = json!("prerequisites/base");
    assert_invalid(&archive(&manifest, &valid_files()), "the pack itself");

    let pack = ValidatedSolutionPack::from_tar_zst(&prerequisite_archive()).unwrap();
    assert!(
        validate_blueprint_mapping_requests(
            &pack,
            &[BlueprintMappingRequest {
                key: "blueprints/category".into(),
                code: "shared_category".into(),
            }],
        )
        .unwrap_err()
        .to_string()
        .contains("reused from a prerequisite")
    );
    for (resolution, reason) in [
        (
            crate::solution_pack_seeds::PrerequisiteResolution::Missing,
            "prerequisite_missing",
        ),
        (
            crate::solution_pack_seeds::PrerequisiteResolution::Incompatible {
                versions: vec!["2.0.0".into()],
            },
            "prerequisite_incompatible",
        ),
    ] {
        let plan = build_solution_pack_plan(
            &pack,
            "shop",
            BlueprintPublication::Publish,
            &seed_workspace(SeedWorkspaceSnapshot {
                prerequisites: BTreeMap::from([("prerequisites/base".to_owned(), resolution)]),
                ..Default::default()
            }),
        )
        .unwrap();
        assert!(!plan.ready);
        let prerequisite = action(&plan, "prerequisites/base");
        assert_eq!(
            (prerequisite.action, prerequisite.reason_code),
            (PlanActionKind::Blocked, reason)
        );
        let category = action(&plan, "blueprints/category");
        assert_eq!(
            (category.action, category.reason_code),
            (PlanActionKind::Blocked, "prerequisite_unavailable")
        );
        assert_eq!(
            action(&plan, "blueprints/product").action,
            PlanActionKind::Blocked
        );
    }

    let application_id = uuid::Uuid::from_u128(42);
    let satisfied = || SeedWorkspaceSnapshot {
        prerequisites: BTreeMap::from([(
            "prerequisites/base".to_owned(),
            crate::solution_pack_seeds::PrerequisiteResolution::Satisfied {
                application_id,
                pack_version: "1.4.0".into(),
            },
        )]),
        ..Default::default()
    };
    let unavailable = build_solution_pack_plan(
        &pack,
        "shop",
        BlueprintPublication::Publish,
        &seed_workspace(satisfied()),
    )
    .unwrap();
    assert_eq!(
        action(&unavailable, "prerequisites/base").action,
        PlanActionKind::Map
    );
    assert_eq!(
        action(&unavailable, "blueprints/category").reason_code,
        "prerequisite_blueprint_unavailable"
    );

    let existing = |hash: String| ExistingBlueprintSnapshot {
        id: uuid::Uuid::from_u128(9),
        code: "base_category".into(),
        version: 3,
        kind: "entity".into(),
        canonical_definition_hash: hash,
        definition_hash: "0".repeat(64),
    };
    let mut workspace = seed_workspace(satisfied());
    workspace.existing_blueprints =
        BTreeMap::from([("blueprints/category".to_owned(), existing("0".repeat(64)))]);
    let incompatible =
        build_solution_pack_plan(&pack, "shop", BlueprintPublication::Publish, &workspace).unwrap();
    assert_eq!(
        action(&incompatible, "blueprints/category").reason_code,
        "prerequisite_blueprint_incompatible"
    );
    let exact_hash =
        incompatible.blueprint_canonical_definition_hashes["blueprints/category"].clone();
    workspace.existing_blueprints =
        BTreeMap::from([("blueprints/category".to_owned(), existing(exact_hash))]);
    let reused =
        build_solution_pack_plan(&pack, "shop", BlueprintPublication::Publish, &workspace).unwrap();
    assert!(reused.ready, "{:?}", reused.actions);
    let category = action(&reused, "blueprints/category");
    assert_eq!(
        (category.action, category.reason_code),
        (PlanActionKind::Map, "prerequisite_blueprint_match")
    );
    assert_eq!(
        category.summary["reuse"],
        json!({"prerequisite": "prerequisites/base", "blueprint": "blueprints/category"})
    );
    let product = action(&reused, "blueprints/product");
    assert_eq!(product.action, PlanActionKind::Create);
    assert!(
        product.normalized_payload.as_ref().unwrap()["definition"]
            .as_str()
            .unwrap()
            .contains("target_blueprint = \"base_category\"")
    );
    assert_eq!(
        mapping(&reused, "prerequisites/base").snapshot["pack_version"],
        "1.4.0"
    );
}

const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nnot-a-real-image";
const PDF_BYTES: &[u8] = b"%PDF-1.7\nsynthetic";

fn sample_seed_archive(sample: &[u8], file_paths: &[(&'static str, &'static [u8])]) -> Vec<u8> {
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"]
        .as_array_mut()
        .unwrap()
        .push(resource(
            "blueprints/document",
            "blueprints/document.toml",
            DOCUMENT_BLUEPRINT,
        ));
    manifest["resources"]["contexts"] =
        json!([resource("contexts/eu", "contexts/eu.json", EU_CONTEXT)]);
    manifest["resources"]["sample_data"] = json!({
        "key": "sample-data/default",
        "path": "sample-data/sample-data.json",
        "sha256": digest(sample),
        "files": file_paths.iter().map(|(path, bytes)| json!({"path": path, "sha256": digest(bytes)})).collect::<Vec<_>>(),
    });
    let mut files = valid_files();
    files.push(("blueprints/document.toml", DOCUMENT_BLUEPRINT));
    files.push(("contexts/eu.json", EU_CONTEXT));
    files.push(("sample-data/sample-data.json", sample));
    files.extend_from_slice(file_paths);
    archive(&manifest, &files)
}

fn document_sample(entity: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "solution_pack_sample_data",
        "classification": "synthetic",
        "entities": [entity],
    }))
    .unwrap()
}

#[test]
fn samples_cannot_share_a_unique_key() {
    let keyed = format!(
        "{}\n[[unique_keys]]\ncode = \"title\"\nattributes = [\"title\"]\n",
        String::from_utf8(DOCUMENT_BLUEPRINT.to_vec()).unwrap()
    );
    let keyed: &'static [u8] = Box::leak(keyed.into_bytes().into_boxed_slice());
    let archive_for = |titles: [&str; 2]| {
        let sample = serde_json::to_vec(&json!({
            "format_version": 1,
            "kind": "solution_pack_sample_data",
            "classification": "synthetic",
            "entities": titles.iter().enumerate().map(|(index, title)| json!({
                "key": format!("sample-entities/doc-{index}"),
                "blueprint": "blueprints/document",
                "facts": [{"attribute": "blueprints/document/attributes/title", "value": title}],
                "relationships": []
            })).collect::<Vec<_>>(),
        }))
        .unwrap();
        let sample: &'static [u8] = Box::leak(sample.into_boxed_slice());
        let mut manifest = manifest_value();
        manifest["resources"]["blueprints"]
            .as_array_mut()
            .unwrap()
            .push(resource(
                "blueprints/document",
                "blueprints/document.toml",
                keyed,
            ));
        manifest["resources"]["sample_data"] = json!({
            "key": "sample-data/default",
            "path": "sample-data/sample-data.json",
            "sha256": digest(sample),
            "files": [],
        });
        let mut files = valid_files();
        files.push(("blueprints/document.toml", keyed));
        files.push(("sample-data/sample-data.json", sample));
        archive(&manifest, &files)
    };
    assert!(ValidatedSolutionPack::from_tar_zst(&archive_for(["Spec one", "Spec two"])).is_ok());
    // Compared like stored keys: trimmed, whitespace collapsed, case folded.
    assert_invalid(
        &archive_for(["Sample  spec", " sample SPEC"]),
        "share unique key 'title'",
    );
}

#[test]
fn samples_cannot_set_status_values() {
    let statused = format!(
        "{}\n[[attributes]]\ncode = \"state\"\nvalue_type = \"string\"\nvalue_schema = '{{\"type\":\"string\",\"enum\":[\"draft\"],\"x-attricat-status\":{{\"version\":1,\"options\":[{{\"code\":\"draft\",\"label\":\"Draft\"}}]}}}}'\n",
        String::from_utf8(DOCUMENT_BLUEPRINT.to_vec()).unwrap()
    );
    let statused: &'static [u8] = Box::leak(statused.into_bytes().into_boxed_slice());
    let sample = document_sample(json!({
        "key": "sample-entities/doc",
        "blueprint": "blueprints/document",
        "facts": [
            {"attribute": "blueprints/document/attributes/title", "value": "Spec"},
            {"attribute": "blueprints/document/attributes/state", "value": "draft"}
        ],
        "relationships": []
    }));
    let sample: &'static [u8] = Box::leak(sample.into_boxed_slice());
    let mut manifest = manifest_value();
    manifest["resources"]["blueprints"]
        .as_array_mut()
        .unwrap()
        .push(resource(
            "blueprints/document",
            "blueprints/document.toml",
            statused,
        ));
    manifest["resources"]["sample_data"] = json!({
        "key": "sample-data/default",
        "path": "sample-data/sample-data.json",
        "sha256": digest(sample),
        "files": [],
    });
    let mut files = valid_files();
    files.push(("blueprints/document.toml", statused));
    files.push(("sample-data/sample-data.json", sample));
    assert_invalid(
        &archive(&manifest, &files),
        "cannot set status or principal attribute",
    );
}

#[test]
fn samples_attach_bundled_files_and_set_contextual_values() {
    let valid = json!({
        "key": "sample-entities/spec",
        "blueprint": "blueprints/document",
        "facts": [
            {"attribute": "blueprints/document/attributes/title", "value": "Sample specification"},
            {"attribute": "blueprints/document/attributes/summary", "value": "Default summary"},
            {"attribute": "blueprints/document/attributes/summary", "value": "EU summary", "context": "contexts/eu"}
        ],
        "relationships": [],
        "files": [
            {"attribute": "blueprints/document/attributes/scan", "files": [{"path": "sample-data/files/scan.png", "filename": "scan.png", "media_type": "image/png"}]},
            {"attribute": "blueprints/document/attributes/evidence", "context": "contexts/eu", "files": [
                {"path": "sample-data/files/spec.pdf", "filename": "spec.pdf", "media_type": "application/pdf"},
                {"path": "sample-data/files/scan.png", "filename": "scan.png", "media_type": "image/png"}
            ]}
        ]
    });
    let bundled = [
        ("sample-data/files/scan.png", PNG_BYTES),
        ("sample-data/files/spec.pdf", PDF_BYTES),
    ];
    let sample = document_sample(valid.clone());
    let pack =
        ValidatedSolutionPack::from_tar_zst(&sample_seed_archive(&sample, &bundled)).unwrap();
    let validated = pack.sample_data().unwrap();
    assert_eq!(validated.files.len(), 2);
    assert_eq!(
        validated.files["sample-data/files/spec.pdf"].filename,
        "spec.pdf"
    );

    let unchanged = document_sample(json!({
        "key": "sample-entities/plain",
        "blueprint": "blueprints/document",
        "facts": [{"attribute": "blueprints/document/attributes/title", "value": "Plain"}],
        "relationships": []
    }));
    let plain = crate::solution_pack_sample_data::validate_sample_data(
        &unchanged,
        &BTreeSet::from(["blueprints/document"]),
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap();
    assert!(
        !String::from_utf8(serde_json::to_vec(&plain.declaration).unwrap())
            .unwrap()
            .contains("files"),
        "format-1 samples without files keep their canonical encoding"
    );

    let mutate = |pointer: &str, value: Value| {
        let mut entity = valid.clone();
        *entity.pointer_mut(pointer).unwrap() = value;
        document_sample(entity)
    };
    for (sample, files, expected) in [
        (
            mutate("/facts/2/context", json!("contexts/missing")),
            bundled.to_vec(),
            "undeclared context",
        ),
        (
            mutate(
                "/facts/2/attribute",
                json!("blueprints/document/attributes/internal_note"),
            ),
            bundled.to_vec(),
            "outside the default context",
        ),
        (
            mutate("/files/0/files/0/path", json!("sample-data/files/spec.pdf")),
            bundled.to_vec(),
            "different filenames or media types",
        ),
        (
            mutate(
                "/files/0/files/0",
                json!({"path": "sample-data/files/spec.pdf", "filename": "spec.pdf", "media_type": "application/pdf"}),
            ),
            bundled.to_vec(),
            "not allowed by the file policy",
        ),
        (
            mutate("/files/0/files/0/filename", json!("scan.pdf")),
            bundled.to_vec(),
            "matching extension",
        ),
        (
            document_sample(valid.clone()),
            vec![
                ("sample-data/files/scan.png", PDF_BYTES),
                ("sample-data/files/spec.pdf", PDF_BYTES),
            ],
            "does not match media type",
        ),
        (
            document_sample(valid.clone()),
            vec![
                ("sample-data/files/scan.png", PNG_BYTES),
                ("sample-data/files/spec.pdf", PDF_BYTES),
                ("sample-data/files/unused.pdf", PDF_BYTES),
            ],
            "not attached by any sample entity",
        ),
    ] {
        let Err(error) = ValidatedSolutionPack::from_tar_zst(&sample_seed_archive(&sample, &files))
        else {
            panic!("sample was accepted; expected {expected}");
        };
        let error = error.to_string();
        assert!(
            error.contains(expected),
            "{error} does not contain {expected}"
        );
    }
}

#[test]
fn context_mapping_requests_name_declared_contexts_and_distinct_targets() {
    use crate::solution_pack_seeds::{ContextMappingRequest, validate_context_mapping_requests};
    let (manifest, files) = seed_manifest();
    let pack = ValidatedSolutionPack::from_tar_zst(&archive(&manifest, &files)).unwrap();
    let mapping = |key: &str, code: &str| ContextMappingRequest {
        key: key.into(),
        code: code.into(),
    };
    validate_context_mapping_requests(&pack, &[mapping("contexts/pl", "PL")]).unwrap();
    for (mappings, expected) in [
        (
            vec![mapping("contexts/missing", "PL")],
            "unknown context mapping key",
        ),
        (
            vec![mapping("contexts/pl", "PL"), mapping("contexts/eu", "PL")],
            "more than one pack context",
        ),
        (
            vec![mapping("contexts/pl", "P L")],
            "invalid existing context code",
        ),
    ] {
        let error = validate_context_mapping_requests(&pack, &mappings)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn published_seed_schemas_accept_the_seed_fixtures() {
    let schema = |source: &str| -> Value {
        let schema = serde_json::from_str(source).unwrap();
        catalog_validation::validate_json_schema_definition(&schema).unwrap();
        schema
    };
    let accepts = |schema: &Value, value: &Value| {
        catalog_validation::validate_json_schema(schema, value)
            .unwrap()
            .is_empty()
    };
    let manifest = schema(include_str!(
        "../../../../contracts/solution-pack-manifest-v1.schema.json"
    ));
    let (mut seeds, _) = seed_manifest();
    seeds["prerequisites"] =
        json!([{"key": "prerequisites/base", "id": "attricat.base", "version": "^1.2"}]);
    seeds["resources"]["blueprints"][1]["reuse"] =
        json!({"prerequisite": "prerequisites/base", "blueprint": "blueprints/category"});
    seeds["resources"]["sample_data"] = json!({
        "key": "sample-data/default",
        "path": "sample-data/sample-data.json",
        "sha256": "0".repeat(64),
        "files": [{"path": "sample-data/files/spec.pdf", "sha256": "0".repeat(64)}],
    });
    assert!(accepts(&manifest, &seeds));
    let mut reused_context = seeds.clone();
    reused_context["resources"]["contexts"][0]["reuse"] =
        seeds["resources"]["blueprints"][1]["reuse"].clone();
    assert!(!accepts(&manifest, &reused_context));

    let context = schema(include_str!(
        "../../../../contracts/solution-pack-context-v1.schema.json"
    ));
    for fixture in [EU_CONTEXT, PL_CONTEXT] {
        assert!(accepts(&context, &serde_json::from_slice(fixture).unwrap()));
    }
    let mut gated: Value = serde_json::from_slice(PL_CONTEXT).unwrap();
    gated["publication_channel"]["required_rules"] = json!(["rules/name-required"]);
    gated["publication_channel"]["require_valid_entity"] = json!(true);
    assert!(accepts(&context, &gated));
    gated["publication_channel"]["required_rules"] = json!(["name-required"]);
    assert!(!accepts(&context, &gated));
    assert!(!accepts(
        &context,
        &json!({"format_version": 1, "code": "eu", "data": {}})
    ));

    let search = schema(include_str!(
        "../../../../contracts/solution-pack-saved-search-v1.schema.json"
    ));
    let mut fixture: Value = serde_json::from_slice(UNNAMED_SEARCH).unwrap();
    assert!(accepts(&search, &fixture));
    fixture["state"]["sort"]["field"] = json!("a.b.c.d.e");
    assert!(!accepts(&search, &fixture));
    fixture = serde_json::from_slice(UNNAMED_SEARCH).unwrap();
    fixture["state"]["version"] = json!(1);
    assert!(!accepts(&search, &fixture));

    let sample = schema(include_str!(
        "../../../../contracts/solution-pack-sample-data-v1.schema.json"
    ));
    let declaration = json!({
        "format_version": 1,
        "kind": "solution_pack_sample_data",
        "classification": "synthetic",
        "entities": [{
            "key": "sample-entities/spec",
            "blueprint": "blueprints/document",
            "facts": [{"attribute": "blueprints/document/attributes/summary", "value": "EU", "context": "contexts/eu"}],
            "relationships": [],
            "files": [{
                "attribute": "blueprints/document/attributes/evidence",
                "context": "contexts/eu",
                "files": [{"path": "sample-data/files/spec.pdf", "filename": "spec.pdf", "media_type": "application/pdf"}]
            }]
        }]
    });
    assert!(accepts(&sample, &declaration));
    let mut unsupported = declaration;
    unsupported["entities"][0]["files"][0]["files"][0]["media_type"] = json!("image/svg+xml");
    assert!(!accepts(&sample, &unsupported));
}

#[test]
fn publication_channel_required_rules_compare_as_a_set() {
    let channel = |codes: &[&str]| crate::solution_pack_seeds::ExistingPublicationChannel {
        enabled: true,
        required_rule_codes: codes.iter().map(|code| (*code).to_owned()).collect(),
        require_valid_entity: false,
    };
    assert!(channel(&["a", "b"]).same_settings(&channel(&["b", "a"])));
    assert!(channel(&["a", "b"]).same_settings(&channel(&["b", "a", "b"])));
    assert!(!channel(&["a", "b"]).same_settings(&channel(&["a"])));
    let mut disabled = channel(&["a"]);
    disabled.enabled = false;
    assert!(!channel(&["a"]).same_settings(&disabled));
}
