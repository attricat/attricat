//! Walks a relationship path such as `spec.grade.code` through the latest
//! published revisions of the blueprints each hop links to, by the rule in
//! [`attricat_validation::relationship_path`]. Table columns, sorts, filters
//! and the search syntax all resolve their paths here.

use std::collections::HashMap;
use std::sync::Arc;

use attricat_domain::model::Attribute;
use attricat_validation::relationship_path::{
    CodeMatch, PathAttribute, PathError, Segment, resolve_in,
};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// A blueprint a path reaches, as its latest published revision.
#[derive(Clone, Debug)]
pub(crate) struct PathBlueprint {
    pub id: Uuid,
    pub code: String,
    pub attributes: Arc<[Attribute]>,
}

/// The relationship hops of a path, walked from its source blueprint.
#[derive(Debug)]
pub(crate) struct WalkedHops {
    /// The codes of the relationships followed, as their blueprints declare
    /// them.
    pub relationships: Vec<String>,
    /// The blueprints each hop links to, in hop order.
    pub frontiers: Vec<Vec<PathBlueprint>>,
    /// Whether every relationship holds a single value on every blueprint
    /// that declares it.
    pub single_valued: bool,
}

impl WalkedHops {
    /// The blueprints the last hop links to.
    pub fn targets(&self) -> &[PathBlueprint] {
        self.frontiers.last().map(Vec::as_slice).unwrap_or_default()
    }
}

/// A path of at least one relationship hop and its leaf.
#[derive(Debug)]
pub(crate) struct ResolvedPath {
    pub hops: WalkedHops,
    /// The leaf's first declaration, whose name and options the path shows.
    pub leaf: Attribute,
    /// The blueprints of the last hop's targets that declare the leaf, when
    /// not all of them do.
    pub partially_declared_on: Option<Vec<String>>,
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
    /// The error as a repository error, by
    /// [`RepositoryError::from_path_error`].
    pub fn into_repository(
        self,
        invalid: impl FnOnce(PathError) -> RepositoryError,
    ) -> RepositoryError {
        match self {
            Self::Path { error, .. } => RepositoryError::from_path_error(error, invalid),
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

/// The blueprints walks have loaded, by code, for [`OnceLoader`].
pub(crate) type LoadedBlueprints = HashMap<String, Option<PathBlueprint>>;

/// Loads each blueprint once into `loaded`, which walks of several paths
/// that reach the same targets, such as a table's columns, share.
pub(crate) struct OnceLoader<'m, L> {
    pub inner: L,
    pub loaded: &'m mut LoadedBlueprints,
}

impl<L: PathLoader + Send> PathLoader for OnceLoader<'_, L> {
    async fn load(&mut self, code: &str) -> Result<Option<PathBlueprint>, RepositoryError> {
        if let Some(loaded) = self.loaded.get(code) {
            return Ok(loaded.clone());
        }
        let loaded = self.inner.load(code).await?;
        self.loaded.insert(code.to_owned(), loaded.clone());
        Ok(loaded)
    }
}

/// A resolved hop: the relationship's code, whether it holds a single value
/// everywhere, and the targets the path continues in.
fn hop<A: PathAttribute>(
    path: &str,
    segment: &Segment<'_, A>,
) -> Result<(String, bool, Vec<String>), PathError> {
    Ok((
        segment.first().path_code().to_owned(),
        segment.single_valued(),
        segment.next_frontier(path)?,
    ))
}

/// The frontier for [`resolve_in`]: each blueprint's code and attributes.
fn frontier_attributes(frontier: &[PathBlueprint]) -> impl Iterator<Item = (&str, &[Attribute])> {
    frontier
        .iter()
        .map(|target| (target.code.as_str(), &*target.attributes))
}

fn frontier_codes(frontier: &[PathBlueprint]) -> Vec<String> {
    frontier.iter().map(|target| target.code.clone()).collect()
}

/// Walks `hops`, the relationships of a path, from `source`, a blueprint's
/// code and attributes, loading the blueprints each links to with `load`.
pub(crate) async fn walk_hops<A, L>(
    path: &str,
    hops: &[&str],
    (source_code, source_attributes): (&str, &[A]),
    code_match: CodeMatch,
    mut load: L,
) -> Result<WalkedHops, PathWalkError>
where
    A: PathAttribute + Sync,
    L: PathLoader + Send,
{
    let mut walked = WalkedHops {
        relationships: Vec::with_capacity(hops.len()),
        frontiers: Vec::with_capacity(hops.len()),
        single_valued: true,
    };
    for segment in hops {
        // The source's attributes have their own type, so its hop resolves
        // apart from those of the loaded targets.
        let resolved = match walked.frontiers.last() {
            None => resolve_in(
                path,
                segment,
                [(source_code, source_attributes)],
                code_match,
            )
            .and_then(|segment| hop(path, &segment))
            .map_err(|error| PathWalkError::Path {
                error,
                frontier: vec![source_code.to_owned()],
            }),
            Some(frontier) => resolve_in(path, segment, frontier_attributes(frontier), code_match)
                .and_then(|segment| hop(path, &segment))
                .map_err(|error| PathWalkError::Path {
                    error,
                    frontier: frontier_codes(frontier),
                }),
        };
        let (relationship, single_valued, targets) = resolved?;
        let mut next = Vec::with_capacity(targets.len());
        for target in &targets {
            if let Some(blueprint) = load.load(target).await? {
                next.push(blueprint);
            }
        }
        walked.relationships.push(relationship);
        walked.single_valued &= single_valued;
        walked.frontiers.push(next);
    }
    Ok(walked)
}

/// Walks `parts`, a path of at least one relationship and a leaf, from
/// `source`, loading the blueprints it reaches with `load`.
pub(crate) async fn walk_relationship_path<A, L>(
    path: &str,
    parts: &[&str],
    source: (&str, &[A]),
    load: L,
) -> Result<ResolvedPath, PathWalkError>
where
    A: PathAttribute + Sync,
    L: PathLoader + Send,
{
    let (leaf, hops) = parts.split_last().expect("a path has a leaf");
    debug_assert!(!hops.is_empty(), "a relationship path has a hop");
    let hops = walk_hops(path, hops, source, CodeMatch::Exact, load).await?;
    let targets = hops.targets();
    let segment = resolve_in(path, leaf, frontier_attributes(targets), CodeMatch::Exact).map_err(
        |error| PathWalkError::Path {
            error,
            frontier: frontier_codes(targets),
        },
    )?;
    let leaf = segment.first().clone();
    let partially_declared_on = segment.partially_declared_on();
    Ok(ResolvedPath {
        hops,
        leaf,
        partially_declared_on,
    })
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
        walk_relationship_path(path, parts, (source_code, source_attributes), self).await
    }
}
