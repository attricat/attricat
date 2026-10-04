use super::PublicationReadiness;
use super::*;
use crate::domain_events::{ENTITY_PUBLISHED_V1, ENTITY_UNPUBLISHED_V1, EntityPublicationV1};
use crate::persistence_rows::{Db, IntoDomain};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// A required predicate: source, code, custom message, severity, predicate.
type GatePredicate = (
    super::CheckSource,
    String,
    Option<String>,
    Option<String>,
    catalog_validation::predicate::Predicate,
);

struct PublicationMutation<'a> {
    event_type: &'a str,
    entity_id: Uuid,
    context_id: Uuid,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    published_by_user_id: Option<Uuid>,
    reason: Option<&'a str>,
}

impl CatalogRepository {
    pub async fn list_publication_channels(
        &self,
    ) -> Result<Vec<PublicationChannel>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<PublicationChannel>>(
            "SELECT c.context_id, a.code AS context_code, c.enabled, c.required_rule_codes, c.require_valid_entity FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 ORDER BY a.code",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn set_publication_channel(
        &self,
        context_id: Uuid,
        enabled: bool,
    ) -> Result<PublicationChannel, RepositoryError> {
        self.update_publication_channel(
            context_id,
            crate::model::UpdatePublicationChannel {
                enabled,
                required_rule_codes: None,
                require_valid_entity: None,
            },
        )
        .await
    }

    /// Enables a channel and, optionally, replaces the checks it requires
    /// before publication. Omitted check settings are kept.
    pub async fn update_publication_channel(
        &self,
        context_id: Uuid,
        input: crate::model::UpdatePublicationChannel,
    ) -> Result<PublicationChannel, RepositoryError> {
        if let Some(codes) = &input.required_rule_codes {
            let unique: std::collections::HashSet<_> = codes.iter().collect();
            if codes.len() > 32
                || unique.len() != codes.len()
                || codes.iter().any(|code| !is_valid_code(code))
            {
                return Err(RepositoryError::InvalidRuleDefinition(
                    "required_rule_codes must be at most 32 unique rule codes".into(),
                ));
            }
        }
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let context = sqlx::query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 AND id = $2",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RepositoryError::InvalidContext)?
        .into_domain();
        let (required_rule_codes, require_valid_entity): (Vec<String>, bool) = sqlx::query_as("INSERT INTO publication_channels (workspace_id, context_id, enabled, required_rule_codes, require_valid_entity) VALUES ($1, $2, $3, COALESCE($4, '{}'::text[]), COALESCE($5, false)) ON CONFLICT (workspace_id, context_id) DO UPDATE SET enabled = EXCLUDED.enabled, required_rule_codes = COALESCE($4, publication_channels.required_rule_codes), require_valid_entity = COALESCE($5, publication_channels.require_valid_entity), updated_at = now() RETURNING required_rule_codes, require_valid_entity")
            .bind(workspace_id).bind(context_id).bind(input.enabled).bind(input.required_rule_codes).bind(input.require_valid_entity).fetch_one(&mut *tx).await?;
        self.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(PublicationChannel {
            context_id,
            context_code: context.code,
            enabled: input.enabled,
            required_rule_codes,
            require_valid_entity,
        })
    }

    /// Readiness of the entity for every enabled channel: the checks each
    /// channel requires, evaluated on the current state without publishing.
    pub async fn publication_readiness(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<PublicationReadiness>, RepositoryError> {
        let entity = self
            .get_entity(entity_id)
            .await?
            .ok_or(RepositoryError::NotFound("entity"))?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *tx)
            .await?;
        let channels = sqlx::query_as::<_, Db<PublicationChannel>>(
            "SELECT c.context_id, a.code AS context_code, c.enabled, c.required_rule_codes, c.require_valid_entity FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 AND c.enabled ORDER BY a.code",
        )
        .bind(self.workspace_id.0)
        .fetch_all(&mut *tx)
        .await?
        .into_domain();
        let mut readiness = Vec::with_capacity(channels.len());
        for channel in channels {
            let violations = self
                .publication_violations(&mut tx, &entity, &channel)
                .await?;
            readiness.push(PublicationReadiness {
                context_id: channel.context_id,
                context_code: channel.context_code,
                ready: violations.is_empty(),
                violations,
            });
        }
        tx.rollback().await?;
        Ok(readiness)
    }

    pub async fn publication_statuses(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        if self.get_entity(entity_id).await?.is_none() {
            return Err(RepositoryError::NotFound("entity"));
        }
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<EntityPublicationStatus>>(
            "SELECT c.context_id, a.code AS context_code, CASE WHEN p.published_at IS NULL THEN 'not_published' ELSE 'published' END AS status, p.published_at, p.published_by_user_id FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id LEFT JOIN entity_channel_publications p ON p.workspace_id = c.workspace_id AND p.entity_id = $2 AND p.context_id = c.context_id WHERE c.workspace_id = $1 AND c.enabled ORDER BY a.code",
        )
        .bind(workspace_id)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn publish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let status = self
            .publish_to_channel(&mut tx, entity_id, context_id)
            .await?;
        self.commit_publication_mutation(
            tx,
            PublicationMutation {
                event_type: ENTITY_PUBLISHED_V1,
                entity_id,
                context_id,
                published_at: status.published_at,
                published_by_user_id: status.published_by_user_id,
                reason: None,
            },
        )
        .await?;
        Ok(status)
    }

    pub async fn publish_entity_all_channels(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityPublicationStatus>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let channels: Vec<Uuid> = sqlx::query_scalar(
            "SELECT context_id FROM publication_channels WHERE workspace_id = $1 AND enabled ORDER BY context_id FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut statuses = Vec::with_capacity(channels.len());
        for context_id in channels {
            statuses.push(
                self.publish_to_channel(&mut tx, entity_id, context_id)
                    .await?,
            );
        }
        self.write_audit_event(&mut tx).await?;
        for status in &statuses {
            self.enqueue_event(
                &mut tx,
                self.publication_event(
                    ENTITY_PUBLISHED_V1,
                    entity_id,
                    status.context_id,
                    status.published_at,
                    status.published_by_user_id,
                    None,
                ),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(statuses)
    }

    pub async fn publish_blueprint_entities(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        context_id: Option<Uuid>,
    ) -> Result<BlueprintEntityPublicationSummary, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let blueprint_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3 AND kind = 'entity' AND deleted_at IS NULL)",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_one(&mut *tx)
        .await?;
        if !blueprint_exists {
            return Err(RepositoryError::NotFound("entity blueprint revision"));
        }
        let entity_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM entities WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL ORDER BY id FOR UPDATE",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_all(&mut *tx)
        .await?;
        let channel_ids: Vec<Uuid> = match context_id {
            Some(context_id) => vec![context_id],
            None => sqlx::query_scalar(
                "SELECT context_id FROM publication_channels WHERE workspace_id = $1 AND enabled ORDER BY context_id FOR UPDATE",
            )
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await?,
        };
        if let Some(context_id) = context_id {
            let enabled: bool = sqlx::query_scalar(
                "SELECT enabled FROM publication_channels WHERE workspace_id = $1 AND context_id = $2 FOR UPDATE",
            )
            .bind(workspace_id)
            .bind(context_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
            if !enabled {
                return Err(RepositoryError::PublicationChannelDisabled);
            }
        }
        let actor = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
            .ok_or(RepositoryError::PublicationActorRequired)?;
        // Gated channels evaluate every entity first; one failure rejects the
        // whole bulk publication with the failing entities in the evidence.
        for context_id in &channel_ids {
            let channel = sqlx::query_as::<_, Db<PublicationChannel>>(
                "SELECT c.context_id, a.code AS context_code, c.enabled, c.required_rule_codes, c.require_valid_entity FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 AND c.context_id = $2",
            )
            .bind(workspace_id)
            .bind(context_id)
            .fetch_optional(&mut *tx)
            .await?
            .into_domain();
            let Some(channel) = channel.filter(|channel| {
                channel.require_valid_entity || !channel.required_rule_codes.is_empty()
            }) else {
                continue;
            };
            let mut failures = Vec::new();
            for entity_id in &entity_ids {
                let entity = self.lock_entity(&mut tx, *entity_id).await?;
                for mut violation in self
                    .publication_violations(&mut tx, &entity, &channel)
                    .await?
                {
                    if failures.len() >= super::MAX_REPORTED_VIOLATIONS {
                        break;
                    }
                    if let Some(evidence) = violation.evidence.as_object_mut() {
                        evidence.insert("entity_id".into(), serde_json::json!(entity_id));
                    }
                    failures.push(violation);
                }
            }
            if !failures.is_empty() {
                return Err(RepositoryError::PublicationChecksFailed {
                    context: channel.context_code,
                    violations: failures,
                });
            }
        }
        let published_at = chrono::Utc::now();
        for entity_id in &entity_ids {
            for context_id in &channel_ids {
                sqlx::query("INSERT INTO entity_channel_publications (workspace_id, entity_id, context_id, published_at, published_by_user_id) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, entity_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
                    .bind(workspace_id)
                    .bind(entity_id)
                    .bind(context_id)
                    .bind(published_at)
                    .bind(actor)
                    .execute(&mut *tx)
                    .await?;
                self.enqueue_event(
                    &mut tx,
                    self.publication_event(
                        ENTITY_PUBLISHED_V1,
                        *entity_id,
                        *context_id,
                        Some(published_at),
                        Some(actor),
                        Some("blueprint_bulk"),
                    ),
                )
                .await?;
            }
        }
        self.write_audit_event(&mut tx).await?;
        tx.commit().await?;
        Ok(BlueprintEntityPublicationSummary {
            entity_count: entity_ids.len() as i64,
            channel_count: channel_ids.len() as i64,
            publication_count: (entity_ids.len() * channel_ids.len()) as i64,
        })
    }

    pub async fn unpublish_entity(
        &self,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        self.lock_entity(&mut tx, entity_id).await?;
        let updated = sqlx::query("UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND entity_id = $2 AND context_id = $3 AND published_at IS NOT NULL")
            .bind(workspace_id).bind(entity_id).bind(context_id).execute(&mut *tx).await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("active entity publication"));
        }
        self.commit_publication_mutation(
            tx,
            PublicationMutation {
                event_type: ENTITY_UNPUBLISHED_V1,
                entity_id,
                context_id,
                published_at: None,
                published_by_user_id: None,
                reason: Some("manual"),
            },
        )
        .await
    }

    pub(crate) async fn reconcile_entity_publication(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        reason: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let Some(actor_id) = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
        else {
            self.clear_entity_publications(tx, entity_id, reason)
                .await?;
            return Ok(None);
        };
        let workspace_id = self.workspace_id.0;
        let definition: Option<String> = sqlx::query_scalar(
            "SELECT b.definition FROM entities e JOIN blueprints b ON b.workspace_id = e.workspace_id AND b.id = e.blueprint_id AND b.version = e.blueprint_version WHERE e.workspace_id = $1 AND e.id = $2 AND e.deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(definition) = definition else {
            return Ok(None);
        };
        let roles = catalog_blueprint::parse(&definition)
            .map_err(RepositoryError::invalid_blueprint_definition)?
            .publication
            .retain_on_edit_roles;
        if roles.is_empty() {
            self.clear_entity_publications(tx, entity_id, reason)
                .await?;
            return Ok(None);
        }
        let retained_role: Option<String> = sqlx::query_scalar(
            "SELECT r.code FROM workspace_memberships m JOIN role_grants g ON g.membership_id = m.id AND g.workspace_id = m.workspace_id JOIN roles r ON r.id = g.role_id WHERE m.workspace_id = $1 AND m.user_id = $2 AND m.state = 'active' AND g.scope_type = 'workspace' AND g.scope_target_id = $1 AND r.code = ANY($3) AND (r.is_system OR r.workspace_id = $1) ORDER BY r.code LIMIT 1",
        )
        .bind(workspace_id)
        .bind(actor_id)
        .bind(&roles)
        .fetch_optional(&mut **tx)
        .await?;
        if retained_role.is_none() {
            self.clear_entity_publications(tx, entity_id, reason)
                .await?;
        }
        Ok(retained_role)
    }

    pub(crate) async fn clear_entity_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        reason: &str,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let contexts: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND entity_id = $2 AND published_at IS NOT NULL RETURNING context_id",
        )
        .bind(workspace_id)
        .bind(entity_id)
        .fetch_all(&mut **tx)
        .await?;
        for context_id in contexts {
            self.enqueue_event(
                tx,
                self.publication_event(
                    ENTITY_UNPUBLISHED_V1,
                    entity_id,
                    context_id,
                    None,
                    None,
                    Some(reason),
                ),
            )
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn clear_context_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let entities: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE entity_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND context_id = $2 AND published_at IS NOT NULL RETURNING entity_id",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_all(&mut **tx)
        .await?;
        for entity_id in entities {
            self.enqueue_event(
                tx,
                self.publication_event(
                    ENTITY_UNPUBLISHED_V1,
                    entity_id,
                    context_id,
                    None,
                    None,
                    Some("context_changed"),
                ),
            )
            .await?;
        }
        Ok(())
    }

    async fn publish_to_channel(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity_id: Uuid,
        context_id: Uuid,
    ) -> Result<EntityPublicationStatus, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let channel = sqlx::query_as::<_, Db<PublicationChannel>>(
            "SELECT c.context_id, a.code AS context_code, c.enabled, c.required_rule_codes, c.require_valid_entity FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1 AND c.context_id = $2 FOR UPDATE OF c",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?
        .filter(|channel| channel.enabled)
        .ok_or(RepositoryError::PublicationChannelDisabled)?
        .into_domain();
        let entity = self.lock_entity(tx, entity_id).await?;
        let violations = self.publication_violations(tx, &entity, &channel).await?;
        if !violations.is_empty() {
            return Err(RepositoryError::PublicationChecksFailed {
                context: channel.context_code,
                violations,
            });
        }
        let actor = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
            .ok_or(RepositoryError::PublicationActorRequired)?;
        let published_at = chrono::Utc::now();
        sqlx::query("INSERT INTO entity_channel_publications (workspace_id, entity_id, context_id, published_at, published_by_user_id) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, entity_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
            .bind(workspace_id).bind(entity_id).bind(context_id).bind(published_at).bind(actor).execute(&mut **tx).await?;
        Ok(EntityPublicationStatus {
            context_id,
            context_code: channel.context_code,
            status: "published".to_owned(),
            published_at: Some(published_at),
            published_by_user_id: Some(actor),
        })
    }

    /// Evaluates a channel's required checks for one entity in the channel
    /// context. Required rules that are not enabled for the entity's
    /// blueprint revision, or that are scoped to another context, do not apply.
    pub(super) async fn publication_violations(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        channel: &PublicationChannel,
    ) -> Result<Vec<super::CheckViolation>, RepositoryError> {
        use super::checks::{self, CheckScope, CheckSource};
        if !channel.require_valid_entity && channel.required_rule_codes.is_empty() {
            return Ok(Vec::new());
        }
        let workspace_id = self.workspace_id.0;
        let scope = CheckScope::load(tx, workspace_id).await?;
        let path = scope.path(channel.context_id)?;
        let subject = checks::load_entities(tx, workspace_id, &[entity.id])
            .await?
            .remove(&entity.id)
            .ok_or(RepositoryError::NotFound("entity"))?;
        let mut violations = Vec::new();
        let mut predicates: Vec<GatePredicate> = Vec::new();
        if channel.require_valid_entity {
            let entity_schema: Option<Value> = sqlx::query_scalar(
                "SELECT entity_schema FROM blueprints WHERE id = $1 AND version = $2",
            )
            .bind(entity.blueprint_id)
            .bind(entity.blueprint_version)
            .fetch_one(&mut **tx)
            .await?;
            if let Some(schema) = entity_schema {
                let record = subject.resolve(&path);
                for error in catalog_validation::validate_json_schema(
                    &schema,
                    &Value::Object(record.values.clone()),
                )
                .map_err(RepositoryError::InvalidBlueprintDefinition)?
                .into_iter()
                .take(10)
                {
                    let attribute = error
                        .instance_path
                        .trim_start_matches('/')
                        .split('/')
                        .next()
                        .filter(|segment| !segment.is_empty())
                        .map(str::to_owned);
                    violations.push(super::CheckViolation {
                        source: CheckSource::EntitySchema,
                        code: "entity_schema".to_owned(),
                        message: error.message,
                        contexts: vec![channel.context_code.clone()],
                        attributes: attribute.into_iter().collect(),
                        severity: None,
                        transition: None,
                        evidence: serde_json::json!({ "instance_path": error.instance_path }),
                    });
                }
                for check in catalog_validation::predicate::entity_checks(&schema)
                    .map_err(RepositoryError::InvalidBlueprintDefinition)?
                {
                    predicates.push((
                        CheckSource::EntityCheck,
                        check.code,
                        check.message,
                        None,
                        check.predicate,
                    ));
                }
            }
        }
        if !channel.required_rule_codes.is_empty() {
            for rule in checks::enabled_rules(
                tx,
                workspace_id,
                entity.blueprint_id,
                entity.blueprint_version,
            )
            .await?
            {
                if channel.required_rule_codes.contains(&rule.code)
                    && rule
                        .context_id
                        .is_none_or(|context| context == channel.context_id)
                {
                    let severity = serde_json::to_value(&rule.compiled.severity)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned));
                    predicates.push((
                        CheckSource::Rule,
                        rule.code,
                        None,
                        severity,
                        rule.compiled.predicate,
                    ));
                }
            }
        }
        let references: Vec<&catalog_validation::predicate::Predicate> =
            predicates.iter().map(|item| &item.4).collect();
        let outcomes =
            checks::evaluate_in_context(tx, &scope, &subject, channel.context_id, &[], &references)
                .await?;
        for ((source, code, message, severity, _), outcome) in predicates.iter().zip(outcomes) {
            if let Err(failure) = outcome {
                violations.push(super::CheckViolation {
                    source: *source,
                    code: code.clone(),
                    message: message.clone().unwrap_or(failure.message),
                    contexts: vec![channel.context_code.clone()],
                    attributes: failure.attributes,
                    severity: severity.clone(),
                    transition: None,
                    evidence: failure.evidence,
                });
            }
        }
        Ok(violations)
    }

    async fn commit_publication_mutation(
        &self,
        mut tx: Transaction<'_, Postgres>,
        mutation: PublicationMutation<'_>,
    ) -> Result<(), RepositoryError> {
        self.write_audit_event(&mut tx).await?;
        self.enqueue_event(
            &mut tx,
            self.publication_event(
                mutation.event_type,
                mutation.entity_id,
                mutation.context_id,
                mutation.published_at,
                mutation.published_by_user_id,
                mutation.reason,
            ),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    fn publication_event(
        &self,
        event_type: &str,
        entity_id: Uuid,
        context_id: Uuid,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
        published_by_user_id: Option<Uuid>,
        reason: Option<&str>,
    ) -> crate::domain_events::NewDomainEvent {
        self.core_event(
            event_type,
            "entity",
            entity_id,
            serde_json::to_value(EntityPublicationV1 {
                entity_id,
                context_id,
                published_at,
                published_by_user_id,
                reason: reason.map(str::to_owned),
            })
            .expect("publication event serializable"),
        )
    }
}

impl<S: super::RepositoryScope> CatalogRepository<S> {
    /// Installs publication permissions in application code, preserving the
    /// declarative-only migration contract. Owners and administrators receive
    /// publication authority; editors deliberately do not.
    pub async fn ensure_entity_publication_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('entities.publish', 'Publish catalog entities to channel contexts') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'entities.publish') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
