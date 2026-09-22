-- Delivery/task association is declarative only. Existing deliveries are
-- assigned IDs and linked tasks by the application coordinator so migrations
-- do not encode execution or backfill behavior.
ALTER TABLE event_deliveries ADD COLUMN id UUID;
ALTER TABLE event_deliveries ADD COLUMN task_id UUID REFERENCES tasks(id);
ALTER TABLE event_deliveries ADD CONSTRAINT event_deliveries_task_link CHECK (
    (id IS NULL AND task_id IS NULL) OR (id IS NOT NULL AND task_id IS NOT NULL)
);
CREATE UNIQUE INDEX event_deliveries_id_unique_idx
    ON event_deliveries (id) WHERE id IS NOT NULL;
CREATE UNIQUE INDEX event_deliveries_task_id_unique_idx
    ON event_deliveries (task_id) WHERE task_id IS NOT NULL;
