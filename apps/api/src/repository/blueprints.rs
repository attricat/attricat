use super::*;
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
        self.commit_mutation(transaction).await?;
        Ok(result)
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
        self.commit_mutation(transaction).await?;
        Ok(result)
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
            attributes.push(
                sqlx::query_as::<_, Attribute>(
                    r#"INSERT INTO attributes (id, workspace_id, blueprint_id, blueprint_version, code, value_type, value_schema, target_blueprint_code, tags, context_fallback, context_editable, position)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                       RETURNING id, blueprint_id, blueprint_version, code, value_type, value_schema, target_blueprint_code, tags, context_fallback, context_editable, position, created_at, updated_at, deleted_at"#,
                )
                .bind(Uuid::new_v4())
                .bind(self.workspace_id.unwrap_or(Self::DEFAULT_WORKSPACE_ID))
                .bind(blueprint_id)
                .bind(version)
                .bind(attribute.code)
                .bind(attribute.value_type)
                .bind(attribute.value_schema)
                .bind(attribute.target_blueprint)
                .bind(serde_json::to_value(attribute.tags).expect("attribute tags serialize"))
                .bind(attribute.context_fallback)
                .bind(attribute.context_editable)
                .bind(attribute.position)
                .fetch_one(&mut **transaction)
                .await?,
            );
        }

        Ok(BlueprintWithAttributes {
            blueprint,
            attributes,
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
        }
        self.commit_mutation(transaction).await?;
        self.get_blueprint_revision(blueprint_id, version)
            .await?
            .ok_or(RepositoryError::NotFound("blueprint version"))
    }

    pub async fn list_attributes(
        &self,
        blueprint_id: Uuid,
        blueprint_version: i64,
    ) -> Result<Vec<Attribute>, RepositoryError> {
        Ok(sqlx::query_as::<_, Attribute>(
            r#"SELECT id, blueprint_id, blueprint_version, code, value_type, value_schema, target_blueprint_code, tags, context_fallback, context_editable, position, created_at, updated_at, deleted_at
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

    async fn with_attributes(
        &self,
        blueprint: Option<Blueprint>,
    ) -> Result<Option<BlueprintWithAttributes>, RepositoryError> {
        match blueprint {
            Some(blueprint) => {
                let attributes = self
                    .list_attributes(blueprint.id, blueprint.version)
                    .await?;
                Ok(Some(BlueprintWithAttributes {
                    blueprint,
                    attributes,
                }))
            }
            None => Ok(None),
        }
    }
}
