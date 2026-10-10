use attricat_lexicon::{Entry, EntryIdentity, Report, UsedReference};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// A stored translation. `source` is `workspace` for entries written through
/// the API or `solution_pack` for entries a pack supplied and nobody has
/// edited since.
#[derive(Clone, Debug, FromRow, Serialize)]
pub struct LexiconEntry {
    pub key: String,
    pub context: Option<String>,
    pub language: String,
    pub plural_category: String,
    pub text: String,
    pub source: String,
    pub solution_pack_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl From<LexiconEntry> for Entry {
    fn from(entry: LexiconEntry) -> Self {
        Self {
            key: entry.key,
            context: entry.context,
            language: entry.language,
            plural_category: entry.plural_category,
            text: entry.text,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LexiconImportMode {
    /// Upsert the imported entries and keep every other entry.
    Merge,
    /// Make the language contain exactly the imported entries.
    Replace,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct LexiconImportSummary {
    pub created: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub deleted: u64,
}

const FIELDS: &str = "key, NULLIF(context, '') AS context, language, plural_category, text, source, solution_pack_id, updated_at";
const ORDER: &str = "ORDER BY language, key, context, array_position(ARRAY['zero','one','two','few','many','other'], plural_category)";

/// Column arrays for an `UNNEST` bulk upsert.
#[derive(Default)]
struct EntryColumns {
    ids: Vec<Uuid>,
    keys: Vec<String>,
    contexts: Vec<String>,
    languages: Vec<String>,
    plural_categories: Vec<String>,
    texts: Vec<String>,
}

impl EntryColumns {
    fn new(entries: &[Entry]) -> Self {
        let mut columns = Self::default();
        for entry in entries {
            columns.ids.push(Uuid::new_v4());
            columns.keys.push(entry.key.clone());
            columns
                .contexts
                .push(entry.context.clone().unwrap_or_default());
            columns.languages.push(entry.language.clone());
            columns
                .plural_categories
                .push(entry.plural_category.clone());
            columns.texts.push(entry.text.clone());
        }
        columns
    }
}

impl AttricatRepository {
    pub async fn list_lexicon_entries(
        &self,
        language: Option<&str>,
    ) -> Result<Vec<LexiconEntry>, RepositoryError> {
        Ok(sqlx::query_as(&format!(
            "SELECT {FIELDS} FROM lexicon_entries WHERE workspace_id = $1 AND ($2::text IS NULL OR language = $2) {ORDER}"
        ))
        .bind(self.workspace_id_for_runtime())
        .bind(language)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Creates or replaces one entry. A workspace write takes over an entry a
    /// solution pack supplied, so later pack releases leave it unchanged.
    pub async fn upsert_lexicon_entry(
        &self,
        entry: Entry,
    ) -> Result<LexiconEntry, RepositoryError> {
        let entry = entry.validated()?;
        let mut transaction = self.pool.begin().await?;
        let stored = sqlx::query_as(&format!(
            "INSERT INTO lexicon_entries (id, workspace_id, key, context, language, plural_category, text, source)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'workspace')
             ON CONFLICT (workspace_id, key, context, language, plural_category) DO UPDATE
             SET text = EXCLUDED.text, source = 'workspace', solution_pack_id = NULL, updated_at = now()
             RETURNING {FIELDS}"
        ))
        .bind(Uuid::new_v4())
        .bind(self.workspace_id_for_runtime())
        .bind(&entry.key)
        .bind(entry.context.as_deref().unwrap_or_default())
        .bind(&entry.language)
        .bind(&entry.plural_category)
        .bind(&entry.text)
        .fetch_one(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(stored)
    }

    pub async fn delete_lexicon_entry(
        &self,
        identity: &EntryIdentity,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM lexicon_entries WHERE workspace_id = $1 AND key = $2 AND context = $3 AND language = $4 AND plural_category = $5",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(&identity.key)
        .bind(identity.context.as_deref().unwrap_or_default())
        .bind(&identity.language)
        .bind(&identity.plural_category)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
            > 0;
        if deleted {
            self.commit_mutation(transaction).await?;
        }
        Ok(deleted)
    }

    /// Imports validated entries of one language as workspace entries.
    pub async fn import_lexicon_entries(
        &self,
        language: &str,
        entries: &[Entry],
        mode: LexiconImportMode,
    ) -> Result<LexiconImportSummary, RepositoryError> {
        debug_assert!(entries.iter().all(|entry| entry.language == language));
        let workspace_id = self.workspace_id_for_runtime();
        let columns = EntryColumns::new(entries);
        let mut transaction = self.pool.begin().await?;
        let mut summary = LexiconImportSummary::default();
        if mode == LexiconImportMode::Replace {
            summary.deleted = sqlx::query(
                "DELETE FROM lexicon_entries e WHERE e.workspace_id = $1 AND e.language = $2
                 AND NOT EXISTS (
                     SELECT 1 FROM UNNEST($3::text[], $4::text[], $5::text[]) AS i(key, context, plural_category)
                     WHERE i.key = e.key AND i.context = e.context AND i.plural_category = e.plural_category
                 )",
            )
            .bind(workspace_id)
            .bind(language)
            .bind(&columns.keys)
            .bind(&columns.contexts)
            .bind(&columns.plural_categories)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        }
        // `xmax = 0` distinguishes inserted rows; unchanged rows are filtered
        // by the conflict predicate and not returned.
        let written: Vec<bool> = sqlx::query_scalar(
            "INSERT INTO lexicon_entries (id, workspace_id, key, context, language, plural_category, text, source)
             SELECT i.id, $1, i.key, i.context, i.language, i.plural_category, i.text, 'workspace'
             FROM UNNEST($2::uuid[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[])
                 AS i(id, key, context, language, plural_category, text)
             ON CONFLICT (workspace_id, key, context, language, plural_category) DO UPDATE
             SET text = EXCLUDED.text, source = 'workspace', solution_pack_id = NULL, updated_at = now()
             WHERE lexicon_entries.text <> EXCLUDED.text OR lexicon_entries.source <> 'workspace'
             RETURNING xmax = 0",
        )
        .bind(workspace_id)
        .bind(&columns.ids)
        .bind(&columns.keys)
        .bind(&columns.contexts)
        .bind(&columns.languages)
        .bind(&columns.plural_categories)
        .bind(&columns.texts)
        .fetch_all(&mut *transaction)
        .await?;
        summary.created = written.iter().filter(|inserted| **inserted).count() as u64;
        summary.updated = written.len() as u64 - summary.created;
        summary.unchanged = entries.len() as u64 - written.len() as u64;
        self.commit_mutation(transaction).await?;
        Ok(summary)
    }

    /// Writes solution-pack entries inside an apply step. Entries the
    /// workspace wrote or took over are left unchanged. Returns the number of
    /// inserted or updated rows.
    pub(super) async fn apply_solution_pack_lexicon_in_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        solution_pack_id: &str,
        entries: &[Entry],
    ) -> Result<u64, RepositoryError> {
        let columns = EntryColumns::new(entries);
        Ok(sqlx::query(
            "INSERT INTO lexicon_entries (id, workspace_id, key, context, language, plural_category, text, source, solution_pack_id)
             SELECT i.id, $1, i.key, i.context, i.language, i.plural_category, i.text, 'solution_pack', $8
             FROM UNNEST($2::uuid[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[])
                 AS i(id, key, context, language, plural_category, text)
             ON CONFLICT (workspace_id, key, context, language, plural_category) DO UPDATE
             SET text = EXCLUDED.text, solution_pack_id = EXCLUDED.solution_pack_id, updated_at = now()
             WHERE lexicon_entries.source = 'solution_pack'
               AND (lexicon_entries.text <> EXCLUDED.text OR lexicon_entries.solution_pack_id <> EXCLUDED.solution_pack_id)",
        )
        .bind(workspace_id)
        .bind(&columns.ids)
        .bind(&columns.keys)
        .bind(&columns.contexts)
        .bind(&columns.languages)
        .bind(&columns.plural_categories)
        .bind(&columns.texts)
        .bind(solution_pack_id)
        .execute(&mut **transaction)
        .await?
        .rows_affected())
    }

    /// References used by every non-deleted blueprint revision (records can
    /// stay pinned to older ones), reusable attribute name, and status option
    /// label of a reusable attribute revision.
    pub async fn lexicon_references(&self) -> Result<Vec<UsedReference>, RepositoryError> {
        let workspace_id = self.workspace_id_for_runtime();
        let definitions: Vec<String> = sqlx::query_scalar(
            "SELECT definition FROM blueprints WHERE workspace_id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        let mut references: Vec<UsedReference> = definitions
            .iter()
            .filter_map(|definition| attricat_blueprint::parse(definition).ok())
            .flat_map(|definition| attricat_blueprint::lexicon_references(&definition))
            .collect();
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM reusable_attribute_definitions WHERE workspace_id = $1 AND deleted_at IS NULL",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        references.extend(names.iter().flat_map(|name| {
            attricat_lexicon::references(name)
                .unwrap_or_default()
                .into_iter()
                .map(|reference| UsedReference {
                    reference,
                    counted: false,
                })
        }));
        let schemas: Vec<Value> = sqlx::query_scalar(
            "SELECT r.value_schema FROM reusable_attribute_revisions r
             JOIN reusable_attribute_definitions d ON d.id = r.definition_id
             WHERE r.workspace_id = $1 AND d.deleted_at IS NULL AND r.value_schema IS NOT NULL",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        references.extend(
            schemas
                .iter()
                .flat_map(|schema| attricat_blueprint::status_option_texts("attribute", schema))
                .flat_map(|text| attricat_lexicon::references(text.text).unwrap_or_default())
                .map(|reference| UsedReference {
                    reference,
                    counted: false,
                }),
        );
        Ok(references)
    }

    pub async fn lexicon_report(&self, languages: &[String]) -> Result<Report, RepositoryError> {
        let references = self.lexicon_references().await?;
        let entries: Vec<Entry> = self
            .list_lexicon_entries(None)
            .await?
            .into_iter()
            .map(Entry::from)
            .collect();
        Ok(attricat_lexicon::report(&references, &entries, languages))
    }
}
