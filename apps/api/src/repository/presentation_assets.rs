use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{CatalogRepository, RepositoryError};

pub const MAX_PRESENTATION_ASSET_PAGE_SIZE: i64 = 100;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct PresentationAsset {
    pub id: Uuid,
    pub purpose: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sha256: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub(crate) object_key: String,
}

impl CatalogRepository {
    pub async fn list_presentation_assets(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PresentationAsset>, RepositoryError> {
        let workspace_id = self.workspace_id.expect("workspace-scoped repository");
        Ok(sqlx::query_as::<_, PresentationAsset>(
            "SELECT id,purpose,media_type,byte_size,sha256,width,height,created_at,object_key FROM presentation_assets WHERE workspace_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3",
        )
        .bind(workspace_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_presentation_asset(
        &self,
        id: Uuid,
    ) -> Result<PresentationAsset, RepositoryError> {
        let workspace_id = self.workspace_id.expect("workspace-scoped repository");
        sqlx::query_as::<_, PresentationAsset>(
            "SELECT id,purpose,media_type,byte_size,sha256,width,height,created_at,object_key FROM presentation_assets WHERE workspace_id=$1 AND id=$2",
        )
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(RepositoryError::NotFound("presentation asset"))
    }
}
