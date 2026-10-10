use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

#[derive(Debug, serde::Serialize, FromRow)]
pub struct WorkspaceRole {
    pub id: Uuid,
    pub code: String,
    pub is_system: bool,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize, FromRow)]
pub struct Permission {
    pub code: String,
    pub description: String,
}

#[derive(Debug, serde::Serialize, FromRow)]
pub struct WorkspaceGrantTarget {
    pub id: Uuid,
    pub label: String,
}

const OWNER_ROLE_ID: Uuid = Uuid::from_u128(0x00000000000040008000000000000101);

impl<S: super::RepositoryScope> CatalogRepository<S> {
    async fn require_permission(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
    ) -> Result<(), RepositoryError> {
        if self
            .is_authorized(actor_id, workspace_id, permission, None, None)
            .await?
        {
            Ok(())
        } else {
            Err(RepositoryError::NotFound("permission"))
        }
    }

    /// Serialize authority changes before taking role, membership or credential
    /// locks, then authorize against the committed state after any lock wait.
    /// NO KEY UPDATE preserves this ordering without blocking unrelated FK checks.
    pub(super) async fn lock_workspace_permission_on(
        connection: &mut sqlx::PgConnection,
        actor_id: Uuid,
        workspace_id: Uuid,
        permission: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(workspace_id)
            .execute(&mut *connection)
            .await?;
        if !Self::is_authorized_on(connection, actor_id, workspace_id, permission, None, None)
            .await?
        {
            return Err(RepositoryError::NotFound("permission"));
        }
        Ok(())
    }

    async fn validate_role_input_on(
        connection: &mut sqlx::PgConnection,
        actor_id: Uuid,
        workspace_id: Uuid,
        code: &str,
        permissions: &[String],
    ) -> Result<(), RepositoryError> {
        if !code.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            || !code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(RepositoryError::InvalidCode);
        }
        let known: i64 =
            sqlx::query_scalar("SELECT count(*) FROM permissions WHERE code = ANY($1)")
                .bind(permissions)
                .fetch_one(&mut *connection)
                .await?;
        if known != permissions.len() as i64 {
            return Err(RepositoryError::NotFound("permission"));
        }
        // An actor may only delegate permissions it holds workspace-wide;
        // one query covers every requested permission.
        let requested: Vec<&str> = permissions.iter().map(String::as_str).collect();
        let held =
            Self::workspace_permissions_on(connection, actor_id, workspace_id, &requested).await?;
        if requested
            .iter()
            .any(|permission| !held.contains(*permission))
        {
            return Err(RepositoryError::NotFound("permission"));
        }
        Ok(())
    }

    pub(super) async fn roles_delegable_on(
        connection: &mut sqlx::PgConnection,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM role_permissions requested WHERE requested.role_id = $3 AND NOT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id=m.user_id JOIN workspaces w ON w.id=m.workspace_id JOIN role_grants g ON g.membership_id=m.id AND g.workspace_id=m.workspace_id JOIN role_permissions granted ON granted.role_id=g.role_id WHERE m.user_id=$1 AND m.workspace_id=$2 AND m.state='active' AND u.state='active' AND w.deleted_at IS NULL AND g.scope_type='workspace' AND g.scope_target_id=$2 AND granted.permission_code=requested.permission_code))")
            .bind(actor_id).bind(workspace_id).bind(role_id).fetch_one(connection).await?)
    }

    async fn workspace_role(
        &self,
        workspace_id: Uuid,
        role_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1 AND (is_system OR workspace_id = $2))").bind(role_id).bind(workspace_id).fetch_one(&self.pool).await?)
    }

    pub async fn list_workspace_roles(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceRole>, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "roles.manage")
            .await?;
        Ok(sqlx::query_as("SELECT r.id, r.code, r.is_system, coalesce(array_agg(rp.permission_code ORDER BY rp.permission_code) FILTER (WHERE rp.permission_code IS NOT NULL), ARRAY[]::text[]) AS permissions, r.created_at FROM roles r LEFT JOIN role_permissions rp ON rp.role_id = r.id WHERE r.is_system OR r.workspace_id = $1 GROUP BY r.id ORDER BY r.is_system DESC, r.code").bind(workspace_id).fetch_all(&self.pool).await?)
    }

    pub async fn list_workspace_assignable_roles(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceRole>, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "members.manage")
            .await?;
        Ok(sqlx::query_as("SELECT r.id, r.code, r.is_system, coalesce(array_agg(rp.permission_code ORDER BY rp.permission_code) FILTER (WHERE rp.permission_code IS NOT NULL), ARRAY[]::text[]) AS permissions, r.created_at FROM roles r LEFT JOIN role_permissions rp ON rp.role_id = r.id WHERE r.is_system OR r.workspace_id = $1 GROUP BY r.id ORDER BY r.is_system DESC, r.code").bind(workspace_id).fetch_all(&self.pool).await?)
    }

    pub async fn list_workspace_permissions(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<Permission>, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "roles.manage")
            .await?;
        Ok(
            sqlx::query_as("SELECT code, description FROM permissions ORDER BY code")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn list_workspace_token_permissions(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<Permission>, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "tokens.manage")
            .await?;
        Ok(sqlx::query_as("SELECT p.code, p.description FROM permissions p WHERE EXISTS (SELECT 1 FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN role_permissions rp ON rp.role_id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND rp.permission_code = p.code) ORDER BY p.code").bind(actor_id).bind(workspace_id).fetch_all(&self.pool).await?)
    }

    pub async fn list_workspace_grant_targets(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        scope_type: &str,
    ) -> Result<Vec<WorkspaceGrantTarget>, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "members.manage")
            .await?;
        let query = match scope_type {
            "workspace" => "SELECT id, name AS label FROM workspaces WHERE id = $1",
            "blueprint_family" => {
                "SELECT id, code AS label FROM blueprints WHERE workspace_id = $1 ORDER BY code"
            }
            "context_subtree" => {
                "SELECT id, code AS label FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code"
            }
            "record" => {
                "SELECT id, id::text AS label FROM records WHERE workspace_id = $1 ORDER BY id"
            }
            _ => return Err(RepositoryError::NotFound("grant scope")),
        };
        Ok(sqlx::query_as(query)
            .bind(workspace_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn create_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        code: &str,
        permissions: &[String],
    ) -> Result<Uuid, RepositoryError> {
        self.require_permission(actor_id, workspace_id, "roles.manage")
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        Self::lock_workspace_permission_on(&mut tx, actor_id, workspace_id, "roles.manage").await?;
        Self::validate_role_input_on(&mut tx, actor_id, workspace_id, code, permissions).await?;
        self.ensure_token_can_delegate_on(&mut tx, actor_id, workspace_id, permissions)
            .await?;
        sqlx::query(
            "INSERT INTO roles (id, code, workspace_id, is_system) VALUES ($1, $2, $3, false)",
        )
        .bind(id)
        .bind(code)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
        for permission in permissions {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, $2)")
                .bind(id)
                .bind(permission)
                .execute(&mut *tx)
                .await?;
        }
        self.commit_mutation(tx).await?;
        Ok(id)
    }

    pub async fn update_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
        code: &str,
        permissions: &[String],
    ) -> Result<(), RepositoryError> {
        self.require_permission(actor_id, workspace_id, "roles.manage")
            .await?;
        let mut tx = self.pool.begin().await?;
        Self::lock_workspace_permission_on(&mut tx, actor_id, workspace_id, "roles.manage").await?;
        Self::validate_role_input_on(&mut tx, actor_id, workspace_id, code, permissions).await?;
        self.ensure_token_can_delegate_on(&mut tx, actor_id, workspace_id, permissions)
            .await?;
        if sqlx::query(
            "UPDATE roles SET code = $1 WHERE id = $2 AND workspace_id = $3 AND NOT is_system",
        )
        .bind(code)
        .bind(role_id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            != 1
        {
            return Err(RepositoryError::NotFound("workspace role"));
        }
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        for permission in permissions {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, $2)")
                .bind(role_id)
                .bind(permission)
                .execute(&mut *tx)
                .await?;
        }
        self.commit_mutation(tx).await?;
        Ok(())
    }

    pub async fn duplicate_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        source_role_id: Uuid,
        code: &str,
    ) -> Result<Uuid, RepositoryError> {
        if !self.workspace_role(workspace_id, source_role_id).await? {
            return Err(RepositoryError::NotFound("source role"));
        }
        let permissions: Vec<String> = sqlx::query_scalar("SELECT permission_code FROM role_permissions WHERE role_id = $1 ORDER BY permission_code").bind(source_role_id).fetch_all(&self.pool).await?;
        self.create_workspace_role(actor_id, workspace_id, code, &permissions)
            .await
    }

    pub async fn retire_workspace_role(
        &self,
        actor_id: Uuid,
        workspace_id: Uuid,
        role_id: Uuid,
        replacement_role_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        self.require_permission(actor_id, workspace_id, "roles.manage")
            .await?;
        if role_id == OWNER_ROLE_ID || replacement_role_id == Some(role_id) {
            return Err(RepositoryError::NotFound("workspace-local role"));
        }
        let mut tx = self.pool.begin().await?;
        Self::lock_workspace_permission_on(&mut tx, actor_id, workspace_id, "roles.manage").await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1 AND workspace_id = $2 AND NOT is_system FOR UPDATE)").bind(role_id).bind(workspace_id).fetch_one(&mut *tx).await?;
        if !exists {
            return Err(RepositoryError::NotFound("workspace role"));
        }
        let grants: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM role_grants WHERE role_id = $1 AND workspace_id = $2",
        )
        .bind(role_id)
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        let Some(replacement) = replacement_role_id else {
            if grants > 0 {
                return Err(RepositoryError::NotFound("replacement role"));
            };
            sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM roles WHERE id = $1")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
            self.commit_mutation(tx).await?;
            return Ok(());
        };
        self.ensure_token_can_delegate_role_on(&mut tx, actor_id, workspace_id, replacement)
            .await?;
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM roles WHERE id=$1 AND (is_system OR workspace_id=$2))",
        )
        .bind(replacement)
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        if !valid || !Self::roles_delegable_on(&mut tx, actor_id, workspace_id, replacement).await?
        {
            return Err(RepositoryError::NotFound("replacement role"));
        }
        // Reassigned grants become owner grants only under the rules for
        // granting the owner role directly: an active owner grants it, and
        // only at workspace scope.
        if replacement == OWNER_ROLE_ID {
            let narrower_scope: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM role_grants WHERE role_id = $1 AND workspace_id = $2 AND (scope_type <> 'workspace' OR scope_target_id <> $2))")
                .bind(role_id)
                .bind(workspace_id)
                .fetch_one(&mut *tx)
                .await?;
            if narrower_scope || !Self::active_owner_on(&mut tx, actor_id, workspace_id).await? {
                return Err(RepositoryError::NotFound("replacement role"));
            }
        }
        sqlx::query("DELETE FROM role_grants old USING role_grants replacement WHERE old.role_id = $1 AND replacement.role_id = $2 AND replacement.workspace_id = old.workspace_id AND replacement.membership_id = old.membership_id AND replacement.scope_type = old.scope_type AND replacement.scope_target_id = old.scope_target_id").bind(role_id).bind(replacement).execute(&mut *tx).await?;
        sqlx::query("UPDATE role_grants SET role_id = $1 WHERE role_id = $2 AND workspace_id = $3")
            .bind(replacement)
            .bind(role_id)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM roles WHERE id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        self.commit_mutation(tx).await?;
        Ok(())
    }
}
