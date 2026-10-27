//! Gapless counters behind generated attributes.
//!
//! A record create (or a migration onto a revision that adds a generated
//! attribute) takes the next number with one upsert on the attribute's
//! `attribute_sequences` row. The row stays locked until the transaction
//! ends, so creates that number the same family attribute run one at a time,
//! and a rollback returns the number. Writers lock counters after their
//! record rows and before the family's unique-key lock, ordered by
//! blueprint, attribute code and period. A single create or migration takes
//! one family's counters in code order; a record batch, which may create on
//! several families, takes all of its counters in that order before its
//! first operation ([`AttricatRepository::lock_generated_sequences_in`]).
//! Publication never touches counters.

use std::collections::{BTreeMap, HashSet};

use attricat_blueprint::{GeneratedValue, MAX_GENERATED_NUMBER};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::write_context::WriteContext;
use super::{AttricatRepository, RepositoryError};
use crate::model::{
    AdvanceGeneratedSequence, Attribute, GeneratedSequence, GeneratedSequenceAdvance,
    NewAttributeValue, Record,
};

/// The audit metadata key an advance records its old and new values under.
const ADVANCE_AUDIT_KEY: &str = "generated_sequence";

/// A stored attribute's generated declaration.
fn generated_value(attribute: &Attribute) -> Result<Option<GeneratedValue>, RepositoryError> {
    attribute
        .generated
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| sqlx::Error::Decode(Box::new(error)).into())
}

impl AttricatRepository {
    /// Takes the next number of each generated attribute of `record`'s
    /// revision that `skip` does not list, rendered for `at`, as
    /// default-context values for [`Self::insert_generated_values_in`].
    pub(super) async fn generated_values_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        write: &WriteContext,
        record: &Record,
        at: DateTime<Utc>,
        skip: &HashSet<Uuid>,
    ) -> Result<Vec<NewAttributeValue>, RepositoryError> {
        let mut attributes: Vec<_> = write
            .attributes()
            .iter()
            .filter(|attribute| !skip.contains(&attribute.id))
            .filter_map(|attribute| {
                attribute
                    .generated
                    .as_ref()
                    .map(|generated| (attribute, &generated.0))
            })
            .collect();
        if attributes.is_empty() {
            return Ok(Vec::new());
        }
        // Code order within the family, the order every writer uses; a batch
        // already holds these rows.
        attributes.sort_by(|(left, _), (right, _)| left.code.cmp(&right.code));
        let default_context_id = write.default_context_id()?;
        let mut values = Vec::with_capacity(attributes.len());
        for (attribute, generated) in attributes {
            let number = sqlx::query_scalar::<_, i64>(
                r#"INSERT INTO attribute_sequences (workspace_id, blueprint_id, attribute_code, period, next_value)
                   VALUES ($1, $2, $3, $4, $5 + 1)
                   ON CONFLICT (workspace_id, blueprint_id, attribute_code, period)
                   DO UPDATE SET next_value = attribute_sequences.next_value + 1, updated_at = now()
                   RETURNING next_value - 1"#,
            )
            .bind(self.workspace_id.0)
            .bind(record.blueprint_id)
            .bind(&attribute.code)
            .bind(generated.period(at))
            .bind(generated.start)
            .fetch_one(&mut **transaction)
            .await?;
            values.push(NewAttributeValue::Scalar {
                attribute_id: Some(attribute.id),
                attribute_code: None,
                context_id: Some(default_context_id),
                value: generated.render(number, at),
            });
        }
        Ok(values)
    }

    /// Locks the counter of every generated attribute of `revisions` for the
    /// current period, ordered by blueprint, attribute code and period, so a
    /// transaction that creates records on several blueprints holds them all
    /// before it numbers any. A missing counter is created at its `start`,
    /// which [`Self::generated_values_in`] then hands out; a rollback removes
    /// it again.
    pub(super) async fn lock_generated_sequences_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        revisions: &[(Uuid, i64)],
    ) -> Result<(), RepositoryError> {
        if revisions.is_empty() {
            return Ok(());
        }
        let (blueprint_ids, versions): (Vec<_>, Vec<_>) = revisions.iter().copied().unzip();
        let declared = sqlx::query_as::<_, (Uuid, String, sqlx::types::Json<GeneratedValue>)>(
            r#"SELECT a.blueprint_id, a.code, a.generated FROM attributes a
               JOIN UNNEST($2::uuid[], $3::bigint[]) AS r(blueprint_id, version)
                 ON r.blueprint_id = a.blueprint_id AND r.version = a.blueprint_version
               WHERE a.workspace_id = $1 AND a.generated IS NOT NULL AND a.deleted_at IS NULL"#,
        )
        .bind(self.workspace_id.0)
        .bind(&blueprint_ids)
        .bind(&versions)
        .fetch_all(&mut **transaction)
        .await?;
        if declared.is_empty() {
            return Ok(());
        }
        // Records created in this transaction take its start time.
        let at = sqlx::query_scalar::<_, DateTime<Utc>>("SELECT now()")
            .fetch_one(&mut **transaction)
            .await?;
        let counters: BTreeMap<_, _> = declared
            .into_iter()
            .map(|(blueprint_id, code, generated)| {
                ((blueprint_id, code, generated.period(at)), generated.start)
            })
            .collect();
        for ((blueprint_id, code, period), start) in counters {
            sqlx::query(
                r#"INSERT INTO attribute_sequences (workspace_id, blueprint_id, attribute_code, period, next_value)
                   VALUES ($1, $2, $3, $4, $5)
                   ON CONFLICT (workspace_id, blueprint_id, attribute_code, period)
                   DO UPDATE SET next_value = attribute_sequences.next_value"#,
            )
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(&code)
            .bind(&period)
            .bind(start)
            .execute(&mut **transaction)
            .await?;
        }
        Ok(())
    }

    /// Codes of the generated attributes of one blueprint revision.
    pub(super) async fn generated_attribute_codes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<HashSet<String>, RepositoryError> {
        Ok(sqlx::query_scalar::<_, String>(
            r#"SELECT code FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND workspace_id = $3
                 AND generated IS NOT NULL AND deleted_at IS NULL"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(self.workspace_id.0)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .collect())
    }

    /// The family's counters, plus the current period of each generated
    /// attribute of the latest published revision that no record has
    /// numbered yet, at its `start`.
    pub async fn list_generated_sequences(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<GeneratedSequence>, RepositoryError> {
        let blueprint = self
            .get_current_blueprint(blueprint_id)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))?;
        let mut sequences = sqlx::query_as::<_, (String, String, i64, DateTime<Utc>)>(
            r#"SELECT attribute_code, period, next_value, updated_at
               FROM attribute_sequences
               WHERE workspace_id = $1 AND blueprint_id = $2"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(
            |(attribute_code, period, next_value, updated_at)| GeneratedSequence {
                attribute_code,
                period,
                next_value,
                updated_at: Some(updated_at),
            },
        )
        .collect::<Vec<_>>();
        let now = Utc::now();
        for attribute in &blueprint.attributes {
            let Some(generated) = generated_value(attribute)? else {
                continue;
            };
            let period = generated.period(now);
            if !sequences.iter().any(|sequence| {
                sequence.attribute_code == attribute.code && sequence.period == period
            }) {
                sequences.push(GeneratedSequence {
                    attribute_code: attribute.code.clone(),
                    period,
                    next_value: generated.start,
                    updated_at: None,
                });
            }
        }
        sequences.sort_by(|left, right| {
            (&left.attribute_code, &left.period).cmp(&(&right.attribute_code, &right.period))
        });
        Ok(sequences)
    }

    /// Raises a generated attribute's counter so the next record receives
    /// `input.next_value`. Lowering it would hand out numbers that are
    /// already in use, so a lower value is refused; an equal one changes
    /// nothing. The audit event records the old and new values.
    pub async fn advance_generated_sequence(
        &self,
        blueprint_id: Uuid,
        attribute_code: &str,
        input: AdvanceGeneratedSequence,
    ) -> Result<GeneratedSequenceAdvance, RepositoryError> {
        let blueprint = self
            .get_current_blueprint(blueprint_id)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))?;
        let generated = match blueprint
            .attributes
            .iter()
            .find(|attribute| attribute.code == attribute_code)
        {
            Some(attribute) => generated_value(attribute)?,
            None => None,
        }
        .ok_or(RepositoryError::NotFound("generated attribute"))?;
        if !(0..=MAX_GENERATED_NUMBER).contains(&input.next_value) {
            return Err(RepositoryError::InvalidGeneratedSequence(format!(
                "next_value must be between 0 and {MAX_GENERATED_NUMBER}"
            )));
        }
        let period = match input.period {
            None => generated.period(Utc::now()),
            Some(period) if generated.is_period(&period) => period,
            Some(period) => {
                return Err(RepositoryError::InvalidGeneratedSequence(format!(
                    "'{period}' is not a {} period of this attribute",
                    generated.reset
                )));
            }
        };
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO attribute_sequences (workspace_id, blueprint_id, attribute_code, period, next_value)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (workspace_id, blueprint_id, attribute_code, period) DO NOTHING"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(attribute_code)
        .bind(&period)
        .bind(generated.start)
        .execute(&mut *transaction)
        .await?;
        let previous = sqlx::query_scalar::<_, i64>(
            r#"SELECT next_value FROM attribute_sequences
               WHERE workspace_id = $1 AND blueprint_id = $2 AND attribute_code = $3 AND period = $4
               FOR UPDATE"#,
        )
        .bind(self.workspace_id.0)
        .bind(blueprint_id)
        .bind(attribute_code)
        .bind(&period)
        .fetch_one(&mut *transaction)
        .await?;
        if input.next_value < previous {
            return Err(RepositoryError::GeneratedSequenceNotRaised {
                current: previous,
                requested: input.next_value,
            });
        }
        if input.next_value > previous {
            sqlx::query(
                r#"UPDATE attribute_sequences SET next_value = $5, updated_at = now()
                   WHERE workspace_id = $1 AND blueprint_id = $2 AND attribute_code = $3 AND period = $4"#,
            )
            .bind(self.workspace_id.0)
            .bind(blueprint_id)
            .bind(attribute_code)
            .bind(&period)
            .bind(input.next_value)
            .execute(&mut *transaction)
            .await?;
        }
        let advance = GeneratedSequenceAdvance {
            attribute_code: attribute_code.to_owned(),
            period,
            previous,
            next_value: input.next_value,
        };
        self.commit_mutation_with_audit_metadata(transaction, ADVANCE_AUDIT_KEY, json!(advance))
            .await?;
        Ok(advance)
    }
}
