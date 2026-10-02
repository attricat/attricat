use super::*;

impl<S: RepositoryScope> CatalogRepository<S> {
    pub fn pool_for_runtime(&self) -> PgPool {
        self.pool.clone()
    }

    /// Pure scope derivation: no connection acquisition, provisioning or SQL.
    /// The async Result signature is retained for existing runtime callers.
    pub async fn for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<CatalogRepository, RepositoryError> {
        Ok(CatalogRepository {
            pool: self.pool.clone(),
            workspace_id: WorkspaceScope(workspace_id),
            audit_context: self.audit_context.clone(),
            event_context: self.event_context.clone(),
            task_fence: self.task_fence.clone(),
            extension_id: self.extension_id.clone(),
        })
    }

    /// Explicitly leave tenant scope for process-level coordination. System
    /// repositories cannot call workspace data methods until re-scoped.
    pub fn system_scope(&self) -> SystemRepository {
        CatalogRepository::system(self.pool.clone())
    }
}

impl From<CatalogRepository> for SystemRepository {
    fn from(repository: CatalogRepository) -> Self {
        repository.system_scope()
    }
}
