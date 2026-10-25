//! Whether a principal may act: live role grants, scoped targets and personal
//! API token permissions.
use super::*;

impl<S: RepositoryScope> AttricatRepository<S> {
    /// Checks the durable membership/grant graph in one database operation so
    /// callers cannot learn whether an out-of-scope target exists.
    /// Returns which of `permissions` the user holds workspace-wide, in one
    /// query. Equivalent to calling [`Self::is_authorized`] with no target for
    /// each permission: only active, workspace-scoped grants count.
    pub async fn workspace_permissions(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permissions: &[Permission],
    ) -> Result<HashSet<String>, RepositoryError> {
        let codes: Vec<&str> = permissions
            .iter()
            .map(|permission| permission.as_str())
            .collect();
        let mut connection = self.pool.acquire().await?;
        Self::workspace_permissions_on(&mut connection, user_id, workspace_id, Some(&codes)).await
    }

    /// Every permission the user holds workspace-wide at `generations`, from
    /// the process-local cache. The key carries the access generation the
    /// request's credential query read, so a revoked grant, deactivated
    /// membership or edited role is never served afterwards.
    pub async fn cached_workspace_permissions(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        generations: WorkspaceGenerations,
    ) -> Result<Arc<HashSet<String>>, RepositoryError> {
        self.cache
            .fetch(
                CacheKey::new(
                    "workspace_permissions",
                    &[&workspace_id, &user_id, &generations.access_generation],
                ),
                &[],
                // System role permissions are bootstrapped at startup without
                // advancing any generation, so entries never leave the process.
                Policy::LocalGeneration,
                || async {
                    let mut connection = self.pool.acquire().await?;
                    Self::workspace_permissions_on(&mut connection, user_id, workspace_id, None)
                        .await
                },
            )
            .await
    }

    /// [`Self::is_authorized`] that first consults the user's cached
    /// workspace-wide permissions; a workspace-wide grant authorizes every
    /// target. Anything else, including a denial, is decided by the query.
    /// `generations` must have been read for `workspace_id` in this request.
    pub async fn is_authorized_at(
        &self,
        generations: Option<WorkspaceGenerations>,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        if let Some(generations) = generations
            && self
                .cached_workspace_permissions(user_id, workspace_id, generations)
                .await?
                .contains(permission.as_str())
        {
            return Ok(true);
        }
        self.is_authorized(user_id, workspace_id, permission, target_id, target_code)
            .await
    }

    /// `permissions` limits the result; `None` returns every held permission.
    pub(super) async fn workspace_permissions_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permissions: Option<&[&str]>,
    ) -> Result<HashSet<String>, RepositoryError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT rp.permission_code FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN role_permissions rp ON rp.role_id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL AND g.scope_type = 'workspace' AND g.scope_target_id = $2 AND ($3::text[] IS NULL OR rp.permission_code = ANY($3))",
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(permissions)
        .fetch_all(connection)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `record_ids` one principal may access with `permission`:
    /// the user's grants and, for a token, its live token permission.
    pub(crate) async fn principal_record_ids_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: Permission,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty()
            || !Self::token_permits_on(connection, actor, workspace_id, permission).await?
        {
            return Ok(HashSet::new());
        }
        Self::authorized_record_ids_on(
            connection,
            actor.user_id,
            workspace_id,
            permission,
            record_ids,
        )
        .await
    }

    /// Whether one principal may act: the user's current grant for the target
    /// and, for a personal API token, its live token permission.
    pub async fn principal_may(
        &self,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: Permission,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::principal_may_on(
            &mut connection,
            actor,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await
    }

    pub(crate) async fn principal_may_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: Permission,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        Ok(Self::is_authorized_on(
            connection,
            actor.user_id,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await?
            && Self::token_permits_on(connection, actor, workspace_id, permission).await?)
    }

    /// Whether a principal's personal API token (if any) still carries
    /// `permission`. A session always passes.
    pub async fn principal_token_permits(
        &self,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: Permission,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::token_permits_on(&mut connection, actor, workspace_id, permission).await
    }

    /// A live token's permission; a session (no token) always passes.
    async fn token_permits_on(
        connection: &mut sqlx::PgConnection,
        actor: AuthorizationActor,
        workspace_id: Uuid,
        permission: Permission,
    ) -> Result<bool, RepositoryError> {
        let Some(token_id) = actor.token_id else {
            return Ok(true);
        };
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM personal_api_tokens t JOIN personal_api_token_permissions p ON p.token_id = t.id WHERE t.id = $1 AND t.user_id = $2 AND t.workspace_id = $3 AND t.revoked_at IS NULL AND (t.expires_at IS NULL OR t.expires_at > clock_timestamp()) AND p.permission_code = $4)",
        )
        .bind(token_id)
        .bind(actor.user_id)
        .bind(workspace_id)
        .bind(permission.as_str())
        .fetch_one(&mut *connection)
        .await?)
    }

    /// Returns the subset of `record_ids` the user may access with
    /// `permission`, in one query. Equivalent to calling
    /// [`Self::is_authorized`] with each ID as the target: a workspace grant
    /// authorizes every ID, while record and blueprint-family grants require
    /// the record to exist in this workspace.
    pub async fn authorized_record_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut connection = self.pool.acquire().await?;
        Self::authorized_record_ids_on(
            &mut connection,
            user_id,
            workspace_id,
            permission,
            record_ids,
        )
        .await
    }

    pub(super) async fn authorized_record_ids_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        record_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if record_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let sql = format!(
            "WITH {} SELECT requested.id FROM unnest($4::uuid[]) AS requested(id) LEFT JOIN records e ON e.id = requested.id AND e.workspace_id = $2 WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (e.id IS NOT NULL AND ((g.scope_type = 'record' AND g.scope_target_id = e.id) OR (g.scope_type = 'blueprint_family' AND g.scope_target_id = e.blueprint_id))))",
            GrantCodes::Permission(permission).grant_set_sql()
        );
        let rows: Vec<Uuid> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(GrantCodes::Permission(permission).codes())
            .bind(record_ids)
            .fetch_all(connection)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `target_codes` the user may access with `permission`,
    /// in one query. Equivalent to calling [`Self::is_authorized`] with each
    /// code as the target code and no target ID.
    pub async fn authorized_target_codes(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        target_codes: &[String],
    ) -> Result<HashSet<String>, RepositoryError> {
        if target_codes.is_empty() {
            return Ok(HashSet::new());
        }
        let codes = GrantCodes::Permission(permission);
        let sql = format!(
            "WITH RECURSIVE {}, requested AS (SELECT DISTINCT code FROM unnest($4::text[]) AS requested(code)), ancestors AS (SELECT r.code, c.id, c.parent_id FROM requested r JOIN attribute_contexts c ON c.workspace_id = $2 AND c.code = r.code UNION ALL SELECT a.code, p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT r.code FROM requested r WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM blueprints b WHERE b.workspace_id = $2 AND b.code = r.code AND b.id = g.scope_target_id)) OR (g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors a WHERE a.code = r.code AND a.id = g.scope_target_id)) OR (r.code = '__context_list__' AND g.scope_type = 'context_subtree'))",
            codes.grant_set_sql()
        );
        let rows: Vec<String> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_codes)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// The subset of `target_ids` of one kind the user may access with
    /// `permission`, in one query. Equivalent to calling
    /// [`Self::is_authorized`] with each ID as the target: a workspace grant
    /// covers every ID, while scoped grants require the target to exist in
    /// this workspace.
    pub async fn authorized_target_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        kind: AuthorizationTargetKind,
        target_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, RepositoryError> {
        if target_ids.is_empty() {
            return Ok(HashSet::new());
        }
        let codes = GrantCodes::Permission(permission);
        let scoped = match kind {
            AuthorizationTargetKind::Record => {
                return self
                    .authorized_record_ids(user_id, workspace_id, permission, target_ids)
                    .await;
            }
            AuthorizationTargetKind::Blueprint => {
                "g.scope_type = 'blueprint_family' AND g.scope_target_id = r.id AND EXISTS (SELECT 1 FROM blueprints b WHERE b.workspace_id = $2 AND b.id = r.id)"
            }
            AuthorizationTargetKind::Context => {
                "g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors a WHERE a.requested_id = r.id AND a.id = g.scope_target_id)"
            }
        };
        let sql = format!(
            "WITH RECURSIVE {}, requested AS (SELECT DISTINCT id FROM unnest($4::uuid[]) AS requested(id)), ancestors AS (SELECT r.id AS requested_id, c.id, c.parent_id FROM requested r JOIN attribute_contexts c ON c.workspace_id = $2 AND c.id = r.id UNION ALL SELECT a.requested_id, p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT r.id FROM requested r WHERE EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR ({scoped}))",
            codes.grant_set_sql()
        );
        let rows: Vec<Uuid> = sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn is_authorized(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let mut connection = self.pool.acquire().await?;
        Self::is_authorized_on(
            &mut connection,
            user_id,
            workspace_id,
            permission,
            target_id,
            target_code,
        )
        .await
    }

    /// Transaction-owning callers must reuse their connection rather than
    /// acquiring a second pool slot while holding the first.
    pub(super) async fn is_authorized_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        permission: Permission,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        Self::is_granted_on(
            connection,
            user_id,
            workspace_id,
            GrantCodes::Permission(permission),
            target_id,
            target_code,
        )
        .await
    }

    /// Whether the user holds a matching grant whose scope covers the
    /// target: the workspace, a blueprint family, a record (directly or
    /// through its blueprint family) or a context subtree (through the
    /// context's ancestors).
    pub(crate) async fn is_granted_on(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        workspace_id: Uuid,
        codes: GrantCodes<'_>,
        target_id: Option<Uuid>,
        target_code: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        // Resolve grant scope in the application-owned repository query. A
        // non-workspace grant can only authorize the requested tenant target.
        let sql = format!(
            "WITH RECURSIVE {}, target AS (SELECT 'blueprint'::text kind, id FROM blueprints WHERE workspace_id = $2 AND (id = $4 OR code = $5) UNION ALL SELECT 'record', id FROM records WHERE workspace_id = $2 AND id = $4 UNION ALL SELECT 'context', id FROM attribute_contexts WHERE workspace_id = $2 AND (id = $4 OR code = $5)), ancestors AS (SELECT c.id, c.parent_id FROM attribute_contexts c JOIN target t ON t.kind = 'context' AND t.id = c.id UNION ALL SELECT p.id, p.parent_id FROM attribute_contexts p JOIN ancestors a ON a.parent_id = p.id WHERE p.workspace_id = $2) SELECT EXISTS (SELECT 1 FROM grants g WHERE (g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (($4 IS NOT NULL OR $5 IS NOT NULL) AND ((g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM target WHERE kind = 'blueprint' AND id = g.scope_target_id)) OR (g.scope_type = 'record' AND EXISTS (SELECT 1 FROM target WHERE kind = 'record' AND id = g.scope_target_id)) OR (g.scope_type = 'blueprint_family' AND EXISTS (SELECT 1 FROM records e JOIN target t ON t.kind = 'record' AND t.id = e.id WHERE e.workspace_id = $2 AND e.blueprint_id = g.scope_target_id)) OR (g.scope_type = 'context_subtree' AND EXISTS (SELECT 1 FROM ancestors WHERE id = g.scope_target_id)) OR ($5 = '__context_list__' AND g.scope_type = 'context_subtree'))))",
            codes.grant_set_sql()
        );
        Ok(sqlx::query_scalar(&sql)
            .bind(user_id)
            .bind(workspace_id)
            .bind(codes.codes())
            .bind(target_id)
            .bind(target_code)
            .fetch_one(connection)
            .await?)
    }
}

/// The kind of target [`AttricatRepository::authorized_target_ids`] checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationTargetKind {
    Record,
    Blueprint,
    Context,
}

/// Which of a user's live grants count: those whose role carries a
/// permission, or those of any of the named roles.
#[derive(Clone, Copy)]
pub(crate) enum GrantCodes<'a> {
    Permission(Permission),
    Roles(&'a [String]),
}

impl<'a> GrantCodes<'a> {
    /// The `grants(scope_type, scope_target_id)` CTE for user `$1` in
    /// workspace `$2`, matching the codes bound as `$3`: active user,
    /// membership and workspace only.
    fn grant_set_sql(&self) -> String {
        let filter = match self {
            Self::Permission(_) => {
                "JOIN role_permissions rp ON rp.role_id = g.role_id WHERE rp.permission_code = ANY($3)"
            }
            Self::Roles(_) => {
                "JOIN roles r ON r.id = g.role_id WHERE (r.is_system OR r.workspace_id = $2) AND r.code = ANY($3)"
            }
        };
        format!(
            "grants AS (SELECT g.scope_type, g.scope_target_id FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id {filter} AND m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL)"
        )
    }

    /// The codes to bind as `$3`.
    fn codes(&self) -> Vec<&'a str> {
        match *self {
            Self::Permission(permission) => vec![permission.as_str()],
            Self::Roles(roles) => roles.iter().map(String::as_str).collect(),
        }
    }
}
