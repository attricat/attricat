-- Relationship cardinality is blueprint metadata enforced by repository
-- transactions. The database deliberately imposes no business constraint on
-- the allowed metadata values.
ALTER TABLE attributes
    DROP CONSTRAINT attributes_relationship_cardinality_check;

ALTER TABLE attributes
    RENAME COLUMN relationship_cardinality TO cardinality;

ALTER TABLE attributes
    ADD COLUMN target_cardinality TEXT;
