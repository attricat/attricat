-- Workspace translations for `{{key}}` / `{{key|context}}` references in
-- catalog labels. An empty context means none. Rows supplied by a solution
-- pack keep its identifier until a workspace edit takes them over.
CREATE TABLE lexicon_entries (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces (id),
    key TEXT NOT NULL CHECK (key <> ''),
    context TEXT NOT NULL DEFAULT '',
    language TEXT NOT NULL CHECK (language <> ''),
    plural_category TEXT NOT NULL
        CHECK (plural_category IN ('zero', 'one', 'two', 'few', 'many', 'other')),
    text TEXT NOT NULL CHECK (text <> ''),
    source TEXT NOT NULL CHECK (source IN ('workspace', 'solution_pack')),
    solution_pack_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, key, context, language, plural_category),
    CHECK ((source = 'solution_pack') = (solution_pack_id IS NOT NULL))
);
CREATE INDEX lexicon_entries_language_idx ON lexicon_entries (workspace_id, language);
