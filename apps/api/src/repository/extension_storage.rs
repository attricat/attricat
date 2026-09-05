use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use super::{AuditContext, CatalogRepository};

pub const MAX_EXTENSION_STORAGE_KEY_BYTES: usize = 256;
pub const MAX_EXTENSION_STORAGE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_EXTENSION_STORAGE_KEYS: i64 = 1_024;
pub const MAX_EXTENSION_STORAGE_BYTES: i64 = 5 * 1024 * 1024;
pub const MAX_EXTENSION_STORAGE_LIST_LIMIT: u32 = 100;

#[derive(Clone, Debug)]
pub struct ExtensionStorageEntry {
    pub key: String,
    pub value: Value,
    pub revision: i64,
}

#[derive(Clone, Debug)]
pub struct ExtensionStoragePage {
    pub entries: Vec<ExtensionStorageEntry>,
    pub cursor: Option<String>,
}

#[derive(Debug, Error)]
pub enum ExtensionStorageError {
    #[error("extension storage access is denied")]
    Denied,
    #[error("storage key must be normalized, non-empty, and at most 256 bytes")]
    InvalidKey,
    #[error("storage value must be valid JSON no larger than 64 KiB")]
    InvalidValue,
    #[error("storage list limit must be between 1 and 100")]
    InvalidLimit,
    #[error("storage revision conflict")]
    Conflict,
    #[error("extension storage quota exceeded")]
    QuotaExceeded,
    #[error("extension storage database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("extension storage audit operation failed: {0}")]
    Audit(String),
}

impl CatalogRepository {
    pub async fn extension_storage_get(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        key: &str,
    ) -> Result<Option<ExtensionStorageEntry>, ExtensionStorageError> {
        let key = normalized_key(key)?;
        self.require_storage_access(extension_id, expected_release_id)
            .await?;
        Ok(sqlx::query_as::<_, (Value, i64)>(
            "SELECT value, revision FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2 AND key = $3",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?
        .map(|(value, revision)| ExtensionStorageEntry { key: key.to_owned(), value, revision }))
    }

    pub async fn extension_storage_set(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        key: &str,
        value: Value,
        expected_revision: Option<i64>,
    ) -> Result<i64, ExtensionStorageError> {
        let key = normalized_key(key)?;
        let byte_size = value_size(&value)?;
        let mut transaction = self.pool.begin().await?;
        self.require_storage_access_locked(&mut transaction, extension_id, expected_release_id)
            .await?;
        let existing: Option<(i64, i32)> = sqlx::query_as(
            "SELECT revision, byte_size FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2 AND key = $3 FOR UPDATE",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .bind(&key)
        .fetch_optional(&mut *transaction)
        .await?;
        if expected_revision
            .is_some_and(|revision| existing.as_ref().map(|item| item.0) != Some(revision))
        {
            return Err(ExtensionStorageError::Conflict);
        }
        let revision = existing.as_ref().map_or(1, |item| item.0 + 1);
        let previous_size = existing.map_or(0_i64, |item| i64::from(item.1));
        let (key_count, total_bytes): (i64, i64) = sqlx::query_as(
            "SELECT count(*), COALESCE(sum(byte_size), 0) FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2",
        )
        .bind(self.extension_workspace())
        .bind(extension_id)
        .fetch_one(&mut *transaction)
        .await?;
        if (previous_size == 0 && key_count >= MAX_EXTENSION_STORAGE_KEYS)
            || total_bytes - previous_size + byte_size as i64 > MAX_EXTENSION_STORAGE_BYTES
        {
            return Err(ExtensionStorageError::QuotaExceeded);
        }
        sqlx::query("INSERT INTO extension_storage_entries (workspace_id, extension_id, key, value, revision, byte_size) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (workspace_id, extension_id, key) DO UPDATE SET value = EXCLUDED.value, revision = EXCLUDED.revision, byte_size = EXCLUDED.byte_size, updated_at = clock_timestamp()")
            .bind(self.extension_workspace()).bind(extension_id).bind(&key).bind(value).bind(revision).bind(byte_size as i32).execute(&mut *transaction).await?;
        self.commit_storage_mutation(transaction, "set", extension_id, &key)
            .await?;
        Ok(revision)
    }

    pub async fn extension_storage_delete(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        key: &str,
        expected_revision: Option<i64>,
    ) -> Result<(), ExtensionStorageError> {
        let key = normalized_key(key)?;
        let mut transaction = self.pool.begin().await?;
        self.require_storage_access_locked(&mut transaction, extension_id, expected_release_id)
            .await?;
        let revision: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2 AND key = $3 FOR UPDATE",
        )
        .bind(self.extension_workspace()).bind(extension_id).bind(&key)
        .fetch_optional(&mut *transaction).await?;
        if expected_revision.is_some_and(|item| revision != Some(item)) {
            return Err(ExtensionStorageError::Conflict);
        }
        sqlx::query("DELETE FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2 AND key = $3")
            .bind(self.extension_workspace()).bind(extension_id).bind(&key).execute(&mut *transaction).await?;
        self.commit_storage_mutation(transaction, "delete", extension_id, &key)
            .await?;
        Ok(())
    }

    pub async fn extension_storage_list(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
        prefix: Option<&str>,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ExtensionStoragePage, ExtensionStorageError> {
        if limit == 0 || limit > MAX_EXTENSION_STORAGE_LIST_LIMIT {
            return Err(ExtensionStorageError::InvalidLimit);
        }
        let prefix = prefix
            .map(normalized_key)
            .transpose()?
            .map(escape_like_prefix);
        let cursor = cursor.map(normalized_key).transpose()?;
        self.require_storage_access(extension_id, expected_release_id)
            .await?;
        let rows: Vec<(String, Value, i64)> = sqlx::query_as(
            "SELECT key, value, revision FROM extension_storage_entries WHERE workspace_id = $1 AND extension_id = $2 AND ($3::text IS NULL OR key LIKE $3 || '%' ESCAPE '\\') AND ($4::text IS NULL OR key > $4) ORDER BY key LIMIT $5",
        )
        .bind(self.extension_workspace()).bind(extension_id).bind(prefix).bind(cursor).bind(i64::from(limit) + 1)
        .fetch_all(&self.pool).await?;
        let has_more = rows.len() > limit as usize;
        let entries: Vec<_> = rows
            .into_iter()
            .take(limit as usize)
            .map(|(key, value, revision)| ExtensionStorageEntry {
                key,
                value,
                revision,
            })
            .collect();
        let cursor = has_more.then(|| entries.last().expect("page is non-empty").key.clone());
        Ok(ExtensionStoragePage { entries, cursor })
    }

    async fn require_storage_access(
        &self,
        extension_id: &str,
        expected_release_id: Uuid,
    ) -> Result<(), ExtensionStorageError> {
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_installations i JOIN extension_grants g ON g.installation_id = i.id WHERE i.workspace_id = $1 AND i.extension_id = $2 AND i.installed_release_id = $3 AND i.state = 'enabled' AND g.grant_kind = 'capability' AND g.grant_id = 'storage.extension')")
            .bind(self.extension_workspace()).bind(extension_id).bind(expected_release_id).fetch_one(&self.pool).await?;
        allowed.then_some(()).ok_or(ExtensionStorageError::Denied)
    }

    async fn require_storage_access_locked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        extension_id: &str,
        expected_release_id: Uuid,
    ) -> Result<(), ExtensionStorageError> {
        let installation_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM extension_installations WHERE workspace_id = $1 AND extension_id = $2 AND installed_release_id = $3 AND state = 'enabled' FOR UPDATE")
            .bind(self.extension_workspace()).bind(extension_id).bind(expected_release_id).fetch_optional(&mut **transaction).await?;
        let Some(installation_id) = installation_id else {
            return Err(ExtensionStorageError::Denied);
        };
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_grants WHERE installation_id = $1 AND grant_kind = 'capability' AND grant_id = 'storage.extension')")
            .bind(installation_id).fetch_one(&mut **transaction).await?;
        allowed.then_some(()).ok_or(ExtensionStorageError::Denied)
    }

    async fn commit_storage_mutation(
        &self,
        mut transaction: Transaction<'_, Postgres>,
        operation: &str,
        extension_id: &str,
        key: &str,
    ) -> Result<(), ExtensionStorageError> {
        if self.audit_context.is_some() {
            self.write_audit_event(&mut transaction)
                .await
                .map_err(|error| ExtensionStorageError::Audit(error.to_string()))?;
        } else {
            let mut system_repository = self.clone();
            system_repository.audit_context = Some(AuditContext {
                actor_user_id: None,
                actor_token_id: None,
                request_id: Uuid::new_v4(),
                correlation_id: Uuid::new_v4(),
                action: format!("extension.storage.{operation}"),
                authorization_scope: json!({"type": "workspace"}),
                target: json!({"type": "extension_storage", "extension_id": extension_id, "key": key}),
                metadata: json!({"operation": operation}),
                agent: None,
            });
            system_repository
                .write_audit_event(&mut transaction)
                .await
                .map_err(|error| ExtensionStorageError::Audit(error.to_string()))?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

fn normalized_key(key: &str) -> Result<&str, ExtensionStorageError> {
    if key.is_empty()
        || key.len() > MAX_EXTENSION_STORAGE_KEY_BYTES
        || key.trim() != key
        || key.chars().any(char::is_control)
    {
        Err(ExtensionStorageError::InvalidKey)
    } else {
        Ok(key)
    }
}
fn escape_like_prefix(prefix: &str) -> String {
    prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn value_size(value: &Value) -> Result<usize, ExtensionStorageError> {
    let size = serde_json::to_vec(value)
        .map_err(|_| ExtensionStorageError::InvalidValue)?
        .len();
    (size <= MAX_EXTENSION_STORAGE_VALUE_BYTES)
        .then_some(size)
        .ok_or(ExtensionStorageError::InvalidValue)
}
