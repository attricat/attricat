# Field-level read restrictions

Status: investigation and design for
[#346](https://github.com/attricat/attricat/issues/346). Nothing in this note
is implemented yet. It records what Core does today and what restricted
attributes would need.

## Findings

**Core cannot hide individual attributes.** Anyone who can read an entity can
read every attribute value it has, in every context, plus that value's
history. The permission model has no attribute dimension, and no read path
filters by attribute.

### Authorization model today

- Every request is authorized once at the HTTP boundary.
  `crates/http/src/http/auth/policy.rs` maps the route to a
  `(permission, TargetKind)` pair, and `CatalogRepository::is_authorized`
  (`crates/repository/src/repository/mod.rs`) checks it against
  `role_grants`.
- A grant's scope is `workspace`, `blueprint_family`, `entity` or
  `context_subtree`.
  - Entity routes (`TargetKind::EntityId`) accept workspace,
    blueprint-family and entity grants.
  - Collection routes (`TargetKind::None`: search, facets, saved searches,
    `GET /entities`) need a **workspace** grant. Narrower grants get `403`.
    The routes never filter rows.
  - `context_subtree` grants only match context routes (`TargetKind::Context*`).
    They do not authorize entity reads or value writes, and they never filter
    values by context. `apps/docs/.../operate/workspaces.md` describes an
    editor grant on a context subtree as letting the person edit values
    there. Current code does not do that.
- After the middleware, handlers get a workspace-scoped repository that does
  not know who the caller is. The only exception is
  `authorization_actor`, which interactive extension runs use, and it is
  entity-level.
- No attribute-level permission concept exists. `reusable_attribute_groups`
  only groups attributes for attaching them; it has no permission meaning.
  The reusable-attribute `searchable` flag only controls whether search
  indexes the attribute.

### Read surfaces

Every surface below exposes every attribute of every entity it is allowed to
return.

| Surface | Path | Gate | Attribute values exposed |
| --- | --- | --- | --- |
| Entity form | `GET /v1/entities/{id}` (`http/entities.rs`, `CatalogReadService::entity_with_values`, `values.rs` `form_values`/`reusable_form_values`) | `entities.read`, entity target | All values, plus the full `projections.preview` (every scalar in every context). |
| Raw entity | `GET /entities/{id}` (`entity_reads.rs`, `entity_commands.rs` `get_entity`) | same | `projections` (all scalars in all contexts). |
| Current values | `GET /entities/{id}/values/current` (`values.rs` `current_values`) | same | Every value row. |
| Previews | `GET /entities/{id}/preview`, `/resolved-preview`, `/hierarchy` (`entity_projection.rs`) | same | All values, plus related entities' previews and display labels. **Related entities are not authorized.** |
| Related and incoming lists | `GET /entities?related_from=`, `POST /v1/entities/{id}/incoming-relationships` (`entity_search.rs`) | workspace grant / entity target | Previews of the target or source entities. Incoming sources are not authorized. |
| Display labels | `entity_projection.rs` `display_label` | wherever a label is returned | Built at read time from `views.dropdown_option.fields` over the stored preview projection. A sensitive field listed there appears in pickers, facets, related columns, hierarchy and incoming lists. |
| Write responses | update, append, duplicate, restore, migration preview | `entities.write` | The full entity or values. |
| Search | `POST /v1/entities/search` (`entity_reads.rs`, `entity_search.rs`) | `entities.read`, workspace grant | Preview, display, table values, related values, and `match_explanations[].matching_attribute_code`. |
| Facets | relationship-tree facet routes | workspace grant | Target display labels. |
| Saved searches | `/saved-views`, `/view-state-links` | workspace grant | Stored filter values and query text, shared across the workspace. |
| Explorer | the web app calls the search and entity routes above | — | Client-side column hiding is presentation only. Full previews are in the payload and the query cache. |
| Value history | `GET /entities/{id}/values/history` (`values.rs`) | `entities.read` | Every archived value. |
| Entity changes | `GET /entities/{id}/changes` (`entity_commands.rs`) | `entities.read` | Before and after values per attribute. |
| Audit log | `GET /audit-events` | `audit.read` | No values (it does not join `audit_event_changes`). |
| Domain events | `crates/events` `AffectedFactV1` | extension capability `catalog.read`, workflows | Before and after values for every changed fact. Delivered verbatim to extensions and the extension changes feed, and snapshotted in workflow runs. HTTP workflow-run reads and the agent omit the snapshot. |
| Extension reads | `extension_runtime.rs` read, page, changes, lookup and connector `schema`/`page` calls (`extension_catalog_data.rs`, `values.rs` `extension_catalog_values_at`) | installation capability grants, workspace-wide, not tied to a user | Full entities, values at a point in time, event payloads. Lookup finds an entity by attribute value. |
| Interactive extension runs | `extension_interactive_operations.rs` `interactive_selection_page` | `entities.read` per selected entity, for the person who started the run | The only read path that knows the caller, and it is entity-level. |
| Connector exports | `blueprint_connector_jobs.rs` | `extensions.manage` | Through the extension reads above. `schema` lists every attribute. |
| Publication channels | `entity_publications.rs` | `entities.publish`, `contexts.*` | Flags only. Channel consumers read values through extensions. |
| Agent tools | `crates/agent-runtime/src/agent_tools.rs` (`get_entity`, `get_entity_context_preview`, `get_entity_changes`, `get_value_history`, `search_entities`) | the person who started the conversation, checked on each call | Same as HTTP. Tool results are stored in conversations, which the workspace shares (filtered only by whether the anchor entity is readable), and are sent to the model provider. Smart-fill sends the editable values to the provider. |
| Rules | `workers/src/rule_runtime.rs`, findings API | `rules.read` | No values, but findings name attributes and say whether they are missing or stale. |
| Workflows | `crates/workflow` | run as the system | A trigger can match on any value. `AttributeWrite` from an event field can copy one attribute's value into another attribute or into system metadata. |
| Files | `file_access.rs`, `files.rs` `file_read_targets` | entity readability | Resolves file → entity **without** the attribute. File metadata is embedded in form and resolved values. |
| Data health | `health.rs` | `data_health.read` | Aggregates only (fill rates per attribute). |
| Comments | `entity_comments.rs` | `entities.read` | Free text only. |
| Personal API tokens | token permissions | permission codes | Restrict which permissions apply, not which attributes. |

### Ways the values can still be inferred

Removing a value from a response does not hide it. Each of these still gives
it away:

- Search filters: `eq`, `contains` and `starts_with` act as an oracle, and
  ranges allow a binary search.
- Sort order, `total_count`, free-text match sets and
  `matching_attribute_code`.
- Facet narrowing, saved-search filters and extension lookup.
- Rule findings ("has no value", "is stale") and completeness metrics.
- Display labels built from the stored preview.
- History, entity changes, event payloads and dead-letter replays.
- Agent conversations, which are stored, shared and sent to the model
  provider.
- Workflows that copy a value somewhere visible.

## Guidance today

Treat every attribute of a readable entity as readable. To keep sensitive data
from some people, put it in a separate blueprint (for example a "Valuation"
entity linked to the object), and grant `entities.read` on that blueprint only
to the people who need it, with blueprint-family grants. Know the gaps that
remain:

- People with only blueprint-family grants cannot use search, facets or
  saved searches. Those need a workspace grant.
- A workspace grant reads everything.
- Relationship previews and incoming-relationship lists on a readable entity
  do not check the linked entity. Do not show the sensitive blueprint's
  fields in the readable blueprint's views or `dropdown_option`.
- Extensions, workflows and connector exports see everything.

## Design proposal: restricted attributes

### Model

- An attribute declares `restricted = true` (inline blueprint attributes and
  reusable attribute revisions). It is part of the definition, so changing it
  needs a new revision, and the revision diff shows it.
- A new system permission, `entities.read_restricted`, grants reading
  restricted values. It goes through roles and scopes like every other
  permission (workspace, blueprint family, entity). Owners and admins get it.
  Editors and viewers do not, and custom roles can add it. A personal API
  token needs it explicitly.
- Writing a restricted attribute needs `entities.write` **and**
  `entities.read_restricted`. A save from someone without it leaves
  restricted values unchanged, so a form that never received them cannot
  clear them by accident.
- A later extension could replace the single flag with named classes (for
  example `valuation`, `custody`), each tied to its own permission. Do not
  start there. One class covers the cases in the issue, and named classes
  multiply the permission catalogue.

### Enforcement

1. **The read principal.** Generalize `authorization_actor` into a read
   principal that the middleware attaches to the scoped repository: the user,
   their PAT permission subset, or a system or extension principal. Add
   `restricted_readable(principal, entity_or_blueprint) -> bool` next to
   `is_authorized`, batched like `authorized_entity_ids`.
2. **One redaction function.** Add `redact(preview_or_values, restricted_codes)`
   and call it at the existing chokepoints:
   - value reads: `form_values`, `file_form_values`, `reusable_form_values`,
     `current_values`, `extension_catalog_values_at`, `value_history*` and
     `entity_audit_changes*`;
   - projection reads: `get_entity`, every `projections -> 'preview'` select in
     `entity_search.rs`, `build_preview`/`resolved_preview`, and
     `display_label(s)` (redact before building the label).

   Omit restricted keys rather than nulling them, and return
   `restricted_attributes: [codes]` so the web app can show **Restricted**
   instead of **Not set**.
3. **Display labels.** Blueprint validation rejects restricted attributes in
   `views.dropdown_option.fields`, so labels never need redacting. A label
   built from a restricted value would leak into every picker.
4. **Search.** For a principal that cannot read restricted values:
   - reject filters and sorts on restricted attributes with
     `403 restricted_attribute`, in `resolve_search_filter` and
     `resolve_table_sort` (HTTP and agent);
   - leave restricted attribute IDs out of the free-text SQL and
     `match_explanations`;
   - redact table columns per row.

   Saved searches whose filters use a restricted attribute show the filter as
   unavailable instead of its value.
5. **History and events.** History and entity-change reads filter by
   attribute code through the same function. Domain events keep full values,
   because they are system data. Delivery to an extension redacts restricted
   facts unless the installation holds a new `catalog.read_restricted`
   capability, which administrators grant like the other capabilities.
6. **Extensions.** Catalog read calls redact unless the installation has
   `catalog.read_restricted`. Interactive runs also require the person who
   started the run to have `entities.read_restricted`. Connector `schema`
   marks restricted attributes, so export profiles can leave them out.
7. **Agent.** Tools read through the same redacting layer as the person who
   started the conversation. Smart-fill leaves restricted attributes out of
   what it sends to the model provider. Because conversations are shared,
   tool results that contain restricted values are stored with a marker, and
   the conversation read API redacts them for readers without the
   permission. Explain restricted attributes in `SYSTEM_PROMPT`.
8. **Workflows and rules.** Publishing a workflow is rejected when an
   `AttributeWrite` or metadata merge copies a restricted attribute's value
   into an attribute that is not restricted. Rule findings on restricted
   attributes are visible only to principals that can read them.
9. **Files.** Add `attribute_code` to `file_read_targets` and
   `FileAccessOperation`, so downloading a file from a restricted file
   attribute needs the permission.

### Prerequisites

These gaps exist without restricted attributes and should be fixed first:

- Preview, hierarchy, related and incoming-relationship reads must authorize
  each linked entity, and leave out or mark those the caller cannot read.
- Collection endpoints should filter rows for blueprint-family and entity
  grants instead of requiring a workspace grant.
- `context_subtree` grants must either authorize value writes in the subtree,
  as `operate/workspaces.md` says, or that page must be corrected.

### Follow-up issues

Implementation is split into these follow-up issues:

1. [#366](https://github.com/attricat/attricat/issues/366): authorize linked
   entities in preview and relationship reads, and correct what
   `context_subtree` grants do.
2. [#367](https://github.com/attricat/attricat/issues/367): restricted
   attributes: declaration, the `entities.read_restricted` permission, the
   read principal and redaction of entity reads and history.
3. [#368](https://github.com/attricat/attricat/issues/368): restricted
   attributes in search, facets, saved searches and the Explorer.
4. [#369](https://github.com/attricat/attricat/issues/369): restricted
   attributes for extensions, events, workflows, agent tools and files.
