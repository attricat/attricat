use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::account::{ActionTokenDigest, PasswordHash, SessionDigest};

use super::{CatalogRepository, RepositoryError};

pub struct LocalLoginCredential {
    pub user_id: Uuid,
    pub password_hash: PasswordHash,
    pub active: bool,
    pub email_verified: bool,
    pub security_version: i32,
    pub credential_version: i32,
}

pub struct ValidBrowserSession {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub csrf_digest: SessionDigest,
    /// The session's workspace is not deleted. With the validated user and
    /// membership this is exactly `is_active_principal`, so callers need no
    /// second round trip; a deleted workspace still authenticates and is
    /// rejected as forbidden.
    pub workspace_active: bool,
    /// The session workspace's cache generations.
    pub generations: super::WorkspaceGenerations,
}

pub struct DiscoveredWorkspace {
    pub id: Uuid,
    pub login_identifier: String,
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    pub async fn discover_workspace(
        &self,
        login_identifier: &str,
    ) -> Result<Option<DiscoveredWorkspace>, RepositoryError> {
        let row = sqlx::query("SELECT id, login_identifier FROM workspaces WHERE login_identifier = lower(btrim($1)) AND deleted_at IS NULL")
            .bind(login_identifier)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| {
            Ok::<DiscoveredWorkspace, sqlx::Error>(DiscoveredWorkspace {
                id: row.try_get("id")?,
                login_identifier: row.try_get("login_identifier")?,
            })
        })
        .transpose()
        .map_err(RepositoryError::from)
    }

    pub async fn workspace_login_identifier(
        &self,
        workspace_id: Uuid,
    ) -> Result<String, RepositoryError> {
        Ok(
            sqlx::query_scalar("SELECT login_identifier FROM workspaces WHERE id = $1")
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn reserve_workspace_discovery_attempt(
        &self,
        key: &SessionDigest,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let allowed = sqlx::query_scalar::<_, bool>(
            "INSERT INTO workspace_discovery_rate_limits (key_digest, window_started_at, attempts) VALUES ($1, clock_timestamp(), 1) ON CONFLICT (key_digest) DO UPDATE SET attempts = CASE WHEN workspace_discovery_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN 1 ELSE workspace_discovery_rate_limits.attempts + 1 END, window_started_at = CASE WHEN workspace_discovery_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN clock_timestamp() ELSE workspace_discovery_rate_limits.window_started_at END RETURNING attempts <= 20",
        ).bind(key.as_ref()).fetch_one(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(allowed)
    }

    pub async fn reserve_password_reset_attempt(
        &self,
        key: &SessionDigest,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let allowed = sqlx::query_scalar::<_, bool>(
            "INSERT INTO password_reset_rate_limits (key_digest, window_started_at, attempts) VALUES ($1, clock_timestamp(), 1) ON CONFLICT (key_digest) DO UPDATE SET attempts = CASE WHEN password_reset_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN 1 ELSE password_reset_rate_limits.attempts + 1 END, window_started_at = CASE WHEN password_reset_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN clock_timestamp() ELSE password_reset_rate_limits.window_started_at END RETURNING attempts <= 5",
        )
        .bind(key.as_ref())
        .fetch_one(&mut *tx)
        .await?;
        self.commit_mutation(tx).await?;
        Ok(allowed)
    }

    pub async fn local_login_credential(
        &self,
        email: &str,
    ) -> Result<Option<LocalLoginCredential>, RepositoryError> {
        let row = sqlx::query(
            "SELECT credential.user_id, credential.password_hash, user_account.state AS user_state, user_account.email_verified_at, user_account.security_version, credential.credential_version FROM local_password_credentials credential JOIN users user_account ON user_account.id = credential.user_id WHERE user_account.email = lower(btrim($1))",
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
                email_verified: row
                    .try_get::<Option<DateTime<Utc>>, _>("email_verified_at")?
                    .is_some(),
                security_version: row.try_get("security_version")?,
                credential_version: row.try_get("credential_version")?,
            })
        })
        .transpose()
        .map_err(RepositoryError::from)
    }

    /// Atomically consumes a password-reset action, changes the password, and
    /// invalidates every session and outstanding lifecycle action for its user.
    pub async fn consume_password_reset(
        &self,
        digest: &ActionTokenDigest,
        password_hash: &PasswordHash,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let token_user_id: Uuid = sqlx::query_scalar(
            "SELECT user_id FROM user_lifecycle_action_tokens WHERE token_digest = $1 AND purpose = 'password_reset'",
        )
        .bind(digest.as_ref())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("lifecycle token"))?;
        let user =
            sqlx::query("SELECT state, security_version FROM users WHERE id = $1 FOR UPDATE")
                .bind(token_user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(RepositoryError::NotFound("lifecycle token"))?;
        let token = sqlx::query("SELECT issued_security_version, issued_credential_version, expires_at, consumed_at, revoked_at FROM user_lifecycle_action_tokens WHERE token_digest = $1 AND purpose = 'password_reset' FOR UPDATE")
            .bind(digest.as_ref())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(RepositoryError::NotFound("lifecycle token"))?;
        let credential_version: i32 = sqlx::query_scalar(
            "SELECT credential_version FROM local_password_credentials WHERE user_id = $1 FOR UPDATE",
        )
        .bind(token_user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("lifecycle token"))?;
        let valid = user.try_get::<String, _>("state")? == "active"
            && token
                .try_get::<Option<DateTime<Utc>>, _>("consumed_at")?
                .is_none()
            && token
                .try_get::<Option<DateTime<Utc>>, _>("revoked_at")?
                .is_none()
            && token.try_get::<DateTime<Utc>, _>("expires_at")? > Utc::now()
            && token.try_get::<i32, _>("issued_security_version")?
                == user.try_get::<i32, _>("security_version")?
            && token.try_get::<i32, _>("issued_credential_version")? == credential_version;
        if !valid {
            return Err(RepositoryError::NotFound("lifecycle token"));
        }
        sqlx::query("UPDATE user_lifecycle_action_tokens SET consumed_at = clock_timestamp() WHERE token_digest = $1")
            .bind(digest.as_ref())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE local_password_credentials SET password_hash = $1, credential_version = credential_version + 1, updated_at = clock_timestamp() WHERE user_id = $2")
            .bind(password_hash.as_phc())
            .bind(token_user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE users SET security_version = security_version + 1, updated_at = clock_timestamp() WHERE id = $1")
            .bind(token_user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE browser_sessions SET revoked_at = clock_timestamp() WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(token_user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE user_lifecycle_action_tokens SET revoked_at = clock_timestamp() WHERE user_id = $1 AND consumed_at IS NULL AND revoked_at IS NULL")
            .bind(token_user_id)
            .execute(&mut *tx)
            .await?;
        self.commit_mutation(tx).await
    }

    pub async fn reserve_login_attempt(
        &self,
        key: &SessionDigest,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let allowed = sqlx::query_scalar::<_, bool>(
            "INSERT INTO browser_login_rate_limits (key_digest, window_started_at, failures) VALUES ($1, clock_timestamp(), 1) ON CONFLICT (key_digest) DO UPDATE SET failures = CASE WHEN browser_login_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN 1 ELSE browser_login_rate_limits.failures + 1 END, window_started_at = CASE WHEN browser_login_rate_limits.window_started_at <= clock_timestamp() - interval '15 minutes' THEN clock_timestamp() ELSE browser_login_rate_limits.window_started_at END RETURNING failures <= 5",
        ).bind(key.as_ref()).fetch_one(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(allowed)
    }

    pub async fn clear_login_failures(&self, key: &SessionDigest) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM browser_login_rate_limits WHERE key_digest = $1")
            .bind(key.as_ref())
            .execute(&mut *tx)
            .await?;
        self.commit_mutation(tx).await?;
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
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("INSERT INTO browser_sessions (id, user_id, workspace_id, issued_security_version, issued_credential_version, session_digest, csrf_digest, expires_at) SELECT $1, $2, $3, $4, $5, $6, $7, $8 WHERE EXISTS (SELECT 1 FROM users u JOIN local_password_credentials c ON c.user_id = u.id JOIN workspace_memberships m ON m.user_id = u.id WHERE u.id = $2 AND u.state = 'active' AND u.security_version = $4 AND c.credential_version = $5 AND m.workspace_id = $3 AND m.state = 'active')")
            .bind(id).bind(credential.user_id).bind(workspace_id).bind(credential.security_version).bind(credential.credential_version).bind(session.as_ref()).bind(csrf.as_ref()).bind(expires_at)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::NotFound("active credential or membership"));
        }
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn validate_browser_session(
        &self,
        session: &SessionDigest,
    ) -> Result<Option<ValidBrowserSession>, RepositoryError> {
        let row = sqlx::query(
            "SELECT s.user_id, s.workspace_id, s.csrf_digest, (w.deleted_at IS NULL) AS workspace_active, w.catalog_generation, w.contexts_generation, w.extensions_generation FROM browser_sessions s JOIN users u ON u.id = s.user_id JOIN local_password_credentials c ON c.user_id = u.id JOIN workspace_memberships m ON m.user_id = u.id AND m.workspace_id = s.workspace_id JOIN workspaces w ON w.id = s.workspace_id WHERE s.session_digest = $1 AND s.revoked_at IS NULL AND s.expires_at > clock_timestamp() AND u.state = 'active' AND u.security_version = s.issued_security_version AND c.credential_version = s.issued_credential_version AND m.state = 'active'",
        )
        .bind(session.as_ref())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok::<ValidBrowserSession, sqlx::Error>(ValidBrowserSession {
                user_id: row.try_get("user_id")?,
                workspace_id: row.try_get("workspace_id")?,
                csrf_digest: SessionDigest::from_slice(&row.try_get::<Vec<u8>, _>("csrf_digest")?)
                    .map_err(|_| sqlx::Error::Protocol("stored csrf digest is invalid".into()))?,
                workspace_active: row.try_get("workspace_active")?,
                generations: super::WorkspaceGenerations {
                    catalog_generation: row.try_get("catalog_generation")?,
                    contexts_generation: row.try_get("contexts_generation")?,
                    extensions_generation: row.try_get("extensions_generation")?,
                },
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
        let mut tx = self.pool.begin().await?;
        // Serialize with membership and role revocation before locking the
        // session. Otherwise a revoker waiting on the old row can miss the
        // replacement inserted after its UPDATE statement took its snapshot.
        sqlx::query(
            "SELECT id FROM workspaces WHERE id = $1 AND deleted_at IS NULL FOR NO KEY UPDATE",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("browser session"))?;
        let previous_row = sqlx::query("SELECT user_id, issued_security_version, issued_credential_version FROM browser_sessions WHERE session_digest = $1 AND workspace_id = $2 AND revoked_at IS NULL AND expires_at > clock_timestamp() FOR UPDATE")
            .bind(previous.as_ref()).bind(workspace_id).fetch_optional(&mut *tx).await?;
        let Some(previous_row) = previous_row else {
            return Err(RepositoryError::NotFound("browser session"));
        };
        let user_id: Uuid = previous_row.try_get("user_id")?;
        sqlx::query("UPDATE browser_sessions SET revoked_at = clock_timestamp(), rotated_at = clock_timestamp() WHERE session_digest = $1")
            .bind(previous.as_ref()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO browser_sessions (id, user_id, workspace_id, issued_security_version, issued_credential_version, session_digest, csrf_digest, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
            .bind(id).bind(user_id).bind(workspace_id).bind(previous_row.try_get::<i32, _>("issued_security_version")?).bind(previous_row.try_get::<i32, _>("issued_credential_version")?).bind(session.as_ref()).bind(csrf.as_ref()).bind(expires_at).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn revoke_browser_session(
        &self,
        session: &SessionDigest,
        workspace_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE browser_sessions SET revoked_at = clock_timestamp() WHERE session_digest = $1 AND workspace_id = $2 AND revoked_at IS NULL")
            .bind(session.as_ref())
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn link_external_identity(
        &self,
        user_id: Uuid,
        issuer: &str,
        subject: &str,
    ) -> Result<(), RepositoryError> {
        if issuer.trim() != issuer
            || issuer.is_empty()
            || subject.trim() != subject
            || subject.is_empty()
        {
            return Err(RepositoryError::NotFound("external identity"));
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO external_identities (issuer, subject, user_id) VALUES ($1, $2, $3)",
        )
        .bind(issuer)
        .bind(subject)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn external_identity_user(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<Uuid>, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT user_id FROM external_identities WHERE issuer = $1 AND subject = $2",
        )
        .bind(issuer)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn issue_lifecycle_token(
        &self,
        id: Uuid,
        user_id: Uuid,
        purpose: &str,
        digest: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        if !matches!(
            purpose,
            "email_verification" | "password_setup" | "password_reset"
        ) || digest.len() != 32
            || expires_at <= Utc::now()
        {
            return Err(RepositoryError::NotFound("lifecycle token"));
        }
        let mut tx = self.pool.begin().await?;
        let user = sqlx::query(
            "SELECT security_version, email_verified_at, state FROM users WHERE id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::NotFound("active user"))?;
        if user.try_get::<String, _>("state")? != "active"
            || (matches!(purpose, "password_setup" | "password_reset")
                && user
                    .try_get::<Option<DateTime<Utc>>, _>("email_verified_at")?
                    .is_none())
        {
            return Err(RepositoryError::NotFound("lifecycle token"));
        }
        let credential_version: i32 = sqlx::query_scalar(
            "SELECT credential_version FROM local_password_credentials WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        if (purpose == "password_setup" && credential_version != 0)
            || (purpose == "password_reset" && credential_version == 0)
        {
            return Err(RepositoryError::NotFound("lifecycle token"));
        }
        Self::revoke_outstanding_lifecycle_tokens(&mut tx, user_id, Some(purpose)).await?;
        sqlx::query("INSERT INTO user_lifecycle_action_tokens (id, user_id, purpose, token_digest, issued_security_version, issued_credential_version, expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7)").bind(id).bind(user_id).bind(purpose).bind(digest).bind(user.try_get::<i32,_>("security_version")?).bind(credential_version).bind(expires_at).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn consume_lifecycle_token(
        &self,
        digest: &[u8],
        purpose: &str,
    ) -> Result<Uuid, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        let user_id: Uuid = sqlx::query_scalar("SELECT user_id FROM user_lifecycle_action_tokens WHERE token_digest = $1 AND purpose = $2 FOR UPDATE").bind(digest).bind(purpose).fetch_optional(&mut *tx).await?.ok_or(RepositoryError::NotFound("lifecycle token"))?;
        let valid: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM user_lifecycle_action_tokens t JOIN users u ON u.id = t.user_id LEFT JOIN local_password_credentials c ON c.user_id = t.user_id WHERE t.token_digest = $1 AND t.purpose = $2 AND t.consumed_at IS NULL AND t.revoked_at IS NULL AND t.expires_at > clock_timestamp() AND u.state = 'active' AND t.issued_security_version = u.security_version AND t.issued_credential_version = coalesce(c.credential_version, 0))").bind(digest).bind(purpose).fetch_one(&mut *tx).await?;
        if !valid {
            return Err(RepositoryError::NotFound("lifecycle token"));
        }
        sqlx::query("UPDATE user_lifecycle_action_tokens SET consumed_at = clock_timestamp() WHERE token_digest = $1 AND consumed_at IS NULL").bind(digest).execute(&mut *tx).await?;
        self.commit_mutation(tx).await?;
        Ok(user_id)
    }

    pub async fn revoke_lifecycle_tokens(
        &self,
        user_id: Uuid,
        purpose: Option<&str>,
    ) -> Result<(), RepositoryError> {
        if purpose.is_some_and(|p| {
            !matches!(
                p,
                "email_verification" | "password_setup" | "password_reset"
            )
        }) {
            return Err(RepositoryError::NotFound("lifecycle purpose"));
        }
        let mut tx = self.pool.begin().await?;
        Self::revoke_outstanding_lifecycle_tokens(&mut tx, user_id, purpose).await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }

    /// Revokes a user's unconsumed lifecycle tokens, of one purpose or all,
    /// in the caller's transaction. Consumed tokens keep their record.
    pub(super) async fn revoke_outstanding_lifecycle_tokens(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        purpose: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE user_lifecycle_action_tokens SET revoked_at = clock_timestamp() WHERE user_id = $1 AND ($2::text IS NULL OR purpose = $2) AND consumed_at IS NULL AND revoked_at IS NULL").bind(user_id).bind(purpose).execute(&mut **tx).await?;
        Ok(())
    }
}
