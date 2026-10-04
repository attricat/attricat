//! Workspace teams and the user-or-team directory that assignment attributes
//! reference (`x-attricat-principal`).
use std::collections::HashSet;

use catalog_validation::principal::{
    CURRENT_USER_FILTER_VALUE, PrincipalKind, PrincipalRef, principal_kinds,
    validate_principal_value,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::entity_commands::Revalidation;
use super::{CatalogRepository, RepositoryError};
use catalog_domain::model::Entity;

pub const MAX_TEAMS: i64 = 1000;
pub const MAX_TEAM_MEMBERS: usize = 1000;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Team {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    /// User IDs of the team's active and inactive members.
    pub member_user_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A workspace member as an assignment target. Inactive members are listed
/// so existing assignments keep their names, but cannot be newly assigned.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DirectoryUser {
    pub id: Uuid,
    pub display_name: Option<String>,
    pub email: String,
    pub active: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DirectoryTeam {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    /// Deleted teams are listed so existing assignments keep their names.
    pub deleted: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceDirectory {
    pub users: Vec<DirectoryUser>,
    pub teams: Vec<DirectoryTeam>,
}

const TEAM_FIELDS: &str = "t.id, t.code, t.name, t.created_at, t.updated_at, COALESCE(array_agg(m.user_id ORDER BY m.user_id) FILTER (WHERE m.user_id IS NOT NULL), '{}') AS member_user_ids";

fn validate_team_input(code: Option<&str>, name: &str) -> Result<(), RepositoryError> {
    if code.is_some_and(|code| !catalog_validation::is_valid_code(code) || code.len() > 128) {
        return Err(RepositoryError::InvalidCode);
    }
    if name.trim().is_empty() || name.chars().count() > 200 {
        return Err(RepositoryError::InvalidTeam(
            "name must contain 1 to 200 characters".into(),
        ));
    }
    Ok(())
}

impl CatalogRepository {
    pub async fn list_teams(&self) -> Result<Vec<Team>, RepositoryError> {
        Ok(sqlx::query_as::<_, Team>(&format!(
            "SELECT {TEAM_FIELDS} FROM teams t
             LEFT JOIN team_members tm ON tm.team_id = t.id
             LEFT JOIN workspace_memberships m ON m.id = tm.membership_id
             WHERE t.workspace_id = $1 AND t.deleted_at IS NULL
             GROUP BY t.id ORDER BY t.name, t.code"
        ))
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn team_on(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Team, RepositoryError> {
        sqlx::query_as::<_, Team>(&format!(
            "SELECT {TEAM_FIELDS} FROM teams t
             LEFT JOIN team_members tm ON tm.team_id = t.id
             LEFT JOIN workspace_memberships m ON m.id = tm.membership_id
             WHERE t.workspace_id = $1 AND t.id = $2 AND t.deleted_at IS NULL
             GROUP BY t.id"
        ))
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(RepositoryError::NotFound("team"))
    }

    pub async fn create_team(
        &self,
        code: &str,
        name: &str,
        member_user_ids: &[Uuid],
    ) -> Result<Team, RepositoryError> {
        validate_team_input(Some(code), name)?;
        let mut transaction = self.pool.begin().await?;
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM teams WHERE workspace_id = $1 AND deleted_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .fetch_one(&mut *transaction)
        .await?;
        if count >= MAX_TEAMS {
            return Err(RepositoryError::InvalidTeam(format!(
                "a workspace can have at most {MAX_TEAMS} teams"
            )));
        }
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO teams (id, workspace_id, code, name) VALUES ($1, $2, $3, btrim($4))",
        )
        .bind(id)
        .bind(self.workspace_id.0)
        .bind(code)
        .bind(name)
        .execute(&mut *transaction)
        .await?;
        self.replace_team_members(&mut transaction, id, member_user_ids)
            .await?;
        let team = Self::team_on(&mut transaction, self.workspace_id.0, id).await?;
        self.commit_mutation(transaction).await?;
        Ok(team)
    }

    /// Renames a team and/or replaces its members. The code is immutable
    /// because assignments store the team ID, and integrations may key on it.
    pub async fn update_team(
        &self,
        id: Uuid,
        name: Option<&str>,
        member_user_ids: Option<&[Uuid]>,
    ) -> Result<Team, RepositoryError> {
        if let Some(name) = name {
            validate_team_input(None, name)?;
        }
        let mut transaction = self.pool.begin().await?;
        // The row lock waits for a concurrent delete, so members are never
        // written to a deleted team.
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM teams WHERE workspace_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(self.workspace_id.0)
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("team"))?;
        if let Some(member_user_ids) = member_user_ids {
            self.replace_team_members(&mut transaction, id, member_user_ids)
                .await?;
        }
        if name.is_some() || member_user_ids.is_some() {
            sqlx::query("UPDATE teams SET name = COALESCE(btrim($3), name), updated_at = now() WHERE workspace_id = $1 AND id = $2")
                .bind(self.workspace_id.0)
                .bind(id)
                .bind(name)
                .execute(&mut *transaction)
                .await?;
        }
        let team = Self::team_on(&mut transaction, self.workspace_id.0, id).await?;
        self.commit_mutation(transaction).await?;
        Ok(team)
    }

    /// Deletes a team and its memberships. Existing assignments keep the ID
    /// and display the deleted team's name; new assignments are rejected.
    pub async fn delete_team(&self, id: Uuid) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = sqlx::query(
            "UPDATE teams SET deleted_at = now(), updated_at = now() WHERE workspace_id = $1 AND id = $2 AND deleted_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .bind(id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        if deleted == 0 {
            return Err(RepositoryError::NotFound("team"));
        }
        sqlx::query("DELETE FROM team_members WHERE workspace_id = $1 AND team_id = $2")
            .bind(self.workspace_id.0)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        self.commit_mutation(transaction).await
    }

    async fn replace_team_members(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        team_id: Uuid,
        user_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let user_ids: Vec<Uuid> = user_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if user_ids.len() > MAX_TEAM_MEMBERS {
            return Err(RepositoryError::InvalidTeam(format!(
                "a team can have at most {MAX_TEAM_MEMBERS} members"
            )));
        }
        let memberships: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM workspace_memberships WHERE workspace_id = $1 AND user_id = ANY($2)",
        )
        .bind(self.workspace_id.0)
        .bind(&user_ids)
        .fetch_all(&mut **transaction)
        .await?;
        if memberships.len() != user_ids.len() {
            return Err(RepositoryError::InvalidTeam(
                "every team member must be a workspace member".into(),
            ));
        }
        sqlx::query("DELETE FROM team_members WHERE workspace_id = $1 AND team_id = $2")
            .bind(self.workspace_id.0)
            .bind(team_id)
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO team_members (workspace_id, team_id, membership_id) SELECT $1, $2, unnest($3::uuid[])",
        )
        .bind(self.workspace_id.0)
        .bind(team_id)
        .bind(&memberships)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Everyone and every team an assignment can show, for pickers and
    /// renderers. Readable by every catalog reader.
    pub async fn workspace_directory(&self) -> Result<WorkspaceDirectory, RepositoryError> {
        let users = sqlx::query_as::<_, DirectoryUser>(
            "SELECT u.id, u.display_name, u.email, (m.state = 'active' AND u.state = 'active') AS active
             FROM workspace_memberships m JOIN users u ON u.id = m.user_id
             WHERE m.workspace_id = $1
             ORDER BY lower(COALESCE(u.display_name, u.email)), u.id",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        let teams = sqlx::query_as::<_, DirectoryTeam>(
            "SELECT id, code, name, deleted_at IS NOT NULL AS deleted FROM teams
             WHERE workspace_id = $1 ORDER BY deleted_at IS NOT NULL, lower(name), code",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?;
        Ok(WorkspaceDirectory { users, teams })
    }

    /// The non-deleted teams `user_id` belongs to through an active membership
    /// in this workspace.
    pub async fn principal_team_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(sqlx::query_scalar(
            "SELECT t.id FROM teams t
             JOIN team_members tm ON tm.team_id = t.id
             JOIN workspace_memberships m ON m.id = tm.membership_id
             WHERE t.workspace_id = $1 AND t.deleted_at IS NULL AND m.user_id = $2 AND m.state = 'active'
             ORDER BY t.id",
        )
        .bind(self.workspace_id.0)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// The stored references that mean "assigned to `user_id`": the user and
    /// each of the user's teams.
    pub async fn principal_references(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<String>, RepositoryError> {
        Ok(std::iter::once(PrincipalRef::user(user_id))
            .chain(
                self.principal_team_ids(user_id)
                    .await?
                    .into_iter()
                    .map(PrincipalRef::team),
            )
            .map(|reference| reference.to_string())
            .collect())
    }

    /// Expands an "assigned to me" filter (`eq` with `@me` on an assignment
    /// attribute) to the value of a [`super::SEARCH_FILTER_EQ_ANY`] filter
    /// matching `caller` and the caller's teams. `None` for other filters.
    pub async fn current_user_filter_value(
        &self,
        value_schema: Option<&Value>,
        operator: &str,
        value: &Value,
        caller: Uuid,
    ) -> Result<Option<String>, RepositoryError> {
        if operator != "eq"
            || value.as_str() != Some(CURRENT_USER_FILTER_VALUE)
            || value_schema.and_then(principal_kinds).is_none()
        {
            return Ok(None);
        }
        Ok(Some(
            Value::from(self.principal_references(caller).await?).to_string(),
        ))
    }

    /// Run on the final transaction state. Every assignment value that
    /// differs from the saved projection must be a valid reference and, on
    /// writes, name an active member or an existing team. Unchanged values
    /// may refer to people who have since left, so unrelated edits never
    /// fail. A structural revalidation only checks the format: a value that
    /// a context reparent newly inherits was assigned earlier, and the
    /// reparent assigns nobody.
    pub(super) async fn validate_principal_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        write: &super::write_context::WriteContext,
        mode: Revalidation,
    ) -> Result<(), RepositoryError> {
        let attributes = write.principal_attributes();
        if attributes.is_empty() {
            return Ok(());
        }
        let after = write.preview(transaction, entity.id).await?;
        let before = entity.projections.get("preview").unwrap_or(&Value::Null);
        let Some(contexts) = after.as_object() else {
            return Ok(());
        };
        let mismatch = |code: &str, context: &str, message: String| {
            RepositoryError::AttributeValueSchemaMismatch {
                attribute: code.to_owned(),
                instance_path: String::new(),
                message: format!("{message} (context: {context})"),
            }
        };
        let mut changed = Vec::new();
        for (code, schema) in &attributes {
            for (context, values) in contexts {
                let Some(value) = values.get(code).filter(|value| !value.is_null()) else {
                    continue;
                };
                if before.get(context).and_then(|values| values.get(code)) == Some(value) {
                    continue;
                }
                validate_principal_value(schema, value)
                    .map_err(|message| mismatch(code, context, message))?;
                let reference = value
                    .as_str()
                    .and_then(PrincipalRef::parse)
                    .ok_or_else(|| mismatch(code, context, "invalid principal reference".into()))?;
                changed.push((code, context, reference));
            }
        }
        if mode == Revalidation::Structural {
            return Ok(());
        }
        let ids = |kind: PrincipalKind| -> Vec<Uuid> {
            changed
                .iter()
                .filter(|(_, _, reference)| reference.kind == kind)
                .map(|(_, _, reference)| reference.id)
                .collect()
        };
        let (user_ids, team_ids) = (ids(PrincipalKind::User), ids(PrincipalKind::Team));
        let active_users: HashSet<Uuid> = if user_ids.is_empty() {
            HashSet::new()
        } else {
            sqlx::query_scalar(
                "SELECT m.user_id FROM workspace_memberships m JOIN users u ON u.id = m.user_id WHERE m.workspace_id = $1 AND m.user_id = ANY($2) AND m.state = 'active' AND u.state = 'active'",
            )
            .bind(self.workspace_id.0)
            .bind(&user_ids)
            .fetch_all(&mut **transaction)
            .await?
            .into_iter()
            .collect()
        };
        let live_teams: HashSet<Uuid> = if team_ids.is_empty() {
            HashSet::new()
        } else {
            sqlx::query_scalar(
                "SELECT id FROM teams WHERE workspace_id = $1 AND id = ANY($2) AND deleted_at IS NULL",
            )
            .bind(self.workspace_id.0)
            .bind(&team_ids)
            .fetch_all(&mut **transaction)
            .await?
            .into_iter()
            .collect()
        };
        for (code, context, reference) in changed {
            let missing = match reference.kind {
                PrincipalKind::User => (!active_users.contains(&reference.id))
                    .then_some("user is not an active workspace member"),
                PrincipalKind::Team => (!live_teams.contains(&reference.id))
                    .then_some("team does not exist in this workspace"),
            };
            if let Some(message) = missing {
                return Err(mismatch(code, context, message.into()));
            }
        }
        Ok(())
    }
}
