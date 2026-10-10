# Field-level read restrictions

Status: investigation and design for
[#346](https://github.com/attricat/attricat/issues/346). The attribute-restriction
proposal is not implemented yet. It records what Core
does today and what restricted attributes would need. Record-level relationship
hydration now checks related records' read authority; this does not add
attribute-level restrictions.

## Findings

**Core cannot hide individual attributes.** Anyone who can read a record can
read every attribute value it has, in every context, plus that value's
history. The permission model has no attribute dimension, and no read path
filters by attribute.

### Authorization model today

- Every request is authorized once at the HTTP boundary.
  `crates/http/src/http/auth/policy.rs` maps the route to a
  `(permission, TargetKind)` pair, and `AttricatRepository::is_authorized`
  (`crates/repository/src/repository/mod.rs`) checks it against
  `role_grants`.
- A grant's scope is `workspace`, `blueprint_family`, `record` or
  `context_subtree`.
  - Record routes (`TargetKind::RecordId`) accept workspace,
    blueprint-family and record grants.
  - Collection routes (`TargetKind::None`: search, facets, saved searches,
    `GET /records`) need a **workspace** grant. Narrower grants get `403`.
    The routes never filter rows.
  - Record label lookups (`TargetKind::RecordList`, `POST /v1/records/labels`)
    keep only the requested records that `authorized_record_ids` accepts, so
    workspace, blueprint-family and record grants each see their own records.
    Other IDs are omitted, and so are unknown and deleted ones.
  - `context_subtree` grants only match context routes (`TargetKind::Context*`).
    They do not authorize record reads or value writes, and they never filter
    values by context. `apps/docs/.../operate/workspaces.md` describes an
    editor grant on a context subtree as letting the person edit values
    there. Current code does not do that.
- Most handlers receive a workspace-scoped repository. Relationship preview,
  hierarchy, incoming-relationship and agent reads additionally carry an
  `authorization_actor`, as do interactive extension runs. This actor checks
  record-level permissions, including personal-token restrictions.
- No attribute-level permission concept exists. `reusable_attribute_groups`
  only groups attributes for attaching them; it has no permission meaning.
  The reusable-attribute `searchable` flag only controls whether search
  indexes the attribute.

### Read surfaces

Every surface below exposes every attribute of every record it is allowed to
return.

| Surface | Path | Gate | Attribute values exposed |
| --- | --- | --- | --- |
| Record form | `GET /v1/records/{id}` (`http/records.rs`, `AttricatReadService::record_with_values`, `values.rs` `form_values`/`reusable_form_values`) | `records.read`, record target | All values, plus the full `projections.preview` (every scalar in every context). |
| Raw record | `GET /records/{id}` (`record_reads.rs`, `record_commands.rs` `get_record`) | same | `projections` (all scalars in all contexts). |
| Current values | `GET /records/{id}/values/current` (`values.rs` `current_values`) | same | Every value row. |
| Previews | `GET /records/{id}/preview`, `/resolved-preview`, `/hierarchy` (`record_projection.rs`) | same | All values of the root, plus previews and display labels of readable related records. |
| Related and incoming lists | `GET /records?related_from=`, `POST /v1/records/{id}/incoming-relationships` (`record_search.rs`) | workspace grant / record target | Previews of the target or source records. Incoming sources are filtered by record read authority; a page may be empty while still carrying a continuation cursor. |
| Display labels | `record_projection.rs` `display_label` | wherever a label is returned | Built at read time from `views.dropdown_option.fields` over the stored preview projection. A sensitive field listed there appears in pickers, facets, related columns, hierarchy and incoming lists. |
| Write responses | update, append, duplicate, restore, migration preview | `records.write` | The full record or values. |
| Search | `POST /v1/records/search` (`record_reads.rs`, `record_search.rs`) | `records.read`, workspace grant | Preview, display, table values, related values, and `match_explanations[].matching_attribute_code`. |
| Record labels | `POST /v1/records/labels` (`record_reads.rs`, `record_search.rs`) | `records.read` per record | Display labels per context: the `dropdown_option` view's fields. |
| Facets | relationship-tree facet routes | workspace grant | Target display labels. |
| Saved searches | `/saved-views`, `/view-state-links` | workspace grant | Stored filter values and query text, shared across the workspace. |
| Explorer | the web app calls the search and record routes above | — | Client-side column hiding is presentation only. Full previews are in the payload and the query cache. |
| Value history | `GET /records/{id}/values/history` (`values.rs`) | `records.read` | Every archived value. |
| Record changes | `GET /records/{id}/changes` (`record_commands.rs`) | `records.read` | Before and after values per attribute. |
| Audit log | `GET /audit-events` | `audit.read` | No values (it does not join `audit_event_changes`). |
| Domain events | `crates/events` `AffectedFactV1` | extension capability `attricat.read`, workflows | Before and after values for every changed fact. Delivered verbatim to extensions and the extension changes feed, and snapshotted in workflow runs. HTTP workflow-run reads and the agent omit the snapshot. |
| Extension reads | `extension_runtime.rs` read, page, changes, lookup and connector `schema`/`page` calls (`extension_attricat_data.rs`, `values.rs` `extension_attricat_values_at`) | installation capability grants, workspace-wide, not tied to a user | Full records, values at a point in time, event payloads. Lookup finds a record by attribute value. |
| Interactive extension runs | `extension_interactive_operations.rs` `interactive_selection_page` | `records.read` per selected record, for the person who started the run | Checks the caller at record level. |
| Connector exports | `blueprint_connector_jobs.rs` | `extensions.manage` | Through the extension reads above. `schema` lists every attribute. |
| Publication channels | `record_publications.rs` | `records.publish`, `contexts.*` | Flags only. Channel consumers read values through extensions. |
| Agent tools | `crates/agent-runtime/src/agent_tools.rs` (`get_record`, `get_record_context_preview`, `get_record_changes`, `get_value_history`, `search_records`, `find_records`, `count_records`; display labels from `get_record_labels`, `get_incoming_relationships` and `get_record_hierarchy`) | the person who started the conversation, checked on each call | Same as HTTP. Tool results are stored in conversations, which the workspace shares (filtered only by whether the anchor record is readable), and are sent to the model provider. Smart-fill sends the editable values to the provider. |
| Rules | `workers/src/rule_runtime.rs`, findings API | `rules.read` | No values, but findings name attributes and say whether they are missing or stale. |
| Workflows | `crates/workflow` | run as the system | A trigger can match on any value. `AttributeWrite` from an event field can copy one attribute's value into another attribute or into system metadata. |
| Files | `file_access.rs`, `files.rs` `file_read_targets` | record readability | Resolves file → record **without** the attribute. File metadata is embedded in form and resolved values. |
| Data health | `health.rs` | `data_health.read` | Aggregates only (fill rates per attribute). |
| Comments | `record_comments.rs` | `records.read` | Free text only. |
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
- History, record changes, event payloads and dead-letter replays.
- Agent conversations, which are stored, shared and sent to the model
  provider.
- Workflows that copy a value somewhere visible.

## Guidance today

Treat every attribute of a readable record as readable. To keep sensitive data
from some people, put it in a separate blueprint (for example a "Valuation"
record linked to the object), and grant `records.read` on that blueprint only
to the people who need it, with blueprint-family grants. Know the gaps that
remain:

- People with only blueprint-family grants cannot use search, facets or
  saved searches. Those need a workspace grant.
- A workspace grant reads everything.
- Relationship previews, hierarchy and incoming-relationship lists omit
  unreadable related records. Attribute values on a readable related record
  are still fully visible, including fields used in its `dropdown_option`.
- Extensions, workflows and connector exports see everything.

## Design proposal: restricted attributes

### Model

- An attribute declares `restricted = true` (inline blueprint attributes and
  reusable attribute revisions). It is part of the definition, so changing it
  needs a new revision, and the revision diff shows it.
- A new system permission, `records.read_restricted`, grants reading
  restricted values. It goes through roles and scopes like every other
  permission (workspace, blueprint family, record). Owners and admins get it.
  Editors and viewers do not, and custom roles can add it. A personal API
  token needs it explicitly.
- Writing a restricted attribute needs `records.write` **and**
  `records.read_restricted`. A save from someone without it leaves
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
   `restricted_readable(principal, record_or_blueprint) -> bool` next to
   `is_authorized`, batched like `authorized_record_ids`.
2. **One redaction function.** Add `redact(preview_or_values, restricted_codes)`
   and call it at the existing chokepoints:
   - value reads: `form_values`, `file_form_values`, `reusable_form_values`,
     `current_values`, `extension_attricat_values_at`, `value_history*` and
     `record_audit_changes*`;
   - projection reads: `get_record`, every `projections -> 'preview'` select in
     `record_search.rs`, `build_preview`/`resolved_preview`, and
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
5. **History and events.** History and record-change reads filter by
   attribute code through the same function. Domain events keep full values,
   because they are system data. Delivery to an extension redacts restricted
   facts unless the installation holds a new `attricat.read_restricted`
   capability, which administrators grant like the other capabilities.
6. **Extensions.** Attricat read calls redact unless the installation has
   `attricat.read_restricted`. Interactive runs also require the person who
   started the run to have `records.read_restricted`. Connector `schema`
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
  each linked record, and leave out or mark those the caller cannot read.
- Collection endpoints should filter rows for blueprint-family and record
  grants instead of requiring a workspace grant.
- `context_subtree` grants must either authorize value writes in the subtree,
  as `operate/workspaces.md` says, or that page must be corrected.

### Follow-up issues

Implementation is split into these follow-up issues:

1. [#366](https://github.com/attricat/attricat/issues/366): authorize linked
   records in preview and relationship reads, and correct what
   `context_subtree` grants do.
2. [#367](https://github.com/attricat/attricat/issues/367): restricted
   attributes: declaration, the `records.read_restricted` permission, the
   read principal and redaction of record reads and history.
3. [#368](https://github.com/attricat/attricat/issues/368): restricted
   attributes in search, facets, saved searches and the Explorer.
4. [#369](https://github.com/attricat/attricat/issues/369): restricted
   attributes for extensions, events, workflows, agent tools and files.
