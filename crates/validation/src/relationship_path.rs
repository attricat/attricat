//! How a relationship path such as `spec.grade.code` resolves through
//! relationships that may link to several target blueprints.
//!
//! A path walks a *frontier*: the blueprints its last hop may reach. It starts
//! as the source blueprint. Each segment resolves against the latest published
//! revision of every frontier blueprint:
//!
//! - it is valid when at least one frontier blueprint declares it;
//! - every blueprint that declares it must agree on the value type, and, for a
//!   relationship the path continues through, on its set of target
//!   blueprints, which becomes the next frontier;
//! - a record whose blueprint lacks the segment simply has no value there.
//!
//! An unrestricted relationship (no target blueprints) cannot be continued
//! through, since no published revision describes what lies beyond it.
//!
//! The repository, solution-pack validation and their callers share this rule
//! so table columns, sorts, filters, saved searches and the search syntax
//! accept the same paths.

use std::collections::BTreeSet;
use std::fmt;

use attricat_domain::attribute_options::Cardinality;
use attricat_domain::model::Attribute;
use attricat_domain::value_type::ValueType;
use serde::Serialize;

/// An attribute as a relationship path sees it.
pub trait PathAttribute {
    fn path_code(&self) -> &str;
    fn path_value_type(&self) -> ValueType;
    /// The relationship's declared target blueprints; empty when it may link
    /// to any record blueprint.
    fn path_target_codes(&self) -> Vec<String>;
    fn path_cardinality(&self) -> Option<Cardinality>;
}

impl PathAttribute for Attribute {
    fn path_code(&self) -> &str {
        &self.code
    }

    fn path_value_type(&self) -> ValueType {
        self.value_type
    }

    fn path_target_codes(&self) -> Vec<String> {
        self.declared_target_codes()
    }

    fn path_cardinality(&self) -> Option<Cardinality> {
        self.cardinality
    }
}

/// Blueprints that declare a path segment with one value type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValueTypeConflict {
    pub value_type: ValueType,
    pub blueprints: Vec<String>,
}

/// Blueprints whose relationship of one code links to one set of targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TargetConflict {
    pub targets: Vec<String>,
    pub blueprints: Vec<String>,
}

/// How the blueprints a path reaches disagree on a segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Conflicts {
    /// They declare it with different value types.
    ValueTypes(Vec<ValueTypeConflict>),
    /// They declare it as relationships to different blueprints.
    Targets(Vec<TargetConflict>),
}

/// A segment the blueprints a path reaches declare differently, reported
/// with the `relationship_path_type_conflict` error code; it serializes as
/// that error's `details`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PathConflict {
    pub path: String,
    pub segment: String,
    pub conflicts: Conflicts,
}

/// Lists blueprint codes in messages.
const BLUEPRINT_LIST_SEPARATOR: &str = ", ";
/// Separates the groups of a conflict in its message.
const CONFLICT_GROUP_SEPARATOR: &str = "; ";
/// Joins a relationship's target blueprints, as in `target_blueprints`
/// listings.
const TARGET_SEPARATOR: &str = " | ";
/// Describes a relationship that names no target blueprint.
const ANY_TARGET: &str = "any blueprint";

impl fmt::Display for PathConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            path,
            segment,
            conflicts,
        } = self;
        let groups: Vec<String> = match conflicts {
            Conflicts::ValueTypes(conflicts) => {
                write!(
                    formatter,
                    "path '{path}' segment '{segment}' has different value types on its target blueprints: "
                )?;
                conflicts
                    .iter()
                    .map(|conflict| {
                        format!(
                            "{} = {}",
                            conflict.blueprints.join(BLUEPRINT_LIST_SEPARATOR),
                            conflict.value_type
                        )
                    })
                    .collect()
            }
            Conflicts::Targets(conflicts) => {
                write!(
                    formatter,
                    "path '{path}' relationship '{segment}' links to different blueprints on its target blueprints: "
                )?;
                conflicts
                    .iter()
                    .map(|conflict| {
                        let targets = if conflict.targets.is_empty() {
                            ANY_TARGET.to_owned()
                        } else {
                            conflict.targets.join(TARGET_SEPARATOR)
                        };
                        format!(
                            "{} -> {targets}",
                            conflict.blueprints.join(BLUEPRINT_LIST_SEPARATOR)
                        )
                    })
                    .collect()
            }
        };
        formatter.write_str(&groups.join(CONFLICT_GROUP_SEPARATOR))
    }
}

/// Why a relationship path does not resolve. Its message completes a
/// sentence about the path, such as "table column 'spec.x' …".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// No frontier blueprint declares the segment.
    Unknown { segment: String },
    /// The segment must be a relationship for the path to continue.
    NotRelationship {
        segment: String,
        value_type: ValueType,
    },
    /// The relationship may link to any blueprint, so the path cannot
    /// continue through it.
    Unrestricted { segment: String },
    /// Frontier blueprints declare the segment differently.
    Conflict(PathConflict),
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { segment } => {
                write!(formatter, "references unknown attribute '{segment}'")
            }
            Self::NotRelationship {
                segment,
                value_type,
            } => write!(
                formatter,
                "segment '{segment}' is a {value_type} attribute, not a relationship"
            ),
            Self::Unrestricted { segment } => write!(
                formatter,
                "relationship '{segment}' may link to any blueprint, so a path cannot continue past it"
            ),
            Self::Conflict(conflict) => conflict.fmt(formatter),
        }
    }
}

impl std::error::Error for PathError {}

/// A path segment resolved against a frontier.
#[derive(Debug)]
pub struct Segment<'a, A> {
    pub code: String,
    pub value_type: ValueType,
    /// The frontier blueprints that declare the segment, with their
    /// declarations, in frontier order.
    pub declarations: Vec<(&'a str, &'a A)>,
    /// How many blueprints the frontier had.
    pub frontier_len: usize,
}

impl<'a, A: PathAttribute> Segment<'a, A> {
    /// The first declaration, whose name and options the path displays.
    pub fn first(&self) -> &'a A {
        self.declarations[0].1
    }

    /// The blueprints that declare the segment when only some frontier
    /// blueprints do; `None` when all of them do.
    pub fn partially_declared_on(&self) -> Option<Vec<String>> {
        (self.declarations.len() < self.frontier_len).then(|| {
            self.declarations
                .iter()
                .map(|(blueprint, _)| (*blueprint).to_owned())
                .collect()
        })
    }

    /// Whether every declaration holds a single value.
    pub fn single_valued(&self) -> bool {
        self.declarations
            .iter()
            .all(|(_, attribute)| attribute.path_cardinality() == Some(Cardinality::One))
    }

    /// The frontier past this relationship: the target blueprints its every
    /// declaration agrees on, in the first declaration's order.
    pub fn next_frontier(&self, path: &str) -> Result<Vec<String>, PathError> {
        if self.value_type != ValueType::Relationship {
            return Err(PathError::NotRelationship {
                segment: self.code.clone(),
                value_type: self.value_type,
            });
        }
        let mut groups: Vec<(BTreeSet<String>, TargetConflict)> = Vec::new();
        for (blueprint, attribute) in &self.declarations {
            let targets = attribute.path_target_codes();
            let set: BTreeSet<_> = targets.iter().cloned().collect();
            match groups.iter_mut().find(|(group, _)| *group == set) {
                Some((_, conflict)) => conflict.blueprints.push((*blueprint).to_owned()),
                None => groups.push((
                    set,
                    TargetConflict {
                        targets,
                        blueprints: vec![(*blueprint).to_owned()],
                    },
                )),
            }
        }
        if groups.len() > 1 {
            return Err(PathError::Conflict(PathConflict {
                path: path.to_owned(),
                segment: self.code.clone(),
                conflicts: Conflicts::Targets(
                    groups.into_iter().map(|(_, conflict)| conflict).collect(),
                ),
            }));
        }
        let (_, agreed) = groups.pop().expect("a segment has a declaration");
        if agreed.targets.is_empty() {
            return Err(PathError::Unrestricted {
                segment: self.code.clone(),
            });
        }
        Ok(agreed.targets)
    }
}

/// Resolves `segment` of `path` against a frontier: each frontier blueprint
/// with its declaration of the segment, if it has one.
pub fn resolve_segment<'a, A: PathAttribute>(
    path: &str,
    segment: &str,
    frontier: &[(&'a str, Option<&'a A>)],
) -> Result<Segment<'a, A>, PathError> {
    let declarations: Vec<(&'a str, &'a A)> = frontier
        .iter()
        .filter_map(|(blueprint, attribute)| attribute.map(|attribute| (*blueprint, attribute)))
        .collect();
    let Some((_, first)) = declarations.first() else {
        return Err(PathError::Unknown {
            segment: segment.to_owned(),
        });
    };
    let value_type = first.path_value_type();
    if declarations
        .iter()
        .any(|(_, attribute)| attribute.path_value_type() != value_type)
    {
        let mut conflicts: Vec<ValueTypeConflict> = Vec::new();
        for (blueprint, attribute) in &declarations {
            let value_type = attribute.path_value_type();
            match conflicts
                .iter_mut()
                .find(|conflict| conflict.value_type == value_type)
            {
                Some(conflict) => conflict.blueprints.push((*blueprint).to_owned()),
                None => conflicts.push(ValueTypeConflict {
                    value_type,
                    blueprints: vec![(*blueprint).to_owned()],
                }),
            }
        }
        return Err(PathError::Conflict(PathConflict {
            path: path.to_owned(),
            segment: segment.to_owned(),
            conflicts: Conflicts::ValueTypes(conflicts),
        }));
    }
    Ok(Segment {
        code: segment.to_owned(),
        value_type,
        declarations,
        frontier_len: frontier.len(),
    })
}

/// How a path compares a segment with attribute codes.
#[derive(Clone, Copy, Debug)]
pub enum CodeMatch {
    /// Exactly, as field paths name attributes.
    Exact,
    /// Ignoring ASCII case, as the search syntax does.
    IgnoreCase,
}

impl CodeMatch {
    pub fn matches(self, code: &str, segment: &str) -> bool {
        match self {
            Self::Exact => code == segment,
            Self::IgnoreCase => code.eq_ignore_ascii_case(segment),
        }
    }
}

/// Resolves `segment` of `path` against a frontier of blueprints, each a
/// code and its attributes: the step every path walk takes per segment.
pub fn resolve_in<'a, A: PathAttribute + 'a>(
    path: &str,
    segment: &str,
    frontier: impl IntoIterator<Item = (&'a str, &'a [A])>,
    code_match: CodeMatch,
) -> Result<Segment<'a, A>, PathError> {
    let declarations: Vec<(&'a str, Option<&'a A>)> = frontier
        .into_iter()
        .map(|(code, attributes)| {
            (
                code,
                attributes
                    .iter()
                    .find(|attribute| code_match.matches(attribute.path_code(), segment)),
            )
        })
        .collect();
    resolve_segment(path, segment, &declarations)
}

/// Why an in-memory walk failed: the path rule, or `load`'s own error.
#[derive(Debug)]
pub enum WalkError<E> {
    Path(PathError),
    Load(E),
}

/// A blueprint for an in-memory walk: its code and attributes.
pub type WalkBlueprint<'a, A> = (&'a str, &'a [A]);

/// Walks `parts`, at least one relationship and a leaf, from `source`, and
/// returns the leaf. `load` returns the blueprint of a target code, `None`
/// when it has none to declare segments, or its own error for a target the
/// walk must not reach.
pub fn walk_path<'a, A, E>(
    path: &str,
    parts: &[&str],
    source: WalkBlueprint<'a, A>,
    mut load: impl FnMut(&str) -> Result<Option<WalkBlueprint<'a, A>>, E>,
) -> Result<Segment<'a, A>, WalkError<E>>
where
    A: PathAttribute,
{
    debug_assert!(parts.len() >= 2, "a relationship path has a hop and a leaf");
    let (leaf, hops) = parts.split_last().expect("a path has a leaf");
    let mut frontier = vec![source];
    for hop in hops {
        let targets = resolve_in(path, hop, frontier.iter().copied(), CodeMatch::Exact)
            .and_then(|segment| segment.next_frontier(path))
            .map_err(WalkError::Path)?;
        frontier = Vec::with_capacity(targets.len());
        for target in &targets {
            if let Some(blueprint) = load(target).map_err(WalkError::Load)? {
                frontier.push(blueprint);
            }
        }
    }
    resolve_in(path, leaf, frontier, CodeMatch::Exact).map_err(WalkError::Path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Field {
        value_type: ValueType,
        targets: Vec<&'static str>,
        cardinality: Option<Cardinality>,
    }

    impl PathAttribute for Field {
        fn path_code(&self) -> &str {
            ""
        }
        fn path_value_type(&self) -> ValueType {
            self.value_type
        }
        fn path_target_codes(&self) -> Vec<String> {
            self.targets.iter().map(|code| (*code).to_owned()).collect()
        }
        fn path_cardinality(&self) -> Option<Cardinality> {
            self.cardinality
        }
    }

    fn scalar(value_type: ValueType) -> Field {
        Field {
            value_type,
            targets: Vec::new(),
            cardinality: None,
        }
    }

    fn relationship(targets: &[&'static str]) -> Field {
        Field {
            value_type: ValueType::Relationship,
            targets: targets.to_vec(),
            cardinality: Some(Cardinality::One),
        }
    }

    #[test]
    fn a_segment_resolves_on_the_blueprints_that_declare_it() {
        let number = scalar(ValueType::Number);
        let frontier = [
            ("sheet", Some(&number)),
            ("profile", Some(&number)),
            ("part", None),
        ];
        let segment = resolve_segment("spec.width_mm", "width_mm", &frontier).unwrap();
        assert_eq!(segment.value_type, ValueType::Number);
        assert_eq!(
            segment.partially_declared_on().unwrap(),
            ["sheet", "profile"]
        );

        let all = [("sheet", Some(&number)), ("part", Some(&number))];
        assert!(
            resolve_segment("spec.width_mm", "width_mm", &all)
                .unwrap()
                .partially_declared_on()
                .is_none()
        );
    }

    #[test]
    fn an_undeclared_segment_is_unknown() {
        let frontier: [(&str, Option<&Field>); 2] = [("sheet", None), ("part", None)];
        assert_eq!(
            resolve_segment("spec.color", "color", &frontier).unwrap_err(),
            PathError::Unknown {
                segment: "color".to_owned()
            }
        );
    }

    #[test]
    fn different_value_types_conflict_and_name_the_blueprints() {
        let number = scalar(ValueType::Number);
        let string = scalar(ValueType::String);
        let frontier = [
            ("sheet", Some(&number)),
            ("profile", Some(&number)),
            ("part", Some(&string)),
        ];
        let error = resolve_segment("spec.thickness_mm", "thickness_mm", &frontier).unwrap_err();
        let PathError::Conflict(conflict) = &error else {
            panic!("{error:?} is not a conflict");
        };
        assert_eq!(
            conflict,
            &PathConflict {
                path: "spec.thickness_mm".to_owned(),
                segment: "thickness_mm".to_owned(),
                conflicts: Conflicts::ValueTypes(vec![
                    ValueTypeConflict {
                        value_type: ValueType::Number,
                        blueprints: vec!["sheet".to_owned(), "profile".to_owned()],
                    },
                    ValueTypeConflict {
                        value_type: ValueType::String,
                        blueprints: vec!["part".to_owned()],
                    },
                ]),
            }
        );
        assert_eq!(
            error.to_string(),
            "path 'spec.thickness_mm' segment 'thickness_mm' has different value types on its target blueprints: sheet, profile = number; part = string"
        );
        assert_eq!(
            serde_json::to_value(conflict).unwrap()["conflicts"][1],
            serde_json::json!({"value_type": "string", "blueprints": ["part"]})
        );
        assert_eq!(
            serde_json::to_value(conflict).unwrap()["conflicts"][1]["blueprints"],
            serde_json::json!(["part"])
        );
    }

    #[test]
    fn a_relationship_continues_in_the_targets_its_declarations_agree_on() {
        let first = relationship(&["grade", "alloy"]);
        let reordered = relationship(&["alloy", "grade"]);
        let frontier = [("sheet", Some(&first)), ("part", Some(&reordered))];
        let segment = resolve_segment("spec.grade.code", "grade", &frontier).unwrap();
        assert_eq!(
            segment.next_frontier("spec.grade.code").unwrap(),
            ["grade", "alloy"]
        );
        assert!(segment.single_valued());
    }

    #[test]
    fn relationships_to_different_targets_conflict() {
        let material = relationship(&["material_grade"]);
        let steel = relationship(&["steel_grade"]);
        let frontier = [
            ("sheet", Some(&material)),
            ("profile", Some(&material)),
            ("part", Some(&steel)),
        ];
        let segment = resolve_segment("spec.grade.code", "grade", &frontier).unwrap();
        let error = segment.next_frontier("spec.grade.code").unwrap_err();
        assert!(matches!(error, PathError::Conflict(_)));
        assert_eq!(
            error.to_string(),
            "path 'spec.grade.code' relationship 'grade' links to different blueprints on its target blueprints: sheet, profile -> material_grade; part -> steel_grade"
        );
    }

    #[test]
    fn an_unrestricted_relationship_does_not_continue() {
        let any = relationship(&[]);
        let frontier = [("product", Some(&any))];
        let segment = resolve_segment("link.code", "link", &frontier).unwrap();
        assert_eq!(
            segment.next_frontier("link.code").unwrap_err(),
            PathError::Unrestricted {
                segment: "link".to_owned()
            }
        );
    }

    #[test]
    fn a_scalar_does_not_continue() {
        let number = scalar(ValueType::Number);
        let frontier = [("sheet", Some(&number))];
        let segment = resolve_segment("spec.width_mm.x", "width_mm", &frontier).unwrap();
        assert_eq!(
            segment.next_frontier("spec.width_mm.x").unwrap_err(),
            PathError::NotRelationship {
                segment: "width_mm".to_owned(),
                value_type: ValueType::Number,
            }
        );
    }

    #[derive(Debug)]
    struct Named {
        code: &'static str,
        field: Field,
    }

    impl PathAttribute for Named {
        fn path_code(&self) -> &str {
            self.code
        }
        fn path_value_type(&self) -> ValueType {
            self.field.value_type
        }
        fn path_target_codes(&self) -> Vec<String> {
            self.field.path_target_codes()
        }
        fn path_cardinality(&self) -> Option<Cardinality> {
            self.field.cardinality
        }
    }

    #[test]
    fn a_walk_follows_every_target_to_the_leaf() {
        let product = [Named {
            code: "spec",
            field: relationship(&["sheet", "part"]),
        }];
        let sheet = [
            Named {
                code: "grade",
                field: relationship(&["grade"]),
            },
            Named {
                code: "width_mm",
                field: scalar(ValueType::Number),
            },
        ];
        let part = [Named {
            code: "grade",
            field: relationship(&["grade"]),
        }];
        let grade = [Named {
            code: "code",
            field: scalar(ValueType::String),
        }];
        let load = |code: &str| -> Result<Option<WalkBlueprint<'_, Named>>, ()> {
            Ok(match code {
                "sheet" => Some(("sheet", &sheet[..])),
                "part" => Some(("part", &part[..])),
                "grade" => Some(("grade", &grade[..])),
                _ => None,
            })
        };
        let code = walk_path(
            "spec.grade.code",
            &["spec", "grade", "code"],
            ("product", &product[..]),
            load,
        )
        .unwrap();
        assert_eq!(code.value_type, ValueType::String);
        assert_eq!(code.declarations[0].0, "grade");

        let width = walk_path(
            "spec.width_mm",
            &["spec", "width_mm"],
            ("product", &product[..]),
            load,
        )
        .unwrap();
        assert_eq!(width.partially_declared_on().unwrap(), ["sheet"]);
    }

    #[test]
    fn sorting_needs_every_declaration_single_valued() {
        let one = relationship(&["grade"]);
        let mut many = relationship(&["grade"]);
        many.cardinality = Some(Cardinality::Many);
        let frontier = [("sheet", Some(&one)), ("part", Some(&many))];
        assert!(
            !resolve_segment("spec.grade.code", "grade", &frontier)
                .unwrap()
                .single_valued()
        );
    }
}
