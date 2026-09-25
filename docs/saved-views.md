# Saved views

Saved searches are the first saved-view kind (`explorer_search`). Future visualizations may use new kinds with independently versioned state schemas; this release does not implement dashboards.

Explorer continues to keep edits in inline URL state. Named searches live at `/?savedView=<uuid>`; large searches copied with **Copy link** use an unnamed snapshot at `/?viewState=<uuid>` instead. No writes occur simply by changing filters. A copied link does not bypass authentication or `entities.read` authorization. Private views are readable only by their owner; workspace views are readable by authorized workspace members. Only the owner can update or delete a named view. Link snapshots are accessible by identifier to authorized workspace members; treat them as shareable, not private.

State is bounded to 32 KiB and validated on write. The JSONB representation is hashed with SHA-256 to reuse identical link snapshots within a workspace (including an equality check). Link snapshots are not listed; named views are listed newest first (up to 100). Each saved view is scoped to a workspace. Snapshot lifecycle and additional view kinds can be added later.

For the web and CLI, the state shape uses the Explorer URL keys: `blueprint`, `version`, `allVersions`, `query`, `context`, `locked`, `sort`, `attributeFilters`, and `relationshipFacets`. The blueprint code is required. Search execution always goes through the usual entity search endpoint, so current blueprint permissions and schema validation still apply.
