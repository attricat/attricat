use super::checks::{CheckScope, CheckTransition, enabled_rules, transition_unmet};
use super::record_values::{
    ContextNode, ContextTree, RecordState, RecordValues, attribute_codes, load_record,
    resolve_on_path,
};
use super::*;
use catalog_validation::status::{
    STATUS_KEY, StatusCoverage, TransitionRequirements, has_record_controls, status_approval,
    status_lock, status_retention_days, transition_edges, transition_requirements,
    validate_status_transition,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// A live status attribute of an entity's blueprint revision or one of its
/// own additional attributes.
#[derive(Clone, Debug, sqlx::FromRow)]
struct StatusAttribute {
    id: Uuid,
    code: String,
    #[sqlx(rename = "value_schema")]
    schema: Value,
    /// `context_fallback` is not `none`.
    inherit: bool,
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
    pub(super) fn transition(&self) -> CheckTransition {
        CheckTransition {
            attribute_code: self.attribute_code.clone(),
            from: self.before.as_str().map(str::to_owned),
            to: self.after.as_str().map(str::to_owned),
        }
    }
}

/// How a row of `entity_status_transitions` came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionKind {
    /// A write moved the status along a declared edge.
    Transition,
    /// Changed content voided an approval and moved to its `void_to` status.
    ApprovalVoid,
}

impl TransitionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Transition => "transition",
            Self::ApprovalVoid => "approval_void",
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
    pub denial_code: Option<super::ErrorCode>,
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

/// A context whose saved effective status locks some of the record.
struct ActiveLock<'a> {
    context: &'a ContextNode,
    /// The status attribute declaring the lock.
    attribute_code: &'a str,
    status: String,
    coverage: StatusCoverage,
}

/// Every context with the path its values are inherited along.
fn context_paths(tree: &ContextTree) -> Result<Vec<(&ContextNode, Vec<Uuid>)>, RepositoryError> {
    tree.nodes()
        .iter()
        .map(|context| Ok((context, tree.path(context.id, true)?)))
        .collect()
}

/// The effective value of `code` in the stored preview projection (keyed by
/// context code) for the context `path`.
fn effective_projection(
    projection: &Value,
    tree: &ContextTree,
    path: &[Uuid],
    inherit: bool,
    code: &str,
) -> Value {
    resolve_on_path(path, inherit, |context| {
        projection
            .get(tree.code(context))
            .and_then(|values| values.get(code))
    })
    .map(|(_, value)| value.clone())
    .unwrap_or(Value::Null)
}

/// The resolved value of `code` for the context `path`, `None` when absent.
fn effective<'a>(record: &'a RecordValues, path: &[Uuid], code: &str) -> Option<&'a Value> {
    record.resolve(code, path).map(|direct| &direct.value)
}

/// SHA-256 of the covered effective content for the context `path`, as
/// canonical JSON with sorted keys. Absent values are omitted.
fn content_digest(
    record: &RecordValues,
    path: &[Uuid],
    coverage: &StatusCoverage,
    status_code: &str,
) -> String {
    let codes: BTreeSet<&str> = match coverage {
        StatusCoverage::All => record.attributes.keys().map(String::as_str).collect(),
        StatusCoverage::Attributes(codes) => codes.iter().map(String::as_str).collect(),
    };
    let document: Map<String, Value> = codes
        .into_iter()
        .filter(|code| coverage.covers(code, status_code))
        .filter_map(|code| Some((code.to_owned(), effective(record, path, code)?.clone())))
        .collect();
    format!(
        "{:x}",
        Sha256::digest(Value::Object(document).to_string().as_bytes())
    )
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
            "SELECT id, code, value_schema, context_fallback <> 'none' AS inherit FROM attributes WHERE ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL AND value_schema ? $4",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(entity.id)
        .bind(STATUS_KEY)
        .fetch_all(&mut **transaction)
        .await?)
    }

    /// The entity's live values; an entity without values is empty.
    async fn record_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        state: RecordState,
    ) -> Result<RecordValues, RepositoryError> {
        Ok(
            load_record(transaction, self.workspace_id.0, entity.id, state)
                .await?
                .unwrap_or_else(|| {
                    RecordValues::empty(entity.id, entity.blueprint_id, entity.blueprint_version)
                }),
        )
    }

    pub(super) async fn has_status_writes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        values: &[NewAttributeValue],
        removed: &[AttributeValueSelector],
    ) -> Result<bool, RepositoryError> {
        let statuses = Self::status_attributes(transaction, entity).await?;
        Ok(statuses.iter().any(|status| {
            removed
                .iter()
                .any(|selector| selector.attribute_code == status.code)
                || values.iter().any(|value| {
                    matches!(value, NewAttributeValue::Scalar { attribute_id, attribute_code, .. }
                        if *attribute_id == Some(status.id) || attribute_code.as_ref() == Some(&status.code))
                })
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
        tree: &ContextTree,
    ) -> Result<Vec<StatusChange>, RepositoryError> {
        let after = Self::build_preview_projection(transaction, entity.id).await?;
        let before = entity.projections.get("preview").unwrap_or(&Value::Null);
        let paths = context_paths(tree)?;
        let mut changes = Vec::new();
        for attribute in attributes {
            for (context, path) in &paths {
                let resolve = |projection| {
                    effective_projection(projection, tree, path, attribute.inherit, &attribute.code)
                };
                let (before, after) = (resolve(before), resolve(&after));
                validate_status_transition(&attribute.schema, &before, &after).map_err(
                    |message| RepositoryError::AttributeValueSchemaMismatch {
                        attribute: attribute.code.clone(),
                        instance_path: String::new(),
                        message: format!("{message} (context: {})", context.code),
                    },
                )?;
                if before != after {
                    changes.push(StatusChange {
                        attribute_code: attribute.code.clone(),
                        schema: attribute.schema.clone(),
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

    /// Run once on the final transaction state, using the locked entity's saved
    /// projection as the baseline. Checks transition edges, their permission,
    /// role and separation-of-duties requirements, and status locks. It has no
    /// side effects; [`Self::apply_status_effects_in`] records the outcome.
    pub(super) async fn validate_status_values(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
    ) -> Result<(), RepositoryError> {
        let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
        self.checked_status_changes(transaction, entity, &tree)
            .await
            .map(drop)
    }

    /// [`Self::validate_status_values`] returning the validated changes. They
    /// are the write's own transitions: system approval voids applied later
    /// by [`Self::apply_status_effects_in`] are not among them.
    pub(super) async fn checked_status_changes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        tree: &ContextTree,
    ) -> Result<Vec<StatusChange>, RepositoryError> {
        let attributes = Self::status_attributes(transaction, entity).await?;
        if attributes.is_empty() {
            return Ok(Vec::new());
        }
        let changes = self
            .status_changes(transaction, entity, &attributes, tree)
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
        self.check_status_locks(transaction, entity, &attributes, tree)
            .await?;
        Ok(changes)
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
                .ensure_principal_may(transaction, actor, permission, &[entity.id])
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
            let holds = Self::is_granted_on(
                transaction,
                actor.user_id,
                self.workspace_id.0,
                super::GrantCodes::Roles(&requirements.roles),
                Some(entity.id),
                None,
            )
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
                "SELECT actor_user_id FROM entity_status_transitions WHERE workspace_id = $1 AND entity_id = $2 AND attribute_code = $3 AND context_id = $4 AND edge_code = $5 AND kind = $6 ORDER BY occurred_at DESC, id DESC LIMIT 1",
            )
            .bind(self.workspace_id.0)
            .bind(entity.id)
            .bind(&change.attribute_code)
            .bind(change.context_id)
            .bind(edge)
            .bind(TransitionKind::Transition.as_str())
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
        tree: &ContextTree,
    ) -> Result<(), RepositoryError> {
        let locks = Self::active_locks(entity, attributes, tree)?;
        if locks.is_empty() {
            return Ok(());
        }
        let before = self
            .record_values(transaction, entity, RecordState::Before)
            .await?;
        let after = self
            .record_values(transaction, entity, RecordState::After)
            .await?;
        let all_codes = attribute_codes(&[&before, &after]);
        for lock in locks {
            let path = tree.path(lock.context.id, true)?;
            let codes: Vec<&str> = match &lock.coverage {
                StatusCoverage::All => all_codes.clone(),
                StatusCoverage::Attributes(codes) => codes.iter().map(String::as_str).collect(),
            };
            if let Some(code) = codes.into_iter().find(|code| {
                lock.coverage.covers(code, lock.attribute_code)
                    && effective(&before, &path, code) != effective(&after, &path, code)
            }) {
                return Err(RepositoryError::RecordLocked {
                    attribute: code.to_owned(),
                    context: lock.context.code.clone(),
                    status: lock.status,
                });
            }
        }
        Ok(())
    }

    /// Every context whose saved effective status declares a lock.
    fn active_locks<'a>(
        entity: &Entity,
        attributes: &'a [StatusAttribute],
        tree: &'a ContextTree,
    ) -> Result<Vec<ActiveLock<'a>>, RepositoryError> {
        let projection = entity.projections.get("preview").unwrap_or(&Value::Null);
        let mut locks = Vec::new();
        let controlled: Vec<&StatusAttribute> = attributes
            .iter()
            .filter(|attribute| has_record_controls(&attribute.schema))
            .collect();
        if controlled.is_empty() {
            return Ok(locks);
        }
        let paths = context_paths(tree)?;
        for attribute in controlled {
            for (context, path) in &paths {
                let status = effective_projection(
                    projection,
                    tree,
                    path,
                    attribute.inherit,
                    &attribute.code,
                );
                if let Some(coverage) = status_lock(&attribute.schema, &status) {
                    locks.push(ActiveLock {
                        context,
                        attribute_code: &attribute.code,
                        status: label(&status),
                        coverage,
                    });
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
            .any(|attribute| has_record_controls(&attribute.schema))
        {
            return Ok(());
        }
        let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
        let inherit = sqlx::query_scalar::<_, String>(
            "SELECT context_fallback FROM attributes WHERE code = $4 AND ((blueprint_id = $1 AND blueprint_version = $2) OR entity_id = $3) AND deleted_at IS NULL",
        )
        .bind(entity.blueprint_id)
        .bind(entity.blueprint_version)
        .bind(entity.id)
        .bind(attribute_code)
        .fetch_optional(&mut **transaction)
        .await?
        .is_none_or(|fallback| fallback != "none");
        if tree.get(context_id).is_none() {
            return Err(RepositoryError::InvalidContext);
        }
        for lock in Self::active_locks(entity, &attributes, &tree)? {
            if lock.coverage.covers(attribute_code, lock.attribute_code)
                && tree.path(lock.context.id, inherit)?.contains(&context_id)
            {
                return Err(RepositoryError::RecordLocked {
                    attribute: attribute_code.to_owned(),
                    context: lock.context.code.clone(),
                    status: lock.status,
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
            .any(|attribute| has_record_controls(&attribute.schema))
        {
            return Ok(());
        }
        let tree = ContextTree::load(transaction, self.workspace_id.0).await?;
        if let Some(lock) = Self::active_locks(entity, &attributes, &tree)?
            .into_iter()
            .next()
        {
            return Err(RepositoryError::RecordLocked {
                attribute: "*".to_owned(),
                context: lock.context.code.clone(),
                status: lock.status,
            });
        }
        Ok(())
    }

    /// Records the transaction's status transitions, approvals and retention
    /// holds, and voids approvals whose covered content changed. Call once per
    /// write, after [`Self::checked_status_changes`] and before the final
    /// projection and audit snapshot are built.
    pub(super) async fn apply_status_effects_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        tree: &ContextTree,
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
        let changes = self
            .status_changes(transaction, entity, &attributes, tree)
            .await?;
        self.record_write_transitions(transaction, entity, &changes)
            .await?;
        let needs_content = active_approvals > 0
            || changes.iter().any(|change| {
                status_approval(&change.schema, &change.after).is_some()
                    || status_retention_days(&change.schema, &change.after).is_some()
            });
        if !needs_content {
            return Ok(());
        }
        let content = self
            .record_values(transaction, entity, RecordState::After)
            .await?;
        self.record_approvals(transaction, entity, &changes, tree, &content)
            .await?;
        self.void_changed_approvals(transaction, entity, &attributes, tree, &content)
            .await?;
        for change in &changes {
            self.place_status_retention_holds(transaction, entity, change, tree, &content)
                .await?;
        }
        Ok(())
    }

    /// Records the write's own transitions, auditing those that leave a
    /// locked status.
    async fn record_write_transitions(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        changes: &[StatusChange],
    ) -> Result<(), RepositoryError> {
        for change in changes {
            let requirements =
                transition_requirements(&change.schema, &change.before, &change.after);
            let unlocked = status_lock(&change.schema, &change.before).is_some();
            self.record_status_transition(
                transaction,
                entity,
                change,
                requirements.code.as_deref(),
                TransitionKind::Transition,
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
        Ok(())
    }

    /// Records an approval for every change into a status that declares one,
    /// superseding the previous decision for that status attribute and context.
    async fn record_approvals(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        changes: &[StatusChange],
        tree: &ContextTree,
        content: &RecordValues,
    ) -> Result<(), RepositoryError> {
        let actor = self.acting_principal();
        for change in changes {
            let Some(approval) = status_approval(&change.schema, &change.after) else {
                continue;
            };
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
            let content_digest = content_digest(
                content,
                &tree.path(change.context_id, true)?,
                &approval.covers,
                &change.attribute_code,
            );
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
        Ok(())
    }

    /// Ends approvals whose covered content no longer matches their digest. A
    /// context still in the approved status moves to the declared `void_to`
    /// status in the same transaction, parents before children so an
    /// inheriting context is not given a redundant local value. These system
    /// transitions bypass edge restrictions, transition conditions and
    /// guarding rules: the write's own changes were checked before them.
    async fn void_changed_approvals(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        attributes: &[StatusAttribute],
        tree: &ContextTree,
        content: &RecordValues,
    ) -> Result<(), RepositoryError> {
        struct Voided<'a> {
            depth: usize,
            attribute: &'a StatusAttribute,
            context: &'a ContextNode,
            status: String,
            void_to: String,
        }
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
            let Some(context) = tree.get(context_id) else {
                continue;
            };
            let coverage = if covers_all {
                StatusCoverage::All
            } else {
                StatusCoverage::Attributes(covered)
            };
            let path = tree.path(context_id, true)?;
            if content_digest(content, &path, &coverage, &attribute_code) == approved_digest {
                continue;
            }
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.code == attribute_code);
            let void_to = attribute
                .and_then(|attribute| {
                    status_approval(&attribute.schema, &Value::String(status.clone()))
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
                voided.push(Voided {
                    depth: tree.depth(context_id)?,
                    attribute,
                    context,
                    status,
                    void_to,
                });
            }
        }
        voided.sort_by_key(|voided| voided.depth);
        for voided in voided {
            let Voided {
                attribute,
                context,
                status,
                void_to,
                ..
            } = voided;
            // An earlier void may already have moved an inheriting context.
            let projection = Self::build_preview_projection(transaction, entity.id).await?;
            let path = tree.path(context.id, true)?;
            let current =
                effective_projection(&projection, tree, &path, attribute.inherit, &attribute.code);
            if current.as_str() != Some(status.as_str()) {
                continue;
            }
            self.insert_value(
                transaction,
                entity,
                NewAttributeValue::Scalar {
                    attribute_id: Some(attribute.id),
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
                    attribute_code: attribute.code.clone(),
                    schema: Value::Null,
                    context_id: context.id,
                    context_code: context.code.clone(),
                    before: Value::String(status),
                    after: Value::String(void_to),
                },
                None,
                TransitionKind::ApprovalVoid,
                false,
            )
            .await?;
        }
        Ok(())
    }

    /// Holds the files covered by the change's destination status for its
    /// `retention_days`. A status that declares retention without a lock is a
    /// malformed definition and fails closed.
    async fn place_status_retention_holds(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
        tree: &ContextTree,
        content: &RecordValues,
    ) -> Result<(), RepositoryError> {
        let Some(days) = status_retention_days(&change.schema, &change.after) else {
            return Ok(());
        };
        let coverage = status_lock(&change.schema, &change.after).ok_or_else(|| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "status '{}' of '{}' declares retention_days without a lock",
                label(&change.after),
                change.attribute_code
            ))
        })?;
        let actor = self.acting_principal();
        let path = tree.path(change.context_id, true)?;
        let file_ids: Vec<(&String, Uuid)> = content
            .attributes
            .iter()
            .filter(|(code, attribute)| {
                attribute.value_type == "file" && coverage.covers(code, &change.attribute_code)
            })
            .flat_map(|(code, _)| {
                effective(content, &path, code)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|file| file["id"].as_str()?.parse::<Uuid>().ok())
                    .map(move |file_id| (code, file_id))
            })
            .collect();
        for (code, file_id) in file_ids {
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
                    "context_code": change.context_code,
                    "status": change.after,
                    "held_until": held_until,
                }),
            )
            .await?;
        }
        Ok(())
    }

    async fn record_status_transition(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        change: &StatusChange,
        edge_code: Option<&str>,
        kind: TransitionKind,
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
        .bind(kind.as_str())
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
        let tree = ContextTree::load(&mut transaction, self.workspace_id.0).await?;
        let context = match context_id {
            Some(id) => tree.get(id).ok_or(RepositoryError::InvalidContext)?,
            None => tree.default_context()?,
        };
        if attributes.is_empty() {
            return Ok(Vec::new());
        }
        let path = tree.path(context.id, true)?;
        let projection = entity.projections.get("preview").unwrap_or(&Value::Null);
        // Every edge is evaluated against the same saved state and rules.
        let rules = enabled_rules(
            &mut transaction,
            self.workspace_id.0,
            entity.blueprint_id,
            entity.blueprint_version,
        )
        .await?;
        let subject = self
            .record_values(&mut transaction, &entity, RecordState::After)
            .await?;
        let scope = CheckScope::new(self.workspace_id.0, tree.clone());
        let mut access = Vec::new();
        for attribute in &attributes {
            let current =
                effective_projection(projection, &tree, &path, attribute.inherit, &attribute.code);
            for edge in transition_edges(&attribute.schema)
                .into_iter()
                .flatten()
                .filter(|edge| edge.from.as_deref() == current.as_str())
            {
                let change = StatusChange {
                    attribute_code: attribute.code.clone(),
                    schema: attribute.schema.clone(),
                    context_id: context.id,
                    context_code: context.code.clone(),
                    before: current.clone(),
                    after: edge.to.clone().map_or(Value::Null, Value::String),
                };
                let requirements =
                    transition_requirements(&attribute.schema, &change.before, &change.after);
                let denial = self
                    .transition_denial(
                        &mut transaction,
                        &entity,
                        &change,
                        &requirements,
                        Some(actor),
                    )
                    .await?;
                let mut denial_code = denial.as_ref().map(RepositoryError::code);
                let mut denial_reason = denial.map(|error| error.to_string());
                let unmet =
                    transition_unmet(&mut transaction, &scope, &subject, &rules, &change).await?;
                if denial_code.is_none() && !unmet.is_empty() {
                    denial_code = Some(super::ErrorCode::TransitionConditionsUnmet);
                    denial_reason = Some(
                        unmet
                            .iter()
                            .map(|violation| violation.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                }
                access.push(StatusTransitionAccess {
                    attribute_code: attribute.code.clone(),
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
