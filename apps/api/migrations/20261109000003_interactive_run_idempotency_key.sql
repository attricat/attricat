-- An interactive idempotency key identifies one request per initiator and
-- release, whichever operation it names; reusing it for another operation
-- must find the earlier run rather than start a second one.
CREATE UNIQUE INDEX extension_operation_runs_interactive_key_idx
    ON extension_operation_runs (workspace_id, extension_id, installed_release_id, idempotency_key)
    WHERE invocation = 'interactive';
