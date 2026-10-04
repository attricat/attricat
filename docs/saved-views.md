# Saved searches and share links

Explorer supports named `explorer_search` views. Dashboards and other saved-view
kinds are not implemented. Search execution still uses the ordinary entity
search endpoint and checks current blueprint permissions and validation.

## Links and access

- `/?savedView=<uuid>` opens a named search. **Private** views are readable by
  their owner; **Workspace** views are readable by authorized workspace members.
  Only the owner can update or delete a named view.
- `/?viewState=<uuid>` opens an unnamed snapshot created by **Share search** when
  inline URL state is too large. Anyone with the ID *and* workspace access and
  `entities.read` can open it; treat it as shareable, not private. Snapshots are
  not listed.
- Small searches remain inline in the Explorer URL. Changing filters does not
  write a saved view.

Named views are listed newest first, up to 100. The Explorer saved searches
dialog filters them by name or description through the list endpoint. Views and snapshots belong to
one workspace; a link never grants access to catalog data.

## State

State is at most 32 KiB and validated on write. It uses the Explorer URL keys:
`blueprint`, `version`, `allVersions`, `query`, `context`, `locked`, `sort`,
`attributeFilters`, and `relationshipFacets`. A blueprint code is required.

The server hashes the JSONB state with SHA-256 and checks equality to reuse
identical link snapshots within a workspace. See the [API reference](api.md#saved-views-and-share-links)
and [CLI commands](cli.md#saved-searches) for the request contract.

## Saved searches installed by solution packs

A [solution pack](solution-packs.md#rules-workflows-and-saved-searches) can
declare named Explorer searches. Applying the pack stores each as an
`explorer_search` view with **Workspace** visibility, owned by the person who
applies the plan, so it appears in Explorer's saved searches for every
authorized member. The pack's blueprint, relationship target, and context
references are replaced with the plan's physical blueprint and context codes,
and the state is normalized like an ordinary write. Packs cannot seed a pinned
blueprint `version` or selected relationship entity IDs, because both identify
workspace data. Like any named view, only its owner can update or delete it
afterwards.
