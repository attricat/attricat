-- Sample record logical keys use the `sample-records/` prefix.
ALTER TABLE solution_pack_plan_sample_evidence DROP CONSTRAINT solution_pack_plan_sample_evidence_logical_key_check;
ALTER TABLE solution_pack_plan_sample_records DROP CONSTRAINT solution_pack_plan_sample_records_logical_key_check;
ALTER TABLE solution_pack_plan_release_changes DROP CONSTRAINT solution_pack_plan_release_changes_check;

UPDATE solution_pack_application_steps SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_actions SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_asset_objects SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_extension_releases SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_extension_requirements SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_mappings SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_release_changes SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_sample_evidence SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';
UPDATE solution_pack_plan_sample_records SET logical_key = 'sample-records/' || substr(logical_key, 17) WHERE logical_key LIKE 'sample-entities/%';

ALTER TABLE solution_pack_plan_sample_evidence ADD CONSTRAINT solution_pack_plan_sample_evidence_logical_key_check
    CHECK (logical_key ~ '^sample-records/[a-z][a-z0-9_-]*$');
ALTER TABLE solution_pack_plan_sample_records ADD CONSTRAINT solution_pack_plan_sample_records_logical_key_check
    CHECK (logical_key ~ '^sample-records/[a-z][a-z0-9_-]*$');
ALTER TABLE solution_pack_plan_release_changes ADD CONSTRAINT solution_pack_plan_release_changes_check
    CHECK ((((change_kind = 'added'::text) AND (prior_target_id IS NULL) AND (prior_target_code IS NULL) AND (prior_target_version IS NULL) AND (prior_canonical_definition_sha256 IS NULL) AND (current_canonical_definition_sha256 IS NOT NULL)) OR ((change_kind = ANY (ARRAY['unchanged'::text, 'changed'::text])) AND (prior_target_id IS NOT NULL) AND (prior_target_code IS NOT NULL) AND ((((logical_key ~~ 'assets/%'::text) OR (logical_key ~~ 'sample-records/%'::text)) AND (prior_target_version IS NULL)) OR ((logical_key !~~ 'assets/%'::text) AND (logical_key !~~ 'sample-records/%'::text) AND (prior_target_version IS NOT NULL))) AND (prior_canonical_definition_sha256 IS NOT NULL) AND (current_canonical_definition_sha256 IS NOT NULL) AND (((change_kind = 'unchanged'::text) AND (current_canonical_definition_sha256 = prior_canonical_definition_sha256)) OR ((change_kind = 'changed'::text) AND ((current_canonical_definition_sha256 <> prior_canonical_definition_sha256) OR (logical_key ~~ 'sample-records/%'::text))))) OR ((change_kind = 'removed'::text) AND (prior_target_id IS NOT NULL) AND (prior_target_code IS NOT NULL) AND ((((logical_key ~~ 'assets/%'::text) OR (logical_key ~~ 'sample-records/%'::text)) AND (prior_target_version IS NULL)) OR ((logical_key !~~ 'assets/%'::text) AND (logical_key !~~ 'sample-records/%'::text) AND (prior_target_version IS NOT NULL))) AND (prior_canonical_definition_sha256 IS NOT NULL) AND (current_canonical_definition_sha256 IS NULL))));
