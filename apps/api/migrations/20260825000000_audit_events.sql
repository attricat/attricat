-- Immutable, tenant-scoped audit evidence. Event metadata is deliberately
-- application supplied and must be redacted before insertion; database triggers
-- below never copy credential hashes or token digests.
CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    workspace_id UUID REFERENCES workspaces (id),
    actor_user_id UUID REFERENCES users (id),
    actor_token_id UUID,
    request_id UUID NOT NULL,
    correlation_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action ~ '^[a-z][a-z0-9_.-]+$'),
    authorization_scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    target JSONB NOT NULL DEFAULT '{}'::jsonb,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_events_workspace_occurred_at_idx ON audit_events (workspace_id, occurred_at DESC);
CREATE INDEX audit_events_actor_occurred_at_idx ON audit_events (actor_user_id, occurred_at DESC);

ALTER TABLE audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_events FORCE ROW LEVEL SECURITY;
CREATE POLICY audit_events_workspace_policy ON audit_events
    -- User and credential lifecycle rows are global identity events and have
    -- no workspace until a membership exists. No audit read API is exposed.
    USING (workspace_id IS NULL OR workspace_id = catalog_workspace_id())
    WITH CHECK (workspace_id IS NULL OR workspace_id = catalog_workspace_id());

-- These triggers preserve security events which are made through database
-- lifecycle functions, where no HTTP request context exists. They intentionally
-- store identifiers and state only, never passwords, hashes, or token values.
CREATE FUNCTION audit_security_change() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
DECLARE
    subject_id UUID;
    event_workspace_id UUID;
    event_action TEXT := TG_ARGV[0];
    event_metadata JSONB := '{}'::jsonb;
BEGIN
    IF TG_TABLE_NAME = 'workspace_memberships' THEN
        subject_id := COALESCE(NEW.user_id, OLD.user_id);
        event_workspace_id := COALESCE(NEW.workspace_id, OLD.workspace_id);
        event_metadata := jsonb_build_object('state', COALESCE(NEW.state, OLD.state));
    ELSIF TG_TABLE_NAME = 'role_grants' THEN
        subject_id := COALESCE(NEW.membership_id, OLD.membership_id);
        event_workspace_id := COALESCE(NEW.workspace_id, OLD.workspace_id);
        event_metadata := jsonb_build_object(
            'role_id', COALESCE(NEW.role_id, OLD.role_id),
            'scope_type', COALESCE(NEW.scope_type, OLD.scope_type),
            'scope_target_id', COALESCE(NEW.scope_target_id, OLD.scope_target_id)
        );
    ELSIF TG_TABLE_NAME = 'user_lifecycle_action_tokens' THEN
        subject_id := COALESCE(NEW.user_id, OLD.user_id);
        event_metadata := jsonb_build_object('purpose', COALESCE(NEW.purpose, OLD.purpose));
    ELSIF TG_TABLE_NAME = 'local_password_credentials' THEN
        subject_id := COALESCE(NEW.user_id, OLD.user_id);
    ELSIF TG_TABLE_NAME = 'users' THEN
        subject_id := COALESCE(NEW.id, OLD.id);
        event_metadata := jsonb_build_object('state', COALESCE(NEW.state, OLD.state));
    END IF;

    INSERT INTO audit_events (
        id, workspace_id, request_id, correlation_id, action, target, outcome, metadata
    ) VALUES (
        gen_random_uuid(), event_workspace_id, gen_random_uuid(), gen_random_uuid(), event_action,
        jsonb_build_object('type', TG_TABLE_NAME, 'id', subject_id), 'success', event_metadata
    );
    RETURN COALESCE(NEW, OLD);
END;
$$;

CREATE TRIGGER users_audit_security_change
AFTER INSERT OR UPDATE OR DELETE ON users
FOR EACH ROW EXECUTE FUNCTION audit_security_change('security.user.changed');
CREATE TRIGGER credentials_audit_security_change
AFTER INSERT OR UPDATE OR DELETE ON local_password_credentials
FOR EACH ROW EXECUTE FUNCTION audit_security_change('security.credential.changed');
CREATE TRIGGER lifecycle_tokens_audit_security_change
AFTER INSERT OR UPDATE OR DELETE ON user_lifecycle_action_tokens
FOR EACH ROW EXECUTE FUNCTION audit_security_change('security.lifecycle_token.changed');
CREATE TRIGGER memberships_audit_security_change
AFTER INSERT OR UPDATE OR DELETE ON workspace_memberships
FOR EACH ROW EXECUTE FUNCTION audit_security_change('security.membership.changed');
CREATE TRIGGER grants_audit_security_change
AFTER INSERT OR UPDATE OR DELETE ON role_grants
FOR EACH ROW EXECUTE FUNCTION audit_security_change('security.role_grant.changed');

GRANT INSERT ON audit_events TO catalog_api;
