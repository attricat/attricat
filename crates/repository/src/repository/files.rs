use super::*;
use crate::constants::CONVERSATION_ATTACHMENT_LIFETIME_SECONDS;
use crate::persistence_rows::{Db, IntoDomain};
use serde::{Deserialize, Serialize};
use sqlx::Transaction;

#[derive(Clone, Debug, Deserialize)]
pub struct FilePolicy {
    pub cardinality: String,
    pub allowed_mime_groups: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub max_bytes: Option<u64>,
    pub image_only: bool,
}

fn publication_disposition_metadata(retained_role: Option<String>) -> serde_json::Value {
    match retained_role {
        Some(role_code) => serde_json::json!({
            "disposition": "retained",
            "role_code": role_code,
        }),
        None => serde_json::json!({
            "disposition": "withdrawn",
            "reason": "entity_changed",
        }),
    }
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

        let current_value = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attribute_values WHERE entity_id = $1 AND attribute_id = $2 AND context_id = $3 AND relationship_target_entity_id IS NULL FOR UPDATE",
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let current_count =
            match current_value {
                Some(value_id) => sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM attribute_file_references WHERE attribute_value_id = $1",
                )
                .bind(value_id)
                .fetch_one(&mut *transaction)
                .await?,
                None => 0,
            };
        if policy.cardinality == "one" && files.len() != 1 {
            return Err(RepositoryError::FileCardinality);
        }
        if policy.cardinality == "many" && current_count + files.len() as i64 > i32::MAX as i64 {
            return Err(RepositoryError::FileCardinality);
        }

        let value_id = if policy.cardinality == "one" {
            self.archive_current_value(
                &mut transaction,
                entity_id,
                attribute_id,
                Some(context_id),
                None,
            )
            .await?;
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .bind(entity_id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut *transaction)
            .await?
        } else if let Some(value_id) = current_value {
            value_id
        } else {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .bind(entity_id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut *transaction)
            .await?
        };
        let start_position = if policy.cardinality == "many" {
            current_count as i32
        } else {
            0
        };
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut result = Vec::with_capacity(files.len());
        for (offset, file) in files.into_iter().enumerate() {
            let id = Uuid::new_v4();
            let status = "queued".to_owned();
            sqlx::query(
                "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
            )
            .bind(id).bind(workspace_id).bind(&file.original_filename).bind(&file.display_filename)
            .bind(&file.mime_type).bind(file.byte_size as i64).bind(&file.sha256).bind(&file.object_key).bind(&status)
            .execute(&mut *transaction).await?;
            sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1,$2,$3,$4)")
                .bind(value_id).bind(workspace_id).bind(id).bind(start_position + offset as i32)
                .execute(&mut *transaction).await?;
            sqlx::query("INSERT INTO file_processing_jobs (id, workspace_id, file_id, kind, status) VALUES ($1,$2,$3,'metadata','queued')")
                .bind(Uuid::new_v4()).bind(workspace_id).bind(id).execute(&mut *transaction).await?;
            result.push(UploadedFile {
                id,
                filename: file.display_filename,
                mime_type: file.mime_type,
                byte_size: file.byte_size as i64,
                sha256: file.sha256,
                status,
            });
        }
        let retained_role = self
            .reconcile_entity_publication(&mut transaction, entity_id, "entity_changed")
            .await?;
        self.write_audit_event_with_publication_metadata(
            &mut transaction,
            Some(publication_disposition_metadata(retained_role)),
        )
        .await?;
        transaction.commit().await?;
        Ok(FileUploadResult {
            attribute_code: attribute_code.to_owned(),
            context_id,
            files: result,
        })
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
        let exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
        )
        .bind(file_id)
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if exists.is_none() {
            return Err(RepositoryError::NotFound("file"));
        }
        let current_value = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM attribute_values WHERE entity_id = $1 AND attribute_id = $2 AND context_id = $3 AND relationship_target_entity_id IS NULL FOR UPDATE",
        )
        .bind(entity_id)
        .bind(attribute_id)
        .bind(context_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let count =
            match current_value {
                Some(value_id) => sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM attribute_file_references WHERE attribute_value_id = $1",
                )
                .bind(value_id)
                .fetch_one(&mut *transaction)
                .await?,
                None => 0,
            };
        if policy.cardinality == "many" && count == i32::MAX as i64 {
            return Err(RepositoryError::FileCardinality);
        }
        let value_id = if policy.cardinality == "one" {
            self.archive_current_value(
                &mut transaction,
                entity_id,
                attribute_id,
                Some(context_id),
                None,
            )
            .await?;
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(entity_id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut *transaction)
            .await?
        } else if let Some(value_id) = current_value {
            value_id
        } else {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO attribute_values (id, workspace_id, entity_id, attribute_id, context_id, active) VALUES ($1, $2, $3, $4, $5, true) RETURNING id",
            )
            .bind(Uuid::new_v4())
            .bind(workspace_id)
            .bind(entity_id)
            .bind(attribute_id)
            .bind(context_id)
            .fetch_one(&mut *transaction)
            .await?
        };
        let position = if policy.cardinality == "one" {
            0
        } else {
            count as i32
        };
        sqlx::query("INSERT INTO attribute_file_references (attribute_value_id, workspace_id, file_id, position) VALUES ($1, $2, $3, $4)")
            .bind(value_id)
            .bind(workspace_id)
            .bind(file_id)
            .bind(position)
            .execute(&mut *transaction)
            .await?;
        let retained_role = self
            .reconcile_entity_publication(&mut transaction, entity_id, "entity_changed")
            .await?;
        self.write_audit_event_with_publication_metadata(
            &mut transaction,
            Some(publication_disposition_metadata(retained_role)),
        )
        .await?;
        transaction.commit().await?;
        self.file_metadata(file_id).await
    }

    /// Persists files uploaded from a conversation without creating an entity
    /// attribute value. A 15-minute attachment window protects them from
    /// reconciliation until the message attachment transaction claims them.
    pub async fn persist_conversation_uploads(
        &self,
        files: Vec<NewUploadedFile>,
    ) -> Result<Vec<UploadedFile>, RepositoryError> {
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let mut transaction = self.pool.begin().await?;
        let mut result = Vec::with_capacity(files.len());
        for file in files {
            let id = Uuid::new_v4();
            let status = "queued".to_owned();
            sqlx::query(
                "INSERT INTO files (id, workspace_id, original_filename, display_filename, mime_type, byte_size, sha256, original_key, status, attachment_expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,now() + make_interval(secs => $10))",
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let row = match variant_kind {
            Some(kind) => sqlx::query_as::<_, FileObject>(
                "SELECT v.mime_type, v.byte_size, ''::TEXT AS display_filename, v.object_key, f.status FROM file_variants v JOIN files f ON f.id = v.file_id AND f.workspace_id = v.workspace_id WHERE v.file_id = $1 AND v.workspace_id = $2 AND v.kind = $3 AND f.deleted_at IS NULL",
            ).bind(file_id).bind(workspace_id).bind(kind).fetch_optional(&self.pool).await?,
            None => sqlx::query_as::<_, FileObject>(
                "SELECT mime_type, byte_size, display_filename, original_key AS object_key, status FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL",
            ).bind(file_id).bind(workspace_id).fetch_optional(&self.pool).await?,
        };
        row.ok_or(RepositoryError::NotFound("file"))
    }

    async fn file_upload_attribute(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_code: &str,
    ) -> Result<(Uuid, FilePolicy, String), RepositoryError> {
        let row = sqlx::query_as::<_, (Uuid, String, Option<Value>, String)>(
            "SELECT id, value_type, file_policy, context_editable FROM attributes WHERE code = $1 AND ((blueprint_id = $2 AND blueprint_version = $3) OR entity_id = $4) AND deleted_at IS NULL",
        )
        .bind(attribute_code).bind(entity.blueprint_id).bind(entity.blueprint_version).bind(entity.id)
        .fetch_optional(&mut **transaction).await?
        .ok_or(RepositoryError::AttributeNotApplicable)?;
        if row.1 != "file" {
            return Err(RepositoryError::AttributeNotApplicable);
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
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
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
