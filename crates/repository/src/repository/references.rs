//! The one "who references X" query: live records whose relationship
//! attribute actively targets a record in any context.
use super::RepositoryError;
use sqlx::PgConnection;
use uuid::Uuid;

/// Which referencing records count.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Referrers<'a> {
    /// Any live record.
    Any,
    /// Records of a blueprint family, by code, whichever revision they use.
    BlueprintCode(&'a str),
    /// Records pinned to one blueprint revision.
    Revision { blueprint_id: Uuid, version: i64 },
}

/// A bounded search for records referencing `target`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReferenceQuery<'a> {
    pub target: Uuid,
    /// Code of the referencing record's relationship attribute: a field of its
    /// blueprint revision or one of its own additional attributes.
    pub attribute_code: &'a str,
    pub referrers: Referrers<'a>,
    /// Narrows the search to one referencing record.
    pub only: Option<Uuid>,
    /// Ignores the target's references to itself.
    pub exclude_target: bool,
    pub limit: i64,
}

/// IDs of the referencing records, in ascending order, at most `limit`.
pub(crate) async fn referencing_record_ids(
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
        r#"SELECT DISTINCT av.record_id
           FROM attribute_values av
           JOIN records e ON e.id = av.record_id AND e.workspace_id = av.workspace_id AND e.deleted_at IS NULL
           JOIN attributes a ON a.id = av.attribute_id AND a.code = $3 AND a.deleted_at IS NULL
            AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version) OR a.record_id = e.id)
           WHERE av.workspace_id = $1 AND av.relationship_target_record_id = $2 AND av.active
             AND ($4::text IS NULL OR EXISTS (
                 SELECT 1 FROM blueprints b
                 WHERE b.workspace_id = $1 AND b.id = e.blueprint_id AND b.version = e.blueprint_version AND b.code = $4))
             AND ($5::uuid IS NULL OR (e.blueprint_id = $5 AND e.blueprint_version = $6))
             AND ($7::uuid IS NULL OR av.record_id = $7)
             AND NOT ($8 AND av.record_id = $2)
           ORDER BY av.record_id
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

/// [`referencing_record_ids`] with [`Referrers::BlueprintCode`] for several
/// targets in one query: each target's IDs, in ascending order, at most
/// `limit` each. Targets with no referencing record are omitted.
pub(crate) async fn referencing_record_ids_by_target(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    targets: &[Uuid],
    attribute_code: &str,
    blueprint_code: &str,
    limit: i64,
) -> Result<std::collections::HashMap<Uuid, Vec<Uuid>>, RepositoryError> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"SELECT t.target, r.record_id
           FROM unnest($2::uuid[]) AS t(target)
           CROSS JOIN LATERAL (
               SELECT DISTINCT av.record_id
               FROM attribute_values av
               JOIN records e ON e.id = av.record_id AND e.workspace_id = av.workspace_id AND e.deleted_at IS NULL
               JOIN attributes a ON a.id = av.attribute_id AND a.code = $3 AND a.deleted_at IS NULL
                AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version) OR a.record_id = e.id)
               WHERE av.workspace_id = $1 AND av.relationship_target_record_id = t.target AND av.active
                 AND EXISTS (
                     SELECT 1 FROM blueprints b
                     WHERE b.workspace_id = $1 AND b.id = e.blueprint_id AND b.version = e.blueprint_version AND b.code = $4)
               ORDER BY av.record_id
               LIMIT $5
           ) r
           ORDER BY t.target, r.record_id"#,
    )
    .bind(workspace_id)
    .bind(targets)
    .bind(attribute_code)
    .bind(blueprint_code)
    .bind(limit)
    .fetch_all(conn)
    .await?;
    let mut by_target: std::collections::HashMap<Uuid, Vec<Uuid>> =
        std::collections::HashMap::new();
    for (target, record_id) in rows {
        by_target.entry(target).or_default().push(record_id);
    }
    Ok(by_target)
}
