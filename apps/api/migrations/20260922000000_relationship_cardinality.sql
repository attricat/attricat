-- Relationship cardinality is immutable metadata on a blueprint revision. It is
-- intentionally not a uniqueness constraint: relationship writes remain an
-- application-level invariant and historical revisions remain representable.
ALTER TABLE attributes
    ADD COLUMN relationship_cardinality TEXT
    CHECK (relationship_cardinality IS NULL OR relationship_cardinality = 'one_to_one');
