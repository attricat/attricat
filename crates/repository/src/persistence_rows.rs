//! Repository-local SQL row mapping for domain DTOs.
//!
//! Domain types deliberately do not depend on SQLx. `Db<T>` supplies the
//! persistence-only `FromRow` implementation while conversions at repository
//! boundaries return the dependency-free public DTO.

use catalog_domain::model::*;
use sqlx::{FromRow, Row, postgres::PgRow};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub(crate) struct Db<T>(T);

impl<T> Db<T> {
    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}

pub(crate) trait IntoDomain {
    type Output;

    fn into_domain(self) -> Self::Output;
}

impl<T> IntoDomain for Db<T> {
    type Output = T;

    fn into_domain(self) -> Self::Output {
        self.into_inner()
    }
}

impl<T> IntoDomain for Option<Db<T>> {
    type Output = Option<T>;

    fn into_domain(self) -> Self::Output {
        self.map(Db::into_inner)
    }
}

impl<T> IntoDomain for Vec<Db<T>> {
    type Output = Vec<T>;

    fn into_domain(self) -> Self::Output {
        self.into_iter().map(Db::into_inner).collect()
    }
}

impl<T> Deref for Db<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Db<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

trait DomainRow: Sized {
    fn from_pg_row(row: &PgRow) -> Result<Self, sqlx::Error>;
}

impl<'r, T: DomainRow> FromRow<'r, PgRow> for Db<T> {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        T::from_pg_row(row).map(Self)
    }
}

macro_rules! domain_row {
    ($type:ty { $($field:ident),+ $(,)? }) => {
        impl DomainRow for $type {
            fn from_pg_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                Ok(Self {
                    $($field: row.try_get(stringify!($field))?,)+
                })
            }
        }
    };
}

domain_row!(Blueprint {
    id,
    code,
    name,
    kind,
    version,
    includes,
    views,
    entity_schema,
    status,
    published_at,
    created_at,
    updated_at,
    deleted_at,
    definition,
    definition_hash,
});
domain_row!(Attribute {
    id,
    blueprint_id,
    blueprint_version,
    code,
    name,
    value_type,
    value_schema,
    extension_type,
    default_value,
    file_policy,
    target_blueprint_code,
    cardinality,
    target_cardinality,
    tags,
    context_fallback,
    context_editable,
    readonly,
    position,
    created_at,
    updated_at,
    deleted_at,
});
domain_row!(ReusableAttribute {
    id,
    definition_id,
    namespace,
    code,
    name,
    version,
    value_type,
    value_schema,
    default_value,
    file_policy,
    target_blueprint_code,
    cardinality,
    target_cardinality,
    tags,
    context_fallback,
    context_editable,
    readonly,
    searchable,
    facetable,
    status,
    published_at,
    definition,
});
domain_row!(EntityReusableAttribute {
    attachment_id,
    attribute_id,
    definition_id,
    revision_id,
    namespace,
    code,
    name,
    version,
    value_type,
    value_schema,
    default_value,
    file_policy,
    target_blueprint_code,
    cardinality,
    target_cardinality,
    tags,
    context_fallback,
    context_editable,
    readonly,
    searchable,
    facetable,
    position,
});
domain_row!(Entity {
    id,
    blueprint_id,
    blueprint_version,
    projections,
    system_tags,
    system_metadata,
    is_sample,
    created_at,
    updated_at,
    deleted_at,
});
domain_row!(AttributeContext {
    id,
    code,
    data,
    parent_id
});
domain_row!(AttributeValue {
    id,
    entity_id,
    attribute_id,
    value,
    relationship_target_entity_id,
    active,
    context_id,
    created_at,
});
domain_row!(PublicationChannel {
    context_id,
    context_code,
    enabled,
    required_rule_codes,
    require_valid_entity,
});
domain_row!(EntityPublicationStatus {
    context_id,
    context_code,
    status,
    published_at,
    published_by_user_id,
});
domain_row!(EntityAuditChange {
    audit_event_id,
    occurred_at,
    actor_user_id,
    actor_display_name,
    actor_email,
    actor_avatar_file_id,
    executor_type,
    agent_run_id,
    approval_decision,
    approved_by_user_id,
    approved_by_display_name,
    approved_by_avatar_file_id,
    attribute_id,
    attribute_code,
    context_id,
    context_code,
    change_kind,
    before_value,
    after_value,
});
domain_row!(FileVariantMetadata {
    kind,
    mime_type,
    width,
    height,
    byte_size,
    sha256,
});
domain_row!(BlueprintMigrationBatch {
    id,
    blueprint_id,
    target_version,
    status,
    removal_policy,
    created_at,
    started_at,
    completed_at,
});
domain_row!(BlueprintMigrationBatchStatus {
    id,
    blueprint_id,
    target_version,
    status,
    removal_policy,
    created_at,
    started_at,
    completed_at,
    total_entities,
    processed_entities,
    migrated_entities,
    needs_input_entities,
    failed_entities,
});
domain_row!(DataHealthSummary {
    active_entities,
    entity_blueprints,
    contexts,
    outdated_entities,
    stale_entities,
    deleted_relationship_targets,
});
domain_row!(BlueprintHealth {
    code,
    name,
    current_version,
    active_entities,
    outdated_entities,
    stale_entities,
    oldest_updated_at,
    newest_updated_at,
});
domain_row!(FreshnessBand { label, entities });
domain_row!(ContextHealth {
    code,
    direct_entities,
    direct_values
});
domain_row!(RelationshipHealth {
    attribute_code,
    source_blueprint,
    active_edges,
    deleted_targets,
});
domain_row!(StorageHealth { table, bytes });
domain_row!(CompletenessHealth {
    code,
    name,
    current_version,
    active_entities,
    outdated_entities,
    default_complete_entities,
});
domain_row!(Workflow {
    id,
    code,
    name,
    version,
    status,
    definition,
    definition_hash,
    compiled_plan,
    published_at,
    created_at,
    enabled_version,
    manual_enabled,
});
domain_row!(Rule {
    id,
    blueprint_id,
    blueprint_version,
    context_id,
    code,
    name,
    version,
    status,
    definition,
    definition_hash,
    compiled_plan,
    published_at,
    created_at,
    enabled_version,
});
domain_row!(RuleRun {
    id,
    rule_id,
    rule_version,
    source,
    dry_run,
    scope_entity_id,
    status,
    candidate_cursor,
    candidates_evaluated,
    findings_created,
    findings_resolved,
    attempts,
    last_error,
    completed_at,
    created_at,
});
domain_row!(RuleFinding {
    id,
    rule_id,
    rule_version,
    entity_id,
    context_id,
    severity,
    message,
    evidence,
    state,
    acknowledged_at,
    resolved_at,
    created_at,
    updated_at,
});
