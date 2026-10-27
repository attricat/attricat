-- A notification can link to a data quality rule, for example when a newly
-- published blueprint revision no longer fits it.
ALTER TABLE user_notifications DROP CONSTRAINT user_notifications_subject_kind_check;
ALTER TABLE user_notifications ADD CONSTRAINT user_notifications_subject_kind_check
    CHECK (subject_kind IN ('record', 'agent_conversation', 'rule'));
