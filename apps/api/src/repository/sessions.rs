use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::account::{PasswordHash, SessionDigest};

use super::{CatalogRepository, RepositoryError};

pub struct LocalLoginCredential {
    pub user_id: Uuid,
    pub password_hash: PasswordHash,
    pub active: bool,
    pub security_version: i32,
    pub credential_version: i32,
}

pub struct ValidBrowserSession {
    pub user_id: Uuid,
    pub csrf_digest: SessionDigest,
}

impl CatalogRepository {
    pub async fn local_login_credential(
        &self,
        email: &str,
    ) -> Result<Option<LocalLoginCredential>, RepositoryError> {
        let row = sqlx::query(
            "SELECT user_id, password_hash, user_state, security_version, credential_version FROM find_local_password_credential($1)",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok::<LocalLoginCredential, sqlx::Error>(LocalLoginCredential {
                user_id: row.try_get("user_id")?,
                password_hash: PasswordHash::from_phc(row.try_get::<String, _>("password_hash")?)
                    .map_err(|_| {
                    sqlx::Error::Protocol("stored password hash is invalid".into())
                })?,
                active: row.try_get::<String, _>("user_state")? == "active",
                security_version: row.try_get("security_version")?,
                credential_version: row.try_get("credential_version")?,
            })
        })
        .transpose()
        .map_err(RepositoryError::from)
    }

    pub async fn reserve_login_attempt(
        &self,
        key: &SessionDigest,
    ) -> Result<bool, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT reserve_browser_login_attempt($1)")
                .bind(key.as_ref())
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn clear_login_failures(&self, key: &SessionDigest) -> Result<(), RepositoryError> {
        sqlx::query("SELECT clear_browser_login_failures($1)")
            .bind(key.as_ref())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn issue_login_session(
        &self,
        id: Uuid,
        credential: &LocalLoginCredential,
        workspace_id: Uuid,
        session: &SessionDigest,
        csrf: &SessionDigest,
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT issue_browser_login_session($1, $2, $3, $4, $5, $6, $7, $8)")
            .bind(id)
            .bind(credential.user_id)
            .bind(workspace_id)
            .bind(credential.security_version)
            .bind(credential.credential_version)
            .bind(session.as_ref())
            .bind(csrf.as_ref())
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn validate_browser_session(
        &self,
        session: &SessionDigest,
        workspace_id: Uuid,
    ) -> Result<Option<ValidBrowserSession>, RepositoryError> {
        let row = sqlx::query("SELECT user_id, csrf_digest FROM validate_browser_session($1, $2)")
            .bind(session.as_ref())
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| {
            Ok::<ValidBrowserSession, sqlx::Error>(ValidBrowserSession {
                user_id: row.try_get("user_id")?,
                csrf_digest: SessionDigest::from_slice(&row.try_get::<Vec<u8>, _>("csrf_digest")?)
                    .map_err(|_| sqlx::Error::Protocol("stored csrf digest is invalid".into()))?,
            })
        })
        .transpose()
        .map_err(RepositoryError::from)
    }

    pub async fn rotate_browser_session(
        &self,
        previous: &SessionDigest,
        id: Uuid,
        session: &SessionDigest,
        csrf: &SessionDigest,
        workspace_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT rotate_browser_session($1, $2, $3, $4, $5, $6)")
            .bind(previous.as_ref())
            .bind(id)
            .bind(session.as_ref())
            .bind(csrf.as_ref())
            .bind(workspace_id)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_browser_session(
        &self,
        session: &SessionDigest,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT revoke_browser_session($1, $2)")
            .bind(session.as_ref())
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
