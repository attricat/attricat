//! Blueprint-declared structural constraints: unique business keys and
//! acyclic relationship hierarchies.
//!
//! The latest published revision of a blueprint family decides which keys and
//! hierarchies apply to every entity in that family, whichever revision the
//! entity is pinned to. Key and hierarchy attributes are matched by code.
//!
//! Unique keys are enforced by `entity_unique_key_values`: each write rebuilds
//! the written entity's rows inside its transaction, and the table's unique
//! constraint makes the second of two concurrent duplicate writers fail.
//! Publication takes an exclusive per-family lock that every writer shares,
//! so a revision that adds a key indexes a stable family snapshot.
//!
//! Hierarchy checks run while a relationship edge is inserted, under the
//! workspace relationship lock every relationship writer holds, so two
//! concurrent writes cannot each add half of a cycle.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::values::NativeValueRow;
use super::{CatalogRepository, RepositoryError};
use crate::model::Entity;

/// Duplicate groups and hierarchy violations reported when publication fails.
const MAX_REPORTED_VIOLATIONS: usize = 20;

/// One group of entities that already share a key value.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UniqueKeyDuplicate {
    pub key: String,
    pub context: String,
    pub values: Value,
    pub entity_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct EnforcedUniqueKey {
    code: String,
    attributes: Vec<String>,
    scope: String,
    #[serde(default)]
    case_sensitive: bool,
}

#[derive(sqlx::FromRow)]
struct KeySourceRow {
    entity_id: Uuid,
    attribute_code: String,
    context_id: Uuid,
    context_fallback: String,
    relationship_target_entity_id: Option<Uuid>,
    #[sqlx(flatten)]
    native: NativeValueRow,
}

#[derive(Clone, sqlx::FromRow)]
struct KeyContext {
    id: Uuid,
    code: String,
    parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
struct KeyRow {
    entity_id: Uuid,
    key_code: String,
    context_id: Uuid,
    key_hash: String,
    key_values: Value,
}

const KEY_SOURCE_SQL: &str = r#"SELECT av.entity_id, a.code AS attribute_code, av.context_id, a.context_fallback,
                  av.relationship_target_entity_id,
                  a.value_type, av.value_text, av.value_number, av.value_integer,
                  av.value_boolean, av.value_date, av.value_datetime, av.value_time,
                  av.value_time_zone, av.value_json
           FROM attribute_values av
           JOIN entities e ON e.id = av.entity_id AND e.workspace_id = av.workspace_id
           JOIN attributes a ON a.id = av.attribute_id
            AND a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version
           WHERE av.workspace_id = $1 AND e.blueprint_id = $2 AND e.deleted_at IS NULL
             AND ($3::uuid IS NULL OR e.id = $3)
             AND a.code = ANY($4) AND a.deleted_at IS NULL
             AND (av.relationship_target_entity_id IS NULL OR av.active)"#;

impl CatalogRepository {
    /// Applies unique keys to one entity after its values changed. Called by
    /// every value-write path through entity validation.
    pub(super) async fn sync_entity_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        lock_unique_keys(transaction, self.workspace_id.0, entity.blueprint_id, false).await?;
        let keys = self
            .enforced_unique_keys(transaction, entity.blueprint_id)
            .await?;
        let contexts = if keys.is_empty() {
            Vec::new()
        } else {
            self.key_contexts(transaction).await?
        };
        let desired = if keys.is_empty() {
            Vec::new()
        } else {
            let sources = self
                .key_sources(transaction, entity.blueprint_id, Some(entity.id), &keys)
                .await?;
            key_rows(&keys, &contexts, &sources)?
        };
        let existing = sqlx::query_as::<_, (String, Uuid, String)>(
            "SELECT key_code, context_id, key_hash FROM entity_unique_key_values WHERE workspace_id = $1 AND entity_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .fetch_all(&mut **transaction)
        .await?;
        let wanted: HashSet<_> = desired
            .iter()
            .map(|row| (row.key_code.as_str(), row.context_id, row.key_hash.as_str()))
            .collect();
        let held: HashSet<_> = existing
            .iter()
            .map(|(code, context, hash)| (code.as_str(), *context, hash.as_str()))
            .collect();
        for (key_code, context_id, key_hash) in existing.iter().filter(|(code, context, hash)| {
            !wanted.contains(&(code.as_str(), *context, hash.as_str()))
        }) {
            sqlx::query(
                "DELETE FROM entity_unique_key_values WHERE workspace_id = $1 AND entity_id = $2 AND key_code = $3 AND context_id = $4 AND key_hash = $5",
            )
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(key_code)
            .bind(context_id)
            .bind(key_hash)
            .execute(&mut **transaction)
            .await?;
        }
        for row in desired.iter().filter(|row| {
            !held.contains(&(row.key_code.as_str(), row.context_id, row.key_hash.as_str()))
        }) {
            // A concurrent writer of the same value holds the index entry
            // until it finishes; DO NOTHING then reports its committed row.
            let inserted = sqlx::query_scalar::<_, Uuid>(
                r#"INSERT INTO entity_unique_key_values
                       (workspace_id, blueprint_id, key_code, context_id, key_hash, key_values, entity_id)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)
                   ON CONFLICT ON CONSTRAINT entity_unique_key_values_value_key DO NOTHING
                   RETURNING entity_id"#,
            )
            .bind(self.workspace_id.0)
            .bind(entity.blueprint_id)
            .bind(&row.key_code)
            .bind(row.context_id)
            .bind(&row.key_hash)
            .bind(&row.key_values)
            .bind(row.entity_id)
            .fetch_optional(&mut **transaction)
            .await?;
            if inserted.is_some() {
                continue;
            }
            let conflicting_entity_id = sqlx::query_scalar::<_, Uuid>(
                "SELECT entity_id FROM entity_unique_key_values WHERE workspace_id = $1 AND blueprint_id = $2 AND key_code = $3 AND context_id = $4 AND key_hash = $5",
            )
            .bind(self.workspace_id.0)
            .bind(entity.blueprint_id)
            .bind(&row.key_code)
            .bind(row.context_id)
            .bind(&row.key_hash)
            .fetch_one(&mut **transaction)
            .await?;
            return Err(RepositoryError::UniqueKeyConflict {
                key: row.key_code.clone(),
                context: context_code(&contexts, row.context_id),
                values: row.key_values.clone(),
                conflicting_entity_id,
            });
        }
        Ok(())
    }

    /// Releases a deleted entity's key values.
    pub(super) async fn delete_entity_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM entity_unique_key_values WHERE workspace_id = $1 AND entity_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// A new context resolves exactly like its parent until it receives
    /// values, so context-scoped key values are copied from the parent.
    pub(super) async fn seed_context_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
        parent_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let has_context_keys = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS (SELECT 1 FROM blueprints
                WHERE workspace_id = $1 AND status = 'published' AND deleted_at IS NULL
                  AND unique_keys @> '[{"scope":"context"}]'::jsonb)"#,
        )
        .bind(self.workspace_id.0)
        .fetch_one(&mut **transaction)
        .await?;
        if !has_context_keys {
            return Ok(());
        }
        // Wait for in-flight entity writers so every committed parent row is
        // copied; later writes recompute every context for their entity.
        sqlx::query("LOCK TABLE entities IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            r#"INSERT INTO entity_unique_key_values
                   (workspace_id, blueprint_id, key_code, context_id, key_hash, key_values, entity_id)
               SELECT k.workspace_id, k.blueprint_id, k.key_code, $3, k.key_hash, k.key_values, k.entity_id
               FROM entity_unique_key_values k
               WHERE k.workspace_id = $1 AND k.context_id = $2
                 AND EXISTS (
                     SELECT 1
                     FROM LATERAL (
                         SELECT b.unique_keys FROM blueprints b
                         WHERE b.workspace_id = k.workspace_id AND b.id = k.blueprint_id
                           AND b.status = 'published' AND b.deleted_at IS NULL
                         ORDER BY b.version DESC LIMIT 1
                     ) latest, jsonb_array_elements(latest.unique_keys) declared
                     WHERE declared ->> 'code' = k.key_code AND declared ->> 'scope' = 'context'
                 )"#,
        )
        .bind(self.workspace_id.0)
        .bind(parent_id)
        .bind(context_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Runs after a revision's status becomes `published`. When the family's
    /// enforced keys or hierarchies change, existing data is checked in the
    /// same transaction so publication fails with a report instead of
    /// leaving entities that violate the new constraints.
    pub(super) async fn apply_published_structural_constraints(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        previous_keys: &Value,
        previous_hierarchies: &BTreeMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let hierarchies = self.enforced_hierarchies(transaction, blueprint_id).await?;
        let changed_hierarchies: Vec<_> = hierarchies
            .iter()
            .filter(|(code, hierarchy)| previous_hierarchies.get(*code) != Some(*hierarchy))
            .collect();
        if !changed_hierarchies.is_empty() {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
            for (code, hierarchy) in changed_hierarchies {
                self.validate_existing_hierarchy(transaction, blueprint_id, code, hierarchy)
                    .await?;
            }
        }
        lock_unique_keys(transaction, self.workspace_id.0, blueprint_id, true).await?;
        let keys_value = self
            .enforced_unique_keys_value(transaction, blueprint_id)
            .await?;
        if &keys_value == previous_keys {
            return Ok(());
        }
        self.rebuild_family_unique_keys(transaction, blueprint_id)
            .await
    }

    /// The enforced key declarations and hierarchies before a publication,
    /// for [`Self::apply_published_structural_constraints`].
    pub(super) async fn enforced_structural_constraints(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<(Value, BTreeMap<String, String>), RepositoryError> {
        Ok((
            self.enforced_unique_keys_value(transaction, blueprint_id)
                .await?,
            self.enforced_hierarchies(transaction, blueprint_id).await?,
        ))
    }

    async fn rebuild_family_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM entity_unique_key_values WHERE workspace_id = $1 AND blueprint_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .execute(&mut **transaction)
        .await?;
        let keys = self.enforced_unique_keys(transaction, blueprint_id).await?;
        if keys.is_empty() {
            return Ok(());
        }
        let contexts = self.key_contexts(transaction).await?;
        let sources = self
            .key_sources(transaction, blueprint_id, None, &keys)
            .await?;
        let rows = key_rows(&keys, &contexts, &sources)?;
        let mut groups: BTreeMap<(&str, Uuid, &str), Vec<&KeyRow>> = BTreeMap::new();
        for row in &rows {
            groups
                .entry((row.key_code.as_str(), row.context_id, row.key_hash.as_str()))
                .or_default()
                .push(row);
        }
        let duplicates: Vec<_> = groups
            .values()
            .filter(|group| group.len() > 1)
            .map(|group| {
                let mut entity_ids: Vec<_> = group.iter().map(|row| row.entity_id).collect();
                entity_ids.sort();
                UniqueKeyDuplicate {
                    key: group[0].key_code.clone(),
                    context: context_code(&contexts, group[0].context_id),
                    values: group[0].key_values.clone(),
                    entity_ids,
                }
            })
            .collect();
        if !duplicates.is_empty() {
            let total = duplicates.len();
            return Err(RepositoryError::UniqueKeyDuplicates {
                duplicates: duplicates
                    .into_iter()
                    .take(MAX_REPORTED_VIOLATIONS)
                    .collect(),
                total,
            });
        }
        for chunk in rows.chunks(1000) {
            sqlx::query(
                r#"INSERT INTO entity_unique_key_values
                       (workspace_id, blueprint_id, key_code, context_id, key_hash, key_values, entity_id)
                   SELECT $1, $2, row.key_code, row.context_id, row.key_hash, row.key_values, row.entity_id
                   FROM UNNEST($3::text[], $4::uuid[], $5::text[], $6::jsonb[], $7::uuid[])
                        AS row(key_code, context_id, key_hash, key_values, entity_id)"#,
            )
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(chunk.iter().map(|row| row.key_code.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.context_id).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.key_hash.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.key_values.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.entity_id).collect::<Vec<_>>())
            .execute(&mut **transaction)
            .await?;
        }
        Ok(())
    }

    async fn enforced_unique_keys_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<Value, RepositoryError> {
        Ok(sqlx::query_scalar::<_, Value>(
            r#"SELECT unique_keys FROM blueprints
               WHERE workspace_id = $1 AND id = $2 AND status = 'published' AND deleted_at IS NULL
               ORDER BY version DESC LIMIT 1"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .fetch_optional(&mut **transaction)
        .await?
        .unwrap_or_else(|| Value::Array(Vec::new())))
    }

    async fn enforced_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<Vec<EnforcedUniqueKey>, RepositoryError> {
        serde_json::from_value(
            self.enforced_unique_keys_value(transaction, blueprint_id)
                .await?,
        )
        .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))
    }

    async fn enforced_hierarchies(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<BTreeMap<String, String>, RepositoryError> {
        Ok(sqlx::query_as::<_, (String, String)>(
            r#"SELECT a.code, a.hierarchy
               FROM attributes a
               WHERE a.workspace_id = $1 AND a.hierarchy IS NOT NULL AND a.deleted_at IS NULL
                 AND (a.blueprint_id, a.blueprint_version) = (
                     SELECT b.id, b.version FROM blueprints b
                     WHERE b.workspace_id = $1 AND b.id = $2 AND b.status = 'published' AND b.deleted_at IS NULL
                     ORDER BY b.version DESC LIMIT 1)"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .collect())
    }

    async fn key_contexts(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Vec<KeyContext>, RepositoryError> {
        Ok(sqlx::query_as::<_, KeyContext>(
            "SELECT id, code, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?)
    }

    async fn key_sources(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        entity_id: Option<Uuid>,
        keys: &[EnforcedUniqueKey],
    ) -> Result<Vec<KeySourceRow>, RepositoryError> {
        let codes: Vec<_> = keys
            .iter()
            .flat_map(|key| key.attributes.iter().cloned())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Ok(sqlx::query_as::<_, KeySourceRow>(KEY_SOURCE_SQL)
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(entity_id)
            .bind(codes)
            .fetch_all(&mut **transaction)
            .await?)
    }

    /// Rejects a new edge that would close a cycle in a hierarchy, or give an
    /// entity a second parent in a tree. Edges of every revision of the
    /// blueprint family's field count; the latest published revision decides
    /// whether the field is a hierarchy.
    pub(super) async fn validate_relationship_hierarchy(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        target_entity_id: Uuid,
    ) -> Result<(), RepositoryError> {
        // Entity-scoped reusable attributes are not part of a blueprint field.
        let is_blueprint_field = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attributes WHERE id = $1 AND workspace_id = $2 AND blueprint_id = $3)",
        )
        .bind(attribute_id)
        .bind(self.workspace_id.0)
        .bind(entity.blueprint_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !is_blueprint_field {
            return Ok(());
        }
        let Some(hierarchy) = self
            .enforced_hierarchies(transaction, entity.blueprint_id)
            .await?
            .remove(attribute_code)
        else {
            return Ok(());
        };
        self.lock_relationship_cardinality_writes(transaction)
            .await?;
        if hierarchy == "tree" {
            let other_parent = sqlx::query_scalar::<_, Uuid>(
                r#"SELECT av.relationship_target_entity_id
                   FROM attribute_values av JOIN attributes a ON a.id = av.attribute_id
                   WHERE av.workspace_id = $1 AND av.entity_id = $2
                     AND a.blueprint_id = $3 AND a.code = $4
                     AND av.context_id IS NOT DISTINCT FROM $5
                     AND av.relationship_target_entity_id IS NOT NULL AND av.active
                     AND av.relationship_target_entity_id <> $6
                   LIMIT 1"#,
            )
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(entity.blueprint_id)
            .bind(attribute_code)
            .bind(context_id)
            .bind(target_entity_id)
            .fetch_optional(&mut **transaction)
            .await?;
            if let Some(existing_target) = other_parent {
                return Err(RepositoryError::RelationshipCardinalityConflict {
                    attribute: attribute_code.to_owned(),
                    context_id,
                    source_entity_id: entity.id,
                    target_entity_id: existing_target,
                    conflicting_source_entity_id: None,
                });
            }
        }
        if target_entity_id == entity.id {
            return Err(RepositoryError::RelationshipCycle {
                attribute: attribute_code.to_owned(),
                path: vec![entity.id, entity.id],
            });
        }
        // Breadth-first walk from the new target along the same field. The
        // new edge closes a cycle exactly when the walk reaches the source.
        let mut predecessor: HashMap<Uuid, Uuid> = HashMap::new();
        let mut visited = HashSet::from([target_entity_id]);
        let mut frontier = vec![target_entity_id];
        while !frontier.is_empty() {
            let edges = sqlx::query_as::<_, (Uuid, Uuid)>(
                r#"SELECT av.entity_id, av.relationship_target_entity_id
                   FROM attribute_values av JOIN attributes a ON a.id = av.attribute_id
                   WHERE av.workspace_id = $1 AND a.blueprint_id = $2 AND a.code = $3
                     AND av.context_id IS NOT DISTINCT FROM $4
                     AND av.relationship_target_entity_id IS NOT NULL AND av.active
                     AND av.entity_id = ANY($5)"#,
            )
            .bind(self.workspace_id.0)
            .bind(entity.blueprint_id)
            .bind(attribute_code)
            .bind(context_id)
            .bind(&frontier)
            .fetch_all(&mut **transaction)
            .await?;
            let mut next = Vec::new();
            for (source, target) in edges {
                if target == entity.id {
                    let mut path = vec![source];
                    while let Some(previous) = predecessor.get(path.last().expect("path")) {
                        path.push(*previous);
                    }
                    path.reverse();
                    path.insert(0, entity.id);
                    path.push(entity.id);
                    return Err(RepositoryError::RelationshipCycle {
                        attribute: attribute_code.to_owned(),
                        path,
                    });
                }
                if visited.insert(target) {
                    predecessor.insert(target, source);
                    next.push(target);
                }
            }
            frontier = next;
        }
        Ok(())
    }

    async fn validate_existing_hierarchy(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        attribute_code: &str,
        hierarchy: &str,
    ) -> Result<(), RepositoryError> {
        let edges = sqlx::query_as::<_, (Option<Uuid>, Uuid, Uuid)>(
            r#"SELECT av.context_id, av.entity_id, av.relationship_target_entity_id
               FROM attribute_values av
               JOIN attributes a ON a.id = av.attribute_id
               JOIN entities e ON e.id = av.entity_id AND e.workspace_id = av.workspace_id
               WHERE av.workspace_id = $1 AND a.blueprint_id = $2 AND a.code = $3
                 AND e.deleted_at IS NULL
                 AND av.relationship_target_entity_id IS NOT NULL AND av.active
               ORDER BY av.context_id, av.entity_id, av.relationship_target_entity_id"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(attribute_code)
        .fetch_all(&mut **transaction)
        .await?;
        let mut graphs: BTreeMap<Option<Uuid>, BTreeMap<Uuid, Vec<Uuid>>> = BTreeMap::new();
        for (context_id, source, target) in edges {
            graphs
                .entry(context_id)
                .or_default()
                .entry(source)
                .or_default()
                .push(target);
        }
        let mut cycles = Vec::new();
        let mut multiple_parents = Vec::new();
        for graph in graphs.values() {
            if hierarchy == "tree" {
                multiple_parents.extend(
                    graph
                        .iter()
                        .filter(|(_, targets)| targets.len() > 1)
                        .map(|(source, _)| *source),
                );
            }
            cycles.extend(find_cycles(graph));
        }
        if cycles.is_empty() && multiple_parents.is_empty() {
            return Ok(());
        }
        multiple_parents.sort();
        multiple_parents.dedup();
        Err(RepositoryError::RelationshipHierarchyViolations {
            attribute: attribute_code.to_owned(),
            cycles: cycles.into_iter().take(MAX_REPORTED_VIOLATIONS).collect(),
            multiple_parents: multiple_parents
                .into_iter()
                .take(MAX_REPORTED_VIOLATIONS)
                .collect(),
        })
    }
}

/// Readable summary for error messages, which agents and CLI users see
/// without the structured `details`.
pub(super) fn describe_duplicates(duplicates: &[UniqueKeyDuplicate]) -> String {
    duplicates
        .iter()
        .map(|duplicate| {
            format!(
                "key '{}' values {} in context '{}' on entities {}",
                duplicate.key,
                duplicate.values,
                duplicate.context,
                join_ids(&duplicate.entity_ids)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn describe_hierarchy_violations(
    cycles: &[Vec<Uuid>],
    multiple_parents: &[Uuid],
) -> String {
    let mut parts: Vec<_> = cycles
        .iter()
        .map(|cycle| {
            format!(
                "cycle {}",
                cycle
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            )
        })
        .collect();
    if !multiple_parents.is_empty() {
        parts.push(format!(
            "more than one parent: {}",
            join_ids(multiple_parents)
        ));
    }
    parts.join("; ")
}

fn join_ids(ids: &[Uuid]) -> String {
    ids.iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every key writer shares the family lock; publication takes it exclusively
/// while it re-indexes the family.
async fn lock_unique_keys(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    blueprint_id: Uuid,
    exclusive: bool,
) -> Result<(), RepositoryError> {
    let lock_key = format!("unique-keys:{workspace_id}:{blueprint_id}");
    let statement = if exclusive {
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))"
    } else {
        "SELECT pg_advisory_xact_lock_shared(hashtextextended($1, 0))"
    };
    sqlx::query(statement)
        .bind(lock_key)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

fn context_code(contexts: &[KeyContext], context_id: Uuid) -> String {
    contexts
        .iter()
        .find(|context| context.id == context_id)
        .map(|context| context.code.clone())
        .unwrap_or_else(|| context_id.to_string())
}

/// Computes every key row for the entities in `sources`. A key without a
/// value for one of its attributes in a context is absent there.
fn key_rows(
    keys: &[EnforcedUniqueKey],
    contexts: &[KeyContext],
    sources: &[KeySourceRow],
) -> Result<Vec<KeyRow>, RepositoryError> {
    let Some(default_context) = contexts.iter().find(|context| context.parent_id.is_none()) else {
        return Err(RepositoryError::InvalidContext);
    };
    let by_id: HashMap<_, _> = contexts
        .iter()
        .map(|context| (context.id, context))
        .collect();
    let paths: Vec<(Uuid, Vec<Uuid>)> = contexts
        .iter()
        .map(|context| {
            let mut path = Vec::new();
            let mut current = Some(context);
            while let Some(item) = current {
                path.push(item.id);
                current = item
                    .parent_id
                    .and_then(|parent| by_id.get(&parent).copied());
            }
            (context.id, path)
        })
        .collect();
    // Relationship keys use single-target attributes; should legacy data hold
    // several targets, the smallest ID keeps the result deterministic.
    let mut by_entity: BTreeMap<Uuid, HashMap<(&str, Uuid), &KeySourceRow>> = BTreeMap::new();
    for row in sources {
        let slot = by_entity
            .entry(row.entity_id)
            .or_default()
            .entry((row.attribute_code.as_str(), row.context_id))
            .or_insert(row);
        if row.relationship_target_entity_id < slot.relationship_target_entity_id {
            *slot = row;
        }
    }
    let mut rows = Vec::new();
    for (entity_id, values) in &by_entity {
        for key in keys {
            let scoped: Vec<(Uuid, &[Uuid])> = if key.scope == "context" {
                paths
                    .iter()
                    .map(|(id, path)| (*id, path.as_slice()))
                    .collect()
            } else {
                vec![(
                    default_context.id,
                    std::slice::from_ref(&default_context.id),
                )]
            };
            'context: for (context_id, path) in scoped {
                let mut components = Vec::with_capacity(key.attributes.len());
                for attribute in &key.attributes {
                    let mut found = None;
                    for (index, source_context) in path.iter().enumerate() {
                        if let Some(row) = values.get(&(attribute.as_str(), *source_context)) {
                            if index > 0 && row.context_fallback == "none" {
                                break;
                            }
                            found = Some(*row);
                            break;
                        }
                    }
                    let Some(component) = found
                        .map(|row| key_component(row, key.case_sensitive))
                        .transpose()?
                        .flatten()
                    else {
                        continue 'context;
                    };
                    components.push(component);
                }
                let key_values = Value::Array(components);
                rows.push(KeyRow {
                    entity_id: *entity_id,
                    key_code: key.code.clone(),
                    context_id,
                    key_hash: format!("{:x}", Sha256::digest(key_values.to_string().as_bytes())),
                    key_values,
                });
            }
        }
    }
    Ok(rows)
}

/// Normalizes one key component. Strings are trimmed, internal whitespace runs
/// become one space, and unless the key is case-sensitive they are lowercased;
/// a blank string counts as missing. Numbers compare by value (`1.50` equals
/// `1.5`), date-times by instant, and relationships by target entity.
fn key_component(
    row: &KeySourceRow,
    case_sensitive: bool,
) -> Result<Option<Value>, RepositoryError> {
    if row.native.value_type == "relationship" {
        return Ok(row
            .relationship_target_entity_id
            .map(|target| Value::String(target.to_string())));
    }
    if row.native.value_type == "string" {
        let Some(text) = &row.native.value_text else {
            return Ok(None);
        };
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            return Ok(None);
        }
        return Ok(Some(Value::String(if case_sensitive {
            normalized
        } else {
            normalized.to_lowercase()
        })));
    }
    if row.native.value_type == "number" {
        return Ok(row
            .native
            .value_number
            .map(|number| Value::String(number.normalize().to_string())));
    }
    super::values::native_value_json(row.native.clone())
        .map(|value| (!value.is_null()).then_some(value))
}

/// Returns up to [`MAX_REPORTED_VIOLATIONS`] cycles, each as a closed path
/// `[a, b, ..., a]`, found by iterative depth-first search.
fn find_cycles(graph: &BTreeMap<Uuid, Vec<Uuid>>) -> Vec<Vec<Uuid>> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Active,
        Done,
    }
    let mut state: HashMap<Uuid, State> = HashMap::new();
    let mut cycles = Vec::new();
    for root in graph.keys() {
        if state.contains_key(root) {
            continue;
        }
        let mut stack: Vec<(Uuid, usize)> = vec![(*root, 0)];
        state.insert(*root, State::Active);
        while let Some((node, index)) = stack.last_mut() {
            let node = *node;
            let targets = graph.get(&node).map(Vec::as_slice).unwrap_or_default();
            if let Some(target) = targets.get(*index).copied() {
                *index += 1;
                match state.get(&target) {
                    None => {
                        state.insert(target, State::Active);
                        stack.push((target, 0));
                    }
                    Some(State::Active) => {
                        let start = stack
                            .iter()
                            .position(|(item, _)| *item == target)
                            .expect("active nodes are on the stack");
                        let mut cycle: Vec<_> =
                            stack[start..].iter().map(|(item, _)| *item).collect();
                        cycle.push(target);
                        cycles.push(cycle);
                        if cycles.len() >= MAX_REPORTED_VIOLATIONS {
                            return cycles;
                        }
                    }
                    Some(State::Done) => {}
                }
            } else {
                state.insert(node, State::Done);
                stack.pop();
            }
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(id: u128, code: &str, parent: Option<u128>) -> KeyContext {
        KeyContext {
            id: Uuid::from_u128(id),
            code: code.to_owned(),
            parent_id: parent.map(Uuid::from_u128),
        }
    }

    fn text(entity: u128, attribute: &str, context: u128, value: &str) -> KeySourceRow {
        KeySourceRow {
            entity_id: Uuid::from_u128(entity),
            attribute_code: attribute.to_owned(),
            context_id: Uuid::from_u128(context),
            context_fallback: "default".to_owned(),
            relationship_target_entity_id: None,
            native: NativeValueRow {
                value_type: "string".to_owned(),
                value_text: Some(value.to_owned()),
                value_number: None,
                value_integer: None,
                value_boolean: None,
                value_date: None,
                value_datetime: None,
                value_time: None,
                value_time_zone: None,
                value_json: None,
            },
        }
    }

    fn key(code: &str, attributes: &[&str], scope: &str) -> EnforcedUniqueKey {
        EnforcedUniqueKey {
            code: code.to_owned(),
            attributes: attributes.iter().map(|code| (*code).to_owned()).collect(),
            scope: scope.to_owned(),
            case_sensitive: false,
        }
    }

    #[test]
    fn strings_are_trimmed_collapsed_and_case_folded_unless_case_sensitive() {
        let contexts = [context(1, "default", None)];
        let sources = [
            text(10, "sku", 1, "  ab-1\t  X "),
            text(11, "sku", 1, "AB-1 x"),
            text(12, "sku", 1, "   "),
        ];
        let rows = key_rows(&[key("sku", &["sku"], "workspace")], &contexts, &sources).unwrap();
        assert_eq!(rows.len(), 2, "a blank string is missing");
        assert_eq!(rows[0].key_values, serde_json::json!(["ab-1 x"]));
        assert_eq!(rows[0].key_hash, rows[1].key_hash);

        let mut sensitive = key("sku", &["sku"], "workspace");
        sensitive.case_sensitive = true;
        let rows = key_rows(&[sensitive], &contexts, &sources).unwrap();
        assert_ne!(rows[0].key_hash, rows[1].key_hash);
        assert_eq!(rows[1].key_values, serde_json::json!(["AB-1 x"]));
    }

    #[test]
    fn composite_keys_need_every_component_and_context_keys_resolve_inheritance() {
        let contexts = [
            context(1, "default", None),
            context(2, "pl", Some(1)),
            context(3, "pl-web", Some(2)),
        ];
        let sources = [
            text(10, "document", 1, "D-1"),
            text(10, "revision", 1, "A"),
            text(11, "document", 1, "D-1"),
            text(10, "slug", 1, "shirt"),
            text(10, "slug", 2, "koszula"),
        ];
        let composite = key_rows(
            &[key("revision", &["document", "revision"], "workspace")],
            &contexts,
            &sources,
        )
        .unwrap();
        assert_eq!(composite.len(), 1, "entity 11 has no revision");
        assert_eq!(composite[0].key_values, serde_json::json!(["d-1", "a"]));
        assert_eq!(composite[0].context_id, Uuid::from_u128(1));

        let slugs = key_rows(&[key("slug", &["slug"], "context")], &contexts, &sources).unwrap();
        let by_context: HashMap<_, _> = slugs
            .iter()
            .map(|row| (row.context_id.as_u128(), row.key_values.clone()))
            .collect();
        assert_eq!(by_context[&1], serde_json::json!(["shirt"]));
        assert_eq!(by_context[&2], serde_json::json!(["koszula"]));
        assert_eq!(
            by_context[&3],
            serde_json::json!(["koszula"]),
            "inherited from pl"
        );

        let mut none = sources;
        none[4].context_fallback = "none".to_owned();
        none[3].context_fallback = "none".to_owned();
        let slugs = key_rows(&[key("slug", &["slug"], "context")], &contexts, &none).unwrap();
        assert_eq!(slugs.len(), 2, "fallback none stops inheritance");
    }

    #[test]
    fn finds_closed_cycles_and_ignores_dags() {
        let [a, b, c, d] = [1u128, 2, 3, 4].map(Uuid::from_u128);
        let dag = BTreeMap::from([(a, vec![b, c]), (b, vec![d]), (c, vec![d])]);
        assert!(find_cycles(&dag).is_empty());
        let cyclic = BTreeMap::from([(a, vec![b]), (b, vec![c]), (c, vec![a]), (d, vec![d])]);
        let cycles = find_cycles(&cyclic);
        assert_eq!(cycles, vec![vec![a, b, c, a], vec![d, d]]);
    }
}
