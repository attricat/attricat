-- Group members name a reusable attribute by its immutable namespace and code.
-- Applying a group resolves each one to its latest published revision.
DROP TABLE reusable_attribute_group_members;
CREATE TABLE reusable_attribute_group_members (
    group_id UUID NOT NULL REFERENCES reusable_attribute_groups (id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL,
    namespace TEXT NOT NULL,
    code TEXT NOT NULL,
    position BIGINT NOT NULL,
    PRIMARY KEY (group_id, namespace, code),
    UNIQUE (group_id, position),
    FOREIGN KEY (workspace_id, namespace, code)
        REFERENCES reusable_attribute_definitions (workspace_id, namespace, code)
);
