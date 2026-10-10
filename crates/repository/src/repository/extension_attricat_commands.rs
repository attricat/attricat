//! Extension catalog batches: bounded, idempotent create, update,
//! relationship, upsert and annotation intents. Each intent runs through the
//! ordinary record seams in its own transaction.

use super::extension_attricat_data::{
    ExtensionAttricatBatch, ExtensionAttricatIntent, ExtensionAttricatIntentOutcome,
    ExtensionAttricatIntentStatus, MAX_EXTENSION_BATCH_INTENTS, MAX_EXTENSION_BATCH_KEY_BYTES,
    MAX_EXTENSION_INTENT_KEY_BYTES,
};
use super::record_commands::{ChosenIdRecordCreate, writes_relationship_values};
use super::*;
use crate::model::UpdateRecordFormRequest;
use sha2::Digest;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeSet;

/// Whether [`AttricatRepository::extension_lookup`] resolves a read or the
/// target of an upsert.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtensionLookupMode {
    Read,
    Upsert,
}

impl AttricatRepository {
    /// Applies one bounded extension batch. Each intent receives its own
    /// transaction so a retry may return durable per-intent outcomes without
    /// repeating a catalog mutation. The marker, audit rows, outbox event and
    /// task-fence check share that transaction.
    pub async fn execute_extension_attricat_batch(
        &self,
        batch: ExtensionAttricatBatch,
    ) -> Result<Vec<ExtensionAttricatIntentOutcome>, RepositoryError> {
        if batch.batch_key.is_empty()
            || batch.batch_key.len() > MAX_EXTENSION_BATCH_KEY_BYTES
            || !batch.batch_key.is_ascii()
        {
            return Err(RepositoryError::InvalidExtension(
                "batch key must be 1-256 ASCII bytes".into(),
            ));
        }
        if batch.intents.is_empty() || batch.intents.len() > MAX_EXTENSION_BATCH_INTENTS {
            return Err(RepositoryError::InvalidExtension(
                "batch must contain 1-100 intents".into(),
            ));
        }
        let extension_id = self.extension_id.as_deref().ok_or_else(|| {
            RepositoryError::InvalidExtension(
                "extension batch requires extension provenance".into(),
            )
        })?;
        let mut outcomes = Vec::with_capacity(batch.intents.len());
        let mut keys = HashSet::new();
        for intent in batch.intents {
            let key = extension_intent_key(&intent).to_owned();
            if key.is_empty()
                || key.len() > MAX_EXTENSION_INTENT_KEY_BYTES
                || !key.is_ascii()
                || !keys.insert(key.clone())
            {
                return Err(RepositoryError::InvalidExtension(
                    "intent keys must be unique 1-128 ASCII bytes".into(),
                ));
            }
            match self
                .execute_extension_attricat_intent(
                    extension_id,
                    &batch.batch_key,
                    batch.dry_run,
                    intent,
                )
                .await
            {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => outcomes.push(ExtensionAttricatIntentOutcome {
                    intent_key: key,
                    status: ExtensionAttricatIntentStatus::Rejected,
                    record_id: None,
                    error: Some(error.to_string()),
                    annotation_revision: None,
                }),
            }
        }
        Ok(outcomes)
    }

    async fn execute_extension_attricat_intent(
        &self,
        extension_id: &str,
        batch_key: &str,
        dry_run: bool,
        intent: ExtensionAttricatIntent,
    ) -> Result<ExtensionAttricatIntentOutcome, RepositoryError> {
        let key = extension_intent_key(&intent).to_owned();
        let serialized = serde_json::to_vec(&intent).expect("extension intent serializes");
        let input_hash = format!("{:x}", sha2::Sha256::digest(serialized));
        let ws = self.workspace_id.0;
        let mut transaction = self.pool.begin().await?;
        if !dry_run {
            let existing: Option<(String, Value)> = sqlx::query_as("SELECT input_hash,outcome FROM extension_attricat_batch_intents WHERE workspace_id=$1 AND extension_id=$2 AND batch_key=$3 AND intent_key=$4 FOR UPDATE")
                .bind(ws).bind(extension_id).bind(batch_key).bind(&key).fetch_optional(&mut *transaction).await?;
            if let Some((existing_hash, outcome)) = existing {
                if existing_hash != input_hash {
                    return Err(RepositoryError::InvalidExtension(
                        "intent key was reused with different input".into(),
                    ));
                }
                let mut outcome: ExtensionAttricatIntentOutcome = serde_json::from_value(outcome)
                    .map_err(|_| {
                    RepositoryError::InvalidExtension("stored batch outcome is invalid".into())
                })?;
                outcome.status = ExtensionAttricatIntentStatus::AlreadyApplied;
                transaction.commit().await?;
                return Ok(outcome);
            }
        }
        let result = match intent {
            ExtensionAttricatIntent::Annotate {
                record_id,
                add_tags,
                remove_tags,
                set_metadata,
                remove_metadata,
                expected_revision,
                ..
            } => self
                .apply_extension_annotation_patch(
                    &mut transaction,
                    extension_id,
                    record_id,
                    &super::ExtensionAnnotationPatch {
                        add_tags,
                        remove_tags,
                        set_metadata,
                        remove_metadata,
                        expected_revision,
                    },
                    super::extension_annotations::AnnotationPatchAuthority::Extension,
                )
                .await
                .map(|revision| (record_id, Some(revision))),
            intent => self
                .apply_extension_attricat_intent(&mut transaction, intent)
                .await
                .map(|record_id| (record_id, None)),
        };
        match result {
            Ok((record_id, annotation_revision)) => {
                let status = if dry_run {
                    ExtensionAttricatIntentStatus::Validated
                } else {
                    ExtensionAttricatIntentStatus::Applied
                };
                let outcome = ExtensionAttricatIntentOutcome {
                    intent_key: key.clone(),
                    status,
                    record_id: Some(record_id),
                    error: None,
                    annotation_revision,
                };
                if dry_run {
                    transaction.rollback().await?;
                    return Ok(outcome);
                }
                self.ensure_task_fence(&mut transaction).await?;
                sqlx::query("INSERT INTO extension_attricat_batch_intents(workspace_id,extension_id,batch_key,intent_key,input_hash,outcome) VALUES($1,$2,$3,$4,$5,$6)")
                    .bind(ws).bind(extension_id).bind(batch_key).bind(&key).bind(input_hash).bind(serde_json::to_value(&outcome).expect("outcome serializes")).execute(&mut *transaction).await?;
                transaction.commit().await?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await?;
                if dry_run {
                    Ok(ExtensionAttricatIntentOutcome {
                        intent_key: key,
                        status: ExtensionAttricatIntentStatus::Rejected,
                        record_id: None,
                        error: Some(error.to_string()),
                        annotation_revision: None,
                    })
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn apply_extension_attricat_intent(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        intent: ExtensionAttricatIntent,
    ) -> Result<Uuid, RepositoryError> {
        // An interactive run is bounded by its frozen selection; it cannot
        // create catalog records outside that selection.
        if self.authorization_actor().is_some()
            && matches!(
                intent,
                ExtensionAttricatIntent::Create { .. } | ExtensionAttricatIntent::Upsert { .. }
            )
        {
            return Err(RepositoryError::InvalidExtension(
                "interactive runs cannot create or upsert records".into(),
            ));
        }
        match intent {
            ExtensionAttricatIntent::Create {
                blueprint_id,
                blueprint_version,
                values,
                system_tags,
                system_metadata,
                ..
            } => {
                self.apply_extension_attricat_create(
                    transaction,
                    blueprint_id,
                    blueprint_version,
                    values,
                    system_tags,
                    system_metadata,
                )
                .await
            }
            ExtensionAttricatIntent::Update {
                record_id,
                values,
                relationships,
                ..
            } => {
                self.apply_extension_attricat_update(transaction, record_id, values, relationships)
                    .await
            }
            ExtensionAttricatIntent::Relationships {
                record_id,
                relationships,
                ..
            } => {
                self.apply_extension_attricat_update(
                    transaction,
                    record_id,
                    Vec::new(),
                    relationships,
                )
                .await
            }
            ExtensionAttricatIntent::Annotate { .. } => Err(RepositoryError::InvalidExtension(
                "annotation intents use the namespace patch path".into(),
            )),
            ExtensionAttricatIntent::Upsert {
                blueprint_id,
                blueprint_version,
                lookup_attribute_id,
                lookup_value,
                values,
                relationships,
                system_tags,
                system_metadata,
                ..
            } => {
                if lookup_value.is_empty()
                    || lookup_value.len()
                        > super::extension_attricat_data::MAX_EXTENSION_LOOKUP_VALUE_BYTES
                {
                    return Err(RepositoryError::InvalidExtension(
                        "upsert lookup value must be 1-512 bytes".into(),
                    ));
                }
                // The lookup locks the matched record row, so the workspace
                // relationship lock must be taken before it.
                if !relationships.is_empty() || writes_relationship_values(&values) {
                    self.lock_relationship_cardinality_writes(transaction)
                        .await?;
                }
                let existing = self
                    .extension_lookup(
                        transaction,
                        blueprint_id,
                        blueprint_version,
                        lookup_attribute_id,
                        &lookup_value,
                        ExtensionLookupMode::Upsert,
                    )
                    .await?;
                match existing {
                    Some(record_id) => {
                        self.apply_extension_attricat_update(
                            transaction,
                            record_id,
                            values,
                            relationships,
                        )
                        .await
                    }
                    None => {
                        // A new record has no current targets, so each
                        // declared set is written as plain relationship values.
                        let values = values
                            .into_iter()
                            .chain(relationships.into_iter().flat_map(relationship_values))
                            .collect();
                        self.apply_extension_attricat_create(
                            transaction,
                            blueprint_id,
                            blueprint_version,
                            values,
                            system_tags,
                            system_metadata,
                        )
                        .await
                    }
                }
            }
        }
    }

    /// Finds the record an extension lookup names, which is the record a
    /// legacy upsert with the same lookup updates. When the lookup attribute
    /// alone forms a declared unique key, the key index resolves it across
    /// every revision of the blueprint family, with the key's normalization.
    /// Otherwise the lookup is advisory: an exact text match among records
    /// pinned to the requested revision. More than one match is an error.
    /// [`ExtensionLookupMode::Upsert`] also serializes concurrent upserts of
    /// the value and locks the matched record row.
    pub(super) async fn extension_lookup(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        lookup_attribute_id: Uuid,
        lookup_value: &str,
        mode: ExtensionLookupMode,
    ) -> Result<Option<Uuid>, RepositoryError> {
        let upsert = mode == ExtensionLookupMode::Upsert;
        let row_lock = if upsert { " FOR UPDATE OF e" } else { "" };
        let workspace_id = self.workspace_id.0;
        if upsert {
            // The record-writer lock comes before any record row lock.
            super::record_commands::lock_record_writes(transaction, workspace_id, false).await?;
        }
        let attribute: Option<(String, String)> = sqlx::query_as(
            "SELECT code, value_type FROM attributes WHERE id = $1 AND workspace_id = $2 AND blueprint_id = $3 AND blueprint_version = $4 AND deleted_at IS NULL",
        )
        .bind(lookup_attribute_id)
        .bind(workspace_id)
        .bind(blueprint_id)
        .bind(blueprint_version)
        .fetch_optional(&mut **transaction)
        .await?;
        let unique_key = match &attribute {
            Some((code, value_type)) if value_type == "string" => {
                super::structural_constraints::enforced_unique_keys(
                    transaction,
                    workspace_id,
                    blueprint_id,
                )
                .await?
                .into_iter()
                .find(|key| key.attributes.len() == 1 && &key.attributes[0] == code)
            }
            _ => None,
        };
        let matches: Vec<Uuid> = if let Some(key) = unique_key {
            let Some(component) = attricat_validation::unique_key::normalize_key_component(
                "string",
                &Value::String(lookup_value.to_owned()),
                key.case_sensitive,
            ) else {
                return Ok(None);
            };
            let key_hash = super::structural_constraints::key_hash(&Value::Array(vec![component]));
            // Serializes concurrent absent-key upserts of the same key value,
            // so the second one updates rather than conflicting on the index.
            if upsert {
                sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                    .bind(format!(
                        "extension-upsert-key:{workspace_id}:{blueprint_id}:{}:{key_hash}",
                        key.code
                    ))
                    .execute(&mut **transaction)
                    .await?;
            }
            // Workspace keys are indexed in the default context only; for a
            // context key, the default context's value identifies the record.
            let default_context_id = self
                .resolve_context_id(transaction, None)
                .await?
                .ok_or(RepositoryError::InvalidContext)?;
            sqlx::query_scalar(&format!(
                "SELECT e.id FROM record_unique_key_values k JOIN records e ON e.id = k.record_id AND e.workspace_id = k.workspace_id \
                 WHERE k.workspace_id = $1 AND k.blueprint_id = $2 AND k.key_code = $3 AND k.context_id = $4 AND k.key_hash = $5 AND e.deleted_at IS NULL \
                 ORDER BY e.id{row_lock} LIMIT 2",
            ))
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(&key.code)
            .bind(default_context_id)
            .bind(&key_hash)
            .fetch_all(&mut **transaction)
            .await?
        } else {
            // Without a declared single-attribute unique key nothing prevents
            // duplicates of this value. Serialize the lookup so concurrent
            // absent-key upserts cannot both take the create branch.
            if upsert {
                sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                    .bind(format!("extension-upsert:{workspace_id}:{blueprint_id}:{blueprint_version}:{lookup_attribute_id}:{lookup_value}"))
                    .execute(&mut **transaction)
                    .await?;
            }
            sqlx::query_scalar(&format!(
                "SELECT e.id FROM records e JOIN attribute_values v ON v.record_id=e.id AND v.workspace_id=e.workspace_id AND v.active \
                 WHERE e.workspace_id=$1 AND e.deleted_at IS NULL AND e.blueprint_id=$2 AND e.blueprint_version=$3 \
                   AND v.attribute_id=$4 AND v.relationship_target_record_id IS NULL AND v.value_text=$5 \
                 ORDER BY e.id{row_lock} LIMIT 2",
            ))
            .bind(workspace_id)
            .bind(blueprint_id)
            .bind(blueprint_version)
            .bind(lookup_attribute_id)
            .bind(lookup_value)
            .fetch_all(&mut **transaction)
            .await?
        };
        match matches.as_slice() {
            [] => Ok(None),
            [record_id] => Ok(Some(*record_id)),
            _ => Err(RepositoryError::InvalidExtension(
                "lookup matched multiple records".into(),
            )),
        }
    }

    /// A legacy extension create: the ordinary create seam. Legacy
    /// create/upsert annotation fields predate namespace ownership and cannot
    /// write a claimed namespace, including the caller's own.
    async fn apply_extension_attricat_create(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        blueprint_version: i64,
        values: Vec<NewAttributeValue>,
        system_tags: Vec<String>,
        system_metadata: Value,
    ) -> Result<Uuid, RepositoryError> {
        let (record, changes, event) = self
            .create_record_in_transaction(
                transaction,
                ChosenIdRecordCreate {
                    record_id: Uuid::new_v4(),
                    blueprint_id,
                    blueprint_version,
                    values,
                    files: super::record_commands::CreateFileValues::None,
                    system_tags,
                    system_metadata,
                    host_sample_marker: false,
                },
            )
            .await?;
        self.stage_record_mutation(transaction, changes, event)
            .await?;
        Ok(record.id)
    }

    /// A legacy extension update: the ordinary update seam, bounded for an
    /// interactive run by its initiator's grants. The legacy intent ABI has
    /// no caller-supplied version token, so it cannot change a status.
    async fn apply_extension_attricat_update(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        record_id: Uuid,
        values: Vec<NewAttributeValue>,
        relationships: Vec<RelationshipTargets>,
    ) -> Result<Uuid, RepositoryError> {
        // The seam takes this lock too; the actor checks below come first.
        if !relationships.is_empty() || writes_relationship_values(&values) {
            self.lock_relationship_cardinality_writes(transaction)
                .await?;
        }
        self.ensure_actor_may(transaction, "records.write", &[record_id])
            .await?;
        // A run bound to a user may link only to records that user can read,
        // whether or not they are in the run's selection.
        let targets: Vec<Uuid> = values
            .iter()
            .filter_map(|value| match value {
                NewAttributeValue::Relationship {
                    target_record_id, ..
                } => Some(*target_record_id),
                NewAttributeValue::Scalar { .. } => None,
            })
            .chain(
                relationships
                    .iter()
                    .flat_map(|set| set.target_record_ids.iter().copied()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.ensure_actor_may(transaction, "records.read", &targets)
            .await?;
        let (record, changes, event) = self
            .update_record_in_transaction(
                transaction,
                record_id,
                UpdateRecordFormRequest {
                    expected_updated_at: None,
                    values,
                    relationships,
                    remove_values: Vec::new(),
                    system_tags: None,
                    system_metadata: None,
                },
            )
            .await?;
        self.stage_record_mutation(transaction, changes, event)
            .await?;
        Ok(record.id)
    }
}

/// One relationship value per distinct target of a set.
fn relationship_values(set: RelationshipTargets) -> impl Iterator<Item = NewAttributeValue> {
    let RelationshipTargets {
        attribute_id,
        attribute_code,
        context_id,
        target_record_ids,
    } = set;
    target_record_ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(move |target_record_id| NewAttributeValue::Relationship {
            attribute_id,
            attribute_code: attribute_code.clone(),
            context_id,
            target_record_id,
        })
}

fn extension_intent_key(intent: &ExtensionAttricatIntent) -> &str {
    match intent {
        ExtensionAttricatIntent::Create { intent_key, .. }
        | ExtensionAttricatIntent::Update { intent_key, .. }
        | ExtensionAttricatIntent::Relationships { intent_key, .. }
        | ExtensionAttricatIntent::Upsert { intent_key, .. }
        | ExtensionAttricatIntent::Annotate { intent_key, .. } => intent_key,
    }
}
