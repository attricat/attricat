//! Enforces additive evolution of the host ABI.
//!
//! Every released ABI is frozen in `wit-released/catalog-host-<version>.wit`.
//! The current `wit-host` package must contain every released interface,
//! function, type and world unchanged. New functions, interfaces, types,
//! world imports and worlds may be added; nothing released may be removed,
//! renamed or changed, and released worlds may not gain exports (a component
//! built for the released world would no longer satisfy it). Variants, enums,
//! records and flags are compared whole, so adding a case or field to a released
//! type is rejected too: old guests cannot decode values they do not know.

use std::{collections::BTreeMap, fs, path::Path};

use wit_parser::{
    Function, Handle, InterfaceId, PackageId, Resolve, Type, TypeDefKind, TypeId, TypeOwner,
    WorldItem, WorldKey,
};

const CRATE_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn load(path: &Path) -> (Resolve, PackageId) {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_path(path)
        .unwrap_or_else(|error| panic!("{} must parse: {error:?}", path.display()))
        .0;
    (resolve, package)
}

fn describe_type(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Id(id) => describe_type_id(resolve, *id),
        other => format!("{other:?}").to_lowercase(),
    }
}

fn describe_type_id(resolve: &Resolve, id: TypeId) -> String {
    let def = &resolve.types[id];
    let ty = |ty: &Type| describe_type(resolve, ty);
    let opt = |ty: &Option<Type>| ty.as_ref().map_or_else(|| "_".to_owned(), ty_fmt(resolve));
    match &def.kind {
        TypeDefKind::Record(record) => format!(
            "record {{{}}}",
            record
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, ty(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeDefKind::Variant(variant) => format!(
            "variant {{{}}}",
            variant
                .cases
                .iter()
                .map(|case| format!("{}({})", case.name, opt(&case.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeDefKind::Enum(enumeration) => format!(
            "enum {{{}}}",
            enumeration
                .cases
                .iter()
                .map(|case| case.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeDefKind::Flags(flags) => format!(
            "flags {{{}}}",
            flags
                .flags
                .iter()
                .map(|flag| flag.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeDefKind::Tuple(tuple) => format!(
            "tuple<{}>",
            tuple.types.iter().map(ty).collect::<Vec<_>>().join(", ")
        ),
        TypeDefKind::Option(inner) => format!("option<{}>", ty(inner)),
        TypeDefKind::Result(result) => {
            format!("result<{}, {}>", opt(&result.ok), opt(&result.err))
        }
        TypeDefKind::List(inner) => format!("list<{}>", ty(inner)),
        TypeDefKind::Type(inner) => ty(inner),
        TypeDefKind::Handle(Handle::Own(resource)) => {
            format!("own<{}>", type_name(resolve, *resource))
        }
        TypeDefKind::Handle(Handle::Borrow(resource)) => {
            format!("borrow<{}>", type_name(resolve, *resource))
        }
        TypeDefKind::Resource => format!("resource {}", type_name(resolve, id)),
        other => panic!("unsupported WIT type in host ABI: {other:?}"),
    }
}

fn ty_fmt(resolve: &Resolve) -> impl Fn(&Type) -> String + '_ {
    move |ty| describe_type(resolve, ty)
}

fn type_name(resolve: &Resolve, id: TypeId) -> String {
    let def = &resolve.types[id];
    let owner = match def.owner {
        TypeOwner::Interface(interface) => resolve.interfaces[interface]
            .name
            .clone()
            .unwrap_or_default(),
        _ => String::new(),
    };
    format!("{owner}.{}", def.name.clone().unwrap_or_default())
}

fn describe_function(resolve: &Resolve, function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, describe_type(resolve, &param.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let result = function
        .result
        .as_ref()
        .map_or_else(String::new, |ty| describe_type(resolve, ty));
    format!("{:?} func({params}) -> {result}", function.kind)
}

fn interface_surface(resolve: &Resolve, id: InterfaceId) -> BTreeMap<String, String> {
    let interface = &resolve.interfaces[id];
    let mut surface = BTreeMap::new();
    for (name, ty) in &interface.types {
        surface.insert(format!("type {name}"), describe_type_id(resolve, *ty));
    }
    for (name, function) in &interface.functions {
        surface.insert(format!("func {name}"), describe_function(resolve, function));
    }
    surface
}

fn world_key(resolve: &Resolve, key: &WorldKey, item: &WorldItem) -> String {
    match (key, item) {
        (WorldKey::Interface(id), _) => resolve.interfaces[*id].name.clone().unwrap_or_default(),
        (WorldKey::Name(name), _) => name.clone(),
    }
}

#[test]
fn released_host_abis_are_preserved() {
    let (current, current_package) = load(&Path::new(CRATE_DIR).join("wit-host"));
    let current_package = &current.packages[current_package];
    let current_version = current_package
        .name
        .version
        .clone()
        .expect("the host ABI package is versioned");
    assert_eq!(
        current_version.to_string(),
        crate::extensions::SUPPORTED_HOST_API,
        "SUPPORTED_HOST_API must name the wit-host package version"
    );

    let released_dir = Path::new(CRATE_DIR).join("wit-released");
    let mut released_versions = Vec::new();
    for entry in fs::read_dir(&released_dir).expect("wit-released exists") {
        let path = entry.expect("readable snapshot").path();
        let (released, package) = load(&path);
        let package = &released.packages[package];
        let version = package
            .name
            .version
            .clone()
            .expect("released snapshots are versioned");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(format!("catalog-host-{version}.wit").as_str()),
            "snapshot file name must match its package version"
        );
        assert_eq!(package.name.namespace, current_package.name.namespace);
        assert_eq!(package.name.name, current_package.name.name);
        assert!(
            version <= current_version,
            "released {version} is newer than wit-host {current_version}"
        );
        released_versions.push(version.clone());

        for (name, interface) in &package.interfaces {
            let current_interface = *current_package
                .interfaces
                .get(name)
                .unwrap_or_else(|| panic!("released {version} interface '{name}' was removed"));
            let expected = interface_surface(&released, *interface);
            let actual = interface_surface(&current, current_interface);
            for (item, signature) in expected {
                match actual.get(&item) {
                    Some(current_signature) => assert_eq!(
                        current_signature, &signature,
                        "released {version} {name}::{item} changed; add a new function or type instead"
                    ),
                    None => panic!("released {version} {name}::{item} was removed"),
                }
            }
        }

        for (name, world) in &package.worlds {
            let current_world = &current.worlds[*current_package
                .worlds
                .get(name)
                .unwrap_or_else(|| panic!("released {version} world '{name}' was removed"))];
            let world = &released.worlds[*world];
            let current_imports = current_world
                .imports
                .iter()
                .map(|(key, item)| world_key(&current, key, item))
                .collect::<Vec<_>>();
            for (key, item) in &world.imports {
                let key = world_key(&released, key, item);
                assert!(
                    current_imports.contains(&key),
                    "released {version} world '{name}' lost import '{key}'"
                );
            }
            let mut released_exports = world
                .exports
                .iter()
                .map(|(key, item)| world_key(&released, key, item))
                .collect::<Vec<_>>();
            let mut current_exports = current_world
                .exports
                .iter()
                .map(|(key, item)| world_key(&current, key, item))
                .collect::<Vec<_>>();
            released_exports.sort();
            current_exports.sort();
            assert_eq!(
                current_exports, released_exports,
                "released {version} world '{name}' must keep exactly its exports; add a new world instead"
            );
        }
    }
    assert!(
        released_versions.contains(&current_version),
        "wit-host {current_version} has no frozen snapshot; copy it to wit-released/catalog-host-{current_version}.wit when releasing"
    );
}
