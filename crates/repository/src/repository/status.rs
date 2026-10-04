use super::*;
use catalog_validation::status::{
    STATUS_KEY, StatusCoverage, TransitionRequirements, has_record_controls, status_approval,
    status_lock, status_retention_days, transition_requirements, validate_status_transition,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};

/// A live status attribute: `(id, code, value_schema, context_fallback)`.
type StatusAttribute = (Uuid, String, Value, String);

/// One record state by context code and attribute code. Scalars are their
/// stored columns, relationships the sorted active target IDs and files the
/// ordered `{id, sha256}` references, so equal content compares equal.
type RecordContent = HashMap<String, Map<String, Value>>;

#[derive(Clone, Copy)]
enum RecordState {
    /// The state at the start of the current transaction.
    Before,
    /// The state including this transaction's writes.
    After,
}

#[derive(Clone, sqlx::FromRow)]
struct ContextNode {
    id: Uuid,
    code: String,
    parent_id: Option<Uuid>,
}

/// One effective status change in one context.
pub(super) struct StatusChange {
    pub(super) attribute_code: String,
    pub(super) schema: Value,
    pub(super) context_id: Uuid,
    pub(super) context_code: String,
    pub(super) before: Value,
    pub(super) after: Value,
}

impl StatusChange {
    pub(super) fn transition(&self) -> super::checks::CheckTransition {
        super::checks::CheckTransition {
            attribute_code: self.attribute_code.clone(),
            from: self.before.as_str().map(str::to_owned),
            to: self.after.as_str().map(str::to_owned),
        }
    }
}

/// Whether the current principal may take one declared edge, for the status control.
#[derive(Debug, serde::Serialize)]
pub struct StatusTransitionAccess {
    pub attribute_code: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub code: Option<String>,
    pub allowed: bool,
    /// Stable error code the write would return: `status_transition_forbidden`,
    /// `status_separation_of_duties` or `transition_conditions_unmet`.
    pub denial_code: Option<&'static str>,
    pub denial_reason: Option<String>,
    /// Transition conditions and enforcing rules that the saved state plus
    /// this destination would not satisfy.
    pub unmet: Vec<super::CheckViolation>,
}

/// One recorded approval decision.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct EntityApproval {
    pub id: Uuid,
    pub attribute_code: String,
    pub context_id: Uuid,
    pub context_code: String,
    pub status: String,
    pub covers_all: bool,
    pub covered_attributes: Vec<String>,
    pub content_digest: String,
    pub approved_by_user_id: Option<Uuid>,
    pub approved_by: Option<String>,
    pub approved_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// `content_changed` (voided) or `superseded` by a later approval.
    pub end_reason: Option<String>,
    pub ended_by_user_id: Option<Uuid>,
    pub void_status: Option<String>,
}

const SCALAR_VALUE: &str = "jsonb_build_array(to_jsonb(v.value_text), to_jsonb(v.value_number), to_jsonb(v.value_integer), to_jsonb(v.value_boolean), to_jsonb(v.value_date), to_jsonb(v.value_datetime), to_jsonb(v.value_time::text), to_jsonb(v.value_time_zone), v.value_json)";

fn content_query(state: RecordState) -> String {
    let select = |files: &str| {
        format!(
            "SELECT a.code, COALESCE(c.code, 'default') AS context_code, a.value_type, v.active, v.relationship_target_entity_id, CASE WHEN a.value_type = 'file' THEN {files} WHEN a.value_type = 'relationship' THEN NULL ELSE {SCALAR_VALUE} END AS value"
        )
    };
    let current_files = "(SELECT COALESCE(jsonb_agg(jsonb_build_object('id', f.id, 'sha256', f.sha256) ORDER BY r.position), '[]'::jsonb) FROM attribute_file_references r JOIN files f ON f.id = r.file_id AND f.workspace_id = r.workspace_id WHERE r.attribute_value_id = v.id AND r.workspace_id = v.workspace_id)";
    let archived_files = "(SELECT COALESCE(jsonb_agg(jsonb_build_object('id', f.id, 'sha256', f.sha256) ORDER BY r.position), '[]'::jsonb) FROM attribute_file_reference_history r JOIN files f ON f.id = r.file_id AND f.workspace_id = r.workspace_id WHERE r.attribute_value_history_id = v.id AND r.attribute_value_history_archived_at = v.archived_at)";
    let from = "JOIN attributes a ON a.id = v.attribute_id LEFT JOIN attribute_contexts c ON c.id = v.context_id WHERE v.entity_id = $1 AND v.workspace_id = $2";
    match state {
        RecordState::After => format!("{} FROM attribute_values v {from}", select(current_files)),
        // `now()` is the transaction start: rows created earlier are the
        // original state, and rows archived by this transaction restore what
        // it replaced. Intermediate rows of this transaction are excluded.
        RecordState::Before => format!(
            "{} FROM attribute_values v {from} AND v.created_at < now() UNION ALL {} FROM attribute_value_history v {from} AND v.archived_at = now() AND v.created_at < now()",
            select(current_files),
            select(archived_files)
        ),
    }
}

/// The context resolution path for one attribute, nearest first.
fn context_path<'a>(
    contexts: &'a [ContextNode],
    context: &'a ContextNode,
    inherit: bool,
) -> Result<Vec<&'a str>, RepositoryError> {
    let mut path = vec![context.code.as_str()];
    let mut visited = HashSet::from([context.id]);
    let mut parent = if inherit { context.parent_id } else { None };
    while let Some(item) = parent.and_then(|id| contexts.iter().find(|item| item.id == id)) {
        if !visited.insert(item.id) {
            return Err(RepositoryError::InvalidContext);
        }
        path.push(item.code.as_str());
        parent = item.parent_id;
    }
    Ok(path)
}

fn effective(content: &RecordContent, path: &[&str], code: &str) -> Value {
    path.iter()
        .find_map(|context| content.get(*context).and_then(|values| values.get(code)))
        .cloned()
        .unwrap_or(Value::Null)
}

fn effective_projection(projection: &Value, path: &[&str], code: &str) -> Value {
    path.iter()
        .find_map(|context| projection.get(*context).and_then(|values| values.get(code)))
        .cloned()
        .unwrap_or(Value::Null)
}

/// SHA-256 of the covered effective content in one context, as canonical
/// JSON with sorted keys. Absent values are omitted.
fn content_digest(
    content: &RecordContent,
    inheritance: &HashMap<String, bool>,
    contexts: &[ContextNode],
    context: &ContextNode,
    coverage: &StatusCoverage,
    status_code: &str,
) -> Result<String, RepositoryError> {
    let codes: BTreeSet<&str> = match coverage {
        StatusCoverage::All => content
            .values()
            .flat_map(|values| values.keys().map(String::as_str))
            .chain(inheritance.keys().map(String::as_str))
            .collect(),
        StatusCoverage::Attributes(codes) => codes.iter().map(String::as_str).collect(),
    };
    let mut document = Map::new();
    for code in codes
        .into_iter()
        .filter(|code| coverage.covers(code, status_code))
    {
        let inherit = inheritance.get(code).copied().unwrap_or(true);
        let value = effective(content, &context_path(contexts, context, inherit)?, code);
        if !value.is_null() {
            document.insert(code.to_owned(), value);
        }
    }
    let bytes = serde_json::to_vec(&Value::Object(document)).expect("record content serializes");
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn label(value: &Value) -> String {
    value.as_str().unwrap_or("not set").to_owned()
}

impl CatalogRepository {
    /// Status attributes of the entity's blueprint revision and its own additional attributes.
    async fn status_attributes(
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<Vec<StatusAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, StatusAttribute>(
            "SELECT id, code, value_schema, context_fallback FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_schema ? $4",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(entity.id)
        .bind(STATUS_KEY)
        .fetch_all(&mut **transaction)
        .await?)
    }

    async fn status_contexts(
        &self,
        connection: &mut sqlx::PgConnection,
    ) -> Result<Vec<ContextNode>, RepositoryError> {
        Ok(sqlx::query_as::<_, ContextNode>(
            "SELECT id, code, parent_id FROM attribute_contexts WHERE workspace_id = $1 ORDER BY code",
        )
        .bind(self.workspace_id.0)
        .fetch_all(connection)
        .await?)
    }

    /// Whether each attribute code inherits values from parent contexts.
    async fn attribute_inheritance(
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<HashMap<String, bool>, RepositoryError> {
        Ok(sqlx::query_as::<_, (String, String)>(
            "SELECT code, context_fallback FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(entity.id)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .map(|(code, fallback)| (code, fallback != "none"))
        .collect())
    }

    async fn record_content(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        state: RecordState,
    ) -> Result<RecordContent, RepositoryError> {
        let rows =
            sqlx::query_as::<_, (String, String, String, bool, Option<Uuid>, Option<Value>)>(
                &content_query(state),
            )
            .bind(entity_id)
            .bind(self.workspace_id.0)
            .fetch_all(&mut **transaction)
            .await?;
        let mut content = RecordContent::new();
        for (code, context, value_type, active, target, value) in rows {
            let values = content.entry(context).or_default();
            if value_type == "relationship" {
                // Inactive rows are explicit empty sets in a context.
                let targets = values
                    .entry(code)
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let (true, Some(target), Some(targets)) =
                    (active, target, targets.as_array_mut())
                {
                    targets.push(Value::String(target.to_string()));
                    targets.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
                }
            } else if let Some(value) = value {
                values.insert(code, value);
            }
        }
        Ok(content)
    }

    pub(super) async fn has_status_writes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        values: &[NewAttributeValue],
        removed: &[AttributeValueSelector],
    ) -> Result<bool, RepositoryError> {
        let statuses = Self::status_attributes(transaction, entity).await?;
        Ok(statuses.iter().any(|(id, code, ..)| {
            removed.iter().any(|selector| &selector.attribute_code == code)
                || values.iter().any(|value| matches!(value, NewAttributeValue::Scalar { attribute_id, attribute_code, .. } if attribute_id.as_ref() == Some(id) || attribute_code.as_ref() == Some(code)))
        }))
    }

    pub(super) async fn check_status_precondition(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        expected_updated_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        if expected_updated_at.is_some_and(|expected| expected != entity.updated_at) {
            return Err(RepositoryError::StaleEntity);
        }
        if expected_updated_at.is_none()
            && !Self::status_attributes(transaction, entity)
                .await?
                .is_empty()
        {
            return Err(RepositoryError::StatusPreconditionRequired);
        }
        Ok(())
    }

    /// Validates every effective status change of the transaction and returns
    /// the changes. Inheritance is resolved on both sides.
    async fn status_changes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attributes: &[StatusAttribute],
        contexts: &[ContextNode],
    ) -> Result<Vec<StatusChange>, RepositoryError> {
        let after = Self::build_preview_projection(transaction, entity.id).await?;
        let before = entity.projections.get("preview").unwrap_or(&Value::Null);
        let mut changes = Vec::new();
        for (_, code, schema, fallback) in attributes {
            for context in contexts {
                let path = context_path(contexts, context, fallback != "none")?;
                let (before, after) = (
                    effective_projection(before, &path, code),
                    effective_projection(&after, &path, code),
                );
                validate_status_transition(schema, &before, &after).map_err(|message| {
                    RepositoryError::AttributeValueSchemaMismatch {
                        attribute: code.clone(),
                        instance_path: String::new(),
                        message: format!("{message} (context: {})", context.code),
                    }
                })?;
                if before != after {
                    changes.push(StatusChange {
                        attribute_code: code.clone(),
                        schema: schema.clone(),
                        context_id: context.id,
                        context_code: context.code.clone(),
                        before,
                        after,
                    });
                }
            }
        }
        Ok(changes)
    }

    /// Effective status changes of the transaction, for declarative checks.
    pub(super) async fn effective_status_changes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<Vec<StatusChange>, RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        if attributes.is_empty() {
            return Ok(Vec::new());
        }
        let contexts = self.status_contexts(transaction).await?;
        self.status_changes(transaction, entity, &attributes, &contexts)
            .await
    }

    /// Run once on the final transaction state, using the locked entity's saved
    /// projection as the baseline. Checks transition edges, their permission,
    /// role and separation-of-duties requirements, and status locks. It has no
    /// side effects; [`Self::apply_status_effects`] records the outcome.
    pub(super) async fn validate_status_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        if attributes.is_empty() {
            return Ok(());
        }
        let contexts = self.status_contexts(transaction).await?;
        let changes = self
            .status_changes(transaction, entity, &attributes, &contexts)
            .await?;
        let actor = self.acting_principal();
        for change in &changes {
            let requirements =
                transition_requirements(&change.schema, &change.before, &change.after);
            if let Some(error) = self
                .transition_denial(transaction, entity, change, &requirements, actor)
                .await?
            {
                return Err(error);
            }
        }
        self.check_status_locks(transaction, entity, &attributes, &contexts)
            .await
    }

    /// The principal on whose behalf this repository writes: an interactive
    /// extension's initiator, the request or agent actor, or the user whose
    /// action triggered a workflow.
    fn acting_principal(&self) -> Option<AuthorizationActor> {
        if let Some(actor) = self.authorization_actor {
            return Some(actor);
        }
        if let Some(audit) = &self.audit_context
            && let Some(user_id) = audit.actor_user_id
        {
            return Some(AuthorizationActor {
                user_id,
                token_id: audit.actor_token_id,
            });
        }
        let event = self.event_context.as_ref()?;
        Some(AuthorizationActor {
            user_id: event.initiating_actor_user_id?,
            token_id: event.initiating_actor_token_id,
        })
    }

    async fn transition_denial(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
        requirements: &TransitionRequirements,
        actor: Option<AuthorizationActor>,
    ) -> Result<Option<RepositoryError>, RepositoryError> {
        if !requirements.is_restricted() {
            return Ok(None);
        }
        let forbidden = |reason: String| {
            RepositoryError::StatusTransitionForbidden(Box::new(StatusTransitionDenial {
                attribute: change.attribute_code.clone(),
                context: change.context_code.clone(),
                from: label(&change.before),
                to: label(&change.after),
                reason,
            }))
        };
        let Some(actor) = actor else {
            return Ok(Some(forbidden(
                "the transition requires an identified user".to_owned(),
            )));
        };
        if let Some(permission) = &requirements.permission {
            match self
                .ensure_principal_may(transaction, actor, permission, entity.id)
                .await
            {
                Ok(()) => {}
                Err(RepositoryError::ActorNotAuthorized) => {
                    return Ok(Some(forbidden(format!(
                        "requires the '{permission}' permission"
                    ))));
                }
                Err(error) => return Err(error),
            }
        }
        if !requirements.roles.is_empty() {
            let holds: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id = m.user_id JOIN workspaces w ON w.id = m.workspace_id JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN roles r ON r.id = g.role_id WHERE m.user_id = $1 AND m.workspace_id = $2 AND m.state = 'active' AND u.state = 'active' AND w.deleted_at IS NULL AND (r.is_system OR r.workspace_id = $2) AND r.code = ANY($3) AND ((g.scope_type = 'workspace' AND g.scope_target_id = $2) OR (g.scope_type = 'entity' AND g.scope_target_id = $4) OR (g.scope_type = 'blueprint_family' AND g.scope_target_id = $5)))",
            )
            .bind(actor.user_id)
            .bind(self.workspace_id.0)
            .bind(&requirements.roles)
            .bind(entity.id)
            .bind(entity.blueprint_id)
            .fetch_one(&mut **transaction)
            .await?;
            if !holds {
                return Ok(Some(forbidden(format!(
                    "requires the role {}",
                    requirements
                        .roles
                        .iter()
                        .map(|role| format!("'{role}'"))
                        .collect::<Vec<_>>()
                        .join(" or ")
                ))));
            }
        }
        for edge in &requirements.separate_from {
            let previous: Option<Option<Uuid>> = sqlx::query_scalar(
                "SELECT actor_user_id FROM entity_status_transitions WHERE workspace_id = $1 AND entity_id = $2 AND attribute_code = $3 AND context_id = $4 AND edge_code = $5 AND kind = 'transition' ORDER BY occurred_at DESC, id DESC LIMIT 1",
            )
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(&change.attribute_code)
            .bind(change.context_id)
            .bind(edge)
            .fetch_optional(&mut **transaction)
            .await?;
            if previous == Some(Some(actor.user_id)) {
                return Ok(Some(RepositoryError::StatusSeparationOfDuties {
                    attribute: change.attribute_code.clone(),
                    context: change.context_code.clone(),
                    edge: edge.clone(),
                }));
            }
        }
        Ok(None)
    }

    /// Rejects changes to attributes locked by a context's status at the start
    /// of the transaction. Moving into a locked status may carry final edits;
    /// leaving one must be a pure status change through an explicit edge.
    async fn check_status_locks(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attributes: &[StatusAttribute],
        contexts: &[ContextNode],
    ) -> Result<(), RepositoryError> {
        let locks = Self::active_locks(entity, attributes, contexts)?;
        if locks.is_empty() {
            return Ok(());
        }
        let before = self
            .record_content(transaction, entity.id, RecordState::Before)
            .await?;
        let after = self
            .record_content(transaction, entity.id, RecordState::After)
            .await?;
        let inheritance = Self::attribute_inheritance(transaction, entity).await?;
        for (context, status_code, status, coverage) in locks {
            let codes: BTreeSet<&str> = match &coverage {
                StatusCoverage::All => before
                    .values()
                    .chain(after.values())
                    .flat_map(|values| values.keys().map(String::as_str))
                    .collect(),
                StatusCoverage::Attributes(codes) => codes.iter().map(String::as_str).collect(),
            };
            for code in codes
                .into_iter()
                .filter(|code| coverage.covers(code, &status_code))
            {
                let inherit = inheritance.get(code).copied().unwrap_or(true);
                let path = context_path(contexts, context, inherit)?;
                if effective(&before, &path, code) != effective(&after, &path, code) {
                    return Err(RepositoryError::RecordLocked {
                        attribute: code.to_owned(),
                        context: context.code.clone(),
                        status: status.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// `(context, status attribute, status, coverage)` for every context whose
    /// saved effective status declares a lock.
    fn active_locks<'a>(
        entity: &Entity,
        attributes: &[StatusAttribute],
        contexts: &'a [ContextNode],
    ) -> Result<Vec<(&'a ContextNode, String, String, StatusCoverage)>, RepositoryError> {
        let projection = entity.projections.get("preview").unwrap_or(&Value::Null);
        let mut locks = Vec::new();
        for (_, code, schema, fallback) in attributes {
            if !has_record_controls(schema) {
                continue;
            }
            for context in contexts {
                let path = context_path(contexts, context, fallback != "none")?;
                let status = effective_projection(projection, &path, code);
                if let Some(coverage) = status_lock(schema, &status) {
                    locks.push((context, code.clone(), label(&status), coverage));
                }
            }
        }
        Ok(locks)
    }

    /// Rejects a direct write to `attribute_code` in `context_id` (file
    /// uploads, links and reorders, which bypass value reconstruction) when a
    /// status locks it in that context or a context that inherits from it.
    pub(super) async fn ensure_attribute_unlocked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attribute_code: &str,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        if !attributes
            .iter()
            .any(|(.., schema, _)| has_record_controls(schema))
        {
            return Ok(());
        }
        let contexts = self.status_contexts(transaction).await?;
        let inherit = Self::attribute_inheritance(transaction, entity)
            .await?
            .get(attribute_code)
            .copied()
            .unwrap_or(true);
        let Some(written) = contexts.iter().find(|context| context.id == context_id) else {
            return Err(RepositoryError::InvalidContext);
        };
        for (context, status_code, status, coverage) in
            Self::active_locks(entity, &attributes, &contexts)?
        {
            if coverage.covers(attribute_code, &status_code)
                && context_path(&contexts, context, inherit)?.contains(&written.code.as_str())
            {
                return Err(RepositoryError::RecordLocked {
                    attribute: attribute_code.to_owned(),
                    context: context.code.clone(),
                    status,
                });
            }
        }
        Ok(())
    }

    /// A record with any locked content cannot be deleted; a correction
    /// transition must unlock it first.
    pub(super) async fn ensure_entity_deletable(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        if !attributes
            .iter()
            .any(|(.., schema, _)| has_record_controls(schema))
        {
            return Ok(());
        }
        let contexts = self.status_contexts(transaction).await?;
        if let Some((context, _, status, _)) = Self::active_locks(entity, &attributes, &contexts)?
            .into_iter()
            .next()
        {
            return Err(RepositoryError::RecordLocked {
                attribute: "*".to_owned(),
                context: context.code.clone(),
                status,
            });
        }
        Ok(())
    }

    /// Records the transaction's status transitions, approvals and retention
    /// holds, and voids approvals whose covered content changed. Call once per
    /// write, after [`Self::validate_status_values`] and before the final
    /// projection and audit snapshot are built.
    pub(super) async fn apply_status_effects(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        let active_approvals: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM entity_approvals WHERE workspace_id = $1 AND entity_id = $2 AND ended_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .fetch_one(&mut **transaction)
        .await?;
        if attributes.is_empty() && active_approvals == 0 {
            return Ok(());
        }
        let contexts = self.status_contexts(transaction).await?;
        let changes = self
            .status_changes(transaction, entity, &attributes, &contexts)
            .await?;
        let actor = self.acting_principal();
        for change in &changes {
            let requirements =
                transition_requirements(&change.schema, &change.before, &change.after);
            let unlocked = status_lock(&change.schema, &change.before).is_some();
            self.record_status_transition(
                transaction,
                entity,
                change,
                requirements.code.as_deref(),
                "transition",
                unlocked,
            )
            .await?;
            if unlocked {
                self.write_control_audit_event(
                    transaction,
                    "entity.record.unlock",
                    entity.id,
                    serde_json::json!({
                        "attribute_code": change.attribute_code,
                        "context_code": change.context_code,
                        "from": change.before,
                        "to": change.after,
                        "transition_code": requirements.code,
                    }),
                )
                .await?;
            }
        }
        if changes.is_empty() && active_approvals == 0 {
            return Ok(());
        }
        let needs_content = active_approvals > 0
            || changes.iter().any(|change| {
                status_approval(&change.schema, &change.after).is_some()
                    || status_retention_days(&change.schema, &change.after).is_some()
            });
        if !needs_content {
            return Ok(());
        }
        let content = self
            .record_content(transaction, entity.id, RecordState::After)
            .await?;
        let inheritance = Self::attribute_inheritance(transaction, entity).await?;
        let digest = |context: &ContextNode, coverage: &StatusCoverage, status_code: &str| {
            content_digest(
                &content,
                &inheritance,
                &contexts,
                context,
                coverage,
                status_code,
            )
        };
        // New approvals supersede the previous decision for that status and context.
        for change in &changes {
            let Some(approval) = status_approval(&change.schema, &change.after) else {
                continue;
            };
            let context = contexts
                .iter()
                .find(|context| context.id == change.context_id)
                .expect("changes come from listed contexts");
            sqlx::query(
                "UPDATE entity_approvals SET ended_at = now(), end_reason = 'superseded', ended_by_user_id = $5 WHERE workspace_id = $1 AND entity_id = $2 AND attribute_code = $3 AND context_id = $4 AND ended_at IS NULL",
            )
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(&change.attribute_code)
            .bind(change.context_id)
            .bind(actor.map(|actor| actor.user_id))
            .execute(&mut **transaction)
            .await?;
            let covered = match &approval.covers {
                StatusCoverage::All => Vec::new(),
                StatusCoverage::Attributes(codes) => codes.clone(),
            };
            let content_digest = digest(context, &approval.covers, &change.attribute_code)?;
            let approval_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO entity_approvals (id, workspace_id, entity_id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest, approved_by_user_id, approved_by_token_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(approval_id)
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(&change.attribute_code)
            .bind(change.context_id)
            .bind(change.after.as_str())
            .bind(approval.covers == StatusCoverage::All)
            .bind(&covered)
            .bind(&content_digest)
            .bind(actor.map(|actor| actor.user_id))
            .bind(actor.and_then(|actor| actor.token_id))
            .execute(&mut **transaction)
            .await?;
            self.write_control_audit_event(
                transaction,
                "entity.approval.record",
                entity.id,
                serde_json::json!({
                    "approval_id": approval_id,
                    "attribute_code": change.attribute_code,
                    "context_code": change.context_code,
                    "status": change.after,
                    "content_digest": content_digest,
                }),
            )
            .await?;
        }
        self.void_changed_approvals(
            transaction,
            entity,
            &attributes,
            &contexts,
            &content,
            &inheritance,
        )
        .await?;
        for change in &changes {
            if let Some(days) = status_retention_days(&change.schema, &change.after) {
                let coverage =
                    status_lock(&change.schema, &change.after).expect("retention requires a lock");
                let context = contexts
                    .iter()
                    .find(|context| context.id == change.context_id)
                    .expect("changes come from listed contexts");
                self.place_status_retention_holds(
                    transaction,
                    entity,
                    change,
                    context,
                    &contexts,
                    &content,
                    &inheritance,
                    &coverage,
                    days,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Ends approvals whose covered content no longer matches their digest. A
    /// context still in the approved status moves to the declared `void_to`
    /// status in the same transaction, parents before children so an
    /// inheriting context is not given a redundant local value.
    async fn void_changed_approvals(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attributes: &[StatusAttribute],
        contexts: &[ContextNode],
        content: &RecordContent,
        inheritance: &HashMap<String, bool>,
    ) -> Result<(), RepositoryError> {
        let approvals = sqlx::query_as::<_, (Uuid, String, Uuid, String, bool, Vec<String>, String)>(
            "SELECT id, attribute_code, context_id, status, covers_all, covered_attributes, content_digest FROM entity_approvals WHERE workspace_id = $1 AND entity_id = $2 AND ended_at IS NULL",
        )
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .fetch_all(&mut **transaction)
        .await?;
        let mut voided = Vec::new();
        for (id, attribute_code, context_id, status, covers_all, covered, approved_digest) in
            approvals
        {
            let Some(context) = contexts.iter().find(|context| context.id == context_id) else {
                continue;
            };
            let coverage = if covers_all {
                StatusCoverage::All
            } else {
                StatusCoverage::Attributes(covered)
            };
            if content_digest(
                content,
                inheritance,
                contexts,
                context,
                &coverage,
                &attribute_code,
            )? == approved_digest
            {
                continue;
            }
            let attribute = attributes
                .iter()
                .find(|(_, code, ..)| code == &attribute_code);
            let void_to = attribute
                .and_then(|(_, _, schema, _)| {
                    status_approval(schema, &Value::String(status.clone()))
                })
                .map(|approval| approval.void_to);
            sqlx::query(
                "UPDATE entity_approvals SET ended_at = now(), end_reason = 'content_changed', ended_by_user_id = $2, void_status = $3 WHERE id = $1 AND ended_at IS NULL",
            )
            .bind(id)
            .bind(self.acting_principal().map(|actor| actor.user_id))
            .bind(&void_to)
            .execute(&mut **transaction)
            .await?;
            self.write_control_audit_event(
                transaction,
                "entity.approval.void",
                entity.id,
                serde_json::json!({
                    "approval_id": id,
                    "attribute_code": attribute_code,
                    "context_code": context.code,
                    "status": status,
                    "void_to": void_to,
                }),
            )
            .await?;
            if let (Some(attribute), Some(void_to)) = (attribute, void_to) {
                let depth = context_path(contexts, context, true)?.len();
                voided.push((depth, attribute.clone(), context, status, void_to));
            }
        }
        voided.sort_by_key(|(depth, ..)| *depth);
        for (_, (attribute_id, code, _, fallback), context, status, void_to) in voided {
            let projection = Self::build_preview_projection(transaction, entity.id).await?;
            let path = context_path(contexts, context, fallback != "none")?;
            let current = effective_projection(&projection, &path, &code);
            if current != Value::String(status.clone()) {
                continue;
            }
            self.insert_value(
                transaction,
                entity,
                NewAttributeValue::Scalar {
                    attribute_id: Some(attribute_id),
                    attribute_code: None,
                    context_id: Some(context.id),
                    value: Value::String(void_to.clone()),
                },
            )
            .await?;
            self.record_status_transition(
                transaction,
                entity,
                &StatusChange {
                    attribute_code: code,
                    schema: Value::Null,
                    context_id: context.id,
                    context_code: context.code.clone(),
                    before: Value::String(status),
                    after: Value::String(void_to),
                },
                None,
                "approval_void",
                false,
            )
            .await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn place_status_retention_holds(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
        context: &ContextNode,
        contexts: &[ContextNode],
        content: &RecordContent,
        inheritance: &HashMap<String, bool>,
        coverage: &StatusCoverage,
        days: i64,
    ) -> Result<(), RepositoryError> {
        let file_attributes: Vec<String> = sqlx::query_scalar(
            "SELECT code FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_type = 'file'",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(entity.id)
        .fetch_all(&mut **transaction)
        .await?;
        let actor = self.acting_principal();
        for code in file_attributes
            .iter()
            .filter(|code| coverage.covers(code, &change.attribute_code))
        {
            let inherit = inheritance.get(code).copied().unwrap_or(true);
            let files = effective(content, &context_path(contexts, context, inherit)?, code);
            for file_id in files
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|file| file["id"].as_str())
                .filter_map(|id| id.parse::<Uuid>().ok())
            {
                let hold_id = Uuid::new_v4();
                let held_until: DateTime<Utc> = sqlx::query_scalar(
                    "INSERT INTO file_retention_holds (id, workspace_id, file_id, source, entity_id, attribute_code, status, held_until, created_by_user_id) VALUES ($1, $2, $3, 'status', $4, $5, $6, now() + make_interval(days => $7), $8) RETURNING held_until",
                )
                .bind(hold_id)
                .bind(self.workspace_id.0)
                .bind(file_id)
                .bind(entity.id)
                .bind(code)
                .bind(change.after.as_str())
                .bind(days as i32)
                .bind(actor.map(|actor| actor.user_id))
                .fetch_one(&mut **transaction)
                .await?;
                self.write_control_audit_event(
                    transaction,
                    "file.retention_hold.place",
                    entity.id,
                    serde_json::json!({
                        "hold_id": hold_id,
                        "file_id": file_id,
                        "attribute_code": code,
                        "context_code": context.code,
                        "status": change.after,
                        "held_until": held_until,
                    }),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn record_status_transition(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
        edge_code: Option<&str>,
        kind: &str,
        unlocked: bool,
    ) -> Result<(), RepositoryError> {
        let actor = self.acting_principal();
        sqlx::query(
            "INSERT INTO entity_status_transitions (id, workspace_id, entity_id, attribute_code, context_id, from_status, to_status, edge_code, kind, unlocked, actor_user_id, actor_token_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(Uuid::new_v4())
        .bind(self.workspace_id.0)
        .bind(entity.id)
        .bind(&change.attribute_code)
        .bind(change.context_id)
        .bind(change.before.as_str())
        .bind(change.after.as_str())
        .bind(edge_code)
        .bind(kind)
        .bind(unlocked)
        .bind(actor.map(|actor| actor.user_id))
        .bind(actor.and_then(|actor| actor.token_id))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Adds a specific audit event to the current request's evidence, in the
    /// same transaction. Writes without an audit context are not audited.
    pub(super) async fn write_control_audit_event(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        action: &str,
        entity_id: Uuid,
        details: Value,
    ) -> Result<(), RepositoryError> {
        let Some(audit) = &self.audit_context else {
            return Ok(());
        };
        let mut metadata = audit.metadata.clone();
        if let Some(metadata) = metadata.as_object_mut() {
            metadata.insert("record_control".to_owned(), details);
        }
        let mut repository = self.clone();
        repository.audit_context = Some(AuditContext {
            action: action.to_owned(),
            target: serde_json::json!({"entity_id": entity_id}),
            metadata,
            ..audit.clone()
        });
        repository.write_audit_event(transaction).await?;
        Ok(())
    }

    /// The declared edges leaving each status attribute's current effective
    /// value in `context_id`, and whether `actor` may take each one now.
    pub async fn status_transition_access(
        &self,
        entity_id: Uuid,
        context_id: Option<Uuid>,
        actor: AuthorizationActor,
    ) -> Result<Vec<StatusTransitionAccess>, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let attributes = Self::status_attributes(&mut transaction, &entity).await?;
        let contexts = self.status_contexts(&mut transaction).await?;
        let context = match context_id {
            Some(id) => contexts.iter().find(|context| context.id == id),
            None => contexts.iter().find(|context| context.code == "default"),
        }
        .ok_or(RepositoryError::InvalidContext)?;
        let projection = entity.projections.get("preview").unwrap_or(&Value::Null);
        let mut access = Vec::new();
        for (_, code, schema, fallback) in &attributes {
            let path = context_path(&contexts, context, fallback != "none")?;
            let current = effective_projection(projection, &path, code);
            for edge in schema[STATUS_KEY]["transitions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|edge| edge["from"] == current)
            {
                let change = StatusChange {
                    attribute_code: code.clone(),
                    schema: schema.clone(),
                    context_id: context.id,
                    context_code: context.code.clone(),
                    before: current.clone(),
                    after: edge["to"].clone(),
                };
                let requirements = transition_requirements(schema, &change.before, &change.after);
                let denial = self
                    .transition_denial(
                        &mut transaction,
                        &entity,
                        &change,
                        &requirements,
                        Some(actor),
                    )
                    .await?;
                let mut denial_code = denial.as_ref().map(|error| match error {
                    RepositoryError::StatusSeparationOfDuties { .. } => {
                        "status_separation_of_duties"
                    }
                    _ => "status_transition_forbidden",
                });
                let mut denial_reason = denial.map(|error| error.to_string());
                let unmet = self
                    .transition_unmet(&mut transaction, &entity, &change)
                    .await?;
                if denial_code.is_none() && !unmet.is_empty() {
                    denial_code = Some("transition_conditions_unmet");
                    denial_reason = Some(
                        unmet
                            .iter()
                            .map(|violation| violation.message.clone())
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                }
                access.push(StatusTransitionAccess {
                    attribute_code: code.clone(),
                    from: current.as_str().map(str::to_owned),
                    to: change.after.as_str().map(str::to_owned),
                    code: requirements.code.clone(),
                    allowed: denial_code.is_none(),
                    denial_code,
                    denial_reason,
                    unmet,
                });
            }
        }
        transaction.rollback().await?;
        Ok(access)
    }

    /// Approval decisions recorded on an entity, newest first.
    pub async fn entity_approvals(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityApproval>, RepositoryError> {
        Ok(sqlx::query_as::<_, EntityApproval>(
            "SELECT a.id, a.attribute_code, a.context_id, c.code AS context_code, a.status, a.covers_all, a.covered_attributes, a.content_digest, a.approved_by_user_id, COALESCE(u.display_name, u.email) AS approved_by, a.approved_at, a.ended_at, a.end_reason, a.ended_by_user_id, a.void_status FROM entity_approvals a JOIN attribute_contexts c ON c.id = a.context_id LEFT JOIN users u ON u.id = a.approved_by_user_id WHERE a.workspace_id = $1 AND a.entity_id = $2 ORDER BY a.approved_at DESC, a.id DESC LIMIT 200",
        )
        .bind(self.workspace_id.0)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
