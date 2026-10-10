//! Blueprint-declared structural constraints: unique business keys and
//! acyclic relationship hierarchies.
//!
//! The latest published revision of a blueprint family decides which keys and
//! hierarchies apply to every record in that family, whichever revision the
//! record is pinned to. Key and hierarchy attributes are matched by code.
//! (Record checks, transition conditions and rules instead use the record's
//! pinned revision; see `docs/database.md#structural-constraints`.)
//!
//! Unique keys are enforced by `record_unique_key_values`: each write rebuilds
//! the written record's rows inside its transaction, and the table's unique
//! constraint makes the second of two concurrent duplicate writers fail.
//! Publication takes an exclusive per-family lock that every writer shares,
//! so a revision that adds a key indexes a stable family snapshot. Key values
//! are resolved with [`super::record_values`] and normalized with
//! [`normalize_key_component`], which the `unique` rule predicate shares.
//!
//! Hierarchy checks run while a relationship edge is inserted, under the
//! workspace relationship lock every relationship writer holds, so two
//! concurrent writes cannot each add half of a cycle. Edges are resolved per
//! context like any other value, so an edge inherited from a parent context
//! and a local edge can close a cycle together.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use attricat_validation::unique_key::normalize_key_component;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::record_values::{
    ContextTree, RecordState, RecordValues, Selection, load_records, resolve_on_path,
};
use super::write_context::WriteContext;
use super::{AttricatRepository, RepositoryError};
use crate::model::Record;

/// Duplicate groups and hierarchy violations reported when publication fails.
const MAX_REPORTED_VIOLATIONS: usize = 20;

/// One group of records that already share a key value.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UniqueKeyDuplicate {
    pub key: String,
    pub context: String,
    pub values: Value,
    pub record_ids: Vec<Uuid>,
}

/// Where a unique key's values must be unique.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UniqueKeyScope {
    /// Default-context values, across the family.
    Workspace,
    /// Each context's resolved values, within that context.
    Context,
}

/// A `[[unique_keys]]` declaration of the latest published revision.
#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct EnforcedUniqueKey {
    pub code: String,
    pub attributes: Vec<String>,
    pub scope: UniqueKeyScope,
    #[serde(default)]
    pub case_sensitive: bool,
}

/// The shape a relationship hierarchy enforces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hierarchy {
    /// No cycles.
    Acyclic,
    /// No cycles and at most one parent per record and context.
    Tree,
}

impl Hierarchy {
    pub(super) fn parse(value: &str) -> Result<Self, RepositoryError> {
        match value {
            "acyclic" => Ok(Self::Acyclic),
            "tree" => Ok(Self::Tree),
            other => Err(RepositoryError::InvalidBlueprintDefinition(format!(
                "unknown relationship hierarchy '{other}'"
            ))),
        }
    }
}

/// A family's enforced unique keys and hierarchies before a publication, to
/// tell which of them the publication changes.
pub(super) struct StructuralConstraints {
    /// The latest published revision's `unique_keys`, as stored.
    unique_keys: Value,
    /// Hierarchy fields by attribute code.
    hierarchies: BTreeMap<String, Hierarchy>,
}

#[derive(Clone, Debug, PartialEq)]
struct KeyRow {
    record_id: Uuid,
    key_code: String,
    context_id: Uuid,
    key_hash: String,
    key_values: Value,
}

/// The SHA-256 stored in `record_unique_key_values.key_hash`.
pub(crate) fn key_hash(key_values: &Value) -> String {
    format!("{:x}", Sha256::digest(key_values.to_string().as_bytes()))
}

/// The latest published revision's `unique_keys`, as stored.
async fn enforced_unique_keys_value(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    blueprint_id: Uuid,
) -> Result<Value, RepositoryError> {
    Ok(sqlx::query_scalar::<_, Value>(
        r#"SELECT unique_keys FROM blueprints
           WHERE workspace_id = $1 AND id = $2 AND status = 'published' AND deleted_at IS NULL
           ORDER BY version DESC LIMIT 1"#,
    )
    .bind(workspace_id)
    .bind(blueprint_id)
    .fetch_optional(conn)
    .await?
    .unwrap_or_else(|| Value::Array(Vec::new())))
}

pub(crate) async fn enforced_unique_keys(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    blueprint_id: Uuid,
) -> Result<Vec<EnforcedUniqueKey>, RepositoryError> {
    serde_json::from_value(enforced_unique_keys_value(conn, workspace_id, blueprint_id).await?)
        .map_err(|error| RepositoryError::InvalidBlueprintDefinition(error.to_string()))
}

fn key_codes(keys: &[EnforcedUniqueKey]) -> Vec<String> {
    keys.iter()
        .flat_map(|key| key.attributes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

impl AttricatRepository {
    /// Applies unique keys to one record after its values changed. Called by
    /// every value-write path through record validation.
    pub(super) async fn sync_record_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record: &Record,
    ) -> Result<(), RepositoryError> {
        lock_unique_keys(transaction, self.workspace_id.0, record.blueprint_id, false).await?;
        let keys =
            enforced_unique_keys(transaction, self.workspace_id.0, record.blueprint_id).await?;
        let (contexts, desired) = if keys.is_empty() {
            (ContextTree::default(), Vec::new())
        } else {
            let contexts = ContextTree::load(transaction, self.workspace_id.0).await?;
            let records = load_records(
                transaction,
                self.workspace_id.0,
                Selection::Records(&[record.id]),
                Some(&key_codes(&keys)),
                RecordState::After,
            )
            .await?;
            let rows = key_rows(&keys, &contexts, &records, None)?;
            (contexts, rows)
        };
        let existing = sqlx::query_as::<_, (String, Uuid, String)>(
            "SELECT key_code, context_id, key_hash FROM record_unique_key_values WHERE workspace_id = $1 AND record_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(record.id)
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
        let stale: Vec<_> = held.difference(&wanted).collect();
        if !stale.is_empty() {
            sqlx::query(
                r#"DELETE FROM record_unique_key_values
                   WHERE workspace_id = $1 AND record_id = $2
                     AND (key_code, context_id, key_hash) IN (SELECT * FROM UNNEST($3::text[], $4::uuid[], $5::text[]))"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.id)
            .bind(stale.iter().map(|(code, ..)| *code).collect::<Vec<_>>())
            .bind(stale.iter().map(|(_, context, _)| *context).collect::<Vec<_>>())
            .bind(stale.iter().map(|(.., hash)| *hash).collect::<Vec<_>>())
            .execute(&mut **transaction)
            .await?;
        }
        for row in desired.iter().filter(|row| {
            !held.contains(&(row.key_code.as_str(), row.context_id, row.key_hash.as_str()))
        }) {
            // A concurrent writer of the same value holds the index entry
            // until it finishes; DO NOTHING then reports its committed row.
            let inserted = sqlx::query_scalar::<_, Uuid>(
                r#"INSERT INTO record_unique_key_values
                       (workspace_id, blueprint_id, key_code, context_id, key_hash, key_values, record_id)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)
                   ON CONFLICT ON CONSTRAINT record_unique_key_values_value_key DO NOTHING
                   RETURNING record_id"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.blueprint_id)
            .bind(&row.key_code)
            .bind(row.context_id)
            .bind(&row.key_hash)
            .bind(&row.key_values)
            .bind(row.record_id)
            .fetch_optional(&mut **transaction)
            .await?;
            if inserted.is_some() {
                continue;
            }
            let conflicting_record_id = sqlx::query_scalar::<_, Uuid>(
                "SELECT record_id FROM record_unique_key_values WHERE workspace_id = $1 AND blueprint_id = $2 AND key_code = $3 AND context_id = $4 AND key_hash = $5",
            )
            .bind(self.workspace_id.0)
            .bind(record.blueprint_id)
            .bind(&row.key_code)
            .bind(row.context_id)
            .bind(&row.key_hash)
            .fetch_one(&mut **transaction)
            .await?;
            return Err(RepositoryError::UniqueKeyConflict {
                key: row.key_code.clone(),
                context: contexts.code(row.context_id),
                values: row.key_values.clone(),
                conflicting_record_id,
            });
        }
        Ok(())
    }

    /// Releases a deleted record's key values.
    pub(super) async fn delete_record_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM record_unique_key_values WHERE workspace_id = $1 AND record_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(record_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Takes the per-family key lock exclusively for every family whose
    /// latest published revision has context-scoped keys, in ID order. Every
    /// key writer holds the same lock shared until it commits, so a context
    /// change that holds these locks sees every committed key row and no
    /// writer computes rows against the old context tree. Callers first take
    /// the workspace row and the exclusive record-writer lock (see the lock
    /// order in `mod.rs`), so no writer holding a family lock waits on them. Returns
    /// the families.
    pub(super) async fn lock_context_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let families: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM (
                   SELECT DISTINCT ON (id) id, unique_keys FROM blueprints
                   WHERE workspace_id = $1 AND status = 'published' AND deleted_at IS NULL
                   ORDER BY id, version DESC
               ) latest
               WHERE unique_keys @> '[{"scope":"context"}]'::jsonb
               ORDER BY id"#,
        )
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?;
        for blueprint_id in &families {
            lock_unique_keys(transaction, self.workspace_id.0, *blueprint_id, true).await?;
        }
        Ok(families)
    }

    /// Indexes a new context's context-scoped key values. The context has no
    /// values yet, so each record resolves there like in the parent except
    /// for attributes with `context_fallback = "none"`, which are missing.
    /// The caller holds [`Self::lock_context_unique_keys`].
    pub(super) async fn seed_context_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        families: &[Uuid],
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if families.is_empty() {
            return Ok(());
        }
        let contexts = ContextTree::load(transaction, self.workspace_id.0).await?;
        for blueprint_id in families {
            let keys: Vec<_> =
                enforced_unique_keys(transaction, self.workspace_id.0, *blueprint_id)
                    .await?
                    .into_iter()
                    .filter(|key| key.scope == UniqueKeyScope::Context)
                    .collect();
            if keys.is_empty() {
                continue;
            }
            let records = load_records(
                transaction,
                self.workspace_id.0,
                Selection::Family(*blueprint_id),
                Some(&key_codes(&keys)),
                RecordState::After,
            )
            .await?;
            let rows = key_rows(&keys, &contexts, &records, Some(context_id))?;
            self.store_key_rows(transaction, *blueprint_id, &contexts, &rows)
                .await?;
        }
        Ok(())
    }

    /// Re-indexes `families` after a context reparent changed how values
    /// are inherited. Records are not re-synced one by one, because a row
    /// computed under the new tree could collide with another record's row
    /// that is still stale; real duplicates are reported together. The
    /// caller holds [`Self::lock_context_unique_keys`].
    pub(super) async fn rebuild_context_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        families: &[Uuid],
    ) -> Result<(), RepositoryError> {
        for blueprint_id in families {
            self.rebuild_family_unique_keys(transaction, *blueprint_id)
                .await?;
        }
        Ok(())
    }

    /// Runs after a revision's status becomes `published`. When the family's
    /// enforced keys or hierarchies change, existing data is checked in the
    /// same transaction so publication fails with a report instead of
    /// leaving records that violate the new constraints.
    pub(super) async fn apply_published_structural_constraints(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        previous: &StructuralConstraints,
    ) -> Result<(), RepositoryError> {
        let hierarchies = self.enforced_hierarchies(transaction, blueprint_id).await?;
        let changed_hierarchies: Vec<_> = hierarchies
            .iter()
            .filter(|(code, hierarchy)| previous.hierarchies.get(*code) != Some(*hierarchy))
            .collect();
        if !changed_hierarchies.is_empty() {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
            let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
            for (code, hierarchy) in changed_hierarchies {
                self.validate_existing_hierarchy(
                    transaction,
                    &tree,
                    blueprint_id,
                    code,
                    *hierarchy,
                )
                .await?;
            }
        }
        lock_unique_keys(transaction, self.workspace_id.0, blueprint_id, true).await?;
        let keys_value =
            enforced_unique_keys_value(transaction, self.workspace_id.0, blueprint_id).await?;
        if keys_value == previous.unique_keys {
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
    ) -> Result<StructuralConstraints, RepositoryError> {
        Ok(StructuralConstraints {
            unique_keys: enforced_unique_keys_value(transaction, self.workspace_id.0, blueprint_id)
                .await?,
            hierarchies: self.enforced_hierarchies(transaction, blueprint_id).await?,
        })
    }

    async fn rebuild_family_unique_keys(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "DELETE FROM record_unique_key_values WHERE workspace_id = $1 AND blueprint_id = $2",
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .execute(&mut **transaction)
        .await?;
        let keys = enforced_unique_keys(transaction, self.workspace_id.0, blueprint_id).await?;
        if keys.is_empty() {
            return Ok(());
        }
        let contexts = ContextTree::load(transaction, self.workspace_id.0).await?;
        let records = load_records(
            transaction,
            self.workspace_id.0,
            Selection::Family(blueprint_id),
            Some(&key_codes(&keys)),
            RecordState::After,
        )
        .await?;
        let rows = key_rows(&keys, &contexts, &records, None)?;
        self.store_key_rows(transaction, blueprint_id, &contexts, &rows)
            .await
    }

    /// Inserts a family's freshly computed key rows, reporting values that
    /// several of them share instead of failing on the unique constraint.
    async fn store_key_rows(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        contexts: &ContextTree,
        rows: &[KeyRow],
    ) -> Result<(), RepositoryError> {
        let mut groups: BTreeMap<(&str, Uuid, &str), Vec<&KeyRow>> = BTreeMap::new();
        for row in rows {
            groups
                .entry((row.key_code.as_str(), row.context_id, row.key_hash.as_str()))
                .or_default()
                .push(row);
        }
        let duplicates: Vec<_> = groups
            .values()
            .filter(|group| group.len() > 1)
            .map(|group| {
                let mut record_ids: Vec<_> = group.iter().map(|row| row.record_id).collect();
                record_ids.sort();
                UniqueKeyDuplicate {
                    key: group[0].key_code.clone(),
                    context: contexts.code(group[0].context_id),
                    values: group[0].key_values.clone(),
                    record_ids,
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
                r#"INSERT INTO record_unique_key_values
                       (workspace_id, blueprint_id, key_code, context_id, key_hash, key_values, record_id)
                   SELECT $1, $2, row.key_code, row.context_id, row.key_hash, row.key_values, row.record_id
                   FROM UNNEST($3::text[], $4::uuid[], $5::text[], $6::jsonb[], $7::uuid[])
                        AS row(key_code, context_id, key_hash, key_values, record_id)"#,
            )
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(chunk.iter().map(|row| row.key_code.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.context_id).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.key_hash.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.key_values.clone()).collect::<Vec<_>>())
            .bind(chunk.iter().map(|row| row.record_id).collect::<Vec<_>>())
            .execute(&mut **transaction)
            .await?;
        }
        Ok(())
    }

    async fn enforced_hierarchies(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
    ) -> Result<BTreeMap<String, Hierarchy>, RepositoryError> {
        sqlx::query_as::<_, (String, String)>(
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
        .map(|(code, hierarchy)| Ok((code, Hierarchy::parse(&hierarchy)?)))
        .collect()
    }

    /// Rejects a new edge that would close a cycle in a hierarchy, or give an
    /// record a second parent in a tree. Edges of every revision of the
    /// blueprint family's field count; the latest published revision decides
    /// whether the field is a hierarchy. The edge is checked in every context
    /// whose resolved value of the field it becomes.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn validate_relationship_hierarchy(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
        attribute_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        target_record_id: Uuid,
    ) -> Result<(), RepositoryError> {
        // Record-scoped reusable attributes are not part of a blueprint field.
        let Some(context_fallback) = write
            .by_id(attribute_id)
            .filter(|attribute| !attribute.record_scoped)
            .map(|attribute| attribute.context_fallback.clone())
        else {
            return Ok(());
        };
        let Some(hierarchy) = write
            .family_constraints(transaction, self.workspace_id.0, record.blueprint_id)
            .await?
            .hierarchies
            .get(attribute_code)
            .cloned()
        else {
            return Ok(());
        };
        self.lock_relationship_cardinality_writes(transaction)
            .await?;
        if hierarchy == Hierarchy::Tree {
            let other_parent = sqlx::query_scalar::<_, Uuid>(
                r#"SELECT av.relationship_target_record_id
                   FROM attribute_values av JOIN attributes a ON a.id = av.attribute_id
                   WHERE av.workspace_id = $1 AND av.record_id = $2
                     AND a.blueprint_id = $3 AND a.code = $4
                     AND av.context_id IS NOT DISTINCT FROM $5
                     AND av.relationship_target_record_id IS NOT NULL AND av.active
                     AND av.relationship_target_record_id <> $6
                   LIMIT 1"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.id)
            .bind(record.blueprint_id)
            .bind(attribute_code)
            .bind(context_id)
            .bind(target_record_id)
            .fetch_optional(&mut **transaction)
            .await?;
            if let Some(existing_target) = other_parent {
                return Err(RepositoryError::RelationshipCardinalityConflict {
                    attribute: attribute_code.to_owned(),
                    context_id,
                    source_record_id: record.id,
                    target_record_id: existing_target,
                    conflicting_source_record_id: None,
                });
            }
        }
        if target_record_id == record.id {
            return Err(RepositoryError::RelationshipCycle {
                attribute: attribute_code.to_owned(),
                path: vec![record.id, record.id],
            });
        }
        let tree = &write.tree;
        let written = match context_id {
            Some(context_id) => context_id,
            None => tree.default_context()?.id,
        };
        let field = HierarchyField {
            code: attribute_code,
            family: Some(record.blueprint_id),
        };
        let source_edges =
            hierarchy_edges(transaction, self.workspace_id.0, &field, &[record.id], None).await?;
        let source_contexts: HashSet<Uuid> = source_edges
            .get(&record.id)
            .map(|edges| edges.by_context.keys().copied().collect())
            .unwrap_or_default();
        let inherit = context_fallback != "none";
        // Without edges outside the default context every context resolves
        // the default graph or, without fallback, a subgraph of it, so the
        // default walk decides for all of them.
        let default_only = written == tree.default_context()?.id
            && !sqlx::query_scalar::<_, bool>(
                r#"SELECT EXISTS (
                       SELECT 1 FROM attribute_values av JOIN attributes a ON a.id = av.attribute_id
                       WHERE av.workspace_id = $1 AND a.blueprint_id = $2 AND a.code = $3
                         AND av.context_id <> $4
                         AND av.relationship_target_record_id IS NOT NULL AND av.active)"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.blueprint_id)
            .bind(attribute_code)
            .bind(written)
            .fetch_one(&mut **transaction)
            .await?;
        for context in tree.nodes() {
            if default_only && context.id != written {
                continue;
            }
            let path = tree.path(context.id, inherit)?;
            // The edge becomes this context's value only when no nearer
            // context overrides the field for the source.
            let Some(position) = path.iter().position(|id| *id == written) else {
                continue;
            };
            if path[..position]
                .iter()
                .any(|id| source_contexts.contains(id))
            {
                continue;
            }
            if let HierarchyWalk::Cycle(path) = walk_hierarchy(
                transaction,
                self.workspace_id.0,
                tree,
                context.id,
                &field,
                &[target_record_id],
                record.id,
                None,
            )
            .await?
            {
                return Err(RepositoryError::RelationshipCycle {
                    attribute: attribute_code.to_owned(),
                    path,
                });
            }
        }
        Ok(())
    }

    /// Checks every family's hierarchies against existing edges after a
    /// context reparent changed how edges are inherited. The caller holds the
    /// workspace relationship lock.
    pub(super) async fn validate_workspace_hierarchies(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), RepositoryError> {
        let families: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT blueprint_id FROM records WHERE workspace_id = $1 AND deleted_at IS NULL ORDER BY blueprint_id",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?;
        let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
        for blueprint_id in families {
            for (code, hierarchy) in self.enforced_hierarchies(transaction, blueprint_id).await? {
                self.validate_existing_hierarchy(
                    transaction,
                    &tree,
                    blueprint_id,
                    &code,
                    hierarchy,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Checks a family field's existing edges, resolved in every context of
    /// `tree`, when a publication makes it a hierarchy.
    async fn validate_existing_hierarchy(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        tree: &ContextTree,
        blueprint_id: Uuid,
        attribute_code: &str,
        hierarchy: Hierarchy,
    ) -> Result<(), RepositoryError> {
        let field = HierarchyField {
            code: attribute_code,
            family: Some(blueprint_id),
        };
        let edges = hierarchy_edges(transaction, self.workspace_id.0, &field, &[], None).await?;
        let mut cycles: Vec<Vec<Uuid>> = Vec::new();
        let mut multiple_parents = BTreeSet::new();
        for context in tree.nodes() {
            let graph = resolve_graph(&edges, tree, context.id)?;
            if hierarchy == Hierarchy::Tree {
                multiple_parents.extend(
                    graph
                        .iter()
                        .filter(|(_, targets)| targets.len() > 1)
                        .map(|(source, _)| *source),
                );
            }
            for cycle in find_cycles(&graph) {
                if !cycles.contains(&cycle) {
                    cycles.push(cycle);
                }
            }
        }
        if cycles.is_empty() && multiple_parents.is_empty() {
            return Ok(());
        }
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

/// The relationship field a hierarchy walk follows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HierarchyField<'a> {
    pub code: &'a str,
    /// Restricts edges to one blueprint family; `None` follows the code on
    /// any record, including the record's own additional attributes.
    pub family: Option<Uuid>,
}

/// One record's direct edges of a field.
#[derive(Clone, Debug, Default, PartialEq)]
struct RecordEdges {
    inherit: bool,
    by_context: BTreeMap<Uuid, Vec<Uuid>>,
}

/// Active edges of `field` from `sources` (every live record when empty),
/// optionally only in `contexts`.
async fn hierarchy_edges(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    field: &HierarchyField<'_>,
    sources: &[Uuid],
    contexts: Option<&[Uuid]>,
) -> Result<BTreeMap<Uuid, RecordEdges>, RepositoryError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String)>(
        r#"SELECT av.record_id, av.context_id, av.relationship_target_record_id, a.context_fallback
           FROM attribute_values av
           JOIN records e ON e.id = av.record_id AND e.workspace_id = av.workspace_id AND e.deleted_at IS NULL
           JOIN attributes a ON a.id = av.attribute_id AND a.code = $2 AND a.deleted_at IS NULL
            AND ($3::uuid IS NULL OR a.blueprint_id = $3)
            AND ((a.blueprint_id = e.blueprint_id AND a.blueprint_version = e.blueprint_version) OR a.record_id = e.id)
           WHERE av.workspace_id = $1 AND av.relationship_target_record_id IS NOT NULL AND av.active
             AND (cardinality($4::uuid[]) = 0 OR av.record_id = ANY($4))
             AND ($5::uuid[] IS NULL OR av.context_id = ANY($5))
           ORDER BY av.record_id, av.context_id, av.relationship_target_record_id"#,
    )
    .bind(workspace_id)
    .bind(field.code)
    .bind(field.family)
    .bind(sources)
    .bind(contexts)
    .fetch_all(conn)
    .await?;
    let mut edges: BTreeMap<Uuid, RecordEdges> = BTreeMap::new();
    for (source, context, target, fallback) in rows {
        let entry = edges.entry(source).or_default();
        entry.inherit = fallback != "none";
        entry.by_context.entry(context).or_default().push(target);
    }
    Ok(edges)
}

/// Each source's resolved targets in one context.
fn resolve_graph(
    edges: &BTreeMap<Uuid, RecordEdges>,
    tree: &ContextTree,
    context_id: Uuid,
) -> Result<BTreeMap<Uuid, Vec<Uuid>>, RepositoryError> {
    let path = tree.path(context_id, true)?;
    Ok(edges
        .iter()
        .filter_map(|(source, edges)| {
            resolve_on_path(&path, edges.inherit, |context| {
                edges.by_context.get(&context)
            })
            .map(|(_, targets)| (*source, targets.clone()))
        })
        .collect())
}

/// Outcome of a hierarchy walk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HierarchyWalk {
    Clear,
    /// The closed path `[goal, start, ..., goal]`.
    Cycle(Vec<Uuid>),
    /// More than the allowed number of records were visited.
    LimitReached,
}

/// Breadth-first walk along `field`'s edges as resolved in `context_id`,
/// from `starts` until `goal` is reached. With `goal`'s edges to `starts`
/// this tells whether they form a cycle. Writes walk without a limit; rule
/// evaluation passes one.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn walk_hierarchy(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    tree: &ContextTree,
    context_id: Uuid,
    field: &HierarchyField<'_>,
    starts: &[Uuid],
    goal: Uuid,
    max_visits: Option<usize>,
) -> Result<HierarchyWalk, RepositoryError> {
    if starts.contains(&goal) {
        return Ok(HierarchyWalk::Cycle(vec![goal, goal]));
    }
    let path = tree.path(context_id, true)?;
    let mut predecessor: HashMap<Uuid, Option<Uuid>> =
        starts.iter().map(|start| (*start, None)).collect();
    let mut frontier: Vec<Uuid> = starts.to_vec();
    frontier.sort();
    frontier.dedup();
    while !frontier.is_empty() {
        if max_visits.is_some_and(|limit| predecessor.len() > limit) {
            return Ok(HierarchyWalk::LimitReached);
        }
        let edges = hierarchy_edges(conn, workspace_id, field, &frontier, Some(&path)).await?;
        let mut next = Vec::new();
        for source in &frontier {
            let Some(source_edges) = edges.get(source) else {
                continue;
            };
            let Some((_, targets)) = resolve_on_path(&path, source_edges.inherit, |context| {
                source_edges.by_context.get(&context)
            }) else {
                continue;
            };
            for target in targets {
                if *target == goal {
                    let mut back = vec![*source];
                    let mut cursor = *source;
                    while let Some(Some(previous)) = predecessor.get(&cursor) {
                        back.push(*previous);
                        cursor = *previous;
                    }
                    let chain = std::iter::once(goal)
                        .chain(back.into_iter().rev())
                        .chain(std::iter::once(goal))
                        .collect();
                    return Ok(HierarchyWalk::Cycle(chain));
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = predecessor.entry(*target)
                {
                    entry.insert(Some(*source));
                    next.push(*target);
                }
            }
        }
        frontier = next;
    }
    Ok(HierarchyWalk::Clear)
}

/// Readable summary for error messages, which agents and CLI users see
/// without the structured `details`.
pub(super) fn describe_duplicates(duplicates: &[UniqueKeyDuplicate]) -> String {
    duplicates
        .iter()
        .map(|duplicate| {
            format!(
                "key '{}' values {} in context '{}' on records {}",
                duplicate.key,
                duplicate.values,
                duplicate.context,
                join_ids(&duplicate.record_ids)
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

/// Computes every key row for `records`, or only `only_context`'s. A key
/// without a value for one of its attributes in a context is absent there.
/// Workspace keys use the default context only; context keys resolve every
/// context with inheritance.
fn key_rows(
    keys: &[EnforcedUniqueKey],
    contexts: &ContextTree,
    records: &BTreeMap<Uuid, RecordValues>,
    only_context: Option<Uuid>,
) -> Result<Vec<KeyRow>, RepositoryError> {
    let default_context = contexts.default_context()?.id;
    let wanted = |context_id: Uuid| only_context.is_none_or(|only| only == context_id);
    let mut paths: Vec<(Uuid, Vec<Uuid>)> = Vec::new();
    for context in contexts.nodes() {
        if wanted(context.id) {
            paths.push((context.id, contexts.path(context.id, true)?));
        }
    }
    let workspace_path = if wanted(default_context) {
        vec![(default_context, vec![default_context])]
    } else {
        Vec::new()
    };
    let mut rows = Vec::new();
    for record in records.values() {
        for key in keys {
            let scoped = match key.scope {
                UniqueKeyScope::Context => &paths,
                UniqueKeyScope::Workspace => &workspace_path,
            };
            'context: for (context_id, path) in scoped {
                let mut components = Vec::with_capacity(key.attributes.len());
                for code in &key.attributes {
                    // Keys name blueprint fields, never additional attributes.
                    let component = record
                        .attributes
                        .get(code)
                        .filter(|attribute| !attribute.record_scoped)
                        .and_then(|attribute| {
                            attribute.resolve(path).and_then(|direct| {
                                normalize_key_component(
                                    &attribute.value_type,
                                    &direct.key_input(),
                                    key.case_sensitive,
                                )
                            })
                        });
                    let Some(component) = component else {
                        continue 'context;
                    };
                    components.push(component);
                }
                let key_values = Value::Array(components);
                rows.push(KeyRow {
                    record_id: record.id,
                    key_code: key.code.clone(),
                    context_id: *context_id,
                    key_hash: key_hash(&key_values),
                    key_values,
                });
            }
        }
    }
    Ok(rows)
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
    use super::super::record_values::{AttributeValues, ContextNode, DirectValue};
    use super::*;

    fn contexts(nodes: &[(u128, &str, Option<u128>)]) -> ContextTree {
        ContextTree::new(
            nodes
                .iter()
                .map(|(id, code, parent)| ContextNode {
                    id: Uuid::from_u128(*id),
                    code: (*code).to_owned(),
                    parent_id: parent.map(Uuid::from_u128),
                })
                .collect(),
        )
    }

    /// `(record, attribute, context, value)` string rows.
    fn records(
        rows: &[(u128, &str, u128, &str)],
        no_fallback: &[&str],
    ) -> BTreeMap<Uuid, RecordValues> {
        let mut records: BTreeMap<Uuid, RecordValues> = BTreeMap::new();
        for (record, attribute, context, value) in rows {
            let id = Uuid::from_u128(*record);
            records
                .entry(id)
                .or_insert_with(|| RecordValues::empty(id, Uuid::nil(), 1))
                .attributes
                .entry((*attribute).to_owned())
                .or_insert_with(|| AttributeValues {
                    value_type: "string".to_owned(),
                    inherit: !no_fallback.contains(attribute),
                    ..AttributeValues::default()
                })
                .by_context
                .insert(
                    Uuid::from_u128(*context),
                    DirectValue {
                        value: Value::String((*value).to_owned()),
                        changed_at: chrono::DateTime::<chrono::Utc>::MIN_UTC,
                        exact: None,
                    },
                );
        }
        records
    }

    fn key(code: &str, attributes: &[&str], scope: UniqueKeyScope) -> EnforcedUniqueKey {
        EnforcedUniqueKey {
            code: code.to_owned(),
            attributes: attributes.iter().map(|code| (*code).to_owned()).collect(),
            scope,
            case_sensitive: false,
        }
    }

    #[test]
    fn strings_are_trimmed_collapsed_and_case_folded_unless_case_sensitive() {
        let contexts = contexts(&[(1, "default", None)]);
        let sources = records(
            &[
                (10, "sku", 1, "  ab-1\t  X "),
                (11, "sku", 1, "AB-1 x"),
                (12, "sku", 1, "   "),
            ],
            &[],
        );
        let rows = key_rows(
            &[key("sku", &["sku"], UniqueKeyScope::Workspace)],
            &contexts,
            &sources,
            None,
        )
        .unwrap();
        assert_eq!(rows.len(), 2, "a blank string is missing");
        assert_eq!(rows[0].key_values, serde_json::json!(["ab-1 x"]));
        assert_eq!(rows[0].key_hash, rows[1].key_hash);

        let mut sensitive = key("sku", &["sku"], UniqueKeyScope::Workspace);
        sensitive.case_sensitive = true;
        let rows = key_rows(&[sensitive], &contexts, &sources, None).unwrap();
        assert_ne!(rows[0].key_hash, rows[1].key_hash);
        assert_eq!(rows[1].key_values, serde_json::json!(["AB-1 x"]));
    }

    #[test]
    fn number_keys_hash_the_exact_decimal() {
        let contexts = contexts(&[(1, "default", None)]);
        let mut sources = records(&[(10, "price", 1, "x"), (11, "price", 1, "x")], &[]);
        for (record, exact, json) in [(10u128, "1.50", 1.5), (11, "1.5", 1.5)] {
            let attribute = sources
                .get_mut(&Uuid::from_u128(record))
                .unwrap()
                .attributes
                .get_mut("price")
                .unwrap();
            attribute.value_type = "number".to_owned();
            let direct = attribute.by_context.get_mut(&Uuid::from_u128(1)).unwrap();
            direct.value = serde_json::json!(json);
            direct.exact = Some(exact.to_owned());
        }
        let rows = key_rows(
            &[key("price", &["price"], UniqueKeyScope::Workspace)],
            &contexts,
            &sources,
            None,
        )
        .unwrap();
        assert_eq!(rows[0].key_values, serde_json::json!(["1.5"]));
        assert_eq!(rows[0].key_hash, rows[1].key_hash);
    }

    #[test]
    fn composite_keys_need_every_component_and_context_keys_resolve_inheritance() {
        let contexts = contexts(&[
            (1, "default", None),
            (2, "pl", Some(1)),
            (3, "pl-web", Some(2)),
        ]);
        let rows = [
            (10, "document", 1, "D-1"),
            (10, "revision", 1, "A"),
            (11, "document", 1, "D-1"),
            (10, "slug", 1, "shirt"),
            (10, "slug", 2, "koszula"),
        ];
        let sources = records(&rows, &[]);
        let composite = key_rows(
            &[key(
                "revision",
                &["document", "revision"],
                UniqueKeyScope::Workspace,
            )],
            &contexts,
            &sources,
            None,
        )
        .unwrap();
        assert_eq!(composite.len(), 1, "record 11 has no revision");
        assert_eq!(composite[0].key_values, serde_json::json!(["d-1", "a"]));
        assert_eq!(composite[0].context_id, Uuid::from_u128(1));

        let slugs = key_rows(
            &[key("slug", &["slug"], UniqueKeyScope::Context)],
            &contexts,
            &sources,
            None,
        )
        .unwrap();
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

        let none = records(&rows, &["slug"]);
        let slugs = key_rows(
            &[key("slug", &["slug"], UniqueKeyScope::Context)],
            &contexts,
            &none,
            None,
        )
        .unwrap();
        assert_eq!(slugs.len(), 2, "fallback none stops inheritance");

        let pl_web = Uuid::from_u128(3);
        let only = |records| {
            key_rows(
                &[
                    key("slug", &["slug"], UniqueKeyScope::Context),
                    key("document", &["document"], UniqueKeyScope::Workspace),
                ],
                &contexts,
                records,
                Some(pl_web),
            )
            .unwrap()
        };
        let inherited = only(&sources);
        assert_eq!(inherited.len(), 1, "workspace keys live in default only");
        assert_eq!(inherited[0].context_id, pl_web);
        assert_eq!(inherited[0].key_values, serde_json::json!(["koszula"]));
        assert!(only(&none).is_empty(), "a new context has no direct values");
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

    #[test]
    fn resolved_graphs_combine_inherited_and_local_edges() {
        let contexts = contexts(&[(1, "default", None), (2, "fr", Some(1))]);
        let [a, b, c] = [10u128, 11, 12].map(Uuid::from_u128);
        let [default, fr] = [1u128, 2].map(Uuid::from_u128);
        // A -> B in default; B -> A only in fr. Neither context's direct
        // edges form a cycle, but fr inherits A -> B.
        let mut edges = BTreeMap::from([
            (
                a,
                RecordEdges {
                    inherit: true,
                    by_context: BTreeMap::from([(default, vec![b])]),
                },
            ),
            (
                b,
                RecordEdges {
                    inherit: true,
                    by_context: BTreeMap::from([(fr, vec![a])]),
                },
            ),
        ]);
        assert!(find_cycles(&resolve_graph(&edges, &contexts, default).unwrap()).is_empty());
        assert_eq!(
            find_cycles(&resolve_graph(&edges, &contexts, fr).unwrap()),
            vec![vec![a, b, a]]
        );
        // A local fr edge of A overrides the inherited one.
        edges.get_mut(&a).unwrap().by_context.insert(fr, vec![c]);
        assert!(find_cycles(&resolve_graph(&edges, &contexts, fr).unwrap()).is_empty());
        // Without fallback A has no fr edge at all.
        edges.get_mut(&a).unwrap().by_context.remove(&fr);
        edges.get_mut(&a).unwrap().inherit = false;
        assert!(find_cycles(&resolve_graph(&edges, &contexts, fr).unwrap()).is_empty());
    }
}
