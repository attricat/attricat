-- Versioned entity-type definitions.
CREATE TABLE blueprints (
    id UUID NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    name TEXT NOT NULL,
    definition TEXT NOT NULL,
    definition_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    PRIMARY KEY (id, version),
    UNIQUE (id, definition_hash)
);

CREATE INDEX blueprints_active_version_idx
    ON blueprints (id, version DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX blueprints_active_name_idx
    ON blueprints (name)
    WHERE deleted_at IS NULL;

-- Attribute definitions belong to a specific entity-type version.
CREATE TABLE attributes (
    id UUID PRIMARY KEY,
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL,
    code TEXT NOT NULL,
    value_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT attributes_blueprint_version_fkey
        FOREIGN KEY (blueprint_id, blueprint_version)
        REFERENCES blueprints (id, version),
    CONSTRAINT attributes_blueprint_version_code_key
        UNIQUE (blueprint_id, blueprint_version, code)
);

CREATE INDEX attributes_active_blueprint_version_idx
    ON attributes (blueprint_id, blueprint_version)
    WHERE deleted_at IS NULL;

-- Catalog item instances. Projections are derived read data, not the EAV source of truth.
CREATE TABLE entities (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    blueprint_id UUID NOT NULL,
    blueprint_version BIGINT NOT NULL,
    projections JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT entities_blueprint_version_fkey
        FOREIGN KEY (blueprint_id, blueprint_version)
        REFERENCES blueprints (id, version)
);

CREATE UNIQUE INDEX entities_active_blueprint_code_idx
    ON entities (blueprint_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX entities_blueprint_version_idx
    ON entities (blueprint_id, blueprint_version);

CREATE INDEX entities_projections_gin_idx
    ON entities USING GIN (projections);

-- Reusable context dimensions such as locale, currency, or channel.
CREATE TABLE attribute_contexts (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    data JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX attribute_contexts_data_gin_idx
    ON attribute_contexts USING GIN (data);

-- Canonical EAV facts with context and immutable value history.
CREATE TABLE attribute_values (
    id UUID PRIMARY KEY,
    entity_id UUID NOT NULL REFERENCES entities (id),
    attribute_id UUID NOT NULL REFERENCES attributes (id),
    context_id UUID REFERENCES attribute_contexts (id),
    value JSONB NOT NULL,
    relationship_target_entity_id UUID REFERENCES entities (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX attribute_values_scalar_history_idx
    ON attribute_values (entity_id, attribute_id, context_id, created_at DESC, id DESC)
    WHERE relationship_target_entity_id IS NULL;

CREATE INDEX attribute_values_relationship_history_idx
    ON attribute_values (
        entity_id,
        attribute_id,
        context_id,
        relationship_target_entity_id,
        created_at DESC,
        id DESC
    )
    WHERE relationship_target_entity_id IS NOT NULL;

CREATE INDEX attribute_values_attribute_idx
    ON attribute_values (attribute_id);

CREATE INDEX attribute_values_relationship_target_entity_idx
    ON attribute_values (relationship_target_entity_id)
    WHERE relationship_target_entity_id IS NOT NULL;
