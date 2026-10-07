# Status attributes

A status is a single string attribute. Its JSON Schema `enum` stores stable
codes; the versioned `x-attricat-status` annotation supplies ordered labels,
optional semantic tones, and optional transition constraints. Other string
attributes are unaffected. Both blueprint and reusable attribute definitions
validate this annotation. The metadata contract is
[`status-control-v1.schema.json`](../contracts/status-control-v1.schema.json).

For example, this is a Core blueprint attribute (not a new value type):

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
  "type": "string",
  "enum": ["draft", "live", "retired"],
  "x-attricat-status": {
    "version": 1,
    "options": [
      { "code": "draft", "label": "Draft" },
      { "code": "live", "label": "Live", "tone": "success" },
      { "code": "retired", "label": "Retired" }
    ],
    "transitions": [
      { "from": null, "to": "draft" },
      { "from": "draft", "to": "live" },
      { "from": "live", "to": "retired" }
    ]
  }
}'''
```

## Configuration

- Codes are unique ASCII identifiers (`A-Z`, `a-z`, digits, `_`, `-`), at most
  128 characters. Every enum member must have exactly one option.
- Labels are nonblank text, at most 200 characters. There may be 1–100 options.
- Tones are `default`, `success`, `warning`, `error`, or `info`. Omit a tone for
  neutral presentation. Labels always remain visible; color alone is not a
  status indicator. Labels are catalog-authored text, never HTML.
- Labels resolve `{{key}}` / `{{key|context}}` workspace lexicon references
  like other catalog labels ([Translated labels](blueprints.md#translated-labels)),
  for example `"label": "{{Draft|status}}"`. Saving a blueprint or reusable
  attribute rejects malformed references in option labels, and the lexicon
  coverage report counts them as used. Codes are never translated.
- Omit `transitions` to allow any configured destination. Include it—even an
  empty array—to restrict changes to declared edges. At most 10,000 unique
  edges are allowed. Endpoints must be configured codes or `null`.
- `null` means absence, not a stored string or JSON null. Edges from `null`
  authorize initial values, including defaults. Edges to `null` authorize
  clearing. Clearing is never implicitly enabled by omitting the graph.
- An unchanged status is allowed. Requiredness remains an entity-schema
  constraint; a transition does not override it.

## Transition conditions

An edge may list `conditions` that must hold for the change to be accepted.
A condition has the same shape as an [entity check](json-schema-validation.md#declarative-checks):
a `code`, an optional `message` and a `predicate`.

```json
{ "from": "open", "to": "closed", "conditions": [
  { "code": "has-root-cause", "message": "Record the root cause before closing",
    "predicate": { "type": "required", "attribute_code": "root_cause" } },
  { "code": "no-open-actions",
    "predicate": { "type": "referenced_by", "blueprint_code": "corrective_action",
      "relationship_code": "nonconformance", "max": 0,
      "predicate": { "type": "one_of", "attribute_code": "status", "values": ["open"] } } }
] }
```

- At most 16 conditions per edge, with unique codes. Each `from`/`to` edge is
  declared once. Only synchronous-safe predicates are allowed (`stale`,
  `unique` and `acyclic` are rejected). Conditions are type-checked against
  the blueprint's attributes when it is saved.
- Conditions are evaluated on the transaction's final state, so values saved
  in the same write count: set `root_cause` and `status = "closed"` together.
  They are evaluated in every context where the status changes.
- Unchanged statuses and graphs without `transitions` have no conditions.
- Entity checks run first, then conditions, then enforcing rules that guard the
  transition ([rules](rules.md#enforcement)). Unmet conditions reject the whole
  write with `422 transition_conditions_unmet`, listing every unmet condition
  in the [violation shape](json-schema-validation.md#error-details).

### Available destinations

`GET /v1/entities/{id}/status-transitions?context_id=` (described under
[controlled records](#controlled-records)) also evaluates each declared edge's
conditions and the enforcing rules that guard it, on the saved state plus the
edge's destination. Each item has `unmet` in the violation shape. When the
caller may take the edge but `unmet` is not empty, `allowed` is `false`,
`denial_code` is `transition_conditions_unmet` and `denial_reason` joins the
unmet messages:

```json
{ "items": [{
  "attribute_code": "status", "from": "open", "to": "closed", "code": null,
  "allowed": false, "denial_code": "transition_conditions_unmet",
  "denial_reason": "Record the root cause",
  "unmet": [{ "source": "transition_condition", "code": "has-root-cause", "…": "…" }]
}] }
```

Unsaved form edits are not considered.

## Display and editing

Entity fields and compact value renderers display a labelled chip. Plain-text
renderers use the same label. Every label is resolved for the user's UI
language (then `en`, then the key) through `statusOptionLabel` in
`apps/catalog-web/src/features/entities/status.ts`: chips, the form select,
the Explorer attribute-filter value select (status filters offer `eq` only)
and filter pills. Search requests, saved searches, API payloads, events and
connector exports carry stable codes; consumers that render labels resolve
them from the attribute's `value_schema` and `GET /lexicon/entries`. Missing values display “Not set”; unknown or
retired values remain visible with an explanation, never silently coerced.
The entity form uses a single-select control, with forbidden destinations
disabled. The saved status—not another unsaved selection—is the starting
state. The normal form Save action persists the selection.

Readonly attributes, context restrictions and existing authorization still
apply. Draft restoration and smart-fill are revalidated. Unknown retired codes
may require correcting the definition before saving; retain old codes while
planning transitions and migrations rather than deleting a live option first.

## Contexts, atomicity and concurrency

Transitions compare effective values resolved through the context's parent
chain, unless `context_fallback = "none"`. Removing a local override is a
transition to the newly inherited value, which may differ from absence.
Changes to a parent context are also checked for affected descendants.
Reparenting a context is not an entity edit: entities are revalidated
structurally (schema, unique keys, hierarchies and entity checks) without
transition enforcement or records, approval voids or retention holds, even if
an inherited effective status changes.

The repository validates the transaction's original and final states. A batch
cannot submit intermediate statuses to traverse multiple edges in one save.
Invalid changes roll back together with their audit and event effects. Workflow
writes, history restoration and migrations use the same transition validation;
migrations also honor the source definition's policy. There is no silent
migration bypass.

For entity updates and value appends that target a status, send
`expected_updated_at` with the exact timestamp returned by the entity/form
read. History restoration and migration accept the same field as a query
parameter; migration previews provide `source_updated_at` for this purpose. Missing
preconditions return `428 status_precondition_required`; a changed entity
returns `409 stale_entity`, even if the requested transition is still valid.
Refresh, review the latest state, and explicitly restore/review a draft before
retrying. Non-status writes retain their existing API compatibility.

Legacy extension write paths that cannot supply a precondition cannot edit
statuses; they may continue writing unrelated attributes. Server-owned
transactional workflow actions still validate transitions against their locked
starting state.

## Controlled records

Edges and options accept optional controls. They are part of the same
versioned `x-attricat-status` annotation and are validated with it.

```json
{
  "options": [
    { "code": "approved", "label": "Approved",
      "approval": { "covers": ["title", "procedure"], "void_to": "review" } },
    { "code": "released", "label": "Released",
      "lock": "all", "retention_days": 3650 }
  ],
  "transitions": [
    { "from": "draft", "to": "review", "code": "submit" },
    { "from": "review", "to": "approved", "code": "approve",
      "roles": ["reviewer"], "separate_from": ["submit"] },
    { "from": "approved", "to": "released", "code": "release",
      "permission": "entities.publish" },
    { "from": "released", "to": "draft", "code": "correct", "roles": ["owner"] }
  ]
}
```

Definition rules: each `from`/`to` pair appears once; `separate_from` names
edge `code`s declared in the same graph; an option with `lock` requires a
declared `transitions` array, so leaving a lock is always an explicit edge;
`retention_days` (1–36,600) requires `lock`; `approval.void_to` names another
option. Plain codes in `lock` and `approval.covers` must be attributes of the
effective entity blueprint; qualified `namespace:code` reusable attributes are
attached per entity and are not checked at publication.

Requirements and conditions cannot refer to a
[user or team assignment](blueprints.md#user-or-team-assignments): `roles`,
`permission` and `separate_from` are evaluated for the acting principal only,
and no predicate can name the acting user. Predicates can only test an
assignment value as a string (`required`, `compare` `eq`/`ne` or `one_of`
against a literal `user:<uuid>` / `team:<uuid>`).

### Transition requirements

`permission`, `roles` and `separate_from` add to the edge check, for every
effective change in every context:

- `permission`: the acting principal holds the permission for the entity
  (workspace, blueprint-family or entity grant). A personal API token must also
  carry it.
- `roles`: the actor holds at least one of the role codes (system or
  workspace-local) through a workspace, blueprint-family or entity grant.
- `separate_from`: the actor is not the `actor_user_id` of the most recent
  `entity_status_transitions` row for the same entity, attribute and context
  whose `edge_code` is one of the listed codes.

The acting principal is the interactive extension initiator, otherwise the
request or agent audit actor (the human who approved an agent change), otherwise
the initiating actor recorded on the triggering event for workflows and other
event handlers. A restricted edge with no identifiable actor is denied.

Denials return `403 status_transition_forbidden` or
`403 status_separation_of_duties` and roll back the whole write. Unrestricted
edges behave as before. Every effective change is recorded in
`entity_status_transitions` with its edge code, actor and token.

`GET /v1/entities/{entity_id}/status-transitions?context_id=…` returns the
declared edges from the saved effective status in that context (default context
when omitted), each with `allowed`, `denial_code` and `denial_reason` for the
caller, and `unmet` transition conditions and enforcing rules (see
[available destinations](#available-destinations)). Like the write, it checks
every context whose effective status the edge would change: the selected
context and each context that inherits the status from it. Each `unmet` entry
lists the contexts in which it fails. The status control disables denied edges
and shows the reason; the server remains authoritative.

### Locks

When a context's saved effective status has `lock`, the covered attributes'
effective values in that context must be identical in the transaction's
original and final states. `"all"` covers every attribute, relationship and file
except the declaring status attribute. Comparisons use stored columns,
relationship target sets and ordered file references with their SHA-256, so
resaving identical values is not a change. Original values are reconstructed
from rows created before the transaction plus rows the transaction archived.
File uploads, links and reorders, which modify reference rows in place, are
checked before writing against the written context and every context that
inherits from it.

The check runs on every path that validates statuses: entity create/update,
value append and removal, relationship mutations, workflow actions, extension
catalog batches, file uploads, links and reorders, history restoration,
reusable attribute attachment and migration (against the source definition). Entity deletion is refused while any
context is locked. A violation returns `409 record_locked`.

Locks are evaluated from the starting status, so moving into a locked status may
carry final edits, while leaving one must be a pure status change. An effective
change away from a locking status is recorded with `unlocked = true` and audited
as `entity.record.unlock` with the attribute, context, endpoints and edge code.

### Approvals

Entering an option with `approval` records an `entity_approvals` row for that
attribute and context: actor, time, covered attributes (or `covers_all`) and the
SHA-256 of the covered effective content as canonical JSON. A new approval for
the same attribute and context supersedes the previous one
(`end_reason = 'superseded'`).

After every write, active approvals whose covered content digest changed are
ended with `end_reason = 'content_changed'`. If the context's effective status
still equals the approved option, the same transaction writes the option's
`void_to` value in that context (parents first, skipping contexts that already
inherit the void status) and records an `approval_void` transition. This
automatic change bypasses edge and permission checks by design. Audit actions
are `entity.approval.record` and `entity.approval.void`.
`GET /v1/entities/{entity_id}/approvals` lists approvals newest first.

### Retention

Entering an option with `retention_days` creates `file_retention_holds` rows for
every file referenced by the locked attributes in that context, held until the
transaction time plus the period, and audits each as
`file.retention_hold.place`. Reclamation skips held files; see
[Retention holds](../apps/docs/src/content/docs/operate/workspaces.md#retention-holds).

### Agent

Agent mutations run as the approving user and hit the same checks. Tool errors
carry the stable codes above, and the read-only `get_entity_record_controls`
tool returns transition access, approvals and holds so the agent can explain a
denial instead of retrying.

Every single-entity agent edit (values, value removal and restore,
annotations, relationships, file links, blueprint migration and deletion) and
each update or delete in an agent batch carries `expected_updated_at`. When the agent omits it, the runner records the
entity's `updated_at` when the change is proposed, so the approved write
applies to the state the approver saw and returns `409 stale_entity` if the
entity changed while it waited for approval. The same precondition lets these
edits change a status.
