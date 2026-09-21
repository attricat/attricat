-- Server-computed integrity evidence detects inconsistent modification of the
-- immutable mapping/action rows for plans that reuse existing blueprints.
ALTER TABLE solution_pack_plans
    ADD COLUMN resource_evidence_sha256 TEXT
        CHECK (resource_evidence_sha256 IS NULL OR resource_evidence_sha256 ~ '^[0-9a-f]{64}$');
