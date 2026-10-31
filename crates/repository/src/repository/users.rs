//! The authenticated user's own account fields.
use super::*;

#[derive(Debug, sqlx::FromRow)]
pub struct UserAccount {
    pub display_name: Option<String>,
    pub email: String,
    /// IANA time zone name; `None` follows the client's own zone.
    pub time_zone: Option<String>,
}

impl<S: RepositoryScope> AttricatRepository<S> {
    /// Returns only the authenticated principal's account fields. Callers must
    /// derive `user_id` from authentication rather than accepting it from a request.
    pub async fn user_account(&self, user_id: Uuid) -> Result<UserAccount, RepositoryError> {
        sqlx::query_as("SELECT display_name, email, time_zone FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::from)
    }

    /// Whether the user is active, and the workspace's cache generations, in
    /// one query.
    pub async fn active_user_generations(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(bool, Option<WorkspaceGenerations>), RepositoryError> {
        #[allow(clippy::type_complexity)]
        let (active, attricat, contexts, extensions, access): (bool, Option<i64>, Option<i64>, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND state = 'active'), w.attricat_generation, w.contexts_generation, w.extensions_generation, w.access_generation FROM (SELECT 1) one LEFT JOIN workspaces w ON w.id = $2 AND w.deleted_at IS NULL",
            )
            .bind(user_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        let generations =
            WorkspaceGenerations::from_columns(attricat, contexts, extensions, access);
        Ok((active, generations))
    }

    pub async fn is_active_principal(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL)",
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?)
    }
}

impl AttricatRepository {
    /// Replaces the authenticated principal's display preferences. Callers
    /// validate the zone name; `None` clears the preference.
    pub async fn update_user_preferences(
        &self,
        user_id: Uuid,
        time_zone: Option<&str>,
    ) -> Result<(), RepositoryError> {
        self.update_active_user(user_id, "time_zone", time_zone)
            .await
    }

    /// Replaces the authenticated principal's display name. Callers validate
    /// the name before storing it.
    pub async fn update_user_display_name(
        &self,
        user_id: Uuid,
        display_name: &str,
    ) -> Result<(), RepositoryError> {
        self.update_active_user(user_id, "display_name", Some(display_name))
            .await
    }

    /// Removes a user's display name, so they are shown by their email.
    pub async fn clear_user_display_name(&self, user_id: Uuid) -> Result<(), RepositoryError> {
        self.update_active_user(user_id, "display_name", None).await
    }

    /// Sets one text `users` column for an active user, in an audited
    /// mutation.
    async fn update_active_user(
        &self,
        user_id: Uuid,
        column: &'static str,
        value: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let sql = format!(
            "UPDATE users SET {column} = $2, updated_at = clock_timestamp() WHERE id = $1 AND state = 'active'"
        );
        let updated = sqlx::query(&sql)
            .bind(user_id)
            .bind(value)
            .execute(&mut *transaction)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("user"));
        }
        self.commit_mutation(transaction).await
    }
}
