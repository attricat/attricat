use crate::persistence_rows::{Db, IntoDomain};
use sqlx::query_as;

use super::{CatalogRepository, RepositoryError};
use crate::model::{
    BlueprintHealth, CompletenessHealth, ContextHealth, DataHealthSummary, FreshnessBand,
    RelationshipHealth, StorageHealth,
};

impl CatalogRepository {
    /// Performs a bounded, side-effect-free database dependency probe for the
    /// API readiness endpoint. This is intentionally separate from liveness:
    /// a live process with an unavailable database must not receive traffic.
    pub async fn readiness(&self) -> Result<(), RepositoryError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn data_health_summary(
        &self,
        stale_after_days: i64,
    ) -> Result<DataHealthSummary, RepositoryError> {
        Ok(query_as::<_, Db<DataHealthSummary>>(
            r#"WITH current_blueprints AS (
                    SELECT DISTINCT ON (id) id, version FROM blueprints
                    WHERE workspace_id = $2 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL
                    ORDER BY id, version DESC
                ) SELECT (SELECT count(*) FROM entities WHERE workspace_id = $2 AND deleted_at IS NULL) AS active_entities,
                    (SELECT count(*) FROM current_blueprints) AS entity_blueprints,
                    (SELECT count(*) FROM attribute_contexts WHERE workspace_id = $2) AS contexts,
                    (SELECT count(*) FROM entities e JOIN current_blueprints b ON b.id = e.blueprint_id WHERE e.workspace_id = $2 AND e.deleted_at IS NULL AND e.blueprint_version <> b.version) AS outdated_entities,
                    (SELECT count(*) FROM entities WHERE workspace_id = $2 AND deleted_at IS NULL AND updated_at < now() - ($1 * interval '1 day')) AS stale_entities,
                    (SELECT count(*) FROM attribute_values av JOIN entities target ON target.id = av.relationship_target_entity_id AND target.workspace_id = $2 WHERE av.workspace_id = $2 AND av.active AND av.relationship_target_entity_id IS NOT NULL AND target.deleted_at IS NOT NULL) AS deleted_relationship_targets"#,
        )
        .bind(stale_after_days)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_one(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn data_health_blueprints(
        &self,
        stale_after_days: i64,
    ) -> Result<Vec<BlueprintHealth>, RepositoryError> {
        Ok(query_as::<_, Db<BlueprintHealth>>(r#"WITH current_blueprints AS (
                SELECT DISTINCT ON (id) id, code, name, version FROM blueprints WHERE workspace_id = $2 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL ORDER BY id, version DESC
            ) SELECT b.code, b.name, b.version AS current_version, count(e.id) AS active_entities,
                count(e.id) FILTER (WHERE e.blueprint_version <> b.version) AS outdated_entities,
                count(e.id) FILTER (WHERE e.updated_at < now() - ($1 * interval '1 day')) AS stale_entities,
                min(e.updated_at) AS oldest_updated_at, max(e.updated_at) AS newest_updated_at
            FROM current_blueprints b LEFT JOIN entities e ON e.workspace_id = $2 AND e.blueprint_id = b.id AND e.deleted_at IS NULL
            GROUP BY b.id, b.code, b.name, b.version ORDER BY outdated_entities DESC, stale_entities DESC, b.code"#)
            .bind(stale_after_days)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool).await?
        .into_domain())
    }

    pub async fn data_health_freshness(&self) -> Result<Vec<FreshnessBand>, RepositoryError> {
        Ok(query_as::<_, Db<FreshnessBand>>(r#"SELECT label, count(*)::bigint AS entities FROM (
                SELECT CASE WHEN updated_at >= now() - interval '30 days' THEN '0-29 days' WHEN updated_at >= now() - interval '90 days' THEN '30-89 days' WHEN updated_at >= now() - interval '180 days' THEN '90-179 days' ELSE '180+ days' END AS label
                FROM entities WHERE workspace_id = $1 AND deleted_at IS NULL
            ) bands GROUP BY label ORDER BY CASE label WHEN '0-29 days' THEN 1 WHEN '30-89 days' THEN 2 WHEN '90-179 days' THEN 3 ELSE 4 END"#)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool).await?
        .into_domain())
    }

    pub async fn data_health_contexts(&self) -> Result<Vec<ContextHealth>, RepositoryError> {
        Ok(query_as::<_, Db<ContextHealth>>(r#"SELECT c.code, count(DISTINCT av.entity_id)::bigint AS direct_entities, count(av.id)::bigint AS direct_values FROM attribute_contexts c LEFT JOIN attribute_values av ON av.workspace_id = $1 AND av.context_id = c.id AND av.active WHERE c.workspace_id = $1 GROUP BY c.id, c.code ORDER BY direct_values DESC, c.code"#)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool).await?
        .into_domain())
    }

    pub async fn data_health_completeness(
        &self,
    ) -> Result<Vec<CompletenessHealth>, RepositoryError> {
        Ok(query_as::<_, Db<CompletenessHealth>>(r#"WITH current_blueprints AS (
                SELECT DISTINCT ON (id) id, code, name, version FROM blueprints WHERE workspace_id = $1 AND kind = 'entity' AND status = 'published' AND deleted_at IS NULL ORDER BY id, version DESC
            ), required_attributes AS (
                SELECT b.id AS blueprint_id, b.version AS blueprint_version, required.code, a.id AS attribute_id FROM blueprints b CROSS JOIN LATERAL jsonb_array_elements_text(COALESCE(b.entity_schema->'required', '[]'::jsonb)) required(code) LEFT JOIN attributes a ON a.workspace_id = $1 AND a.blueprint_id = b.id AND a.blueprint_version = b.version AND a.code = required.code AND a.deleted_at IS NULL WHERE b.workspace_id = $1 AND b.kind = 'entity' AND b.deleted_at IS NULL
            ), required_counts AS (SELECT blueprint_id, blueprint_version, count(*)::bigint AS required_attributes FROM required_attributes GROUP BY blueprint_id, blueprint_version), default_context AS (SELECT id FROM attribute_contexts WHERE workspace_id = $1 AND code = 'default'), current_values AS (
                SELECT DISTINCT av.entity_id, av.attribute_id, av.context_id FROM attribute_values av JOIN entities e ON e.id = av.entity_id AND e.workspace_id = $1 AND e.deleted_at IS NULL JOIN required_attributes required ON required.attribute_id = av.attribute_id AND required.blueprint_id = e.blueprint_id AND required.blueprint_version = e.blueprint_version JOIN default_context ON default_context.id = av.context_id WHERE av.workspace_id = $1 AND (av.relationship_target_entity_id IS NULL OR av.active)
            ), direct_satisfied AS (SELECT entity_id, context_id, count(*)::bigint AS attributes FROM current_values GROUP BY entity_id, context_id), active_entities AS (
                SELECT e.id AS entity_id, e.blueprint_id, e.blueprint_version, current_blueprints.code, current_blueprints.name, current_blueprints.version AS current_version FROM entities e JOIN blueprints b ON b.workspace_id = $1 AND b.id = e.blueprint_id AND b.version = e.blueprint_version JOIN current_blueprints ON current_blueprints.id = e.blueprint_id WHERE e.workspace_id = $1 AND e.deleted_at IS NULL AND b.kind = 'entity' AND b.deleted_at IS NULL
            ) SELECT e.code, e.name, e.current_version, count(*)::bigint AS active_entities, count(*) FILTER (WHERE e.blueprint_version <> e.current_version)::bigint AS outdated_entities, count(*) FILTER (WHERE COALESCE(direct_satisfied.attributes, 0) = COALESCE(required_counts.required_attributes, 0))::bigint AS default_complete_entities FROM active_entities e CROSS JOIN default_context LEFT JOIN required_counts ON required_counts.blueprint_id = e.blueprint_id AND required_counts.blueprint_version = e.blueprint_version LEFT JOIN direct_satisfied ON direct_satisfied.entity_id = e.entity_id AND direct_satisfied.context_id = default_context.id GROUP BY e.blueprint_id, e.code, e.name, e.current_version ORDER BY default_complete_entities ASC, e.code"#)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool).await?
        .into_domain())
    }

    pub async fn data_health_relationships(
        &self,
    ) -> Result<Vec<RelationshipHealth>, RepositoryError> {
        Ok(query_as::<_, Db<RelationshipHealth>>(r#"SELECT a.code AS attribute_code, b.code AS source_blueprint, count(*)::bigint AS active_edges, count(*) FILTER (WHERE target.deleted_at IS NOT NULL)::bigint AS deleted_targets FROM attribute_values av JOIN attributes a ON a.workspace_id = $1 AND a.id = av.attribute_id JOIN blueprints b ON b.workspace_id = $1 AND b.id = a.blueprint_id AND b.version = a.blueprint_version JOIN entities target ON target.workspace_id = $1 AND target.id = av.relationship_target_entity_id WHERE av.workspace_id = $1 AND av.active AND av.relationship_target_entity_id IS NOT NULL GROUP BY a.code, b.code ORDER BY deleted_targets DESC, active_edges DESC, b.code, a.code LIMIT 50"#)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_all(&self.pool).await?
        .into_domain())
    }

    pub async fn data_health_storage(&self) -> Result<Vec<StorageHealth>, RepositoryError> {
        // PostgreSQL relation sizes are database-wide. Do not disclose shared physical
        // storage metrics from a workspace-scoped endpoint.
        Ok(Vec::new())
    }
}
