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
    pub value_type: String,
    pub blueprints: Vec<String>,
}

/// Blueprints whose relationship of one code links to one set of targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TargetConflict {
    pub targets: Vec<String>,
    pub blueprints: Vec<String>,
}

/// Why a relationship path does not resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    /// No frontier blueprint declares the segment.
    Unknown { segment: String },
    /// The segment must be a relationship for the path to continue.
    NotRelationship { segment: String },
    /// The relationship may link to any blueprint, so the path cannot
    /// continue through it.
    Unrestricted { segment: String },
    /// Frontier blueprints declare the segment with different value types.
    TypeConflict {
        path: String,
        segment: String,
        conflicts: Vec<ValueTypeConflict>,
    },
    /// Frontier blueprints declare the relationship with different targets.
    TargetConflict {
        path: String,
        segment: String,
        conflicts: Vec<TargetConflict>,
    },
}

impl PathError {
    /// Whether frontier blueprints disagree on the segment, which callers
    /// report with the `relationship_path_type_conflict` error code.
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::TypeConflict { .. } | Self::TargetConflict { .. }
        )
    }

    /// The `details` of a `relationship_path_type_conflict` error.
    pub fn conflict_details(&self) -> Option<serde_json::Value> {
        match self {
            Self::TypeConflict {
                path,
                segment,
                conflicts,
            } => Some(serde_json::json!({
                "path": path,
                "segment": segment,
                "conflicts": conflicts,
            })),
            Self::TargetConflict {
                path,
                segment,
                conflicts,
            } => Some(serde_json::json!({
                "path": path,
                "segment": segment,
                "conflicts": conflicts,
            })),
            _ => None,
        }
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { segment } => write!(formatter, "segment '{segment}' was not found"),
            Self::NotRelationship { segment } => {
                write!(formatter, "segment '{segment}' must be a relationship")
            }
            Self::Unrestricted { segment } => write!(
                formatter,
                "relationship '{segment}' may link to any blueprint, so a path cannot continue past it"
            ),
            Self::TypeConflict {
                path,
                segment,
                conflicts,
            } => {
                write!(
                    formatter,
                    "path '{path}' segment '{segment}' has different value types on its target blueprints: "
                )?;
                let groups: Vec<_> = conflicts
                    .iter()
                    .map(|conflict| {
                        format!(
                            "{} = {}",
                            conflict.blueprints.join(", "),
                            conflict.value_type
                        )
                    })
                    .collect();
                formatter.write_str(&groups.join("; "))
            }
            Self::TargetConflict {
                path,
                segment,
                conflicts,
            } => {
                write!(
                    formatter,
                    "path '{path}' relationship '{segment}' links to different blueprints on its target blueprints: "
                )?;
                let groups: Vec<_> = conflicts
                    .iter()
                    .map(|conflict| {
                        format!(
                            "{} -> {}",
                            conflict.blueprints.join(", "),
                            if conflict.targets.is_empty() {
                                "any".to_owned()
                            } else {
                                conflict.targets.join(" | ")
                            }
                        )
                    })
                    .collect();
                formatter.write_str(&groups.join("; "))
            }
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

    /// The blueprints that declare the segment.
    pub fn declared_on(&self) -> Vec<String> {
        self.declarations
            .iter()
            .map(|(blueprint, _)| (*blueprint).to_owned())
            .collect()
    }

    /// Whether only some frontier blueprints declare the segment.
    pub fn is_partial(&self) -> bool {
        self.declarations.len() < self.frontier_len
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
            });
        }
        let mut groups: Vec<(BTreeSet<String>, Vec<String>, Vec<String>)> = Vec::new();
        for (blueprint, attribute) in &self.declarations {
            let targets = attribute.path_target_codes();
            let set: BTreeSet<_> = targets.iter().cloned().collect();
            match groups.iter_mut().find(|(group, _, _)| *group == set) {
                Some((_, _, blueprints)) => blueprints.push((*blueprint).to_owned()),
                None => groups.push((set, targets, vec![(*blueprint).to_owned()])),
            }
        }
        if groups.len() > 1 {
            return Err(PathError::TargetConflict {
                path: path.to_owned(),
                segment: self.code.clone(),
                conflicts: groups
                    .into_iter()
                    .map(|(_, targets, blueprints)| TargetConflict {
                        targets,
                        blueprints,
                    })
                    .collect(),
            });
        }
        let (_, targets, _) = groups.pop().expect("a segment has a declaration");
        if targets.is_empty() {
            return Err(PathError::Unrestricted {
                segment: self.code.clone(),
            });
        }
        Ok(targets)
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
            let value_type = attribute.path_value_type().as_str();
            match conflicts
                .iter_mut()
                .find(|conflict| conflict.value_type == value_type)
            {
                Some(conflict) => conflict.blueprints.push((*blueprint).to_owned()),
                None => conflicts.push(ValueTypeConflict {
                    value_type: value_type.to_owned(),
                    blueprints: vec![(*blueprint).to_owned()],
                }),
            }
        }
        return Err(PathError::TypeConflict {
            path: path.to_owned(),
            segment: segment.to_owned(),
            conflicts,
        });
    }
    Ok(Segment {
        code: segment.to_owned(),
        value_type,
        declarations,
        frontier_len: frontier.len(),
    })
}

/// A path of at least one relationship hop, walked through blueprints held
/// in memory.
#[derive(Debug)]
pub struct WalkedPath<'a, A> {
    /// The codes of the relationships the path follows, before the leaf.
    pub relationships: Vec<String>,
    /// The leaf, resolved against the blueprints the last relationship
    /// links to.
    pub leaf: Segment<'a, A>,
    /// The blueprints each relationship hop links to, in order.
    pub frontiers: Vec<Vec<&'a str>>,
    /// Whether every relationship holds a single value on every blueprint
    /// that declares it.
    pub single_valued: bool,
}

/// Why an in-memory walk failed: the path rule, or `load`'s own error.
#[derive(Debug)]
pub enum WalkError<E> {
    Path(PathError),
    Load(E),
}

/// A blueprint for an in-memory walk: its code and attributes.
pub type WalkBlueprint<'a, A> = (&'a str, &'a [A]);

/// Walks `parts`, at least one relationship and a leaf, from `source`.
/// `load` returns the blueprint of a target code, `None` when it has none to
/// declare segments, or its own error for a target the walk must not reach.
pub fn walk_path<'a, A, E>(
    path: &str,
    parts: &[&str],
    source: WalkBlueprint<'a, A>,
    mut load: impl FnMut(&str) -> Result<Option<WalkBlueprint<'a, A>>, E>,
) -> Result<WalkedPath<'a, A>, WalkError<E>>
where
    A: PathAttribute,
{
    debug_assert!(parts.len() >= 2, "a relationship path has a hop and a leaf");
    let mut frontier = vec![source];
    let mut relationships = Vec::new();
    let mut frontiers = Vec::new();
    let mut single_valued = true;
    for (index, part) in parts.iter().enumerate() {
        let declarations: Vec<(&'a str, Option<&'a A>)> = frontier
            .iter()
            .map(|(code, attributes)| {
                (
                    *code,
                    attributes
                        .iter()
                        .find(|attribute| attribute.path_code() == *part),
                )
            })
            .collect();
        let segment = resolve_segment(path, part, &declarations).map_err(WalkError::Path)?;
        if index + 1 == parts.len() {
            return Ok(WalkedPath {
                relationships,
                leaf: segment,
                frontiers,
                single_valued,
            });
        }
        single_valued &= segment.single_valued();
        let targets = segment.next_frontier(path).map_err(WalkError::Path)?;
        let mut next = Vec::with_capacity(targets.len());
        for target in &targets {
            if let Some(blueprint) = load(target).map_err(WalkError::Load)? {
                next.push(blueprint);
            }
        }
        frontiers.push(next.iter().map(|(code, _)| *code).collect());
        relationships.push((*part).to_owned());
        frontier = next;
    }
    unreachable!("the loop returns at the leaf")
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
        assert_eq!(segment.declared_on(), ["sheet", "profile"]);
        assert!(segment.is_partial());

        let all = [("sheet", Some(&number)), ("part", Some(&number))];
        assert!(
            !resolve_segment("spec.width_mm", "width_mm", &all)
                .unwrap()
                .is_partial()
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
        assert!(error.is_conflict());
        assert_eq!(
            error,
            PathError::TypeConflict {
                path: "spec.thickness_mm".to_owned(),
                segment: "thickness_mm".to_owned(),
                conflicts: vec![
                    ValueTypeConflict {
                        value_type: "number".to_owned(),
                        blueprints: vec!["sheet".to_owned(), "profile".to_owned()],
                    },
                    ValueTypeConflict {
                        value_type: "string".to_owned(),
                        blueprints: vec!["part".to_owned()],
                    },
                ],
            }
        );
        assert_eq!(
            error.to_string(),
            "path 'spec.thickness_mm' segment 'thickness_mm' has different value types on its target blueprints: sheet, profile = number; part = string"
        );
        assert_eq!(
            error.conflict_details().unwrap()["conflicts"][1]["blueprints"],
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
        assert!(error.is_conflict());
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
                segment: "width_mm".to_owned()
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
    fn a_walk_follows_every_target_and_reports_the_frontiers() {
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
        let walked = walk_path(
            "spec.grade.code",
            &["spec", "grade", "code"],
            ("product", &product[..]),
            load,
        )
        .unwrap();
        assert_eq!(walked.relationships, ["spec", "grade"]);
        assert_eq!(walked.frontiers, [vec!["sheet", "part"], vec!["grade"]]);
        assert_eq!(walked.leaf.value_type, ValueType::String);
        assert!(walked.single_valued);

        let width = walk_path(
            "spec.width_mm",
            &["spec", "width_mm"],
            ("product", &product[..]),
            load,
        )
        .unwrap();
        assert_eq!(width.leaf.declared_on(), ["sheet"]);
        assert!(width.leaf.is_partial());
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
