use super::*;
use crate::constants::CONVERSATION_ATTACHMENT_LIFETIME_SECONDS;
use crate::domain_events::{ATTRIBUTE_VALUE_CHANGED_V1, AttributeValueMutationV1};
use crate::persistence_rows::{Db, IntoDomain};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Transaction;

#[derive(Clone, Debug, Deserialize)]
pub struct FilePolicy {
    pub cardinality: String,
    #[serde(default)]
    pub ordered: bool,
    pub allowed_mime_groups: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub max_bytes: Option<u64>,
    pub image_only: bool,
}

impl FilePolicy {
    /// Whether a file of a detected MIME type is accepted by this policy.
    pub fn allows(&self, mime: &str, filename: &str, size: u64) -> bool {
        catalog_validation::files::FileConstraints {
            allowed_mime_groups: &self.allowed_mime_groups,
            allowed_extensions: &self.allowed_extensions,
            max_bytes: self.max_bytes,
            image_only: self.image_only,
        }
        .allows(mime, filename, size)
    }
}

/// A file attribute's local value before and after one write.
struct FileValueChange<'a> {
    attribute_id: Uuid,
    attribute_code: &'a str,
    context_id: Uuid,
    before: Vec<Uuid>,
    after: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct NewUploadedFile {
    pub original_filename: String,
    pub display_filename: String,
    pub mime_type: String,
    pub byte_size: u64,
    pub sha256: String,
    pub object_key: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadedFile {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileUploadResult {
    pub attribute_code: String,
    pub context_id: Uuid,
    pub files: Vec<UploadedFile>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct FileReadTarget {
    pub entity_id: Uuid,
    pub blueprint_id: Uuid,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct FileObject {
    pub mime_type: String,
    pub byte_size: i64,
    pub display_filename: String,
    pub object_key: String,
    pub status: String,
    /// Digest of the stored bytes; a file and its variants never change.
    pub sha256: String,
}

impl CatalogRepository {
    /// Validates the immutable entity blueprint and returns its file policy before
    /// object bytes are accepted. The entity is locked again while persisting,
    /// so a concurrent mutation cannot evade the cardinality check.
    pub async fn file_upload_policy(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
    ) -> Result<FilePolicy, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let (_, policy, context_editable) = self
            .file_upload_attribute(&mut transaction, &entity, attribute_code)
            .await?;
        let context_id = self
            .file_upload_context(&mut transaction, context_id)
            .await?;
        self.validate_context_editable(&mut transaction, Some(context_id), &context_editable)
            .await?;
        self.ensure_attribute_unlocked(&mut transaction, &entity, attribute_code, context_id)
            .await?;
        transaction.commit().await?;
        Ok(policy)
    }

    pub async fn persist_uploaded_files(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        files: Vec<NewUploadedFile>,
    ) -> Result<FileUploadResult, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let (attribute_id, policy, context_editable) = self
            .file_upload_attribute(&mut transaction, &entity, attribute_code)
            .await?;
        let context_id = self
            .file_upload_context(&mut transaction, context_id)
            .await?;
        self.validate_context_editable(&mut transaction, Some(context_id), &context_editable)
            .await?;
        self.ensure_attribute_unlocked(&mut transaction, &entity, attribute_code, context_id)
            .await?;

        let current = self
            .current_file_ids(&mut transaction, entity_id, attribute_id, context_id)
            .await?;
        if policy.cardinality == "one" && files.len() != 1 {
            return Err(RepositoryError::FileCardinality);
        }
        if policy.cardinality == "many" && current.len() + files.len() > i32::MAX as usize {
            return Err(RepositoryError::FileCardinality);
        }

        let mut file_ids = if policy.cardinality == "many" {
            current.clone()
        } else {
            Vec::new()
        };
        let mut result = Vec::with_capacity(files.len());
        for file in files {
            let id = Uuid::new_v4();
            let status = "queued".to_owned();
            self.insert_file_in_transaction(&mut transaction, id, &file)
                .await?;
            file_ids.push(id);
            result.push(UploadedFile {
                id,
                filename: file.display_filename,
                mime_type: file.mime_type,
                byte_size: file.byte_size as i64,
                sha256: file.sha256,
                status,
            });
        }
        self.replace_file_value(
            transaction,
            &entity,
            FileValueChange {
                attribute_id,
                attribute_code,
                context_id,
                before: current,
                after: file_ids,
            },
        )
        .await?;
        Ok(FileUploadResult {
            attribute_code: attribute_code.to_owned(),
            context_id,
            files: result,
        })
    }

    /// Records an uploaded object as a queued workspace file with a metadata
    /// job, in the caller's transaction. Every attribute file write uses it.
    pub(super) async fn insert_file_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        id: Uuid,
        file: &NewUploadedFile,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        self.finish_file_upload(transaction, &file.object_key)
            .await?;
        sqlx::query(
            "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'queued')",
        )
        .bind(id).bind(workspace_id).bind(&file.original_filename).bind(&file.display_filename)
        .bind(&file.mime_type).bind(file.byte_size as i64).bind(&file.sha256).bind(&file.object_key)
        .execute(&mut **transaction).await?;
        sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status) VALUES ($1,$2,$3,'metadata','queued')")
            .bind(Uuid::new_v4()).bind(workspace_id).bind(id).execute(&mut **transaction).await?;
        Ok(())
    }

    /// References files from an attribute value at the given positions.
    pub(super) async fn insert_attribute_file_references_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        value_id: Uuid,
        references: &[(Uuid, i32)],
    ) -> Result<(), RepositoryError> {
        if references.is_empty() {
            return Ok(());
        }
        let (file_ids, positions): (Vec<Uuid>, Vec<i32>) = references.iter().copied().unzip();
        sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) SELECT $1, $2, file_id, position FROM UNNEST($3::uuid[], $4::int4[]) AS r(file_id, position)")
            .bind(value_id).bind(self.workspace_id.0).bind(file_ids).bind(positions)
            .execute(&mut **transaction).await?;
        Ok(())
    }

    /// References a file from an attribute value at a position.
    pub(super) async fn insert_attribute_file_reference_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        value_id: Uuid,
        file_id: Uuid,
        position: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1,$2,$3,$4)")
            .bind(value_id).bind(self.workspace_id.0).bind(file_id).bind(position)
            .execute(&mut **transaction).await?;
        Ok(())
    }

    /// Removes or reorders existing references only. The expected ordered list
    /// is a compare-and-swap guard against concurrent uploads and edits. Archived
    /// references preserve history; this never deletes shared file objects.
    /// Returns the entity's new `updated_at`, so an open edit form can adopt
    /// its own change instead of treating it as a concurrent edit.
    pub async fn update_file_references(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        expected_file_ids: &[Uuid],
        file_ids: &[Uuid],
    ) -> Result<DateTime<Utc>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let (attribute_id, policy, context_editable) = self
            .file_upload_attribute(&mut transaction, &entity, attribute_code)
            .await?;
        let context_id = self
            .file_upload_context(&mut transaction, context_id)
            .await?;
        self.validate_context_editable(&mut transaction, Some(context_id), &context_editable)
            .await?;
        self.ensure_attribute_unlocked(&mut transaction, &entity, attribute_code, context_id)
            .await?;
        let current = self
            .current_file_ids(&mut transaction, entity_id, attribute_id, context_id)
            .await?;
        if current != expected_file_ids {
            return Err(RepositoryError::FileReferencesChanged);
        }
        let unique: std::collections::HashSet<_> = file_ids.iter().collect();
        let existing: std::collections::HashSet<_> = current.iter().collect();
        if unique.len() != file_ids.len() || !unique.is_subset(&existing) {
            return Err(RepositoryError::InvalidFileReferences);
        }
        if !policy.ordered
            && current
                .iter()
                .filter(|id| unique.contains(id))
                .copied()
                .collect::<Vec<_>>()
                != file_ids
        {
            return Err(RepositoryError::InvalidFileReferences);
        }
        if policy.cardinality == "one" && file_ids.len() > 1 {
            return Err(RepositoryError::FileCardinality);
        }
        if current == file_ids {
            transaction.commit().await?;
            return Ok(entity.updated_at);
        }
        // Keep an explicit empty local value: removing every image must not
        // unexpectedly reveal photos inherited from a fallback context.
        let updated = self
            .replace_file_value(
                transaction,
                &entity,
                FileValueChange {
                    attribute_id,
                    attribute_code,
                    context_id,
                    before: current,
                    after: file_ids.to_vec(),
                },
            )
            .await?;
        Ok(updated.updated_at)
    }

    /// Links an existing workspace file to a file attribute without copying
    /// storage bytes or creating another file record.
    pub async fn link_file_to_attribute(
        &self,
        entity_id: Uuid,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_id: Uuid,
    ) -> Result<FileMetadata, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let (change, file_ids) = self
            .prepare_file_link(
                &mut transaction,
                &entity,
                attribute_code,
                context_id,
                &[file_id],
            )
            .await?;
        self.replace_file_value(
            transaction,
            &entity,
            FileValueChange {
                after: file_ids,
                ..change
            },
        )
        .await?;
        self.file_metadata(file_id).await
    }

    /// Appends files to a file attribute of `entity`, which the caller has
    /// locked, in one value write, without revalidating, auditing or
    /// committing, so the transaction that created the entity can link every
    /// file of an attribute and context without archiving intermediate lists.
    pub(super) async fn link_files_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_code: &str,
        context_id: Option<Uuid>,
        file_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let (change, file_ids) = self
            .prepare_file_link(transaction, entity, attribute_code, context_id, file_ids)
            .await?;
        self.write_file_value(
            transaction,
            entity.id,
            change.attribute_id,
            change.context_id,
            &file_ids,
        )
        .await
    }

    /// Locks the workspace's file rows among `file_ids`, in ID order. A write
    /// that links several file lists takes every lock here first, so two such
    /// writes sharing files lock them in one global order and cannot
    /// deadlock; each list's own lock in [`Self::prepare_file_link`] then
    /// finds them held.
    pub(super) async fn lock_files_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        file_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let mut unique = file_ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        sqlx::query(
            "SELECT id FROM files WHERE id = ANY($1) AND workspace_id = $2 ORDER BY id FOR UPDATE",
        )
        .bind(&unique)
        .bind(self.workspace_id.0)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Checks that `file_ids` can be linked to the attribute and returns the
    /// change with the current files and the file list after linking: the
    /// files appended in order to a many-valued attribute, or replacing the
    /// value of a single-valued one.
    async fn prepare_file_link<'a>(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_code: &'a str,
        context_id: Option<Uuid>,
        file_ids: &[Uuid],
    ) -> Result<(FileValueChange<'a>, Vec<Uuid>), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let (attribute_id, policy, context_editable) = self
            .file_upload_attribute(transaction, entity, attribute_code)
            .await?;
        let context_id = self.file_upload_context(transaction, context_id).await?;
        self.validate_context_editable(transaction, Some(context_id), &context_editable)
            .await?;
        self.ensure_attribute_unlocked(transaction, entity, attribute_code, context_id)
            .await?;
        let mut unique = file_ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        // Lock the files, in ID order, so reconciliation, which locks
        // candidates before marking them deleted, cannot reclaim one before
        // this reference commits.
        let live = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM files WHERE id = ANY($1) AND workspace_id = $2 AND deleted_at IS NULL AND purpose = 'attachment' ORDER BY id FOR UPDATE",
        )
        .bind(&unique)
        .bind(workspace_id)
        .fetch_all(&mut **transaction)
        .await?;
        if live.len() != unique.len() {
            return Err(RepositoryError::NotFound("file"));
        }
        let current = self
            .current_file_ids(transaction, entity.id, attribute_id, context_id)
            .await?;
        let mut linked = if policy.cardinality == "many" {
            current.clone()
        } else {
            Vec::new()
        };
        linked.extend_from_slice(file_ids);
        if linked.len() > i32::MAX as usize || (policy.cardinality == "one" && linked.len() > 1) {
            return Err(RepositoryError::FileCardinality);
        }
        Ok((
            FileValueChange {
                attribute_id,
                attribute_code,
                context_id,
                before: current,
                after: Vec::new(),
            },
            linked,
        ))
    }

    /// Persists files uploaded from a conversation without creating an entity
    /// attribute value. A 15-minute attachment window protects them from
    /// reconciliation until the message attachment transaction claims them.
    pub async fn persist_conversation_uploads(
        &self,
        conversation_id: Uuid,
        uploaded_by_user_id: Uuid,
        files: Vec<NewUploadedFile>,
    ) -> Result<Vec<UploadedFile>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = $1 AND workspace_id = $2 AND archived_at IS NULL)",
        )
        .bind(conversation_id)
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !exists {
            return Err(RepositoryError::NotFound("conversation"));
        }
        let mut result = Vec::with_capacity(files.len());
        for file in files {
            self.finish_file_upload(&mut transaction, &file.object_key)
                .await?;
            let id = Uuid::new_v4();
            let status = "queued".to_owned();
            sqlx::query(
                "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, attachment_expires_at, conversation_upload_conversation_id, conversation_upload_user_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,now() + make_interval(secs => $10),$11,$12)",
            )
            .bind(id)
            .bind(workspace_id)
            .bind(&file.original_filename)
            .bind(&file.display_filename)
            .bind(&file.mime_type)
            .bind(file.byte_size as i64)
            .bind(&file.sha256)
            .bind(&file.object_key)
            .bind(&status)
            .bind(CONVERSATION_ATTACHMENT_LIFETIME_SECONDS)
            .bind(conversation_id)
            .bind(uploaded_by_user_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status) VALUES ($1,$2,$3,'metadata','queued')")
                .bind(Uuid::new_v4())
                .bind(workspace_id)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
            result.push(UploadedFile {
                id,
                filename: file.display_filename,
                mime_type: file.mime_type,
                byte_size: file.byte_size as i64,
                sha256: file.sha256,
                status,
            });
        }
        transaction.commit().await?;
        Ok(result)
    }

    /// Returns active entities that currently reference a file. Deleted entities
    /// and archived attribute values cannot authorize a file read.
    pub async fn file_read_targets(
        &self,
        file_id: Uuid,
    ) -> Result<Vec<FileReadTarget>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, FileReadTarget>(
            "SELECT DISTINCT e.id AS entity_id, e.blueprint_id FROM attribute_file_references r JOIN attribute_values v ON v.id = r.attribute_value_id AND v.workspace_id = r.workspace_id JOIN entities e ON e.id = v.entity_id AND e.workspace_id = v.workspace_id JOIN blueprints b ON b.id = e.blueprint_id AND b.version = e.blueprint_version AND b.workspace_id = e.workspace_id WHERE r.file_id = $1 AND r.workspace_id = $2 AND v.active AND e.deleted_at IS NULL AND b.deleted_at IS NULL",
        )
        .bind(file_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Reads client-safe metadata. Object keys and original filenames remain
    /// repository internals and are never serialized from this method.
    pub async fn file_metadata(&self, file_id: Uuid) -> Result<FileMetadata, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let row = sqlx::query_as::<_, (Uuid, String, String, i64, String, String)>(
            "SELECT id, display_filename, mime_type, byte_size, sha256, status FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(file_id).bind(workspace_id).fetch_optional(&self.pool).await?
        .ok_or(RepositoryError::NotFound("file"))?;
        let variants = sqlx::query_as::<_, Db<FileVariantMetadata>>(
            "SELECT kind, mime_type, width, height, byte_size, sha256 FROM file_variants WHERE file_id = $1 AND workspace_id = $2 ORDER BY kind",
        )
        .bind(file_id).bind(workspace_id).fetch_all(&self.pool).await?.into_domain();
        Ok(FileMetadata {
            id: row.0,
            filename: row.1,
            mime_type: row.2,
            byte_size: row.3,
            sha256: row.4,
            status: row.5,
            variants,
        })
    }

    pub async fn file_object(
        &self,
        file_id: Uuid,
        variant_kind: Option<&str>,
    ) -> Result<FileObject, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let row = match variant_kind {
            Some(kind) => sqlx::query_as::<_, FileObject>(
                "SELECT v.mime_type, v.byte_size, ''::TEXT AS display_filename, v.object_key, f.status, v.sha256 FROM file_variants v JOIN files f ON f.id = v.file_id AND f.workspace_id = v.workspace_id WHERE v.file_id = $1 AND v.workspace_id = $2 AND v.kind = $3 AND f.deleted_at IS NULL",
            ).bind(file_id).bind(workspace_id).bind(kind).fetch_optional(&self.pool).await?,
            None => sqlx::query_as::<_, FileObject>(
                "SELECT mime_type, byte_size, display_filename, original_key AS object_key, status, sha256 FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
            ).bind(file_id).bind(workspace_id).fetch_optional(&self.pool).await?,
        };
        row.ok_or(RepositoryError::NotFound("file"))
    }

    /// The file IDs of the local value, in position order. The caller holds
    /// the entity lock, which serializes every writer of its file values.
    async fn current_file_ids(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Uuid,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT r.file_id FROM attribute_file_references r JOIN attribute_values v ON v.id = r.attribute_value_id AND v.workspace_id = r.workspace_id WHERE v.entity_id = $1 AND v.attribute_id = $2 AND v.context_id = $3 AND v.workspace_id = $4 AND v.relationship_target_entity_id IS NULL ORDER BY r.position, r.file_id",
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .bind(self.workspace_id.0)
        .fetch_all(&mut **transaction)
        .await?)
    }

    /// Replaces a file attribute's local value and commits the entity
    /// mutation. Like [`Self::insert_value`], the previous value row and its
    /// references are archived, so history keeps every earlier file list,
    /// even when files are only appended. The change is audited and emitted
    /// through the shared entity-mutation seam. Returns the stored entity.
    async fn replace_file_value(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        entity: &Entity,
        change: FileValueChange<'_>,
    ) -> Result<Entity, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        self.write_file_value(
            &mut transaction,
            entity.id,
            change.attribute_id,
            change.context_id,
            &change.after,
        )
        .await?;
        let stored = self.revalidate_entity(&mut transaction, entity).await?;
        let context_code: String = sqlx::query_scalar(
            "SELECT code FROM attribute_contexts WHERE id = $1 AND workspace_id = $2",
        )
        .bind(change.context_id)
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await?;
        let file_list = |ids: &[Uuid]| {
            (!ids.is_empty())
                .then(|| Value::from(ids.iter().map(Uuid::to_string).collect::<Vec<_>>()))
        };
        let before_value = file_list(&change.before);
        let after_value = file_list(&change.after);
        let changes = vec![AuditEventChange {
            entity_id: entity.id,
            attribute_id: change.attribute_id,
            attribute_code: change.attribute_code.to_owned(),
            context_id: Some(change.context_id),
            context_code: Some(context_code),
            relationship_target_entity_id: None,
            change_kind: match (&before_value, &after_value) {
                (None, _) => "set",
                (Some(_), None) => "remove",
                (Some(_), Some(_)) => "replace",
            },
            before_value,
            after_value,
        }];
        let event = self.core_event(
            ATTRIBUTE_VALUE_CHANGED_V1,
            "entity",
            entity.id,
            serde_json::to_value(AttributeValueMutationV1 {
                entity_id: entity.id,
                facts: Self::affected_facts(&changes),
            })
            .expect("attribute-value-changed payload is serializable"),
        );
        self.commit_entity_mutation(transaction, changes, event)
            .await?;
        Ok(stored)
    }

    /// Archives the local file value and stores `file_ids` as its new value,
    /// in order, without revalidating, auditing or committing.
    async fn write_file_value(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        attribute_id: Uuid,
        context_id: Uuid,
        file_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        self.archive_current_value(transaction, entity_id, attribute_id, Some(context_id), None)
            .await?;
        let value_id = Uuid::new_v4();
        sqlx::query("INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true)")
            .bind(value_id).bind(self.workspace_id.0).bind(entity_id).bind(attribute_id).bind(context_id)
            .execute(&mut **transaction).await?;
        let references: Vec<(Uuid, i32)> = file_ids
            .iter()
            .enumerate()
            .map(|(position, file_id)| (*file_id, position as i32))
            .collect();
        self.insert_attribute_file_references_in_transaction(transaction, value_id, &references)
            .await
    }

    pub(super) async fn file_upload_attribute(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_code: &str,
    ) -> Result<(Uuid, FilePolicy, String), RepositoryError> {
        let row = sqlx::query_as::<_, (Uuid, String, Option<Value>, String, bool)>(
            "SELECT id, value_type, file_policy, context_editable, readonly FROM attributes WHERE code = $1 AND ((blueprint_id = $2 AND blueprint_version = $3) OR entity_id = $4) AND workspace_id = $5 AND deleted_at IS NULL",
        )
        .bind(attribute_code).bind(entity.blueprint_id).bind(entity.blueprint_version).bind(entity.id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut **transaction).await?
        .ok_or(RepositoryError::AttributeNotApplicable)?;
        if row.1 != "file" {
            return Err(RepositoryError::AttributeNotApplicable);
        }
        if row.4 {
            return Err(RepositoryError::FileAttributeReadonly);
        }
        let policy = row
            .2
            .and_then(|value| serde_json::from_value(value).ok())
            .ok_or(RepositoryError::InvalidFilePolicy)?;
        Ok((row.0, policy, row.3))
    }

    async fn file_upload_context(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        context_id: Option<Uuid>,
    ) -> Result<Uuid, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let context_id = match context_id {
            Some(id) => id,
            None => sqlx::query_scalar(
                "SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'",
            )
            .bind(workspace_id)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or(RepositoryError::InvalidContext)?,
        };
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM attribute_contexts WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(context_id)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?;
        exists
            .then_some(context_id)
            .ok_or(RepositoryError::InvalidContext)
    }
}

/// Every source that can still read a workspace file's stored bytes, as a
/// predicate over a `files f` row. Reconciliation marks a live file deleted
/// only while this holds, and a purge job removes objects only while it still
/// holds, so a new kind of file reference must be added here and nowhere else.
///
/// Extension input artifacts copy the file's object key rather than its ID, so
/// a pending or leased run keeps the original object alive until it finishes.
const FILE_UNREFERENCED: &str = r#"(f.attachment_expires_at IS NULL OR f.attachment_expires_at <= now())
  AND NOT EXISTS (SELECT 1 FROM attribute_file_references r WHERE r.workspace_id = f.workspace_id AND r.file_id = f.id)
  AND NOT EXISTS (SELECT 1 FROM conversation_message_attachments a WHERE a.workspace_id = f.workspace_id AND a.file_id = f.id)
  AND NOT EXISTS (SELECT 1 FROM workspace_memberships m WHERE m.workspace_id = f.workspace_id AND m.avatar_file_id = f.id)
  AND NOT EXISTS (SELECT 1 FROM file_retention_holds h WHERE h.workspace_id = f.workspace_id AND h.file_id = f.id AND h.released_at IS NULL AND h.held_until > now())
  AND NOT EXISTS (SELECT 1 FROM blueprint_connector_jobs j WHERE j.workspace_id = f.workspace_id AND j.input_file_id = f.id)
  AND NOT EXISTS (SELECT 1 FROM extension_operation_schedules s WHERE s.workspace_id = f.workspace_id AND s.source_reference->>'input_file_id' = f.id::text)
  AND NOT EXISTS (SELECT 1 FROM extension_operation_artifacts x JOIN extension_operation_runs run ON run.id = x.operation_run_id AND run.workspace_id = x.workspace_id WHERE x.workspace_id = f.workspace_id AND x.direction = 'input' AND x.object_key = f.original_key AND run.status IN ('pending', 'leased'))"#;

/// Upper bound on files marked, or purge jobs queued, by one reconciliation.
const RECONCILE_BATCH: i64 = 256;

impl SystemRepository {
    /// Marks a bounded batch of unreferenced live files deleted, to be purged
    /// after `grace_seconds`. Returns how many files were marked.
    ///
    /// Every writer that adds a file reference locks the live file row first.
    /// Locking candidates here (skipping a file whose reference is being
    /// written) makes the reference check and the deletion share that lock: a
    /// reference committed first retains the file, and a reference attempted
    /// after this commit fails its own live-file check.
    pub async fn mark_unreferenced_files_deleted(
        &self,
        grace_seconds: i64,
    ) -> Result<u64, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let candidates: Vec<Uuid> = sqlx::query_scalar(&format!(
            "SELECT f.id FROM files f WHERE f.deleted_at IS NULL AND {FILE_UNREFERENCED} ORDER BY f.id LIMIT $1 FOR UPDATE OF f SKIP LOCKED"
        ))
        .bind(RECONCILE_BATCH)
        .fetch_all(&mut *transaction)
        .await?;
        if candidates.is_empty() {
            return Ok(0);
        }
        let marked = sqlx::query(&format!(
            "UPDATE files f SET status = 'deleted', deleted_at = now(), purge_after = now() + make_interval(secs => $1), updated_at = now() WHERE f.id = ANY($2) AND f.deleted_at IS NULL AND {FILE_UNREFERENCED}"
        ))
        .bind(grace_seconds)
        .bind(&candidates)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        transaction.commit().await?;
        Ok(marked)
    }

    /// Queues one purge job for each deleted file whose grace period has
    /// passed. Returns how many jobs were queued.
    pub async fn queue_due_file_purges(&self) -> Result<u64, RepositoryError> {
        // Lock a bounded batch while inserting. A concurrent SELECT can still
        // use an older snapshot, so the unique index is the final safeguard
        // against duplicate purge jobs; ON CONFLICT handles that race.
        let mut transaction = self.pool.begin().await?;
        let due: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
            r#"SELECT f.workspace_id, f.id, f.purge_after FROM files f
            WHERE f.deleted_at IS NOT NULL AND f.purge_after <= now()
              AND NOT EXISTS (SELECT 1 FROM file_processing_jobs j WHERE j.file_id = f.id AND j.kind = 'purge' AND j.status IN ('queued','running','retryable','completed'))
            ORDER BY f.purge_after, f.id
            LIMIT $1 FOR UPDATE OF f SKIP LOCKED"#,
        )
        .bind(RECONCILE_BATCH)
        .fetch_all(&mut *transaction)
        .await?;
        let mut queued = 0;
        for (workspace_id, file_id, available_at) in due {
            queued += sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status, available_at) VALUES ($1,$2,$3,'purge','queued',$4) ON CONFLICT DO NOTHING")
                .bind(Uuid::new_v4())
                .bind(workspace_id)
                .bind(file_id)
                .bind(available_at)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
        }
        transaction.commit().await?;
        Ok(queued)
    }

    /// Returns the object keys (original first, then variants) of a deleted
    /// file whose grace period has passed and that is still unreferenced, or
    /// `None` when its bytes must be kept.
    pub async fn purgeable_file_objects(
        &self,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<Vec<String>>, RepositoryError> {
        let original: Option<String> = sqlx::query_scalar(&format!(
            "SELECT f.original_key FROM files f WHERE f.id = $1 AND f.workspace_id = $2 AND f.deleted_at IS NOT NULL AND f.purge_after <= now() AND {FILE_UNREFERENCED}"
        ))
        .bind(file_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(original) = original else {
            return Ok(None);
        };
        let variants: Vec<String> = sqlx::query_scalar(
            "SELECT object_key FROM file_variants WHERE file_id = $1 AND workspace_id = $2 ORDER BY kind",
        )
        .bind(file_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(std::iter::once(original).chain(variants).collect()))
    }
}
