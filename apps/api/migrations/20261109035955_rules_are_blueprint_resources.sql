-- A rule applies to every revision of its blueprint, and has one current
-- definition. Saving a changed definition adds a revision that becomes current
-- at once; earlier revisions remain as the rule's changelog.
ALTER TABLE rules DROP CONSTRAINT rules_blueprint_id_blueprint_version_fkey;
DROP INDEX rules_workspace_blueprint_idx;
ALTER TABLE rules DROP COLUMN blueprint_version;
CREATE INDEX rules_workspace_blueprint_idx ON rules(workspace_id, blueprint_id, created_at DESC);
-- Who saved the revision, for the changelog. NULL for revisions saved before
-- this was recorded and for solution-pack installs.
ALTER TABLE rules ADD COLUMN created_by_user_id UUID;

-- The current revision is the enabled one, or else the latest published one,
-- so neither enabled behavior nor unpublished drafts change.
ALTER TABLE rule_lifecycles ADD COLUMN current_version BIGINT;
ALTER TABLE rule_lifecycles ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT false;
UPDATE rule_lifecycles l
   SET enabled = l.enabled_version IS NOT NULL,
       current_version = COALESCE(
           l.enabled_version,
           (SELECT max(r.version) FROM rules r
             WHERE r.workspace_id = l.workspace_id AND r.id = l.rule_id AND r.status = 'published'),
           (SELECT max(r.version) FROM rules r
             WHERE r.workspace_id = l.workspace_id AND r.id = l.rule_id));
ALTER TABLE rule_lifecycles ALTER COLUMN current_version SET NOT NULL;
ALTER TABLE rule_lifecycles
    ADD CONSTRAINT rule_lifecycles_workspace_id_rule_id_current_version_fkey
    FOREIGN KEY (workspace_id, rule_id, current_version) REFERENCES rules(workspace_id, id, version);
-- Drops its foreign key and rule_lifecycles_enabled_idx with it.
ALTER TABLE rule_lifecycles DROP COLUMN enabled_version;
CREATE INDEX rule_lifecycles_enabled_idx ON rule_lifecycles(workspace_id) WHERE enabled;

ALTER TABLE rules DROP COLUMN status;
ALTER TABLE rules DROP COLUMN published_at;

-- A run records the definition it evaluated. A dry run may check an unsaved
-- definition, kept on the run; enabling or saving an enforcing definition
-- looks for a completed full dry run of the same definition hash.
ALTER TABLE rule_runs ADD COLUMN definition_hash TEXT;
UPDATE rule_runs rr
   SET definition_hash = r.definition_hash
  FROM rules r
 WHERE r.workspace_id = rr.workspace_id AND r.id = rr.rule_id AND r.version = rr.rule_version;
ALTER TABLE rule_runs ALTER COLUMN definition_hash SET NOT NULL;
ALTER TABLE rule_runs ADD COLUMN compiled_plan JSONB;
ALTER TABLE rule_runs ADD CONSTRAINT rule_runs_unsaved_definition_dry_run_check
    CHECK (compiled_plan IS NULL OR dry_run);
-- Candidates on a blueprint revision the rule does not apply to.
ALTER TABLE rule_runs ADD COLUMN candidates_skipped BIGINT NOT NULL DEFAULT 0
    CHECK (candidates_skipped >= 0);
DROP INDEX rule_runs_enable_gate_idx;
CREATE INDEX rule_runs_dry_run_gate_idx ON rule_runs(workspace_id, rule_id, definition_hash, completed_at)
    WHERE dry_run AND scope_record_id IS NULL AND status = 'completed';
