use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

impl CatalogRepository {
    /// Solution-pack permissions are bootstrapped in application code so the
    /// database migration history remains declarative.
    pub async fn ensure_solution_pack_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('solution_packs.manage', 'Inspect and manage solution packs') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'solution_packs.manage') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
