-- A target's applications in the order they changed its balances. Core
-- numbers them under the target's row lock, so the sequence is the order
-- the balances actually changed in, which `applied_at` (the transaction's
-- start) is not when two saves wait on one target. Readers that replay a
-- target's history, such as stock valuation, page through it by sequence.
ALTER TABLE record_movement_applications ADD COLUMN sequence BIGINT;

UPDATE record_movement_applications AS application
SET sequence = numbered.sequence
FROM (
    SELECT workspace_id, record_id, movement_code,
           row_number() OVER (
               PARTITION BY workspace_id, target_record_id
               ORDER BY applied_at, record_id, movement_code
           ) AS sequence
    FROM record_movement_applications
) AS numbered
WHERE application.workspace_id = numbered.workspace_id
  AND application.record_id = numbered.record_id
  AND application.movement_code = numbered.movement_code;

ALTER TABLE record_movement_applications
    ALTER COLUMN sequence SET NOT NULL,
    ADD CONSTRAINT record_movement_applications_sequence_positive CHECK (sequence > 0),
    ADD CONSTRAINT record_movement_applications_target_sequence_key
        UNIQUE (workspace_id, target_record_id, sequence);

-- The unique constraint's index serves reads by target.
DROP INDEX record_movement_applications_target_idx;
