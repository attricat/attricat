-- Existing deployments may already contain schedules created before an execution
-- principal was introduced. Such schedules remain visible but are disabled from
-- execution until explicitly recreated with a durable principal.
ALTER TABLE agent_schedules ADD COLUMN initiated_by_user_id UUID REFERENCES users (id);
