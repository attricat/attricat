-- A channel can require checks to pass before an entity is published to it:
-- enabled rules selected by code and, optionally, the entity schema and
-- blueprint checks in the channel context. Evaluation lives in Rust.
ALTER TABLE publication_channels
    ADD COLUMN required_rule_codes TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN require_valid_entity BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT publication_channels_required_rule_codes_bounded
        CHECK (cardinality(required_rule_codes) <= 32);
