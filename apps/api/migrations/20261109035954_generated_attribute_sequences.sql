-- Attributes whose values core assigns on record creation, and the gapless
-- counters that number them. The repository increments a counter inside the
-- record-create transaction, so a rolled-back create returns its number.
ALTER TABLE attributes ADD COLUMN generated JSONB;

-- One counter per blueprint family (`blueprints.id`, whatever the version)
-- and attribute code; `period` is '' unless numbering restarts each year,
-- month or day. There is no foreign key to `blueprints`, whose key includes
-- the version, so a deleted and restored family keeps its numbers.
CREATE TABLE attribute_sequences (
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    blueprint_id UUID NOT NULL,
    attribute_code TEXT NOT NULL,
    period TEXT NOT NULL DEFAULT '',
    next_value BIGINT NOT NULL CHECK (next_value >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, blueprint_id, attribute_code, period)
);
