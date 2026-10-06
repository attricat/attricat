-- Retain the initiating credential so deferred agent tools cannot inherit
-- permissions that were excluded from a personal API token.
ALTER TABLE agent_runs
    ADD COLUMN initiated_by_token_id UUID REFERENCES personal_api_tokens (id) ON DELETE RESTRICT,
    ADD COLUMN authority_recorded BOOLEAN NOT NULL DEFAULT false;
