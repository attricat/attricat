-- A run that stopped at the per-run candidate cap while more candidates
-- remained. Such a dry run does not cover the whole blueprint revision.
ALTER TABLE rule_runs ADD COLUMN truncated BOOLEAN NOT NULL DEFAULT false;
