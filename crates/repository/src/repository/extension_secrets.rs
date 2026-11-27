//! Secrets of extension installations.
//!
//! An operator sets named secrets for one installed extension, such as an API
//! key or a webhook signing key. Values are stored only as AES-256-GCM
//! ciphertext under the `extension_credential_key` secret, bound to their
//! workspace, extension and name, so a ciphertext copied to another row does
//! not open. They are decrypted only for that extension's component, inside
//! the host, and never reach the management API, configuration, operation
//! snapshots, logs, traces or audit records.

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use super::extension_connections::credential_key;
use super::{AttricatRepository, RepositoryError};

/// The longest secret value an operator may set.
pub const MAX_EXTENSION_SECRET_VALUE_BYTES: usize = 65_536;
/// The longest secret name, as the table's `name` check allows.
pub const MAX_EXTENSION_SECRET_NAME_BYTES: usize = 128;

/// A secret as management sees it: its name and when it was set, never its
/// value.
#[derive(Clone, Debug, FromRow)]
pub struct ExtensionSecret {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Whether `name` is a secret name the table accepts.
pub fn valid_extension_secret_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_EXTENSION_SECRET_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The additional data that binds a ciphertext to its secret.
fn associated_data(workspace_id: Uuid, extension_id: &str, name: &str) -> String {
    format!("attricat.extension_secret:{workspace_id}:{extension_id}:{name}")
}

fn seal(
    workspace_id: Uuid,
    extension_id: &str,
    name: &str,
    value: &str,
) -> Result<Vec<u8>, RepositoryError> {
    super::sealing::seal(
        credential_key()?,
        &associated_data(workspace_id, extension_id, name),
        value,
    )
    .ok_or_else(|| RepositoryError::InvalidExtension("secret encryption failed".into()))
}

fn open(
    workspace_id: Uuid,
    extension_id: &str,
    name: &str,
    ciphertext: &[u8],
) -> Result<String, RepositoryError> {
    super::sealing::open(
        credential_key()?,
        &associated_data(workspace_id, extension_id, name),
        ciphertext,
    )
    .ok_or_else(|| RepositoryError::InvalidExtension("stored secret cannot be decrypted".into()))
}

impl AttricatRepository {
    /// Lists an installed extension's secret names and timestamps, never
    /// values.
    pub async fn extension_secrets(
        &self,
        extension_id: &str,
    ) -> Result<Vec<ExtensionSecret>, RepositoryError> {
        Ok(sqlx::query_as("SELECT name, created_at, updated_at FROM extension_secrets WHERE workspace_id = $1 AND extension_id = $2 ORDER BY name")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_all(&self.pool).await?)
    }

    /// Sets or replaces a secret of an installed extension. The value is
    /// never copied into the extension's manifest, configuration, operation
    /// snapshots, lifecycle records or audit metadata.
    pub async fn put_extension_secret(
        &self,
        extension_id: &str,
        name: &str,
        value: &str,
    ) -> Result<(), RepositoryError> {
        if !valid_extension_secret_name(name) {
            return Err(RepositoryError::InvalidExtension(format!(
                "secret names are 1-{MAX_EXTENSION_SECRET_NAME_BYTES} letters, digits, '.', '_' or '-'"
            )));
        }
        if value.is_empty() || value.len() > MAX_EXTENSION_SECRET_VALUE_BYTES {
            return Err(RepositoryError::InvalidExtension(format!(
                "a secret value is 1-{MAX_EXTENSION_SECRET_VALUE_BYTES} bytes"
            )));
        }
        let ciphertext = seal(self.workspace_id(), extension_id, name, value)?;
        let mut transaction = self.pool.begin().await?;
        // Locks the installation, so a secret cannot be set for an extension
        // that is being removed.
        let installed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM extension_installations WHERE workspace_id = $1 AND extension_id = $2 FOR SHARE)")
            .bind(self.workspace_id()).bind(extension_id)
            .fetch_one(&mut *transaction).await?;
        if !installed {
            return Err(RepositoryError::NotFound("extension installation"));
        }
        sqlx::query("INSERT INTO extension_secrets (id, workspace_id, extension_id, name, value_ciphertext) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, extension_id, name) DO UPDATE SET value_ciphertext = EXCLUDED.value_ciphertext, updated_at = clock_timestamp()")
            .bind(Uuid::new_v4()).bind(self.workspace_id()).bind(extension_id).bind(name).bind(ciphertext)
            .execute(&mut *transaction).await?;
        self.with_workspace_audit_action(
            "extension.secret.put",
            json!({"type": "extension_secret", "extension_id": extension_id, "name": name}),
            json!({"extension_id": extension_id, "name": name}),
        )
        .write_audit_event(&mut transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn delete_extension_secret(
        &self,
        extension_id: &str,
        name: &str,
    ) -> Result<bool, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM extension_secrets WHERE workspace_id = $1 AND extension_id = $2 AND name = $3",
        )
        .bind(self.workspace_id())
        .bind(extension_id)
        .bind(name)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
            > 0;
        if deleted {
            self.with_workspace_audit_action(
                "extension.secret.delete",
                json!({"type": "extension_secret", "extension_id": extension_id, "name": name}),
                json!({"extension_id": extension_id, "name": name}),
            )
            .write_audit_event(&mut transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(deleted)
    }

    /// The value of `extension_id`'s secret `name`, for that extension's
    /// currently authorized invocation only. Another extension's secret of
    /// the same name is never returned. Do not expose this through a
    /// management response.
    pub async fn extension_secret_value(
        &self,
        extension_id: &str,
        name: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let ciphertext: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT value_ciphertext FROM extension_secrets WHERE workspace_id = $1 AND extension_id = $2 AND name = $3",
        )
        .bind(self.workspace_id())
        .bind(extension_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        ciphertext
            .map(|ciphertext| open(self.workspace_id(), extension_id, name, &ciphertext))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::use_extension_credential_key;

    #[test]
    fn ciphertexts_are_bound_to_their_workspace_extension_and_name() {
        use_extension_credential_key([7u8; 32]);
        let workspace = Uuid::new_v4();
        let sealed = seal(workspace, "acme.shop", "api-key", "secret-value").unwrap();
        assert!(!sealed.windows(12).any(|window| window == b"secret-value"));
        assert_eq!(
            open(workspace, "acme.shop", "api-key", &sealed).unwrap(),
            "secret-value"
        );
        assert!(open(workspace, "acme.other", "api-key", &sealed).is_err());
        assert!(open(workspace, "acme.shop", "other-key", &sealed).is_err());
        assert!(open(Uuid::new_v4(), "acme.shop", "api-key", &sealed).is_err());
        let mut tampered = sealed;
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(workspace, "acme.shop", "api-key", &tampered).is_err());
    }

    #[test]
    fn names_match_the_table_check() {
        assert!(valid_extension_secret_name("api-key.v2_live"));
        assert!(valid_extension_secret_name(
            &"a".repeat(MAX_EXTENSION_SECRET_NAME_BYTES)
        ));
        assert!(!valid_extension_secret_name(""));
        assert!(!valid_extension_secret_name("bad name"));
        assert!(!valid_extension_secret_name("a:b"));
        assert!(!valid_extension_secret_name(
            &"a".repeat(MAX_EXTENSION_SECRET_NAME_BYTES + 1)
        ));
    }
}
