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
