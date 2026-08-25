-- Keep the hot EAV table to one row per current logical value. Superseded
-- values move to the separately retained history table in the same write
-- transaction.
CREATE TABLE attribute_value_history (
    id UUID NOT NULL,
    entity_id UUID NOT NULL REFERENCES entities (id),
    attribute_id UUID NOT NULL REFERENCES attributes (id),
    context_id UUID REFERENCES attribute_contexts (id),
    relationship_target_entity_id UUID REFERENCES entities (id),
    active BOOLEAN NOT NULL,
    value_text TEXT,
    value_number NUMERIC,
    value_integer BIGINT,
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_time TIME,
    value_time_zone TEXT,
    value_json JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT now()
) PARTITION BY RANGE (archived_at);

CREATE TABLE attribute_value_history_default
    PARTITION OF attribute_value_history DEFAULT;

CREATE INDEX attribute_value_history_entity_archived_idx
    ON attribute_value_history (entity_id, archived_at DESC, id DESC);
CREATE INDEX attribute_value_history_id_idx
    ON attribute_value_history (id);

-- A partition is created before each archive operation. The advisory lock
-- prevents concurrent writers from attempting the same monthly DDL.
