//! Each workspace's system user: the author of automated notes, such as the
//! comment left on a record a bulk edit failed to change. It has no
//! membership, credentials or tokens, so it can never sign in or act; it is
//! only ever named as an author.

use uuid::Uuid;

use super::{AttricatRepository, RepositoryError};

/// The system user's display name in every workspace.
pub const SYSTEM_USER_DISPLAY_NAME: &str = "Attricat";

/// The system user's address: unique per workspace and, under the reserved
/// `.invalid` top-level domain, never deliverable.
pub fn system_user_email(workspace_id: Uuid) -> String {
    format!("system+{workspace_id}@attricat.invalid")
}

/// Whether an address belongs to the domain reserved for system users, which
/// no person may be created or invited with.
pub(super) fn is_reserved_email(email: &str) -> bool {
    email.ends_with("@attricat.invalid")
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Returns the workspace's system user, creating it on first use.
    /// [`Self::initialize_workspace`] calls it for every workspace at startup,
    /// which also backfills workspaces created before system users existed.
    /// Safe to run repeatedly and concurrently: the workspace row is locked
    /// while the user is created and linked.
    pub async fn ensure_workspace_system_user(
        &self,
        workspace_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let existing: Option<Option<Uuid>> =
            sqlx::query_scalar("SELECT system_user_id FROM workspaces WHERE id = $1 FOR UPDATE")
                .bind(workspace_id)
                .fetch_optional(&mut *transaction)
                .await?;
        let Some(existing) = existing else {
            return Err(RepositoryError::NotFound("workspace"));
        };
        if let Some(user_id) = existing {
            transaction.commit().await?;
            return Ok(user_id);
        }
        let email = system_user_email(workspace_id);
        sqlx::query(
            "INSERT INTO users (id, email, display_name, kind) VALUES ($1, $2, $3, 'system') ON CONFLICT (email) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(&email)
        .bind(SYSTEM_USER_DISPLAY_NAME)
        .execute(&mut *transaction)
        .await?;
        // A person can never hold the reserved address (see
        // [`is_reserved_email`]), so an existing row is an earlier system
        // user that was not linked; a person's row is never adopted.
        let user_id: Uuid =
            sqlx::query_scalar("SELECT id FROM users WHERE email = $1 AND kind = 'system'")
                .bind(&email)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or(RepositoryError::NotFound("system user"))?;
        sqlx::query("UPDATE workspaces SET system_user_id = $2, updated_at = now() WHERE id = $1")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_user_addresses_are_per_workspace_and_undeliverable() {
        let workspace = Uuid::from_u128(7);
        let email = system_user_email(workspace);
        assert_eq!(
            email,
            "system+00000000-0000-0000-0000-000000000007@attricat.invalid"
        );
        assert_eq!(email, email.trim().to_lowercase());
        assert!(is_reserved_email(&email));
        assert!(!is_reserved_email("person@example.test"));
    }
}
