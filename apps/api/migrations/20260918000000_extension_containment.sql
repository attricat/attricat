-- Workspace-scoped emergency containment. Runtime code evaluates this flag at
-- every extension execution and client-access gate; management state is left
-- intact so operators can remediate safely.
ALTER TABLE workspaces
    ADD COLUMN extensions_enabled BOOLEAN NOT NULL DEFAULT true;
