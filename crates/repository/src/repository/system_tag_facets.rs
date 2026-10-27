//! Counts of the system tags a blueprint's records carry, for choosing a
//! system tag search filter.

use super::{AttricatRepository, RepositoryError, SAMPLE_SYSTEM_TAG};
use crate::model::{SystemTagCount, SystemTagFacetResponse};
use uuid::Uuid;

/// Tags a facet lists when the request names no limit.
pub const DEFAULT_SYSTEM_TAG_FACET_LIMIT: u32 = 50;
/// Most tags one facet request may list.
pub const MAX_SYSTEM_TAG_FACET_LIMIT: u32 = 100;

impl AttricatRepository {
    /// Lists the system tags of a blueprint's live records with how many
    /// records carry each, most used first. The sample-data marker is left
    /// out: search filters cannot name it, and records show it as a badge.
    ///
    /// Tag arrays are unnested per record, so this reads every record of the
    /// blueprint; clients request it only when a tag filter is opened.
    pub async fn system_tag_facet(
        &self,
        blueprint_id: Uuid,
        blueprint_version: Option<i64>,
        prefix: Option<&str>,
        limit: u32,
    ) -> Result<SystemTagFacetResponse, RepositoryError> {
        let mut items = sqlx::query_as::<_, (String, i64)>(
            r#"SELECT t.tag, count(*) AS count
               FROM records e
               CROSS JOIN LATERAL unnest(e.system_tags) AS t(tag)
               WHERE e.workspace_id = $1
                 AND e.blueprint_id = $2
                 AND ($3::bigint IS NULL OR e.blueprint_version = $3)
                 AND e.deleted_at IS NULL
                 AND t.tag <> $4
                 AND ($5::text IS NULL OR starts_with(lower(t.tag), lower($5)))
               GROUP BY t.tag
               ORDER BY count DESC, t.tag
               LIMIT $6"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(SAMPLE_SYSTEM_TAG)
        .bind(prefix)
        .bind(i64::from(limit) + 1)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(tag, count)| SystemTagCount { tag, count })
        .collect::<Vec<_>>();
        let truncated = items.len() > limit as usize;
        items.truncate(limit as usize);
        Ok(SystemTagFacetResponse { items, truncated })
    }
}
