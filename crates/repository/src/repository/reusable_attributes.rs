use super::*;
use crate::persistence_rows::{Db, IntoDomain};
use catalog_blueprint::{CONTEXT_EDITABLE_SCOPES, CONTEXT_FALLBACKS, DIRECTIONAL_CARDINALITIES};
use catalog_validation::{CODE_PATTERN, is_valid_code};
use schemars::JsonSchema;
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

fn workspace(repository: &CatalogRepository) -> Uuid {
    repository.workspace_id.0
}

fn default_context_fallback() -> String {
    "default".to_owned()
}

fn default_context_editable() -> String {
    "all".to_owned()
}

const REUSABLE_ATTRIBUTE_VALUE_TYPES: &[&str] = &[
    "string",
    "number",
    "integer",
    "boolean",
    "date",
    "datetime",
    "time",
    "relationship",
    "file",
];

/// A workspace attribute that entity blueprints can attach by reference.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Attricat reusable attribute definition")]
struct ReusableAttributeDefinition {
    /// Attribute identifier, using ASCII letters, numbers, hyphens, and underscores.
    #[schemars(regex(pattern = CODE_PATTERN))]
    code: String,
    /// Human-readable attribute name. `{{…}}` references resolve from the
    /// workspace lexicon.
    #[schemars(length(min = 1))]
    name: String,
    /// Stored value type.
    #[schemars(extend("enum" = REUSABLE_ATTRIBUTE_VALUE_TYPES))]
    value_type: String,
    /// JSON Schema (Draft 2020-12) for scalar values, written as a TOML table.
    #[serde(default)]
    #[schemars(with = "Option<serde_json::Value>")]
    value_schema: Option<toml::Value>,
    /// Value stored in the default context when an entity is created.
    /// Not supported for relationships and files.
    #[serde(default)]
    #[schemars(with = "Option<serde_json::Value>")]
    default_value: Option<toml::Value>,
    /// File upload policy table.
    #[serde(default)]
    #[schemars(
        with = "Option<serde_json::Value>",
        extend("x-attricat-value-types" = ["file"])
    )]
    file_policy: Option<toml::Value>,
    /// Blueprint that relationship targets must use.
    #[serde(default)]
    #[schemars(extend(
        "x-attricat-reference" = "blueprint",
        "x-attricat-value-types" = ["relationship"]
    ))]
    target_blueprint_code: Option<String>,
    /// How many targets one source may link to.
    #[serde(default)]
    #[schemars(extend(
        "x-attricat-suggestions" = DIRECTIONAL_CARDINALITIES,
        "x-attricat-value-types" = ["relationship", "file"]
    ))]
    cardinality: Option<String>,
    /// How many sources may link to one target.
    #[serde(default)]
    #[schemars(extend(
        "x-attricat-suggestions" = DIRECTIONAL_CARDINALITIES,
        "x-attricat-value-types" = ["relationship"]
    ))]
    target_cardinality: Option<String>,
    /// Free-form metadata. `hidden`, `hidden:form`, `hidden:detail`,
    /// `hidden:explorer`, and `hidden:metadata` hide the attribute from default UI.
    #[serde(default)]
    #[schemars(extend("x-attricat-suggestions" = [
        "hidden",
        "hidden:form",
        "hidden:detail",
        "hidden:explorer",
        "hidden:metadata"
    ]))]
    tags: Vec<String>,
    /// Missing values in a non-default context: `default` inherits from the
    /// nearest ancestor context; `none` leaves the attribute absent.
    #[serde(default = "default_context_fallback")]
    #[schemars(extend("enum" = CONTEXT_FALLBACKS))]
    context_fallback: String,
    /// Contexts that accept writes: `all`, or only the `default` context.
    #[serde(default = "default_context_editable")]
    #[schemars(extend("enum" = CONTEXT_EDITABLE_SCOPES))]
    context_editable: String,
    /// Preview-only in the Catalog web app. API writes remain allowed.
    #[serde(default)]
    readonly: bool,
    /// Include values in search.
    #[serde(default)]
    searchable: bool,
    /// Offer the attribute as an Explorer facet.
    #[serde(default)]
    facetable: bool,
}

fn toml_value_to_json(
    value: Option<toml::Value>,
) -> Result<Option<serde_json::Value>, RepositoryError> {
    value
        .map(|value| {
            serde_json::to_value(value).map_err(|error| {
                RepositoryError::InvalidReusableAttributeDefinition(error.to_string())
            })
        })
        .transpose()
}

fn parse_definition(
    input: &CreateReusableAttribute,
) -> Result<ReusableAttributeDefinition, RepositoryError> {
    let definition = toml::from_str::<ReusableAttributeDefinition>(&input.definition)
        .map_err(|error| RepositoryError::InvalidReusableAttributeDefinition(error.to_string()))?;
    if !is_valid_code(&definition.code) {
        return Err(RepositoryError::InvalidReusableAttributeCode);
    }
    if definition.name.trim().is_empty()
        || !REUSABLE_ATTRIBUTE_VALUE_TYPES.contains(&definition.value_type.as_str())
        || definition.default_value.is_some()
            && matches!(definition.value_type.as_str(), "relationship" | "file")
        || !CONTEXT_FALLBACKS.contains(&definition.context_fallback.as_str())
        || !CONTEXT_EDITABLE_SCOPES.contains(&definition.context_editable.as_str())
    {
        return Err(RepositoryError::InvalidReusableAttributeDefinition(
            "invalid context policy or blank name".to_owned(),
        ));
    }
    catalog_lexicon::parse(&definition.name).map_err(|error| {
        RepositoryError::InvalidReusableAttributeDefinition(format!(
            "name has an invalid lexicon reference: {error}"
        ))
    })?;
    if let Some(schema) = toml_value_to_json(definition.value_schema.clone())? {
        catalog_validation::validate_json_schema_definition(&schema)
            .map_err(RepositoryError::InvalidReusableAttributeDefinition)?;
        if let Some(default) = toml_value_to_json(definition.default_value.clone())? {
            catalog_validation::status::validate_status_transition(&schema, &Value::Null, &default)
                .map_err(RepositoryError::InvalidReusableAttributeDefinition)?;
        }
        if schema.get(catalog_validation::status::STATUS_KEY).is_some()
            && definition.value_type != "string"
        {
            return Err(RepositoryError::InvalidReusableAttributeDefinition(
                "status is only supported on string attributes".into(),
            ));
        }
    }
    Ok(definition)
}

impl CatalogRepository {
    pub async fn create_reusable_attribute(
        &self,
        input: CreateReusableAttribute,
    ) -> Result<ReusableAttribute, RepositoryError> {
        let definition = parse_definition(&input)?;
        let mut transaction = self.pool.begin().await?;
        let definition_id = Uuid::new_v4();
        let revision_id = Uuid::new_v4();
        let ws = workspace(self);
        let namespace: String =
            sqlx::query_scalar("SELECT slug FROM workspaces WHERE id = $1 AND deleted_at IS NULL")
                .bind(ws)
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::query("INSERT INTO reusable_attribute_definitions (id, workspace_id, namespace, code, name) VALUES ($1, $2, $3, $4, $5)")
            .bind(definition_id).bind(ws).bind(&namespace).bind(&definition.code).bind(&definition.name)
            .execute(&mut *transaction).await?;
        self.insert_reusable_revision(
            &mut transaction,
            revision_id,
            definition_id,
            1,
            &input,
            definition,
        )
        .await?;
        transaction.commit().await?;
        self.reusable_attribute_revision(revision_id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute revision"))
    }

    pub async fn create_reusable_attribute_revision(
        &self,
        definition_id: Uuid,
        input: CreateReusableAttribute,
    ) -> Result<ReusableAttribute, RepositoryError> {
        let definition = parse_definition(&input)?;
        let mut transaction = self.pool.begin().await?;
        let ws = workspace(self);
        let existing: Option<(String, String)> = sqlx::query_as("SELECT namespace, code FROM reusable_attribute_definitions WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL FOR UPDATE")
            .bind(definition_id).bind(ws).fetch_optional(&mut *transaction).await?;
        let Some((_namespace, code)) = existing else {
            return Err(RepositoryError::NotFound("reusable attribute"));
        };
        if code != definition.code {
            return Err(RepositoryError::InvalidReusableAttributeDefinition(
                "code is immutable".to_owned(),
            ));
        }
        let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) + 1 FROM reusable_attribute_revisions WHERE definition_id = $1 AND workspace_id = $2")
            .bind(definition_id).bind(ws).fetch_one(&mut *transaction).await?;
        let revision_id = Uuid::new_v4();
        self.insert_reusable_revision(
            &mut transaction,
            revision_id,
            definition_id,
            version,
            &input,
            definition,
        )
        .await?;
        transaction.commit().await?;
        self.reusable_attribute_revision(revision_id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute revision"))
    }

    async fn insert_reusable_revision(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        revision_id: Uuid,
        definition_id: Uuid,
        version: i64,
        input: &CreateReusableAttribute,
        definition: ReusableAttributeDefinition,
    ) -> Result<(), RepositoryError> {
        sqlx::query(r#"INSERT INTO reusable_attribute_revisions (id, workspace_id, definition_id, version, value_type, value_schema, default_value, file_policy, target_blueprint_code, cardinality, target_cardinality, tags, context_fallback, context_editable, readonly, searchable, facetable, status, definition)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,'draft',$18)"#)
            .bind(revision_id).bind(workspace(self)).bind(definition_id).bind(version)
            .bind(definition.value_type).bind(toml_value_to_json(definition.value_schema)?).bind(toml_value_to_json(definition.default_value)?).bind(toml_value_to_json(definition.file_policy)?)
            .bind(definition.target_blueprint_code).bind(definition.cardinality).bind(definition.target_cardinality)
            .bind(serde_json::json!(definition.tags)).bind(definition.context_fallback).bind(definition.context_editable).bind(definition.readonly)
            .bind(definition.searchable).bind(definition.facetable).bind(&input.definition).execute(&mut **transaction).await?;
        Ok(())
    }

    pub async fn publish_reusable_attribute_revision(
        &self,
        revision_id: Uuid,
    ) -> Result<ReusableAttribute, RepositoryError> {
        let result = sqlx::query("UPDATE reusable_attribute_revisions SET status = 'published', published_at = COALESCE(published_at, now()), updated_at = now() WHERE id = $1 AND workspace_id = $2 AND status = 'draft'")
            .bind(revision_id).bind(workspace(self)).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(
                "draft reusable attribute revision",
            ));
        }
        self.reusable_attribute_revision(revision_id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute revision"))
    }

    pub async fn list_reusable_attributes(
        &self,
        include_drafts: bool,
    ) -> Result<Vec<ReusableAttribute>, RepositoryError> {
        let status = if include_drafts {
            None
        } else {
            Some("published")
        };
        Ok(sqlx::query_as::<_, Db<ReusableAttribute>>(r#"SELECT r.id, r.definition_id, d.namespace, d.code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, r.status, r.published_at, r.definition
            FROM reusable_attribute_revisions r JOIN reusable_attribute_definitions d ON d.id = r.definition_id
            WHERE r.workspace_id = $1 AND d.deleted_at IS NULL AND ($2::text IS NULL OR r.status = $2)
            ORDER BY d.namespace, d.code, r.version DESC"#)
            .bind(workspace(self)).bind(status).fetch_all(&self.pool).await?
        .into_domain())
    }

    /// Resolves a reusable field from entity attachments, never from a newer
    /// registry revision that an existing entity has not pinned.
    pub async fn attached_reusable_attribute_for_blueprint(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
        qualified_code: &str,
    ) -> Result<Option<ReusableAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<ReusableAttribute>>(r#"SELECT r.id, r.definition_id, d.namespace, d.code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, r.status, r.published_at, r.definition
            FROM entities e
            JOIN attributes a ON a.entity_id = e.id AND a.deleted_at IS NULL
            JOIN reusable_attribute_revisions r ON r.id = a.reusable_attribute_revision_id
            JOIN reusable_attribute_definitions d ON d.id = r.definition_id
            WHERE e.workspace_id = $1 AND e.blueprint_id = $2 AND e.blueprint_version = $3
              AND e.deleted_at IS NULL AND a.code = $4 AND r.searchable
            ORDER BY r.version DESC LIMIT 1"#)
            .bind(workspace(self)).bind(blueprint_id).bind(blueprint_version).bind(qualified_code)
            .fetch_optional(&self.pool).await?
        .into_domain())
    }

    pub async fn reusable_attribute_by_qualified_code(
        &self,
        qualified_code: &str,
    ) -> Result<Option<ReusableAttribute>, RepositoryError> {
        let Some((namespace, code)) = qualified_code.split_once(':') else {
            return Ok(None);
        };
        Ok(sqlx::query_as::<_, Db<ReusableAttribute>>(r#"SELECT r.id, r.definition_id, d.namespace, d.code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, r.status, r.published_at, r.definition
            FROM reusable_attribute_revisions r JOIN reusable_attribute_definitions d ON d.id = r.definition_id
            WHERE r.workspace_id = $1 AND d.namespace = $2 AND d.code = $3 AND r.status = 'published'
            ORDER BY r.version DESC LIMIT 1"#)
            .bind(workspace(self)).bind(namespace).bind(code).fetch_optional(&self.pool).await?
        .into_domain())
    }

    pub async fn reusable_attribute_revision(
        &self,
        revision_id: Uuid,
    ) -> Result<Option<ReusableAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<ReusableAttribute>>(r#"SELECT r.id, r.definition_id, d.namespace, d.code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, r.status, r.published_at, r.definition
            FROM reusable_attribute_revisions r JOIN reusable_attribute_definitions d ON d.id = r.definition_id
            WHERE r.id = $1 AND r.workspace_id = $2 AND d.deleted_at IS NULL"#)
            .bind(revision_id).bind(workspace(self)).fetch_optional(&self.pool).await?
        .into_domain())
    }

    pub async fn create_reusable_attribute_group(
        &self,
        input: CreateReusableAttributeGroup,
    ) -> Result<ReusableAttributeGroup, RepositoryError> {
        if !is_valid_code(&input.code) || input.name.trim().is_empty() {
            return Err(RepositoryError::InvalidReusableAttributeDefinition(
                "invalid reusable attribute group".to_owned(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let ws = workspace(self);
        let mut definitions = HashSet::new();
        for revision_id in &input.reusable_attribute_revision_ids {
            let definition_id: Option<Uuid> = sqlx::query_scalar("SELECT definition_id FROM reusable_attribute_revisions WHERE id = $1 AND workspace_id = $2 AND status = 'published' FOR UPDATE")
                .bind(revision_id).bind(ws).fetch_optional(&mut *transaction).await?;
            let definition_id =
                definition_id.ok_or(RepositoryError::ReusableAttributeNotPublished)?;
            if !definitions.insert(definition_id) {
                return Err(RepositoryError::ReusableAttributeAlreadyAttached);
            }
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO reusable_attribute_groups (id, workspace_id, code, name, position) VALUES ($1,$2,$3,$4,$5)")
            .bind(id).bind(ws).bind(&input.code).bind(&input.name).bind(input.position).execute(&mut *transaction).await?;
        for (position, revision_id) in input.reusable_attribute_revision_ids.iter().enumerate() {
            sqlx::query("INSERT INTO reusable_attribute_group_members (group_id, reusable_attribute_revision_id, position) VALUES ($1,$2,$3)")
                .bind(id).bind(revision_id).bind(position as i64).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        self.get_reusable_attribute_group(id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute group"))
    }

    pub async fn list_reusable_attribute_groups(
        &self,
    ) -> Result<Vec<ReusableAttributeGroup>, RepositoryError> {
        let groups = sqlx::query_as::<_, (Uuid, String, String, i64)>("SELECT id, code, name, position FROM reusable_attribute_groups WHERE workspace_id = $1 ORDER BY position, code")
            .bind(workspace(self)).fetch_all(&self.pool).await?;
        let mut result = Vec::with_capacity(groups.len());
        for (id, code, name, position) in groups {
            result.push(self.group_with_members(id, code, name, position).await?);
        }
        Ok(result)
    }

    async fn get_reusable_attribute_group(
        &self,
        id: Uuid,
    ) -> Result<Option<ReusableAttributeGroup>, RepositoryError> {
        let group = sqlx::query_as::<_, (Uuid, String, String, i64)>("SELECT id, code, name, position FROM reusable_attribute_groups WHERE id = $1 AND workspace_id = $2")
            .bind(id).bind(workspace(self)).fetch_optional(&self.pool).await?;
        match group {
            Some((id, code, name, position)) => Ok(Some(
                self.group_with_members(id, code, name, position).await?,
            )),
            None => Ok(None),
        }
    }

    async fn group_with_members(
        &self,
        id: Uuid,
        code: String,
        name: String,
        position: i64,
    ) -> Result<ReusableAttributeGroup, RepositoryError> {
        let reusable_attribute_revision_ids = sqlx::query_scalar("SELECT reusable_attribute_revision_id FROM reusable_attribute_group_members WHERE group_id = $1 ORDER BY position")
            .bind(id).fetch_all(&self.pool).await?;
        Ok(ReusableAttributeGroup {
            id,
            code,
            name,
            position,
            reusable_attribute_revision_ids,
        })
    }

    pub async fn attach_reusable_attribute(
        &self,
        entity_id: Uuid,
        input: AttachReusableAttribute,
    ) -> Result<EntityReusableAttribute, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let attachment_id = self
            .attach_reusable_attribute_in_transaction(
                &mut transaction,
                &entity,
                input.reusable_attribute_revision_id,
            )
            .await?;
        transaction.commit().await?;
        self.entity_reusable_attributes(entity_id)
            .await?
            .into_iter()
            .find(|item| item.attachment_id == attachment_id)
            .ok_or(RepositoryError::NotFound("reusable attribute attachment"))
    }

    pub async fn attach_reusable_attribute_group(
        &self,
        entity_id: Uuid,
        group_id: Uuid,
    ) -> Result<Vec<EntityReusableAttribute>, RepositoryError> {
        let group = self
            .get_reusable_attribute_group(group_id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute group"))?;
        let mut transaction = self.pool.begin().await?;
        let entity = self.lock_entity(&mut transaction, entity_id).await?;
        let mut attachment_ids = Vec::with_capacity(group.reusable_attribute_revision_ids.len());
        for revision_id in group.reusable_attribute_revision_ids {
            attachment_ids.push(
                self.attach_reusable_attribute_in_transaction(
                    &mut transaction,
                    &entity,
                    revision_id,
                )
                .await?,
            );
        }
        transaction.commit().await?;
        Ok(self
            .entity_reusable_attributes(entity_id)
            .await?
            .into_iter()
            .filter(|item| attachment_ids.contains(&item.attachment_id))
            .collect())
    }

    async fn attach_reusable_attribute_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        entity: &Entity,
        revision_id: Uuid,
    ) -> Result<Uuid, RepositoryError> {
        let ws = workspace(self);
        let revision = self
            .reusable_attribute_revision_in_transaction(transaction, revision_id)
            .await?
            .ok_or(RepositoryError::NotFound("reusable attribute revision"))?;
        if revision.status != "published" {
            return Err(RepositoryError::ReusableAttributeNotPublished);
        }
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM entity_reusable_attribute_attachments WHERE workspace_id = $1 AND entity_id = $2")
            .bind(ws).bind(entity.id).fetch_one(&mut **transaction).await?;
        let attachment_id = Uuid::new_v4();
        let attached = sqlx::query("INSERT INTO entity_reusable_attribute_attachments (id, workspace_id, entity_id, reusable_attribute_definition_id, reusable_attribute_revision_id, position) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (workspace_id, entity_id, reusable_attribute_definition_id) DO NOTHING")
            .bind(attachment_id).bind(ws).bind(entity.id).bind(revision.definition_id).bind(revision.id).bind(position).execute(&mut **transaction).await?;
        if attached.rows_affected() == 0 {
            return Err(RepositoryError::ReusableAttributeAlreadyAttached);
        }
        let attribute_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO attributes (id, workspace_id, entity_id, reusable_attribute_revision_id, code, value_type, value_schema, default_value, file_policy, target_blueprint_code, cardinality, target_cardinality, tags, context_fallback, context_editable, readonly, position)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)"#)
            .bind(attribute_id).bind(ws).bind(entity.id).bind(revision.id).bind(format!("{}:{}", revision.namespace, revision.code))
            .bind(&revision.value_type).bind(&revision.value_schema).bind(&revision.default_value).bind(&revision.file_policy)
            .bind(&revision.target_blueprint_code).bind(&revision.cardinality).bind(&revision.target_cardinality).bind(&revision.tags)
            .bind(&revision.context_fallback).bind(&revision.context_editable).bind(revision.readonly).bind(position).execute(&mut **transaction).await?;
        if let Some(default_value) = revision.default_value {
            let context_id = self.resolve_context_id(transaction, None).await?;
            self.insert_value(
                transaction,
                entity,
                NewAttributeValue::Scalar {
                    attribute_id: Some(attribute_id),
                    attribute_code: None,
                    context_id,
                    value: default_value,
                },
            )
            .await?;
        }
        self.validate_status_values(transaction, entity).await?;
        let preview = Self::build_preview_projection(transaction, entity.id).await?;
        self.store_preview(transaction, entity.id, preview).await?;
        Ok(attachment_id)
    }

    pub async fn entity_reusable_attributes(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityReusableAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<EntityReusableAttribute>>(r#"SELECT aat.id AS attachment_id, a.id AS attribute_id, d.id AS definition_id, r.id AS revision_id, d.namespace, d.namespace || ':' || d.code AS code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, aat.position
          FROM entity_reusable_attribute_attachments aat
          JOIN reusable_attribute_revisions r ON r.id = aat.reusable_attribute_revision_id
          JOIN reusable_attribute_definitions d ON d.id = r.definition_id
          JOIN attributes a ON a.entity_id = aat.entity_id AND a.reusable_attribute_revision_id = r.id AND a.workspace_id = aat.workspace_id
          WHERE aat.workspace_id = $1 AND aat.entity_id = $2 AND a.deleted_at IS NULL
          ORDER BY aat.position"#).bind(workspace(self)).bind(entity_id).fetch_all(&self.pool).await?
        .into_domain())
    }

    async fn reusable_attribute_revision_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        revision_id: Uuid,
    ) -> Result<Option<ReusableAttribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Db<ReusableAttribute>>(r#"SELECT r.id, r.definition_id, d.namespace, d.code, d.name, r.version, r.value_type, r.value_schema, r.default_value, r.file_policy, r.target_blueprint_code, r.cardinality, r.target_cardinality, r.tags, r.context_fallback, r.context_editable, r.readonly, r.searchable, r.facetable, r.status, r.published_at, r.definition FROM reusable_attribute_revisions r JOIN reusable_attribute_definitions d ON d.id = r.definition_id WHERE r.id = $1 AND r.workspace_id = $2 FOR UPDATE"#)
            .bind(revision_id).bind(workspace(self)).fetch_optional(&mut **transaction).await?
        .into_domain())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use schemars::generate::SchemaSettings;

    use super::{CreateReusableAttribute, ReusableAttributeDefinition, parse_definition};

    /// The JSON Schema for reusable attribute definition TOML, published as
    /// `contracts/reusable-attribute-definition-v1.schema.json`.
    fn definition_json_schema() -> serde_json::Value {
        SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<ReusableAttributeDefinition>()
            .to_value()
    }

    const CONTRACT: &str = "contracts/reusable-attribute-definition-v1.schema.json";
    const UPDATE_CONTRACTS: &str = "UPDATE_CONTRACTS";

    #[test]
    fn definition_schema_contract_is_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join(CONTRACT);
        let mut rendered = serde_json::to_string_pretty(&definition_json_schema()).unwrap();
        rendered.push('\n');
        if std::env::var_os(UPDATE_CONTRACTS).is_some() {
            fs::write(&path, rendered).unwrap();
            return;
        }
        assert!(
            fs::read_to_string(&path).unwrap_or_default() == rendered,
            "{CONTRACT} is stale; run `just contracts`"
        );
    }

    #[test]
    fn schema_matches_parser_for_representative_definitions() {
        let schema = definition_json_schema();
        for (definition, accepted) in [
            (
                "code = \"sku\"\nname = \"SKU\"\nvalue_type = \"string\"\n",
                true,
            ),
            (
                "code = \"brand\"\nname = \"Brand\"\nvalue_type = \"relationship\"\ntarget_blueprint_code = \"brand\"\ncardinality = \"one\"\nsearchable = true\n",
                true,
            ),
            (
                "code = \"images\"\nname = \"Images\"\nvalue_type = \"file\"\nfile_policy = { cardinality = \"many\" }\ntags = [\"hidden:form\"]\n",
                true,
            ),
            (
                "code = \"price\"\nname = \"Price\"\nvalue_type = \"number\"\nvalue_schema = { minimum = 0 }\ndefault_value = 0\ncontext_editable = \"default\"\n",
                true,
            ),
            (
                "code = \"sku\"\nname = \"SKU\"\nvalue_type = \"json\"\n",
                false,
            ),
            (
                "code = \"sku\"\nname = \"SKU\"\nvalue_type = \"string\"\nlabel = \"SKU\"\n",
                false,
            ),
            (
                "code = \"sku\"\nname = \"SKU\"\nvalue_type = \"string\"\ncontext_fallback = \"parent\"\n",
                false,
            ),
        ] {
            let parsed = parse_definition(&CreateReusableAttribute {
                definition: definition.to_owned(),
            });
            assert_eq!(parsed.is_ok(), accepted, "{definition}");
            let instance =
                serde_json::to_value(toml::from_str::<toml::Value>(definition).unwrap()).unwrap();
            let violations = catalog_validation::validate_json_schema(&schema, &instance).unwrap();
            assert_eq!(
                violations.is_empty(),
                accepted,
                "{definition}: {violations:?}"
            );
        }
    }
}
