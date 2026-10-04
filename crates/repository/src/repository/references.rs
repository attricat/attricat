//! The one "who references X" query: live entities whose relationship
//! attribute actively targets an entity in any context.
use super::RepositoryError;
use sqlx::PgConnection;
use uuid::Uuid;

/// Which referencing entities count.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Referrers<'a> {
    /// Any live entity.
    Any,
    /// Entities of a blueprint family, by code, whichever revision they use.
    BlueprintCode(&'a str),
    /// Entities pinned to one blueprint revision.
    Revision { blueprint_id: Uuid, version: i64 },
}

/// A bounded search for entities referencing `target`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReferenceQuery<'a> {
    pub target: Uuid,
    /// Code of the referencing entity's relationship attribute: a field of its
    /// blueprint revision or one of its own additional attributes.
    pub attribute_code: &'a str,
    pub referrers: Referrers<'a>,
    /// Narrows the search to one referencing entity.
    pub only: Option<Uuid>,
    /// Ignores the target's references to itself.
    pub exclude_target: bool,
    pub limit: i64,
}

/// IDs of the referencing entities, in ascending order, at most `limit`.
pub(crate) async fn referencing_entity_ids(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    query: ReferenceQuery<'_>,
) -> Result<Vec<Uuid>, RepositoryError> {
    let (code, blueprint_id, version) = match query.referrers {
        Referrers::Any => (None, None, None),
        Referrers::BlueprintCode(code) => (Some(code), None, None),
        Referrers::Revision {
            blueprint_id,
            version,
        } => (None, Some(blueprint_id), Some(version)),
    };
    Ok(sqlx::query_scalar(
        r#"SELECT DISTINCT av.entity_id
           FROM attribute_values av
           JOIN entities e ON e.id = av.entity_id AND e.workspace_id = av.workspace_id AND e.deleted_at IS NULL
           JOIN attributes a ON a.id = av.attribute_id AND a.code = $3 AND a.deleted_at IS NULL
            AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version) OR a.entity_id = e.id)
           WHERE av.workspace_id = $1 AND av.relationship_target_entity_id = $2 AND av.active
             AND ($4::text IS NULL OR EXISTS (
                 SELECT 1 FROM blueprints b
                 WHERE b.workspace_id = $1 AND b.id = e.blueprint_id AND b.version = e.blueprint_version AND b.code = $4))
             AND ($5::uuid IS NULL OR (e.blueprint_id = $5 AND e.blueprint_version = $6))
             AND ($7::uuid IS NULL OR av.entity_id = $7)
             AND NOT ($8 AND av.entity_id = $2)
           ORDER BY av.entity_id
           LIMIT $9"#,
    )
    .bind(workspace_id)
    .bind(query.target)
    .bind(query.attribute_code)
    .bind(code)
    .bind(blueprint_id)
    .bind(version)
    .bind(query.only)
    .bind(query.exclude_target)
    .bind(query.limit)
    .fetch_all(conn)
    .await?)
}
