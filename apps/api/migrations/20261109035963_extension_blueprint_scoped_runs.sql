-- What an interactive run may reach. A selection run acts on its frozen member
-- records; a blueprint run has no members and may only create or upsert records
-- of its blueprint revision. The scope is stored rather than inferred from
-- having no members, so losing members can never widen a run.
ALTER TABLE extension_operation_runs
    ADD COLUMN selection_scope TEXT NOT NULL DEFAULT 'selection'
        CHECK (selection_scope IN ('selection', 'blueprint'));
