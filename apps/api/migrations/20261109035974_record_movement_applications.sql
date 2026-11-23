-- Each movement applies to its record at most once: when the record is
-- created, or at the first status transition it is declared to apply on.
-- A row is that application: the target whose balance changed, by how much
-- and which way. Reversals, immutability and deletion read it.
CREATE TABLE record_movement_applications (
    workspace_id UUID NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    record_id UUID NOT NULL,
    movement_code TEXT NOT NULL,
    target_record_id UUID NOT NULL,
    balance_code TEXT NOT NULL,
    amount NUMERIC NOT NULL CHECK (amount > 0),
    direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, record_id, movement_code)
);

CREATE INDEX record_movement_applications_target_idx
    ON record_movement_applications (workspace_id, target_record_id);
