use super::*;
use catalog_validation::status::MAX_RETENTION_DAYS;
use chrono::Utc;

/// A retention hold keeps a file's exact bytes: storage reclamation skips the
/// file while any hold is active.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct FileRetentionHold {
    pub id: Uuid,
    pub file_id: Uuid,
    /// `status` holds come from entering a status with `retention_days`;
    /// `explicit` holds are placed by a user.
    pub source: String,
    pub entity_id: Option<Uuid>,
    pub attribute_code: Option<String>,
    pub status: Option<String>,
    pub reason: Option<String>,
    pub held_until: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub released_by_user_id: Option<Uuid>,
    /// Not released and not yet expired.
    pub active: bool,
}

const HOLD_COLUMNS: &str = "id, file_id, source, entity_id, attribute_code, status, reason, held_until, created_by_user_id, created_at, released_at, released_by_user_id, (released_at IS NULL AND held_until > now()) AS active";

impl CatalogRepository {
    /// Holds on one file, newest first, including expired and released ones.
    pub async fn file_retention_holds(
        &self,
        file_id: Uuid,
    ) -> Result<Vec<FileRetentionHold>, RepositoryError> {
        Ok(sqlx::query_as::<_, FileRetentionHold>(&format!(
            "SELECT {HOLD_COLUMNS} FROM file_retention_holds WHERE workspace_id = $1 AND file_id = $2 ORDER BY created_at DESC, id DESC LIMIT 200"
        ))
        .bind(self.workspace_id.0)
        .bind(file_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Holds on files the entity references or placed by its statuses.
    pub async fn entity_retention_holds(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<FileRetentionHold>, RepositoryError> {
        Ok(sqlx::query_as::<_, FileRetentionHold>(&format!(
            "SELECT {HOLD_COLUMNS} FROM file_retention_holds h WHERE workspace_id = $1 AND (entity_id = $2 OR file_id IN (SELECT r.file_id FROM attribute_file_references r JOIN attribute_values v ON v.id = r.attribute_value_id AND v.workspace_id = r.workspace_id WHERE v.entity_id = $2 AND v.workspace_id = $1)) ORDER BY created_at DESC, id DESC LIMIT 200"
        ))
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Places an explicit hold on a live workspace file.
    pub async fn place_file_retention_hold(
        &self,
        file_id: Uuid,
        days: i64,
        reason: &str,
        actor_user_id: Uuid,
    ) -> Result<FileRetentionHold, RepositoryError> {
        if !(1..=MAX_RETENTION_DAYS).contains(&days) {
            return Err(RepositoryError::InvalidRetentionHold(format!(
                "days must be between 1 and {MAX_RETENTION_DAYS}"
            )));
        }
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > 1000 {
            return Err(RepositoryError::InvalidRetentionHold(
                "reason must contain 1 to 1000 characters".to_owned(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        // Lock the file so a concurrent reconciliation cannot mark it deleted
        // between this check and the hold insert.
        let live: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM files WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(file_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut *transaction)
        .await?;
        if live.is_none() {
            return Err(RepositoryError::NotFound("file"));
        }
        let hold = sqlx::query_as::<_, FileRetentionHold>(&format!(
            "INSERT INTO file_retention_holds (id, workspace_id, file_id, source, reason, held_until, created_by_user_id) VALUES ($1, $2, $3, 'explicit', $4, now() + make_interval(days => $5), $6) RETURNING {HOLD_COLUMNS}"
        ))
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.0)
        .bind(file_id)
        .bind(reason)
        .bind(days as i32)
        .bind(actor_user_id)
        .fetch_one(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(hold)
    }

    /// Releases an explicit hold early. Holds placed by a status run until
    /// they expire.
    pub async fn release_file_retention_hold(
        &self,
        file_id: Uuid,
        hold_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<FileRetentionHold, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let source: Option<String> = sqlx::query_scalar(
            "SELECT source FROM file_retention_holds WHERE id = $1 AND file_id = $2 AND workspace_id = $3 AND released_at IS NULL FOR UPDATE",
        )
        .bind(hold_id)
        .bind(file_id)
        .bind(self.workspace_id.0)
        .fetch_optional(&mut *transaction)
        .await?;
        match source.as_deref() {
            None => return Err(RepositoryError::NotFound("retention hold")),
            Some("explicit") => {}
            Some(_) => {
                return Err(RepositoryError::InvalidRetentionHold(
                    "holds placed by a record status cannot be released early".to_owned(),
                ));
            }
        }
        let hold = sqlx::query_as::<_, FileRetentionHold>(&format!(
            "UPDATE file_retention_holds SET released_at = now(), released_by_user_id = $2 WHERE id = $1 RETURNING {HOLD_COLUMNS}"
        ))
        .bind(hold_id)
        .bind(actor_user_id)
        .fetch_one(&mut *transaction)
        .await?;
        self.commit_mutation(transaction).await?;
        Ok(hold)
    }
}

impl<S: RepositoryScope> CatalogRepository<S> {
    /// Registers the permission for explicit file holds and grants it to the
    /// owner and admin system roles.
    pub async fn ensure_retention_hold_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('files.hold', 'Place and release retention holds on files') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'files.hold') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
