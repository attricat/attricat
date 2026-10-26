-- Owners and administrators (records.unlock) may take an edge whose
-- separation of duties they would otherwise fail; such rows are flagged.
ALTER TABLE record_status_transitions
    ADD COLUMN separation_waived boolean NOT NULL DEFAULT false;
