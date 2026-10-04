use super::{CatalogRepository, RepositoryError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct SavedView {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub visibility: String,
    pub state: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const FIELDS: &str =
    "id,owner_user_id,kind,name,description,visibility,state,created_at,updated_at";

pub(super) fn state_hash(state: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(state).expect("JSON value serializes"))
    )
}

impl CatalogRepository {
    /// Lists named views visible to `actor`, optionally narrowed to views whose
    /// name or description contains `search` case-insensitively.
    pub async fn list_saved_views(
        &self,
        actor: Uuid,
        search: &str,
    ) -> Result<Vec<SavedView>, RepositoryError> {
        let query = format!(
            "SELECT {FIELDS} FROM saved_views WHERE workspace_id=$1 AND deleted_at IS NULL AND (owner_user_id=$2 OR visibility='workspace') AND visibility<>'link' AND ($3 = '' OR position(lower($3) in lower(coalesce(name,''))) > 0 OR position(lower($3) in lower(coalesce(description,''))) > 0) ORDER BY updated_at DESC LIMIT 100"
        );
        Ok(sqlx::query_as(&query)
            .bind(self.workspace_id_for_runtime())
            .bind(actor)
            .bind(search)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_saved_view(
        &self,
        actor: Uuid,
        id: Uuid,
        link: bool,
    ) -> Result<Option<SavedView>, RepositoryError> {
        let query = format!(
            "SELECT {FIELDS} FROM saved_views WHERE workspace_id=$1 AND id=$2 AND deleted_at IS NULL AND ((visibility='link' AND $3) OR (visibility<>'link' AND NOT $3 AND (owner_user_id=$4 OR visibility='workspace')))"
        );
        Ok(sqlx::query_as(&query)
            .bind(self.workspace_id_for_runtime())
            .bind(id)
            .bind(link)
            .bind(actor)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn create_saved_view(
        &self,
        actor: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        visibility: &str,
        state: &Value,
    ) -> Result<SavedView, RepositoryError> {
        let hash = state_hash(state);
        let workspace = self.workspace_id_for_runtime();
        if visibility == "link" {
            let query = format!(
                "SELECT {FIELDS} FROM saved_views WHERE workspace_id=$1 AND state_hash=$2 AND state=$3 AND visibility='link' AND deleted_at IS NULL LIMIT 1"
            );
            if let Some(view) = sqlx::query_as(&query)
                .bind(workspace)
                .bind(&hash)
                .bind(state)
                .fetch_optional(&self.pool)
                .await?
            {
                return Ok(view);
            }
        }
        let query = format!(
            "INSERT INTO saved_views (id,workspace_id,owner_user_id,kind,name,description,visibility,state,state_hash) VALUES ($1,$2,$3,'explorer_search',$4,$5,$6,$7,$8) RETURNING {FIELDS}"
        );
        Ok(sqlx::query_as(&query)
            .bind(Uuid::new_v4())
            .bind(workspace)
            .bind(actor)
            .bind(name)
            .bind(description)
            .bind(visibility)
            .bind(state)
            .bind(hash)
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn update_saved_view(
        &self,
        actor: Uuid,
        id: Uuid,
        name: &str,
        description: Option<&str>,
        visibility: &str,
        state: &Value,
    ) -> Result<Option<SavedView>, RepositoryError> {
        let hash = state_hash(state);
        let query = format!(
            "UPDATE saved_views SET name=$4,description=$5,visibility=$6,state=$7,state_hash=$8,updated_at=now() WHERE workspace_id=$1 AND id=$2 AND owner_user_id=$3 AND visibility<>'link' AND deleted_at IS NULL RETURNING {FIELDS}"
        );
        Ok(sqlx::query_as(&query)
            .bind(self.workspace_id_for_runtime())
            .bind(id)
            .bind(actor)
            .bind(name)
            .bind(description)
            .bind(visibility)
            .bind(state)
            .bind(hash)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn delete_saved_view(&self, actor: Uuid, id: Uuid) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "UPDATE saved_views SET deleted_at=now() WHERE workspace_id=$1 AND id=$2 AND owner_user_id=$3 AND visibility<>'link' AND deleted_at IS NULL",
        )
        .bind(self.workspace_id_for_runtime())
        .bind(id)
        .bind(actor)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
