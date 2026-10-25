-- Plan evidence has a single digest format covering every section, so the
-- stored format selector is meaningless. Dropping the column also drops its
-- format and requires-digest checks.
ALTER TABLE solution_pack_plans
    DROP COLUMN resource_evidence_format;
