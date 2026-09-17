use super::values::{NativeValue, ValueType};
use super::*;
use crate::domain_events::{
    BLUEPRINT_CREATED_V1, BLUEPRINT_PUBLISHED_V1, BLUEPRINT_REVISION_CREATED_V1,
    BlueprintRevisionV1, EventSource, EventSourceKind, NewDomainEvent,
};
use crate::model::TablePathAttribute;
use catalog_blueprint::{ViewDefinition, parse};
use catalog_validation::validate_json_schema;
use serde_json::json;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct LatestBlueprint {
    version: i64,
    code: Option<String>,
}

impl CatalogRepository {
    pub async fn list_entity_blueprints(
        &self,
        include_drafts: bool,
    ) -> Result<Vec<Blueprint>, RepositoryError> {
        Ok(sqlx::query_as::<_, Blueprint>(
            r#"SELECT DISTINCT ON (id)
                     id, code, name, kind, version, views, includes, entity_schema, status, published_at, created_at, updated_at,
                    deleted_at, definition, definition_hash
               FROM blueprints
               WHERE kind = 'entity' AND deleted_at IS NULL
                 AND ($1 OR status = 'published')
                 AND workspace_id = $2
               ORDER BY id, version DESC"#,
        )
        .bind(include_drafts)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_blueprints(&self) -> Result<Vec<Blueprint>, RepositoryError> {
        Ok(sqlx::query_as::<_, Blueprint>(
            r#"SELECT DISTINCT ON (id)
                     id, code, name, kind, version, views, includes, entity_schema, status, published_at, created_at, updated_at,
                    deleted_at, definition, definition_hash
               FROM blueprints
               WHERE deleted_at IS NULL AND workspace_id = $1
               ORDER BY id, version DESC"#,
        )
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_blueprint_revisions(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Vec<Blueprint>, RepositoryError> {
        Ok(sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
               ORDER BY version DESC"#,
        )
        .bind(blueprint_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_blueprint(
        &self,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let compiled = compile_definition(
            &mut transaction,
            self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID),
            &input.definition,
        )
        .await?;
        // PostgreSQL cannot express uniqueness across all revisions with the
        // versioned primary key. Serialize writers for this code so the
        // existence check and first-revision insert are one logical operation.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(&compiled.code)
            .execute(&mut *transaction)
            .await?;
        let code_exists = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM blueprints WHERE code = $1 AND workspace_id = $2 LIMIT 1",
        )
        .bind(&compiled.code)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .is_some();
        if code_exists {
            return Err(RepositoryError::BlueprintCodeTaken);
        }

        let result = self
            .insert_blueprint_revision_in_transaction(
                &mut transaction,
                Uuid::new_v4(),
                1,
                input.definition,
                compiled,
            )
            .await?;
        self.commit_mutation_with_event(
            transaction,
            blueprint_event(self, BLUEPRINT_CREATED_V1, &result.blueprint),
        )
        .await?;
        self.get_blueprint_by_code_and_version(&result.blueprint.code, result.blueprint.version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))
    }

    pub async fn create_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        input: CreateBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let latest = sqlx::query_as::<_, LatestBlueprint>(
            "SELECT version, code FROM blueprints WHERE id = $1 AND workspace_id = $2 ORDER BY version DESC LIMIT 1 FOR UPDATE",
        )
        .bind(blueprint_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint"))?;

        let compiled = compile_definition(
            &mut transaction,
            self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID),
            &input.definition,
        )
        .await?;
        if latest.code.as_deref() != Some(compiled.code.as_str()) {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "blueprint code cannot change across revisions".to_owned(),
            ));
        }

        let result = self
            .insert_blueprint_revision_in_transaction(
                &mut transaction,
                blueprint_id,
                latest.version + 1,
                input.definition,
                compiled,
            )
            .await?;
        self.commit_mutation_with_event(
            transaction,
            blueprint_event(self, BLUEPRINT_REVISION_CREATED_V1, &result.blueprint),
        )
        .await?;
        self.get_blueprint_by_code_and_version(&result.blueprint.code, result.blueprint.version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint"))
    }

    async fn insert_blueprint_revision_in_transaction(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        blueprint_id: Uuid,
        version: i64,
        definition: String,
        compiled: catalog_blueprint::CompiledBlueprint,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let includes = serde_json::to_value(&compiled.includes).map_err(|error| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "could not serialize generated includes: {error}"
            ))
        })?;
        let views = serde_json::to_value(&compiled.views).map_err(|error| {
            RepositoryError::InvalidBlueprintDefinition(format!(
                "could not serialize generated views: {error}"
            ))
        })?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"INSERT INTO blueprints (id, workspace_id, code, name, kind, version, includes, views, entity_schema, status, definition, definition_hash)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'draft', $10, $11)
               RETURNING id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash"#,
        )
        .bind(blueprint_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .bind(compiled.code)
        .bind(compiled.name)
        .bind(compiled.kind.as_str())
        .bind(version)
        .bind(includes)
        .bind(views)
        .bind(compiled.entity_schema)
        .bind(definition)
        .bind(compiled.raw_definition_hash)
        .fetch_one(&mut **transaction)
        .await?;

        let mut attributes = Vec::with_capacity(compiled.attributes.len());
        for attribute in compiled.attributes {
            validate_attribute_default_value(&attribute)?;
            attributes.push(
                sqlx::query_as::<_, Attribute>(
                    r#"INSERT INTO attributes (id, workspace_id, blueprint_id, blueprint_version, code, value_type, value_schema, extension_type, default_value, file_policy, target_blueprint_code, cardinality, target_cardinality, tags, context_fallback, context_editable, readonly, position)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                       RETURNING id, blueprint_id, blueprint_version, code, value_type, value_schema, extension_type, default_value, file_policy, target_blueprint_code, cardinality, target_cardinality, tags, context_fallback, context_editable, readonly, position, created_at, updated_at, deleted_at"#,
                )
                .bind(Uuid::new_v4())
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .bind(blueprint_id)
                .bind(version)
                .bind(attribute.code)
                .bind(attribute.value_type)
                .bind(attribute.value_schema)
                .bind(attribute.extension_type)
                .bind(attribute.default_value)
                .bind(attribute.file_policy.map(|policy| serde_json::to_value(policy).expect("file policy serializes")))
                .bind(attribute.target_blueprint)
                .bind(attribute.cardinality)
                .bind(attribute.target_cardinality)
                .bind(serde_json::to_value(attribute.tags).expect("attribute tags serialize"))
                .bind(attribute.context_fallback)
                .bind(attribute.context_editable)
                .bind(attribute.readonly)
                .bind(attribute.position)
                .fetch_one(&mut **transaction)
                .await?,
            );
        }

        Ok(BlueprintWithAttributes {
            blueprint,
            attributes,
            table_path_attributes: Vec::new(),
        })
    }

    pub async fn get_current_blueprint(
        &self,
        blueprint_id: Uuid,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND workspace_id = $2 AND status = 'published' AND deleted_at IS NULL
               ORDER BY version DESC
               LIMIT 1"#,
        )
        .bind(blueprint_id)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        version: i64,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND version = $2 AND workspace_id = $3 AND deleted_at IS NULL"#,
        )
        .bind(blueprint_id)
        .bind(version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_by_code(
        &self,
        code: &str,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND status = 'published' AND workspace_id = $2 AND deleted_at IS NULL
               ORDER BY version DESC
               LIMIT 1"#,
        )
        .bind(code)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_by_code_including_drafts(
        &self,
        code: &str,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND workspace_id = $2 AND deleted_at IS NULL
               ORDER BY version DESC
               LIMIT 1"#,
        )
        .bind(code)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;
        self.with_attributes(blueprint).await
    }

    pub async fn get_blueprint_by_code_and_version(
        &self,
        code: &str,
        version: i64,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND version = $2 AND workspace_id = $3 AND deleted_at IS NULL"#,
        )
        .bind(code)
        .bind(version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;

        self.with_attributes(blueprint).await
    }

    pub async fn get_published_blueprint_by_code_and_version(
        &self,
        code: &str,
        version: i64,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        validate_code(code)?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE code = $1 AND version = $2 AND status = 'published' AND workspace_id = $3 AND deleted_at IS NULL"#,
        )
        .bind(code)
        .bind(version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&self.pool)
        .await?;
        self.with_attributes(blueprint).await
    }

    pub async fn publish_blueprint_revision(
        &self,
        blueprint_id: Uuid,
        version: i64,
    ) -> Result<BlueprintWithAttributes, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let blueprint = sqlx::query_as::<_, Blueprint>(
            r#"SELECT id, code, name, kind, version, includes, views, entity_schema, status, published_at, created_at, updated_at, deleted_at, definition, definition_hash
               FROM blueprints
               WHERE id = $1 AND version = $2 AND workspace_id = $3 AND deleted_at IS NULL
               FOR UPDATE"#,
        )
        .bind(blueprint_id)
        .bind(version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(RepositoryError::NotFound("blueprint version"))?;
        if blueprint.status == "draft" {
            // Installed renderers and target revisions can change after a draft
            // is saved, so repeat resolution at the publication boundary.
            let compiled = crate::blueprint_resolver::compile_definition(
                &mut transaction,
                self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID),
                &blueprint.definition,
            )
            .await?;
            // A draft may sit while its provider is upgraded. Publication is
            // the pinning boundary, so persist the freshly resolved immutable
            // declaration rather than relying on the earlier draft snapshot.
            for attribute in compiled
                .attributes
                .iter()
                .filter(|attribute| attribute.extension_type.is_some())
            {
                validate_attribute_default_value(attribute)?;
                sqlx::query(
                    "UPDATE attributes SET value_type = $1, value_schema = $2, extension_type = $3, updated_at = now() WHERE blueprint_id = $4 AND blueprint_version = $5 AND code = $6 AND workspace_id = $7",
                )
                .bind(&attribute.value_type)
                .bind(&attribute.value_schema)
                .bind(&attribute.extension_type)
                .bind(blueprint_id)
                .bind(version)
                .bind(&attribute.code)
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .execute(&mut *transaction)
                .await?;
            }
            self.validate_publication_roles(&mut transaction, &blueprint.definition)
                .await?;
            self.validate_publication_extension_layout(&blueprint.definition)
                .await?;
            let includes_published = sqlx::query_scalar::<_, bool>(
                r#"SELECT NOT EXISTS (
                       SELECT 1
                       FROM jsonb_array_elements($1) include
                       WHERE NOT EXISTS (
                           SELECT 1
                           FROM blueprints included
                           WHERE included.code = include ->> 'code'
                             AND included.workspace_id = $2
                             AND included.version = (include ->> 'version')::bigint
                             AND included.kind = 'mixin'
                             AND included.status = 'published'
                             AND included.deleted_at IS NULL
                       )
                   )"#,
            )
            .bind(&blueprint.includes)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .fetch_one(&mut *transaction)
            .await?;
            if !includes_published {
                return Err(RepositoryError::BlueprintNotPublished);
            }
            sqlx::query(
                "UPDATE blueprints SET status = 'published', published_at = now(), updated_at = now() WHERE id = $1 AND version = $2 AND workspace_id = $3",
            )
            .bind(blueprint_id)
            .bind(version)
            .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
            .execute(&mut *transaction)
            .await?;
            self.commit_mutation_with_event(
                transaction,
                blueprint_event(self, BLUEPRINT_PUBLISHED_V1, &blueprint),
            )
            .await?;
        } else {
            self.commit_mutation(transaction).await?;
        }
        self.get_blueprint_revision(blueprint_id, version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    /// A layout may retain unknown keys for a removed extension, but a key that
    /// currently resolves must stay in the outlet declared by that enabled
    /// contribution. This prevents a blueprint from moving an extension into
    /// an incompatible host surface at publication time.
    async fn validate_publication_extension_layout(
        &self,
        definition: &str,
    ) -> Result<(), RepositoryError> {
        let blueprint = parse(definition).map_err(RepositoryError::invalid_blueprint_definition)?;
        let Some(ViewDefinition::ExtensionLayout { outlets, .. }) =
            blueprint.views.get("extension_layout")
        else {
            return Ok(());
        };
        let contributions = self.enabled_client_extension_contributions().await?;
        for (outlet, layout) in outlets {
            for key in layout.order.iter().chain(&layout.hidden) {
                if let Some(contribution) = contributions
                    .iter()
                    .find(|item| item.contribution_key == *key)
                {
                    let contribution_outlet = contribution
                        .outlet
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(|error| {
                            RepositoryError::InvalidBlueprintDefinition(error.to_string())
                        })?
                        .and_then(|value| value.as_str().map(str::to_owned));
                    if contribution_outlet.as_deref() != Some(outlet.as_str()) {
                        return Err(RepositoryError::InvalidBlueprintDefinition(
                            "extension layout contribution is incompatible with its outlet".into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    async fn validate_publication_roles(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        definition: &str,
    ) -> Result<(), RepositoryError> {
        let publication = parse(definition)
            .map_err(RepositoryError::invalid_blueprint_definition)?
            .publication;
        if publication.retain_on_edit_roles.is_empty() {
            return Ok(());
        }
        let workspace_id = self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID);
        let known_roles: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM roles WHERE code = ANY($1) AND (is_system OR workspace_id = $2)",
        )
        .bind(&publication.retain_on_edit_roles)
        .bind(workspace_id)
        .fetch_one(&mut **transaction)
        .await?;
        if known_roles != publication.retain_on_edit_roles.len() as i64 {
            return Err(RepositoryError::InvalidBlueprintDefinition(
                "publication retain_on_edit_roles contains an unknown workspace role".to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn list_attributes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Attribute>(
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, value_schema, extension_type, default_value, file_policy, target_blueprint_code, cardinality, target_cardinality, tags, context_fallback, context_editable, readonly, position, created_at, updated_at, deleted_at
               FROM attributes
               WHERE blueprint_id = $1 AND blueprint_version = $2 AND workspace_id = $3 AND deleted_at IS NULL
               ORDER BY position"#,
        )
        .bind(blueprint_id)
        .bind(blueprint_version)
        .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
        .fetch_all(&self.pool)
        .await?)
    }

    async fn resolve_table_path_attributes(
        &self,
        blueprint: &Blueprint,
        attributes: &[Attribute],
    ) -> Result<Vec<TablePathAttribute>, RepositoryError> {
        let Some(columns) = blueprint
            .views
            .get("table")
            .and_then(|table| table.get("columns"))
            .and_then(Value::as_array)
        else {
            return Ok(Vec::new());
        };
        let mut resolved = Vec::new();
        for path in columns
            .iter()
            .filter_map(|column| column.get("field").and_then(Value::as_str))
        {
            let parts: Vec<_> = path.split('.').collect();
            let Some(first) = attributes
                .iter()
                .find(|attribute| attribute.code == parts[0])
            else {
                continue;
            };
            if parts.len() == 1 {
                if first.value_type != "relationship" {
                    resolved.push(TablePathAttribute {
                        code: path.to_owned(),
                        value_type: first.value_type.clone(),
                        sortable: !matches!(first.value_type.as_str(), "file" | "json"),
                    });
                }
                continue;
            }
            let mut attribute = first.clone();
            let mut sortable = true;
            for field in &parts[1..] {
                if attribute.value_type != "relationship" {
                    break;
                }
                sortable &= attribute.cardinality.as_deref() == Some("one");
                let Some(target) = attribute.target_blueprint_code.as_deref() else {
                    break;
                };
                let Some(next) = sqlx::query_as::<_, Attribute>(
                    r#"SELECT a.id, a.blueprint_id, a.blueprint_version, a.code, a.value_type,
                              a.value_schema, a.extension_type, a.default_value, a.file_policy, a.target_blueprint_code,
                              a.cardinality, a.target_cardinality, a.tags, a.context_fallback,
                              a.context_editable, a.readonly, a.position, a.created_at, a.updated_at,
                              a.deleted_at
                         FROM blueprints b
                         JOIN attributes a ON a.blueprint_id = b.id AND a.blueprint_version = b.version
                        WHERE b.workspace_id = $1 AND b.code = $2
                          AND b.status = 'published' AND b.deleted_at IS NULL
                          AND a.code = $3 AND a.deleted_at IS NULL
                        ORDER BY b.version DESC LIMIT 1"#,
                )
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .bind(target)
                .bind(field)
                .fetch_optional(&self.pool)
                .await?
                else {
                    break;
                };
                attribute = next;
            }
            if !matches!(attribute.value_type.as_str(), "relationship" | "file")
                && attribute.code == parts[parts.len() - 1]
            {
                resolved.push(TablePathAttribute {
                    code: path.to_owned(),
                    value_type: attribute.value_type.clone(),
                    sortable: sortable && attribute.value_type != "json",
                });
            }
        }
        Ok(resolved)
    }

    async fn with_attributes(
        &self,
        blueprint: Option<Blueprint>,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        match blueprint {
            Some(blueprint) => {
                let mut attributes = self
                    .list_attributes(blueprint.id, blueprint.version)
                    .await?;
                // Availability is presentation-only metadata. The pinned
                // declaration remains in the revision even when its provider
                // is disabled, so ordinary blueprint/entity reads cannot fail
                // and clients can choose the host-owned read-only fallback.
                let enabled_manifests = sqlx::query_as::<_, (String, Value)>(
                    "SELECT i.extension_id, r.manifest FROM extension_installations i JOIN installed_extension_releases r ON r.id = i.installed_release_id JOIN workspaces w ON w.id = i.workspace_id WHERE i.workspace_id = $1 AND i.state = 'enabled' AND w.extensions_enabled",
                )
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .fetch_all(&self.pool)
                .await?;
                for attribute in &mut attributes {
                    if let Some(metadata) = attribute.extension_type.as_mut()
                        && let Some(object) = metadata.as_object_mut()
                    {
                        let available = object
                            .get("provider")
                            .and_then(Value::as_str)
                            .zip(object.get("type").and_then(Value::as_str))
                            .zip(object.get("version").and_then(Value::as_str))
                            .zip(object.get("primitive").and_then(Value::as_str))
                            .is_some_and(|(((provider, type_id), version), primitive)| {
                                enabled_manifests
                                    .iter()
                                    .any(|(extension_id, raw_manifest)| {
                                        extension_id == provider
                                                && serde_json::from_value::<
                                                    crate::extensions::Manifest,
                                                >(
                                                    raw_manifest.clone()
                                                )
                                                .ok()
                                                .is_some_and(|manifest| {
                                                    manifest.attribute_types.iter().any(
                                                        |declaration| {
                                                            declaration.id == type_id
                                                                && declaration.version == version
                                                                && declaration.primitive
                                                                    == primitive
                                                        },
                                                    )
                                                })
                                    })
                            });
                        object.insert("available".to_owned(), Value::Bool(available));
                    }
                }
                let table_path_attributes = self
                    .resolve_table_path_attributes(&blueprint, &attributes)
                    .await?;
                Ok(Some(BlueprintWithAttributes {
                    blueprint,
                    attributes,
                    table_path_attributes,
                }))
            }
            None => Ok(None),
        }
    }
}

fn validate_attribute_default_value(
    attribute: &catalog_blueprint::EffectiveAttribute,
) -> Result<(), RepositoryError> {
    let Some(default_value) = &attribute.default_value else {
        return Ok(());
    };
    let native = NativeValue::parse(
        ValueType::parse(&attribute.value_type)?,
        default_value.clone(),
    )
    .map_err(|_| {
        RepositoryError::InvalidBlueprintDefinition(format!(
            "default value for attribute '{}' does not match its value type",
            attribute.code
        ))
    })?;
    if let Some(schema) = &attribute.value_schema
        && let Some(error) = validate_json_schema(schema, &native.json())
            .map_err(RepositoryError::invalid_blueprint_definition)?
            .into_iter()
            .next()
    {
        return Err(RepositoryError::InvalidBlueprintDefinition(format!(
            "default value for attribute '{}' does not match its schema at '{}': {}",
            attribute.code, error.instance_path, error.message
        )));
    }
    Ok(())
}

fn blueprint_event(
    repository: &CatalogRepository,
    event_type: &str,
    blueprint: &Blueprint,
) -> NewDomainEvent {
    NewDomainEvent {
        event_type: event_type.to_owned(),
        aggregate_kind: "blueprint".to_owned(),
        aggregate_id: blueprint.id,
        correlation_id: repository
            .audit_context
            .as_ref()
            .map(|audit| audit.correlation_id)
            .unwrap_or_else(Uuid::new_v4),
        causation_id: None,
        source: EventSource {
            kind: EventSourceKind::Api,
            name: "catalog_api".to_owned(),
        },
        metadata: json!({}),
        payload: serde_json::to_value(BlueprintRevisionV1 {
            blueprint_id: blueprint.id,
            code: blueprint.code.clone(),
            kind: blueprint.kind.clone(),
            version: blueprint.version,
        })
        .expect("blueprint event payload is serializable"),
    }
}
