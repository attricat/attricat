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
  status indicator. Labels are catalog-authored text, not translation keys or
  HTML.
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
  write with `422 transition_conditions_unmet`; `error.details.violations`
  lists every unmet condition with `source = "transition_condition"`,
  `transition: {attribute_code, from, to}`, `contexts` and `attributes`.

### Available destinations

`GET /v1/entities/{id}/status-transitions?context_id=` (default context when
omitted; `entities.read`) evaluates each status attribute from the saved state:

```json
[{
  "attribute_code": "status", "context_id": "…", "context_code": "default",
  "current": "open",
  "destinations": [
    { "to": "draft", "allowed": false, "reason": "transition_not_allowed", "unmet": [] },
    { "to": "closed", "allowed": false, "reason": "conditions_unmet",
      "unmet": [{ "source": "transition_condition", "code": "has-root-cause", "…": "…" }] }
  ]
}]
```

Destinations exclude the current value. `reason` is `transition_not_allowed`
for an undeclared edge and `conditions_unmet` when conditions or enforcing
rules guarding that transition fail; `unmet` uses the violation shape. The
saved state plus the destination is evaluated, so unsaved form edits are not
considered.

## Display and editing

Entity fields and compact value renderers display a labelled chip. Plain-text
renderers use the same label. Missing values display “Not set”; unknown or
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

Legacy extension/agent write paths that cannot supply a precondition cannot
edit statuses; they may continue writing unrelated attributes. Server-owned
transactional workflow actions still validate transitions against their locked
starting state.
