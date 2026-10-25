//! The Explorer search state stored by named views, share links and saved
//! searches that solution packs install. Every write path validates and
//! normalizes the state here, so equal searches are stored identically.
use attricat_domain::attribute_options::Cardinality;
use attricat_domain::constants::DEFAULT_CONTEXT_CODE;
use attricat_domain::states::ExportStatus;
use attricat_domain::value_type::ValueType;
use serde_json::Value;

pub const EXPLORER_SEARCH_KIND: &str = "explorer_search";
pub const MAX_STATE_BYTES: usize = 32 * 1024;
pub(crate) const MAX_FILTERS: usize = 20;
pub const MAX_SELECTED_IDS: usize = 100;
pub const MAX_BLUEPRINT_BYTES: usize = 256;
/// Longest Explorer field path, in bytes.
pub(crate) const MAX_FIELD_PATH_BYTES: usize = 512;
/// Explorer follows at most three relationship hops before the leaf field.
pub const MAX_RELATIONSHIP_HOPS: usize = 3;
/// Top-level state fields, named like the Explorer URL keys.
pub(crate) const STATE_KEYS: &[&str] = &[
    "blueprint",
    "version",
    "allVersions",
    "query",
    "context",
    "locked",
    "sort",
    "relationshipFacets",
    "attributeFilters",
    "browseBy",
    "exportStatus",
];
pub const FILTER_OPERATOR_IS_SET: &str = "is_set";
pub const FILTER_OPERATORS: &[&str] = &[
    "eq",
    "contains",
    "starts_with",
    "gt",
    "gte",
    "lt",
    "lte",
    FILTER_OPERATOR_IS_SET,
];
/// Explorer sort fields that are not attribute paths.
pub const SYSTEM_SORT_FIELDS: &[&str] = &["blueprint_version", "publication_status"];

fn non_empty_str(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn valid_attribute_filter(filter: &Value) -> bool {
    filter.as_object().is_some_and(|filter| {
        filter
            .keys()
            .all(|key| matches!(key.as_str(), "field" | "operator" | "value"))
            && non_empty_str(filter.get("field")).is_some()
            && filter
                .get("operator")
                .and_then(Value::as_str)
                .is_some_and(|operator| FILTER_OPERATORS.contains(&operator))
            && filter.get("value").is_some_and(|value| {
                if filter.get("operator").and_then(Value::as_str) == Some(FILTER_OPERATOR_IS_SET) {
                    value.is_boolean()
                } else {
                    value.is_string() || value.is_number() || value.is_boolean()
                }
            })
    })
}

/// `{extension, connection?, status}`: records with that export status at
/// one destination.
fn valid_export_status(filter: &Value) -> bool {
    filter.as_object().is_some_and(|filter| {
        filter
            .keys()
            .all(|key| matches!(key.as_str(), "extension" | "connection" | "status"))
            && non_empty_str(filter.get("extension")).is_some_and(|id| id.len() <= 128)
            && filter.get("connection").is_none_or(|connection| {
                connection
                    .as_str()
                    .is_some_and(crate::is_valid_connection_key)
            })
            && filter
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| ExportStatus::parse(status).is_some())
    })
}

fn valid_relationship_facet(facet: &Value) -> bool {
    facet.as_object().is_some_and(|facet| {
        facet
            .keys()
            .all(|key| {
                matches!(
                    key.as_str(),
                    "field" | "selectedIds" | "targetBlueprint" | "hierarchyField" | "ownTree"
                )
            })
            && non_empty_str(facet.get("field")).is_some()
            && facet.get("selectedIds").is_none_or(|ids| {
                ids.as_array().is_some_and(|ids| {
                    ids.len() <= MAX_SELECTED_IDS
                        && ids.iter().all(|id| {
                            id.as_str()
                                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
                        })
                })
            })
            && facet.get("targetBlueprint").is_none_or(Value::is_string)
            // A browse rail tree: the target's hierarchy field, or the
            // searched blueprint's own tree.
            && facet
                .get("hierarchyField")
                .is_none_or(|field| non_empty_str(Some(field)).is_some())
            && facet
                .get("ownTree")
                .is_none_or(|own_tree| own_tree == &Value::Bool(true))
    })
}

/// The browse rail's field: `false` when hidden, or a relationship path of
/// at most [`MAX_RELATIONSHIP_HOPS`] hops.
fn valid_browse_by(browse_by: &Value) -> bool {
    match browse_by {
        Value::Bool(hidden) => !hidden,
        Value::String(path) => {
            field_path(path).is_ok_and(|segments| segments.len() <= MAX_RELATIONSHIP_HOPS)
        }
        _ => false,
    }
}

/// Validates the shape of an Explorer search state. It does not resolve
/// blueprint, context or attribute references.
pub fn validate_state(kind: &str, state: &Value) -> Result<(), String> {
    if kind != EXPLORER_SEARCH_KIND {
        return Err("unsupported saved view kind".into());
    }
    let bytes = serde_json::to_vec(state).map_err(|_| "invalid state".to_owned())?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("search state exceeds 32 KiB".into());
    }
    let object = state
        .as_object()
        .ok_or_else(|| "search state must be an object".to_owned())?;
    if object.keys().any(|key| !STATE_KEYS.contains(&key.as_str()))
        || !object
            .get("blueprint")
            .and_then(Value::as_str)
            .is_some_and(|code| !code.trim().is_empty() && code.len() <= MAX_BLUEPRINT_BYTES)
    {
        return Err("invalid Explorer search state".into());
    }
    if let Some(filters) = object.get("attributeFilters") {
        let filters = filters
            .as_array()
            .ok_or_else(|| "attributeFilters must be an array".to_owned())?;
        if filters.len() > MAX_FILTERS || !filters.iter().all(valid_attribute_filter) {
            return Err("invalid attributeFilters".into());
        }
    }
    if let Some(facets) = object.get("relationshipFacets") {
        let facets = facets
            .as_array()
            .ok_or_else(|| "relationshipFacets must be an array".to_owned())?;
        if facets.len() > MAX_FILTERS || !facets.iter().all(valid_relationship_facet) {
            return Err("invalid relationshipFacets".into());
        }
    }
    let invalid_sort = object.get("sort").is_some_and(|sort| {
        sort.as_object().is_none_or(|sort| {
            sort.len() != 2
                || non_empty_str(sort.get("field")).is_none()
                || !matches!(
                    sort.get("direction").and_then(Value::as_str),
                    Some("asc" | "desc")
                )
        })
    });
    if invalid_sort
        || object
            .get("browseBy")
            .is_some_and(|value| !valid_browse_by(value))
        || object
            .get("exportStatus")
            .is_some_and(|value| !valid_export_status(value))
        || object
            .get("version")
            .is_some_and(|version| version.as_u64().is_none_or(|version| version == 0))
        || ["allVersions", "locked"]
            .iter()
            .any(|key| object.get(*key).is_some_and(|value| !value.is_boolean()))
        || ["query", "context"].iter().any(|key| {
            object
                .get(*key)
                .is_some_and(|value| value.as_str().is_none_or(str::is_empty))
        })
    {
        return Err("invalid Explorer search state".into());
    }
    Ok(())
}

/// Normalizes a validated state: trims codes and the query, and drops
/// defaults (false flags, empty lists, the default context, a blank query).
pub fn normalize_state(state: &Value) -> Value {
    let mut state = state.clone();
    let Some(object) = state.as_object_mut() else {
        return state;
    };
    for key in ["blueprint", "query", "context"] {
        if let Some(Value::String(value)) = object.get_mut(key) {
            *value = value.trim().to_owned();
        }
    }
    for key in ["allVersions", "locked"] {
        if object.get(key) == Some(&Value::Bool(false)) {
            object.remove(key);
        }
    }
    for key in ["attributeFilters", "relationshipFacets"] {
        if object
            .get(key)
            .is_some_and(|value| value.as_array().is_some_and(Vec::is_empty))
        {
            object.remove(key);
        }
    }
    if object.get("context").and_then(Value::as_str) == Some(DEFAULT_CONTEXT_CODE) {
        object.remove("context");
    }
    if object.get("query").and_then(Value::as_str) == Some("") {
        object.remove("query");
    }
    state
}

/// Whether one hop of a relationship path of `hops` hops can be browsed: a
/// relationship with one target blueprint and, on a path of several hops, a
/// single value (`cardinality = "one"`), as for sortable table paths. Each
/// source then reaches at most one node of the browsed blueprint.
pub fn browse_path_hop(
    value_type: ValueType,
    target_blueprint: Option<&str>,
    cardinality: Option<Cardinality>,
    hops: usize,
) -> bool {
    value_type == ValueType::Relationship
        && target_blueprint.is_some()
        && (hops == 1 || cardinality == Some(Cardinality::One))
}

/// Splits an Explorer field path into relationship hops and the leaf, and
/// checks the hop limit. Every segment must be a valid code.
pub fn field_path(path: &str) -> Result<Vec<&str>, String> {
    let segments = path.split('.').collect::<Vec<_>>();
    if path.len() > MAX_FIELD_PATH_BYTES
        || !segments.iter().all(|segment| crate::is_valid_code(segment))
    {
        return Err(format!("field '{path}' is not a valid attribute path"));
    }
    if segments.len() > MAX_RELATIONSHIP_HOPS + 1 {
        return Err(format!(
            "field '{path}' follows more than {MAX_RELATIONSHIP_HOPS} relationship hops"
        ));
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn export_status_filters_are_saved_and_checked() {
        let filtered = |filter| json!({"blueprint": "product", "exportStatus": filter});
        validate_state(
            EXPLORER_SEARCH_KIND,
            &filtered(
                json!({"extension": "acme.shop", "connection": "shop-eu", "status": "rejected"}),
            ),
        )
        .unwrap();
        validate_state(
            EXPLORER_SEARCH_KIND,
            &filtered(json!({"extension": "acme.shop", "status": "synced"})),
        )
        .unwrap();
        for invalid in [
            json!({"extension": "acme.shop", "status": "lost"}),
            json!({"extension": "", "status": "synced"}),
            json!({"extension": "acme.shop", "connection": "a b", "status": "synced"}),
            json!({"extension": "acme.shop", "status": "synced", "extra": 1}),
        ] {
            assert!(validate_state(EXPLORER_SEARCH_KIND, &filtered(invalid)).is_err());
        }
    }

    #[test]
    fn validates_and_normalizes_like_every_writer() {
        let state = json!({"blueprint":" product ","query":"   ","locked":false,"context":"default","attributeFilters":[]});
        validate_state(EXPLORER_SEARCH_KIND, &state).unwrap();
        assert_eq!(normalize_state(&state), json!({"blueprint":"product"}));
        assert!(validate_state("dashboard", &state).is_err());
        assert!(validate_state(EXPLORER_SEARCH_KIND, &json!({"blueprint":"p","extra":1})).is_err());
        assert!(
            validate_state(EXPLORER_SEARCH_KIND, &json!({"blueprint":"p","query":""})).is_err()
        );
        assert!(
            validate_state(
                EXPLORER_SEARCH_KIND,
                &json!({"blueprint":"p","sort":{"field":"a","direction":"up"}})
            )
            .is_err()
        );
    }

    #[test]
    fn presence_filters_require_boolean_values_and_preserve_false() {
        for value in [json!(true), json!(false)] {
            let state = json!({"blueprint":"task","attributeFilters":[{
                "field":"assignee","operator":"is_set","value":value
            }]});
            validate_state(EXPLORER_SEARCH_KIND, &state).unwrap();
            assert_eq!(normalize_state(&state), state);
        }
        for value in [json!("false"), json!(0), Value::Null, json!([])] {
            assert!(
                validate_state(
                    EXPLORER_SEARCH_KIND,
                    &json!({
                        "blueprint":"task","attributeFilters":[{
                            "field":"assignee","operator":"is_set","value":value
                        }]
                    })
                )
                .is_err()
            );
        }
    }

    #[test]
    fn browse_rail_state_round_trips() {
        let id = uuid::Uuid::new_v4();
        for browse_by in [json!("model.brand"), json!("category"), json!(false)] {
            let state = json!({
                "blueprint": "vehicle",
                "browseBy": browse_by,
                "relationshipFacets": [
                    {"field": "model.body_type", "selectedIds": [id], "targetBlueprint": "body_type", "hierarchyField": "parent"},
                    {"field": "parent", "selectedIds": [id], "hierarchyField": "parent", "ownTree": true},
                ],
            });
            validate_state(EXPLORER_SEARCH_KIND, &state).unwrap();
            assert_eq!(normalize_state(&state), state);
        }
        for browse_by in [
            json!(true),
            json!(""),
            json!("a.b.c.d"),
            json!("a..b"),
            json!(1),
        ] {
            assert!(
                validate_state(
                    EXPLORER_SEARCH_KIND,
                    &json!({"blueprint": "vehicle", "browseBy": browse_by})
                )
                .is_err()
            );
        }
        for facet in [
            json!({"field": "model.brand", "hierarchyField": ""}),
            json!({"field": "parent", "ownTree": false}),
        ] {
            assert!(
                validate_state(
                    EXPLORER_SEARCH_KIND,
                    &json!({"blueprint": "vehicle", "relationshipFacets": [facet]})
                )
                .is_err()
            );
        }
    }

    #[test]
    fn field_paths_follow_at_most_three_hops() {
        assert_eq!(field_path("a.b.c.d").unwrap().len(), 4);
        assert!(field_path("a.b.c.d.e").is_err());
        assert!(field_path("a..b").is_err());
    }
}
