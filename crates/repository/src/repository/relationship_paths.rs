//! Walks a relationship path such as `spec.grade.code` through the latest
//! published revisions of the blueprints each hop links to, by the rule in
//! [`attricat_validation::relationship_path`]. Table columns, sorts, filters
//! and the search syntax all resolve their paths here.

use attricat_domain::model::Attribute;
use attricat_domain::value_type::ValueType;
use attricat_validation::relationship_path::{PathAttribute, PathError, resolve_segment};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// A blueprint a path reaches, as its latest published revision.
#[derive(Clone, Debug)]
pub(crate) struct PathBlueprint {
    pub id: Uuid,
    pub code: String,
    pub attributes: Vec<Attribute>,
}

/// A path of at least one relationship hop and its leaf.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedPath {
    /// The codes of the relationships the path follows, before the leaf.
    pub relationships: Vec<String>,
    pub value_type: ValueType,
    /// The leaf's first declaration, whose name and options the path shows.
    pub leaf: Attribute,
    /// The blueprints the last relationship links to.
    pub leaf_frontier: Vec<PathBlueprint>,
    /// The blueprints of `leaf_frontier` that declare the leaf.
    pub declared_on: Vec<String>,
    /// Whether only some blueprints of `leaf_frontier` declare the leaf.
    pub partial: bool,
    /// Whether every relationship the path follows holds a single value on
    /// every blueprint that declares it.
    pub single_valued: bool,
}

/// Why a path did not resolve.
#[derive(Debug)]
pub(crate) enum PathWalkError {
    /// `frontier` names the blueprints the failing segment resolved against.
    Path {
        error: PathError,
        frontier: Vec<String>,
    },
    Repository(RepositoryError),
}

impl From<RepositoryError> for PathWalkError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl PathWalkError {
    /// The error as a repository error: a conflict keeps its own code, any
    /// other path failure becomes `invalid`'s message.
    pub fn into_repository(
        self,
        invalid: impl FnOnce(PathError) -> RepositoryError,
    ) -> RepositoryError {
        match self {
            Self::Path { error, .. } if error.is_conflict() => {
                RepositoryError::RelationshipPathConflict(error)
            }
            Self::Path { error, .. } => invalid(error),
            Self::Repository(error) => error,
        }
    }
}

/// Loads the latest published revision of a blueprint by code for a path
/// walk; a target without one simply has nothing to declare.
pub(crate) trait PathLoader {
    fn load(
        &mut self,
        code: &str,
    ) -> impl Future<Output = Result<Option<PathBlueprint>, RepositoryError>> + Send;
}

impl PathLoader for &AttricatRepository {
    fn load(
        &mut self,
        code: &str,
    ) -> impl Future<Output = Result<Option<PathBlueprint>, RepositoryError>> + Send {
        self.latest_published_path_blueprint(code)
    }
}

/// Walks `parts`, a path of at least one relationship and a leaf, from
/// `source_attributes` of blueprint `source_code`, loading the blueprints it
/// reaches with `load`.
pub(crate) async fn walk_relationship_path<A, L>(
    path: &str,
    parts: &[&str],
    source_code: &str,
    source_attributes: &[A],
    mut load: L,
) -> Result<ResolvedPath, PathWalkError>
where
    A: PathAttribute + Sync,
    L: PathLoader + Send,
{
    debug_assert!(parts.len() >= 2, "a relationship path has a hop and a leaf");
    let failed = |error, frontier: &[&str]| PathWalkError::Path {
        error,
        frontier: frontier.iter().map(|code| (*code).to_owned()).collect(),
    };
    let first = source_attributes
        .iter()
        .find(|attribute| attribute.path_code() == parts[0]);
    let segment = resolve_segment(path, parts[0], &[(source_code, first)])
        .map_err(|error| failed(error, &[source_code]))?;
    let mut single_valued = segment.single_valued();
    let mut targets = segment
        .next_frontier(path)
        .map_err(|error| failed(error, &[source_code]))?;
    let mut relationships = vec![parts[0].to_owned()];
    let mut frontier = load_frontier(&targets, &mut load).await?;
    for (index, part) in parts.iter().enumerate().skip(1) {
        let codes: Vec<&str> = frontier.iter().map(|target| target.code.as_str()).collect();
        let declarations: Vec<(&str, Option<&Attribute>)> = frontier
            .iter()
            .map(|target| {
                (
                    target.code.as_str(),
                    target
                        .attributes
                        .iter()
                        .find(|attribute| attribute.code == *part),
                )
            })
            .collect();
        let segment =
            resolve_segment(path, part, &declarations).map_err(|error| failed(error, &codes))?;
        if index + 1 == parts.len() {
            return Ok(ResolvedPath {
                relationships,
                value_type: segment.value_type,
                leaf: segment.first().clone(),
                declared_on: segment.declared_on(),
                partial: segment.is_partial(),
                single_valued,
                leaf_frontier: frontier,
            });
        }
        single_valued &= segment.single_valued();
        targets = segment
            .next_frontier(path)
            .map_err(|error| failed(error, &codes))?;
        relationships.push((*part).to_owned());
        frontier = load_frontier(&targets, &mut load).await?;
    }
    unreachable!("the loop returns at the leaf")
}

async fn load_frontier<L: PathLoader + Send>(
    targets: &[String],
    load: &mut L,
) -> Result<Vec<PathBlueprint>, RepositoryError> {
    let mut frontier = Vec::with_capacity(targets.len());
    for target in targets {
        if let Some(blueprint) = load.load(target).await? {
            frontier.push(blueprint);
        }
    }
    Ok(frontier)
}

impl AttricatRepository {
    /// Walks a path from `source_code`'s `source_attributes` through the
    /// latest published revisions of this workspace's blueprints.
    pub(crate) async fn resolve_relationship_path<A: PathAttribute + Sync>(
        &self,
        path: &str,
        parts: &[&str],
        source_code: &str,
        source_attributes: &[A],
    ) -> Result<ResolvedPath, PathWalkError> {
        walk_relationship_path(path, parts, source_code, source_attributes, self).await
    }
}
