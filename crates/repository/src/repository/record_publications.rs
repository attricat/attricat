use super::PublicationReadiness;
use super::checks::{
    CheckScope, CheckSource, enabled_rules, evaluate_in_context, load_current_records,
    record_checks,
};
use super::record_values::RecordValues;
use super::*;
use crate::domain_events::{RECORD_PUBLISHED_V1, RECORD_UNPUBLISHED_V1, RecordPublicationV1};
use crate::persistence_rows::{Db, IntoDomain};
use attricat_validation::predicate::Predicate;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

/// Channel columns as `PublicationChannel` reads them; `$1` is the workspace.
const CHANNEL_SELECT: &str = "SELECT c.context_id, a.code AS context_code, c.enabled, c.required_rule_codes, c.require_valid_record FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id WHERE c.workspace_id = $1";

/// JSON-schema errors reported per record and channel.
const MAX_SCHEMA_VIOLATIONS: usize = 10;

/// One predicate a channel requires before publication.
struct GateCheck {
    source: CheckSource,
    code: String,
    /// Replaces the predicate's own failure message.
    message: Option<String>,
    severity: Option<attricat_rules::Severity>,
    predicate: Predicate,
}

/// A channel's required checks for one blueprint revision, loaded once and
/// evaluated for each record of that revision.
struct PublicationGate {
    context_id: Uuid,
    context_code: String,
    /// The JSON record schema, when the channel requires a valid record.
    record_schema: Option<Value>,
    checks: Vec<GateCheck>,
}

impl PublicationGate {
    /// The gate of `channel` for `(blueprint_id, blueprint_version)`, or
    /// `None` when the channel requires no checks. Required rules that are not
    /// enabled for the revision, or that are scoped to another context, do
    /// not apply.
    async fn load(
        conn: &mut PgConnection,
        workspace_id: Uuid,
        channel: &PublicationChannel,
        (blueprint_id, blueprint_version): (Uuid, i64),
    ) -> Result<Option<Self>, RepositoryError> {
        if !channel.require_valid_record && channel.required_rule_codes.is_empty() {
            return Ok(None);
        }
        let mut gate = Self {
            context_id: channel.context_id,
            context_code: channel.context_code.clone(),
            record_schema: None,
            checks: Vec::new(),
        };
        if channel.require_valid_record {
            gate.record_schema = sqlx::query_scalar(
                "SELECT record_schema FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3",
            )
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .fetch_one(&mut *conn)
            .await?;
            gate.checks
                .extend(
                    record_checks(gate.record_schema.as_ref())?
                        .into_iter()
                        .map(|check| GateCheck {
                            source: CheckSource::RecordCheck,
                            code: check.code,
                            message: check.message,
                            severity: None,
                            predicate: check.predicate,
                        }),
                );
        }
        if !channel.required_rule_codes.is_empty() {
            gate.checks.extend(
                enabled_rules(conn, workspace_id, blueprint_id, blueprint_version)
                    .await?
                    .into_iter()
                    .filter(|rule| {
                        channel.required_rule_codes.contains(&rule.code)
                            && rule.applies_in(channel.context_id)
                    })
                    .map(|rule| GateCheck {
                        source: CheckSource::Rule,
                        code: rule.code,
                        message: None,
                        severity: Some(rule.compiled.severity),
                        predicate: rule.compiled.predicate,
                    }),
            );
        }
        Ok(Some(gate))
    }

    /// The gate's failures for `subject` in the channel context: JSON-schema
    /// errors first, then failing checks and rules.
    async fn violations(
        &self,
        conn: &mut PgConnection,
        scope: &CheckScope,
        subject: &RecordValues,
    ) -> Result<Vec<CheckViolation>, RepositoryError> {
        let mut violations = Vec::new();
        if let Some(schema) = &self.record_schema {
            // The same document the write path validates.
            let document = subject.schema_document(&scope.path(self.context_id)?);
            for error in attricat_validation::validate_json_schema(schema, &Value::Object(document))
                .map_err(RepositoryError::InvalidBlueprintDefinition)?
                .into_iter()
                .take(MAX_SCHEMA_VIOLATIONS)
            {
                let attribute = error
                    .instance_path
                    .trim_start_matches('/')
                    .split('/')
                    .next()
                    .filter(|segment| !segment.is_empty())
                    .map(str::to_owned);
                violations.push(CheckViolation {
                    source: CheckSource::RecordSchema,
                    code: "record_schema".to_owned(),
                    message: error.message,
                    contexts: vec![self.context_code.clone()],
                    attributes: attribute.into_iter().collect(),
                    severity: None,
                    transition: None,
                    evidence: serde_json::json!({ "instance_path": error.instance_path }),
                });
            }
        }
        let predicates: Vec<&Predicate> =
            self.checks.iter().map(|check| &check.predicate).collect();
        let outcomes =
            evaluate_in_context(conn, scope, subject, self.context_id, &[], &predicates).await?;
        for (check, outcome) in self.checks.iter().zip(outcomes) {
            if let Err(failure) = outcome {
                violations.push(CheckViolation {
                    source: check.source,
                    code: check.code.clone(),
                    message: check.message.clone().unwrap_or(failure.message),
                    contexts: vec![self.context_code.clone()],
                    attributes: failure.attributes,
                    severity: check.severity.clone(),
                    transition: None,
                    evidence: failure.evidence,
                });
            }
        }
        Ok(violations)
    }
}

/// The workspace contexts and one live record's values, to evaluate gates.
async fn gate_subject(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    record_id: Uuid,
) -> Result<(CheckScope, RecordValues), RepositoryError> {
    let scope = CheckScope::load(conn, workspace_id).await?;
    let subject = load_current_records(conn, workspace_id, &[record_id])
        .await?
        .remove(&record_id)
        .ok_or(RepositoryError::NotFound("record"))?;
    Ok((scope, subject))
}

struct PublicationMutation<'a> {
    event_type: &'a str,
    record_id: Uuid,
    context_id: Uuid,
    published_at: Option<chrono::DateTime<chrono::Utc>>,
    published_by_user_id: Option<Uuid>,
    reason: Option<&'a str>,
}

/// The outcome of a record edit by an actor holding one of the blueprint's
/// `retain_on_edit_roles`: the publications stay, except in the channels
/// whose checks the edited record now fails.
pub(crate) struct RetainedPublication {
    pub(crate) role_code: String,
    pub(crate) withdrawn_context_ids: Vec<Uuid>,
}

impl AttricatRepository {
    pub async fn list_publication_channels(
        &self,
    ) -> Result<Vec<PublicationChannel>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<PublicationChannel>>(&format!(
            "{CHANNEL_SELECT} ORDER BY a.code"
        ))
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
                require_valid_record: None,
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
        let mut tx = self.pool.begin().await?;
        let channel = self
            .upsert_publication_channel_in_transaction(&mut tx, context_id, input)
            .await?;
        self.commit_mutation(tx).await?;
        Ok(channel)
    }

    /// Validates and writes a channel's settings in the caller's transaction,
    /// creating the channel when the context has none. Omitted check
    /// settings are kept. The caller records the audit event.
    pub(super) async fn upsert_publication_channel_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context_id: Uuid,
        input: crate::model::UpdatePublicationChannel,
    ) -> Result<PublicationChannel, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        if let Some(codes) = &input.required_rule_codes {
            let unique: std::collections::HashSet<_> = codes.iter().collect();
            if codes.len() > 32
                || unique.len() != codes.len()
                || codes.iter().any(|code| !is_valid_code(code))
            {
                return Err(RepositoryError::InvalidPublicationChannel(
                    "required_rule_codes must be at most 32 unique rule codes".into(),
                ));
            }
            // A code that names no rule would never fail, silently disabling
            // the gate. Rules created earlier in this transaction count.
            let known: std::collections::HashSet<String> = sqlx::query_scalar(
                "SELECT DISTINCT code FROM rules WHERE workspace_id = $1 AND code = ANY($2)",
            )
            .bind(workspace_id)
            .bind(codes)
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .collect();
            let unknown: Vec<&str> = codes
                .iter()
                .filter(|code| !known.contains(*code))
                .map(String::as_str)
                .collect();
            if !unknown.is_empty() {
                return Err(RepositoryError::InvalidPublicationChannel(format!(
                    "required_rule_codes name no rule in this workspace: {}",
                    unknown.join(", ")
                )));
            }
        }
        let context = sqlx::query_as::<_, Db<AttributeContext>>(
            "SELECT id, code, data, parent_id FROM attribute_contexts WHERE workspace_id = $1 AND id = $2",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(RepositoryError::InvalidContext)?
        .into_domain();
        let (required_rule_codes, require_valid_record): (Vec<String>, bool) = sqlx::query_as("INSERT INTO publication_channels (workspace_id, context_id, enabled, required_rule_codes, require_valid_record) VALUES ($1, $2, $3, COALESCE($4, '{}'::text[]), COALESCE($5, false)) ON CONFLICT (workspace_id, context_id) DO UPDATE SET enabled = EXCLUDED.enabled, required_rule_codes = COALESCE($4, publication_channels.required_rule_codes), require_valid_record = COALESCE($5, publication_channels.require_valid_record), updated_at = now() RETURNING required_rule_codes, require_valid_record")
            .bind(workspace_id).bind(context_id).bind(input.enabled).bind(input.required_rule_codes).bind(input.require_valid_record).fetch_one(&mut **tx).await?;
        Ok(PublicationChannel {
            context_id,
            context_code: context.code,
            enabled: input.enabled,
            required_rule_codes,
            require_valid_record,
        })
    }

    /// Readiness of the record for every enabled channel: the checks each
    /// channel requires, evaluated on the current state without publishing.
    pub async fn publication_readiness(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<PublicationReadiness>, RepositoryError> {
        let record = self
            .get_record(record_id)
            .await?
            .ok_or(RepositoryError::NotFound("record"))?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *tx)
            .await?;
        let workspace_id = self.workspace_id.0;
        let channels = sqlx::query_as::<_, Db<PublicationChannel>>(&format!(
            "{CHANNEL_SELECT} AND c.enabled ORDER BY a.code"
        ))
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?
        .into_domain();
        // Contexts and the record's values load once, on the first gate.
        let mut evaluation: Option<(CheckScope, RecordValues)> = None;
        let mut readiness = Vec::with_capacity(channels.len());
        for channel in channels {
            let gate = PublicationGate::load(
                &mut tx,
                workspace_id,
                &channel,
                (record.blueprint_id, record.blueprint_version),
            )
            .await?;
            let violations = match gate {
                None => Vec::new(),
                Some(gate) => {
                    let (scope, subject) = match &evaluation {
                        Some(loaded) => loaded,
                        None => {
                            evaluation.insert(gate_subject(&mut tx, workspace_id, record.id).await?)
                        }
                    };
                    gate.violations(&mut tx, scope, subject).await?
                }
            };
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
        record_id: Uuid,
    ) -> Result<Vec<RecordPublicationStatus>, RepositoryError> {
        if self.get_record(record_id).await?.is_none() {
            return Err(RepositoryError::NotFound("record"));
        }
        let workspace_id = self.workspace_id.0;
        Ok(sqlx::query_as::<_, Db<RecordPublicationStatus>>(
            "SELECT c.context_id, a.code AS context_code, CASE WHEN p.published_at IS NULL THEN 'not_published' ELSE 'published' END AS status, p.published_at, p.published_by_user_id FROM publication_channels c JOIN attribute_contexts a ON a.workspace_id = c.workspace_id AND a.id = c.context_id LEFT JOIN record_channel_publications p ON p.workspace_id = c.workspace_id AND p.record_id = $2 AND p.context_id = c.context_id WHERE c.workspace_id = $1 AND c.enabled ORDER BY a.code",
        )
        .bind(workspace_id)
        .bind(record_id)
        .fetch_all(&self.pool)
        .await?
        .into_domain())
    }

    pub async fn publish_record(
        &self,
        record_id: Uuid,
        context_id: Uuid,
    ) -> Result<RecordPublicationStatus, RepositoryError> {
        let mut tx = self.pool.begin().await?;
        self.lock_record(&mut tx, record_id).await?;
        let status = self
            .publish_to_channel(&mut tx, record_id, context_id)
            .await?;
        self.commit_publication_mutation(
            tx,
            PublicationMutation {
                event_type: RECORD_PUBLISHED_V1,
                record_id,
                context_id,
                published_at: status.published_at,
                published_by_user_id: status.published_by_user_id,
                reason: None,
            },
        )
        .await?;
        Ok(status)
    }

    pub async fn publish_record_all_channels(
        &self,
        record_id: Uuid,
    ) -> Result<Vec<RecordPublicationStatus>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        self.lock_record(&mut tx, record_id).await?;
        let channels: Vec<Uuid> = sqlx::query_scalar(
            "SELECT context_id FROM publication_channels WHERE workspace_id = $1 AND enabled ORDER BY context_id FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut statuses = Vec::with_capacity(channels.len());
        for context_id in channels {
            statuses.push(
                self.publish_to_channel(&mut tx, record_id, context_id)
                    .await?,
            );
        }
        for status in &statuses {
            self.enqueue_event(
                &mut tx,
                self.publication_event(
                    RECORD_PUBLISHED_V1,
                    record_id,
                    status.context_id,
                    status.published_at,
                    status.published_by_user_id,
                    None,
                ),
            )
            .await?;
        }
        self.commit_mutation(tx).await?;
        Ok(statuses)
    }

    pub async fn publish_blueprint_records(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        context_id: Option<Uuid>,
    ) -> Result<BlueprintRecordPublicationSummary, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        let blueprint_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM blueprints WHERE workspace_id = $1 AND id = $2 AND version = $3 AND kind = 'record' AND deleted_at IS NULL)",
        )
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_one(&mut *tx)
        .await?;
        if !blueprint_exists {
            return Err(RepositoryError::NotFound("record blueprint revision"));
        }
        super::record_commands::lock_record_writes(&mut tx, workspace_id, false).await?;
        let record_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM records WHERE workspace_id = $1 AND blueprint_id = $2 AND blueprint_version = $3 AND deleted_at IS NULL ORDER BY id FOR UPDATE",
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
        self.check_bulk_publication_gates(
            &mut tx,
            (blueprint_id, blueprint_version),
            &record_ids,
            &channel_ids,
        )
        .await?;
        let published_at = chrono::Utc::now();
        sqlx::query("INSERT INTO record_channel_publications (workspace_id, record_id, context_id, published_at, published_by_user_id) SELECT $1, record.id, channel.id, $4, $5 FROM UNNEST($2::uuid[]) AS record(id) CROSS JOIN UNNEST($3::uuid[]) AS channel(id) ON CONFLICT (workspace_id, record_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
            .bind(workspace_id)
            .bind(&record_ids)
            .bind(&channel_ids)
            .bind(published_at)
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        let events = record_ids
            .iter()
            .flat_map(|record_id| {
                channel_ids
                    .iter()
                    .map(move |context_id| (*record_id, *context_id))
            })
            .map(|(record_id, context_id)| {
                self.publication_event(
                    RECORD_PUBLISHED_V1,
                    record_id,
                    context_id,
                    Some(published_at),
                    Some(actor),
                    Some("blueprint_bulk"),
                )
            })
            .collect();
        self.enqueue_events(&mut tx, events).await?;
        self.commit_mutation(tx).await?;
        Ok(BlueprintRecordPublicationSummary {
            record_count: record_ids.len() as i64,
            channel_count: channel_ids.len() as i64,
            publication_count: (record_ids.len() * channel_ids.len()) as i64,
        })
    }

    pub async fn unpublish_record(
        &self,
        record_id: Uuid,
        context_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let mut tx = self.pool.begin().await?;
        self.lock_record(&mut tx, record_id).await?;
        let updated = sqlx::query("UPDATE record_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND record_id = $2 AND context_id = $3 AND published_at IS NOT NULL")
            .bind(workspace_id).bind(record_id).bind(context_id).execute(&mut *tx).await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::NotFound("active record publication"));
        }
        self.commit_publication_mutation(
            tx,
            PublicationMutation {
                event_type: RECORD_UNPUBLISHED_V1,
                record_id,
                context_id,
                published_at: None,
                published_by_user_id: None,
                reason: Some("manual"),
            },
        )
        .await
    }

    pub(crate) async fn reconcile_record_publication(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        reason: &str,
    ) -> Result<Option<RetainedPublication>, RepositoryError> {
        let Some(actor_id) = self
            .audit_context
            .as_ref()
            .and_then(|audit| audit.actor_user_id)
        else {
            self.clear_record_publications(tx, record_id, reason)
                .await?;
            return Ok(None);
        };
        let workspace_id = self.workspace_id.0;
        let row: Option<(String, Uuid, i64)> = sqlx::query_as(
            "SELECT b.definition, e.blueprint_id, e.blueprint_version FROM records e JOIN blueprints b ON b.workspace_id = e.workspace_id AND b.id = e.blueprint_id AND b.version = e.blueprint_version WHERE e.workspace_id = $1 AND e.id = $2 AND e.deleted_at IS NULL",
        )
        .bind(workspace_id)
        .bind(record_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((definition, blueprint_id, blueprint_version)) = row else {
            return Ok(None);
        };
        let roles = attricat_blueprint::parse(&definition)
            .map_err(RepositoryError::invalid_blueprint_definition)?
            .publication
            .retain_on_edit_roles;
        if roles.is_empty() {
            self.clear_record_publications(tx, record_id, reason)
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
        let Some(role_code) = retained_role else {
            self.clear_record_publications(tx, record_id, reason)
                .await?;
            return Ok(None);
        };
        let withdrawn_context_ids = self
            .withdraw_failing_publications(tx, record_id, (blueprint_id, blueprint_version))
            .await?;
        Ok(Some(RetainedPublication {
            role_code,
            withdrawn_context_ids,
        }))
    }

    /// Re-evaluates the gate of every channel the record is published to
    /// after a retained edit, and withdraws the publications whose channel
    /// checks now fail with reason `checks_failed`. A retained edit keeps a
    /// publication only where the channel would still accept it. Returns the
    /// withdrawn channel context ids in ascending order.
    async fn withdraw_failing_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        revision: (Uuid, i64),
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let channels = sqlx::query_as::<_, Db<PublicationChannel>>(&format!(
            "{CHANNEL_SELECT} AND EXISTS (SELECT 1 FROM record_channel_publications p WHERE p.workspace_id = c.workspace_id AND p.record_id = $2 AND p.context_id = c.context_id AND p.published_at IS NOT NULL) ORDER BY c.context_id"
        ))
        .bind(workspace_id)
        .bind(record_id)
        .fetch_all(&mut **tx)
        .await?
        .into_domain();
        // Contexts and the record's values load once, on the first gate.
        let mut evaluation: Option<(CheckScope, RecordValues)> = None;
        let mut failing = Vec::new();
        for channel in channels {
            let Some(gate) = PublicationGate::load(tx, workspace_id, &channel, revision).await?
            else {
                continue;
            };
            let (scope, subject) = match &evaluation {
                Some(loaded) => loaded,
                None => evaluation.insert(gate_subject(tx, workspace_id, record_id).await?),
            };
            if !gate.violations(tx, scope, subject).await?.is_empty() {
                failing.push(channel.context_id);
            }
        }
        if failing.is_empty() {
            return Ok(Vec::new());
        }
        let mut contexts: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE record_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND record_id = $2 AND context_id = ANY($3) AND published_at IS NOT NULL RETURNING context_id",
        )
        .bind(workspace_id)
        .bind(record_id)
        .bind(&failing)
        .fetch_all(&mut **tx)
        .await?;
        contexts.sort_unstable();
        let events = contexts
            .iter()
            .map(|&context_id| {
                self.publication_event(
                    RECORD_UNPUBLISHED_V1,
                    record_id,
                    context_id,
                    None,
                    None,
                    Some("checks_failed"),
                )
            })
            .collect();
        self.enqueue_events(tx, events).await?;
        Ok(contexts)
    }

    pub(crate) async fn clear_record_publications(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        reason: &str,
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let contexts: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE record_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND record_id = $2 AND published_at IS NOT NULL RETURNING context_id",
        )
        .bind(workspace_id)
        .bind(record_id)
        .fetch_all(&mut **tx)
        .await?;
        for context_id in contexts {
            self.enqueue_event(
                tx,
                self.publication_event(
                    RECORD_UNPUBLISHED_V1,
                    record_id,
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
        let records: Vec<Uuid> = sqlx::query_scalar(
            "UPDATE record_channel_publications SET published_at = NULL, published_by_user_id = NULL WHERE workspace_id = $1 AND context_id = $2 AND published_at IS NOT NULL RETURNING record_id",
        )
        .bind(workspace_id)
        .bind(context_id)
        .fetch_all(&mut **tx)
        .await?;
        for record_id in records {
            self.enqueue_event(
                tx,
                self.publication_event(
                    RECORD_UNPUBLISHED_V1,
                    record_id,
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
        record_id: Uuid,
        context_id: Uuid,
    ) -> Result<RecordPublicationStatus, RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let channel = sqlx::query_as::<_, Db<PublicationChannel>>(&format!(
            "{CHANNEL_SELECT} AND c.context_id = $2 FOR UPDATE OF c"
        ))
        .bind(workspace_id)
        .bind(context_id)
        .fetch_optional(&mut **tx)
        .await?
        .filter(|channel| channel.enabled)
        .ok_or(RepositoryError::PublicationChannelDisabled)?
        .into_domain();
        let record = self.lock_record(tx, record_id).await?;
        let violations = self.publication_violations(tx, &record, &channel).await?;
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
        sqlx::query("INSERT INTO record_channel_publications (workspace_id, record_id, context_id, published_at, published_by_user_id) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (workspace_id, record_id, context_id) DO UPDATE SET published_at = EXCLUDED.published_at, published_by_user_id = EXCLUDED.published_by_user_id")
            .bind(workspace_id).bind(record_id).bind(context_id).bind(published_at).bind(actor).execute(&mut **tx).await?;
        Ok(RecordPublicationStatus {
            context_id,
            context_code: channel.context_code,
            status: "published".to_owned(),
            published_at: Some(published_at),
            published_by_user_id: Some(actor),
        })
    }

    /// Evaluates a channel's required checks for one record in the channel
    /// context. Required rules that are not enabled for the record's
    /// blueprint revision, or that are scoped to another context, do not apply.
    pub(super) async fn publication_violations(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        record: &Record,
        channel: &PublicationChannel,
    ) -> Result<Vec<CheckViolation>, RepositoryError> {
        let Some(gate) = PublicationGate::load(
            tx,
            self.workspace_id.0,
            channel,
            (record.blueprint_id, record.blueprint_version),
        )
        .await?
        else {
            return Ok(Vec::new());
        };
        let (scope, subject) = gate_subject(tx, self.workspace_id.0, record.id).await?;
        gate.violations(tx, &scope, &subject).await
    }

    /// Evaluates every record against each gated channel before a bulk
    /// publication; one failure rejects the whole publication with the failing
    /// records in the evidence. The records are already locked.
    async fn check_bulk_publication_gates(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        revision: (Uuid, i64),
        record_ids: &[Uuid],
        channel_ids: &[Uuid],
    ) -> Result<(), RepositoryError> {
        let workspace_id = self.workspace_id.0;
        let channels = sqlx::query_as::<_, Db<PublicationChannel>>(&format!(
            "{CHANNEL_SELECT} AND c.context_id = ANY($2) ORDER BY c.context_id"
        ))
        .bind(workspace_id)
        .bind(channel_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_domain();
        let mut gates = Vec::new();
        for channel in &channels {
            if let Some(gate) = PublicationGate::load(tx, workspace_id, channel, revision).await? {
                gates.push(gate);
            }
        }
        if gates.is_empty() {
            return Ok(());
        }
        let scope = CheckScope::load(tx, workspace_id).await?;
        let subjects = load_current_records(tx, workspace_id, record_ids).await?;
        for gate in gates {
            let mut failures = Vec::new();
            for record_id in record_ids {
                if failures.len() >= super::MAX_REPORTED_VIOLATIONS {
                    break;
                }
                let subject = subjects
                    .get(record_id)
                    .ok_or(RepositoryError::NotFound("record"))?;
                for mut violation in gate.violations(tx, &scope, subject).await? {
                    if failures.len() >= super::MAX_REPORTED_VIOLATIONS {
                        break;
                    }
                    if let Some(evidence) = violation.evidence.as_object_mut() {
                        evidence.insert("record_id".into(), serde_json::json!(record_id));
                    }
                    failures.push(violation);
                }
            }
            if !failures.is_empty() {
                return Err(RepositoryError::PublicationChecksFailed {
                    context: gate.context_code,
                    violations: failures,
                });
            }
        }
        Ok(())
    }

    async fn commit_publication_mutation(
        &self,
        tx: Transaction<'_, Postgres>,
        mutation: PublicationMutation<'_>,
    ) -> Result<(), RepositoryError> {
        self.commit_mutation_with_event(
            tx,
            self.publication_event(
                mutation.event_type,
                mutation.record_id,
                mutation.context_id,
                mutation.published_at,
                mutation.published_by_user_id,
                mutation.reason,
            ),
        )
        .await
    }

    fn publication_event(
        &self,
        event_type: &str,
        record_id: Uuid,
        context_id: Uuid,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
        published_by_user_id: Option<Uuid>,
        reason: Option<&str>,
    ) -> crate::domain_events::NewDomainEvent {
        self.core_event(
            event_type,
            "record",
            record_id,
            serde_json::to_value(RecordPublicationV1 {
                record_id,
                context_id,
                published_at,
                published_by_user_id,
                reason: reason.map(str::to_owned),
            })
            .expect("publication event serializable"),
        )
    }
}

impl<S: super::RepositoryScope> AttricatRepository<S> {
    /// Installs publication permissions in application code, preserving the
    /// declarative-only migration contract. Owners and administrators receive
    /// publication authority; editors deliberately do not.
    pub async fn ensure_record_publication_permissions(&self) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO permissions (code, description) VALUES ('records.publish', 'Publish catalog records to channel contexts') ON CONFLICT (code) DO NOTHING")
            .execute(&mut *tx)
            .await?;
        for role_id in [
            Uuid::from_u128(0x00000000000040008000000000000101),
            Uuid::from_u128(0x00000000000040008000000000000102),
        ] {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, 'records.publish') ON CONFLICT DO NOTHING")
                .bind(role_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
