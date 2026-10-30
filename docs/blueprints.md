# Blueprint Authoring

Blueprints are versioned YAML definitions. Records remain pinned to the exact
blueprint version used to create them.

```yaml
format_version: 1
code: product
name: Product
kind: record
```

`kind` is either `record` or `mixin`. Mixins can be included but cannot create
records.

An optional `description`, up to 500 characters, says what the blueprint's
records are, such as `description: Product groupings, such as Basic tools`.
Inline attributes accept the same `description` for what they hold; an
attribute selected with `from` uses the mixin's. Record forms show an inline
attribute's description below its field, and the agent reads descriptions in
its catalog map, so describe a blueprint or field in the words people use for
it.

Blueprint codes, attribute codes, include codes, and relationship target
blueprint codes contain only ASCII letters, numbers, hyphens, and underscores.

YAML is indentation-sensitive: indent with spaces, never tabs, and keep list
items at the same depth. Quote strings that contain `: ` or ` #`, or start with
`{`, `[`, `*`, `&`, `!`, `|`, `>`, `'`, `"`, `%`, or `@`. That includes labels
that start with a lexicon reference: `name: '{{Product}}'`. Only `true` and
`false` are booleans; `yes`, `no`, `on`, and `off` are strings. JSON Schema
values (`value_schema`, `record_schema`) are JSON strings: a single-quoted
one-liner or a `|` block with the JSON indented under the key.

## Attributes

Every attribute declares exactly one of `value_type` or `from`. Supported value
types are `string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`,
`relationship`, and `file`. Annotations on a `string` attribute's
`value_schema` add [statuses](#status-attributes) and
[user or team assignments](#user-or-team-assignments).

```yaml
attributes:
  - code: stock
    value_type: integer
    context_fallback: default
    context_editable: default
```

`context_fallback` controls missing values in a non-default context:

- `default` is the default and uses the nearest ancestor with a value or set,
  up to the root `default` context.
- `none` leaves the attribute absent when it has no direct value.

`context_editable` controls writes for every value type, including relationships:

- `all` is the default and permits writes in every context.
- `default` permits writes only in the default context. Other contexts render it
  read-only, and the API rejects writes.

Set `readonly: true` to make an attribute preview-only in the Attricat web app.
It is intended for values managed through authorized API or CLI operations,
including agent and extension actions; `readonly` does not restrict those
server-side writes. Rules evaluate findings and do not write attribute values.
The default is `false`. Read-only attributes remain visible on the record page and in record forms but
cannot be changed, cleared, linked, or uploaded through the web UI.

```yaml
attributes:
  - code: external_id
    value_type: string
    readonly: true
```

`readonly` is a UI hint only. To have Attricat assign a value that nobody can
write, use a [generated value](#generated-values).

Set `name` to give an attribute a human-readable label. The web app shows it in
forms, previews, Explorer filters and facets, and as the default table column
heading. Without it, the app humanizes the code (`product_family` becomes
"product family"). Blank names are rejected.

```yaml
attributes:
  - code: product_family
    name: Family
    value_type: string
```

Blank contextual string fields remove their override instead of storing an empty
string. Missing contextual values resolve according to `context_fallback`.

### Generated values

Set `generated` to have Attricat number a record attribute when the record is
created, such as `INV-2026-00042`. The value is assigned inside the create
transaction, so it is in the create response, the `record.created.v1` event and
the audit entry, and a `record_schema` `required` on it holds. The web create
form leaves the field empty with an "assigned when saved" hint and never marks
it required, since the user has nothing to fill; the server enforces the
requirement.

```yaml
attributes:
  - code: invoice_number
    value_type: string
    generated:
      format: 'INV-{yyyy}-{n:05}'
      reset: yearly
      time_zone: Europe/Warsaw

  - code: sequence
    value_type: integer
    generated: {start: 1000}

unique_keys:
  - code: invoice_number
    attributes: [invoice_number]
```

- Only `string` and `integer` attributes of record blueprints can be generated.
  Mixins, `from` selections and extension types cannot, and `generated` cannot
  be combined with `default_value` or with a status or user-or-team assignment.
- `format` is required on strings and not allowed on integers, which store the
  number itself. It is literal text with these tokens:

  | Token | Output |
  | --- | --- |
  | `{n}` | The counter. Required, exactly once. |
  | `{n:0W}` | The counter zero-padded to W digits (1 to 18). Wider numbers are not truncated: `{n:03}` gives `1000` after `999`. |
  | `{yyyy}`, `{yy}` | Four- or two-digit year. |
  | `{mm}`, `{dd}` | Two-digit month and day. |
  | `{{`, `}}` | A literal brace. |

  Any other `{…}` placeholder is rejected. Formats are at most 100 characters.
- `start` is the first number of a new counter (default 1, from 0 to
  999,999,999,999,999,999).
- `reset` restarts numbering at `start` every `yearly`, `monthly` or `daily`
  period; the default `never` keeps one counter. A reset needs tokens that
  identify the period (`{yyyy}` or `{yy}`, plus `{mm}` for monthly, plus
  `{dd}` for daily), or values would repeat.
- `time_zone` is the IANA zone in which date tokens and periods are read from
  the record's creation time (default `UTC`).

A generated attribute is always read-only and editable in the default context
only, whatever `readonly` and `context_editable` say. No client can write it:
creates, updates, clears, history restores, record batches, workflows, agents
and extensions that try fail with `422 generated_attribute_readonly` and
`error.details = { attribute }`.

Each blueprint family has one gapless counter per generated attribute code (and
per period with `reset`). Creates that number the same attribute run one at a
time while the counter is locked, and a create that fails returns its number.
Deleted records keep their numbers, which are never reused. New revisions
continue the same counter: changing `format` affects only new values, and
`start` only applies when a counter is first created. A revision cannot change
the `reset` of an attribute that an earlier revision of the family generated,
because the new period would restart numbering and repeat numbers already
issued; creating or validating such a revision fails with
`409 generated_reset_changed` and `error.details = { attribute, previous,
requested }`. Changing `start`, `format` or `time_zone`, or dropping
`generated`, is allowed. A unique key on the attribute is optional but
recommended.

- Duplicating a record numbers the copy like any new record.
- Records created before the attribute existed get a number when they
  [migrate](#record-migration-status) onto a revision that adds it, in the order
  they migrate. Date tokens and the period use the record's creation time. A
  value carried over from the previous revision is kept.
- Solution-pack sample records use up numbers; removing them does not return
  the numbers.

`GET /blueprints/{id}/sequences` lists the family's counters and
`POST /blueprints/{id}/sequences/{attribute_code}/advance` raises one, for
example above the highest number of a system you are migrating from; see
[the API reference](api.md). Keep legacy numbers in a separate plain attribute.

### Attribute tags

Attribute `tags` are free-form metadata for Attricat extensions and domain
integrations. Attricat reserves the following visibility tags as default UI
hints:

| Tag | Default omission surface |
| --- | --- |
| `hidden` | Every native default surface listed below |
| `hidden:form` | Editors added outside the layout (**Other attributes**) and the fallback create form |
| `hidden:detail` | Fallback record preview/detail views |
| `hidden:explorer` | Explorer facet and filter candidates |
| `hidden:metadata` | The blueprint Attributes metadata table |

Tags compose: `hidden` applies to every surface, while a scoped tag applies only
to its named surface. They affect automatic/fallback discovery only; an
explicit blueprint view may deliberately render a tagged attribute. Tags are
not access control: attribute metadata and values remain available through the
API, CLI, agents, and extensions according to their existing permissions. Raw
blueprint YAML remains available to blueprint administrators.

```yaml
attributes:
  - code: price_amount
    value_type: integer
    tags: ['hidden:form', 'hidden:detail']
```

Scalar attributes may set `default_value`. The value is stored in the default
context when a record is created, unless the create request supplies a value
for that attribute in the default context. Defaults support `string`, `number`,
`integer`, `boolean`, `date`, `datetime`, and `time`; relationships and files do
not support defaults.

```yaml
attributes:
  - code: status
    value_type: string
    default_value: draft
```

A string attribute becomes a status when its `value_schema` has an `enum` and an
`x-attricat-status` annotation with `version: 1`, one `options` entry
(`code`, `label`, optional `tone`) per enum code, and optional `transitions`
(`from`/`to` codes or `null`). Controlled records add optional keys:

- on a transition: `code` (names the edge), `permission` (a permission code the
  actor needs), `roles` (role codes, any one suffices) and `separate_from`
  (edge codes whose most recent actor may not make this transition);
- on an option: `lock` (`"all"` or attribute codes that become read-only while
  the record has that status; requires declared `transitions`), `approval`
  (`{"covers": "all" | [codes], "void_to": "<option>"}`: entering records an
  approval bound to a digest of the covered content, and a later change to that
  content voids it and moves the record to `void_to`) and `retention_days`
  (holds the locked files for that many days; requires `lock`).

Restricted transitions fail with `403 status_transition_forbidden` or
`403 status_separation_of_duties`; changes to locked content fail with
`409 record_locked`. A correction is a separate, restricted transition out of
the locked status that changes nothing else. Owners and administrators can also
unlock any locking status outside the declared transitions and are exempt from
separation of duties (`records.unlock`).

```yaml
attributes:
  - code: status
    value_type: string
    value_schema: |-
      {"type":"string","enum":["draft","released"],"x-attricat-status":{"version":1,
        "options":[{"code":"draft","label":"Draft"},{"code":"released","label":"Released","lock":"all"}],
        "transitions":[{"from":null,"to":"draft"},
          {"from":"draft","to":"released","code":"release","permission":"records.publish"},
          {"from":"released","to":"draft","code":"correct","roles":["owner","admin"]}]}}
```

A relationship may restrict its target type and directional cardinality.
`cardinality: one` allows at most one active target per source and context;
`target_cardinality: one` allows at most one source to claim a target for the
versioned field and context. Each defaults to `"many"`, so `cardinality: one`
alone models a reusable single-select relationship.

```yaml
attributes:
  - code: category
    value_type: relationship
    target_blueprint: category
    cardinality: one
```

A relationship write that would violate this invariant returns
`409 relationship_cardinality_conflict`. A migration preview reports the same
kind of issue when existing relationship values cannot fit a target revision's
source limit. Set both cardinalities to `"one"` only for exclusive pairing; see
[Tags, labels, and classifications](classifications.md) for the recommended
model for controlled classifications.

### Several target blueprints

`target_blueprints` lists every blueprint a relationship may target. It cannot
be combined with `target_blueprint`; a one-item list is the single-target form.

```yaml
attributes:
  - code: subject
    value_type: relationship
    target_blueprints: [product, product_revision, material, part]
```

A write linking a record of any other blueprint returns
`422 relationship_target_type_mismatch`. Compiled attributes expose the complete
allowed set as `target_blueprint_codes` (empty means any blueprint);
`target_blueprint_code` is set only when exactly one target is allowed, so
single-target consumers such as table column paths, Explorer relationship
filters, and tree facets treat a multi-target relationship like an unrestricted
one. The record relationship picker searches one allowed blueprint at a time with a
**Target blueprint** selector, offering every record blueprint when the
relationship names no target, and an `incoming_relationship_list` on any
allowed target can list the field. A migration preview reports
`relationship_target_changed` when the allowed set differs between revisions.

### Hierarchies

`acyclic: true` rejects relationship writes that would close a cycle through
the attribute. `tree: true` is an acyclic hierarchy where every record has at
most one target (its parent): it implies `acyclic` and defaults `cardinality`
to `"one"` (`"many"` is rejected). Both require `context_editable: default`,
because cycle checks follow direct default-context edges, and the targets must
include the blueprint itself (any target set is allowed for mixins; a consuming
record blueprint must then be among the selected attribute's targets).

```yaml
attributes:
  - code: parent
    value_type: relationship
    target_blueprint: location
    tree: true
    context_editable: default
```

- A write closing a cycle returns `409 relationship_cycle` with
  `error.details = { attribute, path }`. `path` lists record IDs from the
  written record along the cycle back to it, for example `[c, a, b, c]`; a
  self-link is `[a, a]`.
- In a tree, a second target returns `409 relationship_cardinality_conflict`,
  also for records pinned to a revision that allowed several.
- The check walks edges of every revision of the blueprint family's field
  (matched by code), resolved in each context whose value the edge becomes.
  It runs while the edge is inserted, under the workspace relationship lock
  every relationship writer holds, so concurrent writes cannot jointly create
  a cycle.
- Removing an edge is not checked. For a record pinned to an older revision
  that allowed non-default edges, removing its last edge in a context exposes
  the inherited edge there, which can close a cycle. Publishing a revision
  that changes the hierarchy, and moving a context to another parent, recheck
  all edges and report such cycles.
- The latest published revision decides whether a field is a hierarchy, for
  every record in the family.
- Publishing a revision that adds or changes `acyclic`/`tree` checks existing
  edges first. Cycles, or tree records with more than one target, fail
  publication with `409 relationship_hierarchy_violations` and
  `error.details = { attribute, cycles, multiple_parents }` (up to 20 each).

File attributes declare their cardinality and upload policy. `many` values are
ordered by default; set `ordered: false` when callers must not rely on their
order. File policy fields are valid only with `value_type: file`.

```yaml
attributes:
  - code: product_images
    value_type: file
    cardinality: many
    allowed_mime_groups: [image]
    allowed_extensions: [jpg, png, webp]
    max_bytes: 10485760
    purposes: [product_image]
    image_only: true
```

`cardinality` is `one` by default. The compiler rejects a file value schema or
relationship target, duplicate/empty policy entries, invalid purpose codes,
zero size limits, and ordered single-file declarations. The persisted policy is
part of the pinned blueprint revision.

### Status attributes

A status is a `string` attribute whose `value_schema` has an `enum` of option
codes and the versioned `x-attricat-status` annotation:

```yaml
attributes:
  - code: status
    value_type: string
    value_schema: |-
      {
        "type": "string",
        "enum": ["draft", "review", "released"],
        "x-attricat-status": {
          "version": 1,
          "options": [
            { "code": "draft", "label": "Draft" },
            { "code": "review", "label": "In review",
              "approval": { "covers": "all", "void_to": "draft" } },
            { "code": "released", "label": "Released", "tone": "success",
              "lock": "all", "retention_days": 3650 }
          ],
          "transitions": [
            { "from": null, "to": "draft" },
            { "from": "draft", "to": "review", "code": "submit" },
            { "from": "review", "to": "released", "code": "release",
              "roles": ["reviewer"], "separate_from": ["submit"],
              "conditions": [{ "code": "has-approver",
                "predicate": { "type": "required", "attribute_code": "approver" } }] },
            { "from": "released", "to": "draft", "code": "correct",
              "permission": "records.publish" }
          ]
        }
      }
```

- `options` (1–100, required): one per `enum` code, in display order. `code`
  (≤ 128 chars) and nonblank `label` (≤ 200 chars, may use `{{key}}` lexicon
  references) are required. Optional: `tone` (`default`, `success`,
  `warning`, `error`, `info`); `lock` (`"all"` or 1–500 attribute codes,
  `namespace:code` for reusable attributes) makes covered content read-only in
  that status and blocks record deletion, and requires `transitions`;
  `approval` `{covers, void_to}` records an approval bound to a digest of the
  covered content and moves to `void_to` (another option) when it changes;
  `retention_days` (1–36,600, requires `lock`) holds the locked attributes'
  files.
- `transitions` (≤ 10,000): omit to allow any change; an empty array allows
  none. `from`/`to` are option codes or `null` (no value: initial set or
  clear); each pair once; unchanged values are always allowed. Optional
  `code`; `permission` (`area.action`); `roles` (1–20 role codes held through a
  workspace, blueprint-family or record grant); `separate_from` (1–20 edge
  `code`s whose most recent actor on this record and context may not take this
  edge); `conditions` (≤ 16 checks with the
  [predicate](json-schema-validation.md#predicates) shape, synchronous-safe
  only).
- Violations: forbidden edge `422 attribute_value_schema_mismatch`; unmet
  conditions `422 transition_conditions_unmet`; `permission`/`roles`
  `403 status_transition_forbidden`; `separate_from`
  `403 status_separation_of_duties`; locked content or deletion
  `409 record_locked`; status writes without `expected_updated_at`
  `428 status_precondition_required`.
- Statuses compare effective values per context, and every writer (API, CLI,
  workflows, agents, extensions, restores, migrations) is checked. Details:
  [status control](status-control.md).

### User or team assignments

To store who is responsible for a record (assignee, owner, reviewer), use a
`string` attribute with the versioned `x-attricat-principal` annotation instead
of free text. `kinds` lists what it accepts: `user`, `team`, or both. The
contract is [`principal-attribute-v1.schema.json`](../contracts/principal-attribute-v1.schema.json).

```yaml
attributes:
  - code: assignee
    name: Assignee
    value_type: string
    value_schema: |-
      {
        "type": "string",
        "x-attricat-principal": { "version": 1, "kinds": ["user", "team"] }
      }
```

- A value is the canonical text `user:<uuid>` (a user ID) or `team:<uuid>` (a
  [team](api.md#teams) ID), lowercase and hyphenated. A missing value means
  unassigned. Reusable attributes accept the same annotation.
- The schema must be `"type": "string"` with no `enum`, `const`, `pattern` or
  `format`, cannot also be a status, and the attribute cannot declare a
  `default_value` (IDs differ between workspaces).
- On every write path (API, CLI, web app, agent, workflows, history restore,
  migration) a value that changes must name an **active member** of the
  workspace or a **team that is not deleted**, and a kind the attribute
  accepts. Otherwise the save fails with `422 attribute_value_schema_mismatch`.
  Unchanged values are not rechecked, so a record assigned to someone who left
  can still be edited; the web app shows that person or deleted team with an
  explanation.
- `GET /directory` (any catalog reader) lists users (`id`, `display_name`,
  `email`, `active`) and teams (`id`, `code`, `name`, `deleted`); the agent tool
  `get_workspace_directory` returns the same. Look up IDs there before
  assigning.
- Search and Explorer filters match the stored reference exactly (`eq` only in
  the web app). The filter value `@me` with operator `eq` means *assigned to
  me*: it matches the caller and every team the caller belongs to. Saved
  searches keep `@me`, so they resolve for whoever runs them.
- Neither rule predicates nor status transition requirements can refer to the
  assignee or the acting user. Predicates see the value as a string, so
  `required`, `compare` `eq`/`ne` and `one_of` against a literal
  `user:<uuid>` / `team:<uuid>` work. Transition `permission`, `roles` and
  `separate_from` always check the acting principal, never an assignment
  attribute, so "only the assignee may close" cannot be declared. The helper
  `attricat_validation::principal::principal_matches` exists but is not wired
  into either.

## Unique Keys

Record blueprints may declare business keys that no two records of the
blueprint family can share:

```yaml
unique_keys:
  - code: manufacturer_part
    attributes: [manufacturer, part_number]

  - code: slug
    attributes: [slug]
    scope: context
    case_sensitive: true
```

- `code` is unique within the blueprint. `attributes` lists one to eight
  distinct attribute codes (including `from` selections). Each must be a scalar
  other than `json`, or a relationship with `cardinality: one`; files and
  many-target relationships are rejected. Mixins cannot declare keys.
- `scope: workspace` (default) compares default-context values. `scope =
  "context"` compares resolved values, including inherited ones and respecting
  `context_fallback`, separately in every context.
- Normalization: text is trimmed, whitespace runs collapse to one space, and
  unless `case_sensitive: true` text is lowercased. Numbers compare by value
  (`1.50` = `1.5`), datetimes by instant, relationships by target record ID.
- A record missing any key attribute in a context (blank text counts as
  missing) does not participate in that key there. Require the attributes in
  `record_schema` when every record must have the key.
- The latest published revision of the family defines the enforced keys for
  every record in the family, including records pinned to older revisions.
  Attributes are matched by code.

A [generated](#generated-values) attribute can be a key on its own or with
other attributes; its counter never repeats a value, and the key also guards
values carried over by migrations.

A write that gives a second record the same key value returns
`409 unique_key_conflict` with
`error.details = { key, context, values, conflicting_record_id }`; `values` are
the normalized components. The database enforces keys inside the write
transaction, so of two concurrent duplicate writes exactly one commits.
Recover by updating or reusing the conflicting record, or by choosing a
different value.

Publishing a revision that adds or changes keys re-indexes the family. If
existing records already share a value, publication fails with
`409 unique_key_duplicates` and
`error.details = { duplicates: [{ key, context, values, record_ids }], total }`
(up to 20 groups). Fix the duplicates and publish again.

## Record Schema

Record blueprints may define an `record_schema` JSON Schema contract. Root
`required`, `properties`, `dependentRequired`, and `dependentSchemas` entries
must name attributes materialized by the blueprint, including selected include
attributes. Nested schema properties describe an attribute's value and are not
treated as blueprint attribute codes.

## Includes

Includes are exact version-pinned dependencies. Select each mixin attribute that
the consuming blueprint should materialize:

```yaml
includes:
  - alias: seo
    code: seo
    version: 2

attributes:
  - code: meta_title
    from: seo.meta_title
```

A selected attribute keeps the mixin attribute's `name` and cannot declare its
own.

## Dropdown Options

Record blueprints must define how they appear in relationship dropdowns:

```yaml
views:
  dropdown_option:
    type: dropdown_option
    fields: [name, sku]
    separator: ' / '
```

`separator` defaults to `·`. Server-rendered dropdown labels respect each attribute's
`context_fallback` policy.

## Views

Record blueprints can optionally define app views. Existing blueprints without
views use the platform's schema-order fallback.

```yaml
views:
  detail:
    type: stack
    children:
      - type: tabs
        tabs:
          - label: Overview
            children:
              - {type: field, field: title}
          - label: Operations
            children:
              - type: accordion
                sections:
                  - label: Stock
                    children:
                      - {type: field, field: stock_on_hand}
  table:
    type: table
    columns:
      - {field: title, label: Product}
      - {field: category.name, label: Category}
```

`columns` is the current table syntax. Each column names either a local scalar
field, a renderer-compatible direct file field, or a scalar field through at
most three relationship hops. Column paths
must be unique; each relationship and the scalar leaf must exist.
`label` is optional. The legacy `fields: [title, stock_on_hand]`
shorthand remains supported for local scalar fields, but cannot be combined
with `columns`.

A column can use a built-in `attricat.*` renderer or an installed extension cell
renderer. Built-in `attricat.table_image@1` renders a direct image-only,
single-file attribute as a thumbnail:

```yaml
views:
  table:
    columns:
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
```

It is not valid for relationship paths, multi-file attributes, or file
attributes that are not `image_only: true`.

An extension renderer ID and positive version must match an enabled extension
declaration for the resolved scalar value type, and `props` must be an object:

```yaml
views:
  table:
    columns:
      - field: price
        label: Price
        renderer:
          id: example.currency
          version: 1
          props: {currency: USD}
```

See [Extensions](extensions.md#client-extension-runtime-v1) for the renderer
manifest and sandbox contract.

`stack`, `grid`, `section`, `tabs`, and `accordion` are recursive layout
blocks. `heading`, `text`, and `divider` are static blocks. `field` renders a
typed attribute and `relationship_list` renders a relationship attribute.
Table views also support direct file fields with a compatible renderer and
scalar leaves reached through relationship paths, as shown above.

### Incoming Relationships

`incoming_relationship_list` displays records that reference the current
record through configured relationship fields. It opens a modal and does not
request linked records until the user opens it. Results are cursor-paginated;
`page_size` controls each requested page and is bounded by the API's
`INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` setting.

```yaml
views:
  detail:
    type: stack
    children:
      - {type: field, field: name}
      - type: incoming_relationship_list
        label: Products in this category
        page_size: 10
        relationships:
          - {source_blueprint: product, field: categories}
        component: {id: attricat.incoming_relationship_list_display, version: 1}
```

Each selector names a source blueprint and one of its relationship fields. A
source record matched by multiple selectors appears once. Selecting an item
opens that source record. A field with several `target_blueprints` can be
listed on every allowed target blueprint.

`views.detail` lays out both display and editing: the record page renders
every field the user may change as an editor in its place, and the create and
migration forms use the same layout. The compiler rejects a `views.edit`.

Field, relationship-list, and table blocks may optionally reference a
platform-registered component:

```yaml
views:
  detail:
    children:
      - type: field
        field: website
        component: {id: attricat.url_display, version: 1}
```

Component IDs use lowercase, underscore-separated dotted namespaces. React
component definitions are assembled by the TypeScript registry at
`apps/web/src/features/views/components/registry.ts`, with each
definition in its own module. The Rust validation contract is
`contracts/view-components.json`; it validates the
version, allowed props, placement, value type, and requested view capability.
The frontend registry test enforces matching metadata between the two.

See [View Configuration](views.md) for the complete block grammar, heading
configuration, and component contracts. See
[Component Authoring](component-authoring.md) for the implementation workflow.

## Translated labels

Attricat labels are literal text unless they reference the workspace lexicon
with double braces. The text inside the braces is both the lexicon key and the
English default:

```yaml
name: '{{Product}}'

views:
  detail:
    tabs:
      - label: '{{Overview}}'
        children:
          - type: incoming_relationship_list
            label: '{{Products in this category}}'
```

These fields resolve references: the blueprint `name` and `description`,
inline attribute `name` and `description`, `tabs` and `accordion` section `label`, table `columns[].label`,
`incoming_relationship_list` `label`, the `name` of a reusable attribute
definition, and each option `label` in an `x-attricat-status` annotation on an
inline or reusable attribute ([status control](status-control.md)). Headings,
text blocks, separators, and values (including status codes) are always
literal.

- `{{key}}` is a reference. The web app shows the lexicon entry for the user's
  UI language, then the `en` entry, then the key itself, so an untranslated
  label still reads correctly in English.
- `{{key|context}}` adds a disambiguation context (gettext `msgctxt`) for
  homonyms and grammatical agreement, such as `{{Order|purchase}}` and
  `{{Order|sorting}}`. The context is never displayed, and a reference with a
  context never falls back to the entry without one.
- Keys and contexts are compared exactly after trimming and collapsing
  whitespace; case matters. Keys are never derived from attribute codes.
- Text outside braces is literal. `\{{` writes a literal `{{`; other
  backslashes are literal.
- Wrap whole phrases (`"{{Products in this category}}"`), not single words in a
  sentence (`"{{Products}} in this category"`): word order, case, and gender
  differ between languages.

Saving a definition rejects malformed references in these fields: an
unclosed `{{`, an empty key or context, more than one `|`, or a brace inside a
reference. The grammar is implemented once in `crates/lexicon` and mirrored by
the web app; both run the cases in `contracts/lexicon-references.json`.

The lexicon is workspace data outside blueprints, so translation fixes and new
languages never need a new blueprint revision. Each entry is identified by
`(key, context, language, plural_category)`. An `en` entry overrides the
displayed English without re-keying, so existing translations stay attached.
Labels follow the user's UI language, independently of the selected attribute
context.

Labels without a count use separate keys for singular and plural wording
(`{{Product}}`, `{{Products}}`). Where the app renders a count of records (the
Explorer result total), it uses the blueprint name's plural forms: entries for
each CLDR plural category of the language (`one`/`other` in English;
`one`/`few`/`many`/`other` in Polish). Forms contain only the noun; the app's
own string positions the number. Without plural forms for the user's language
or English, the app keeps its generic wording. When an entry has plural forms,
countless lookups use its `one` form. Entries without plural forms use
`other`.

Manage entries on the web app's **Manage → Translations** page or with
`acli lexicon` ([CLI](cli.md#translations), [API](api.md#lexicon))
and use `acli lexicon report` to find untranslated references, missing plural
categories (blueprint names always need them), and orphaned entries no
non-deleted blueprint revision or reusable attribute references.

## JSON Schema Validation

Blueprint attributes can define scalar `value_schema` contracts and record
blueprints can define an `record_schema` for cross-field validation. Both use
JSON Schema Draft 2020-12 and are enforced by the API before values are stored.
See [JSON Schema Validation](json-schema-validation.md) for authoring syntax,
context behavior, and error handling.

## Declarative checks, conditions, and rules

One predicate engine (`required`, `has_tag`, `missing_tag`, `compare`,
`one_of`, `relative_date`, `linked`, `referenced_by`, `all_of`, `any_of`, plus
rules-only `stale`, `unique`, `acyclic`) is used by record checks, transition
conditions and data quality rules. Each is type-checked against the
blueprint's attributes when it is saved.

Record checks reject saves. Put them in `record_schema` under
`x-attricat-checks` (at most 32; `code`, optional `message`, `predicate`):

```yaml
record_schema: |-
  {
    "x-attricat-checks": [
      {"code": "valid-range", "message": "Valid until must not be before valid from",
       "predicate": {"type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from"}},
      {"code": "facility-of-supplier",
       "predicate": {"type": "linked", "relationship_code": "facility",
         "predicate": {"type": "compare", "attribute_code": "supplier", "op": "eq", "subject_attribute_code": "supplier"}}}
    ]
  }
```

Inside `linked`, `attribute_code` refers to the linked record and
`subject_attribute_code` to the record being saved. Changes to a linked record
are not rejected; event-triggered rules report affected records as findings.

Transition conditions guard status edges. Add `conditions` (at most 16, same
shape) to an edge in `x-attricat-status.transitions`:

```yaml
value_schema: |-
  {"type": "string", "enum": ["open", "closed"],
    "x-attricat-status": {"version": 1,
      "options": [{"code": "open", "label": "Open"}, {"code": "closed", "label": "Closed"}],
      "transitions": [{"from": null, "to": "open"},
        {"from": "open", "to": "closed", "conditions": [
          {"code": "has-root-cause", "predicate": {"type": "required", "attribute_code": "root_cause"}}]}]}}
```

Data quality rules use the same predicates but are not part of the blueprint:
they are created separately through `/rules`, report findings, and can also
enforce (`enforcement`). A blueprint definition with a `rules` list is
rejected. A rule checks every revision of its blueprint that it fits, so
publishing a revision never waits for rules: the publish response lists the
enabled rules the revision does not fit as `rule_warnings`, and those rules
skip its records. See [Rules](rules.md#blueprint-revisions).

Failures return `422 record_check_failed`, `transition_conditions_unmet` or
`rule_violation` with `error.details.violations`. See
[JSON Schema Validation](json-schema-validation.md#predicates) for every
predicate field and limit and the `error.details` shape,
[Status attributes](#status-attributes), and [Rules](rules.md) for the dry run
required before an enforcing rule is enabled.

## Record migration status

When a published record blueprint revision is storage-compatible with its
immediately preceding published revision, **Migrate compatible records** starts
a background migration batch. The batch examines every active record pinned to
an older version of that blueprint, not only records on the immediately
preceding version. Each record is migrated when its individual preview is
`ready`; incompatible records remain available for review. Open the blueprint
in **Manage → Blueprints** and select the **Migrations** tab to inspect every
batch for that blueprint. While a batch for the current target version is
`queued` or `running`, the migration action is disabled so another batch cannot
be started for that version.

The table shows the target version, current status, processed/total progress,
migrated, needs-review and failed counts, creation/start/completion timestamps,
and batch ID. While any batch is `queued` or `running`, the page refreshes
automatically every two seconds; **Refresh** requests the latest state
immediately. A completed batch remains in the history. The UI reads this state
from `GET /api/blueprints/{blueprint_id}/migration-batches`; migration batch rows
in `blueprint_migration_batches` remain the source of truth.

Batch statuses are:

- `draft`: created but not queued (retained for compatibility with older rows),
- `queued`: persisted and waiting for a worker,
- `running`: claimed by a migration worker,
- `completed`: the worker inspected every eligible record and finished, and
- `superseded`: replaced by a newer batch.

`completed` describes the batch lifecycle. Individual records that could not be
migrated remain recorded in `record_blueprint_migrations` for separate review.

## Validation

Unknown keys, invalid selectors or policy values, missing mixins, and malformed
definitions return `422 invalid_blueprint_definition`.

## Definition schema and editor assistance

The blueprint and reusable attribute editors complete keys, view blocks, and
values; show the documentation of each key and chosen value on hover, including
JSON Schema keywords, record checks, predicates and status control inside
`record_schema` and `value_schema`; and flag YAML syntax errors, unknown keys, and invalid values while you type. Value
suggestions include workspace blueprints and mixins, attributes declared in the
definition, relationship hops in table column paths, included mixin attributes
for `from`, roles, contexts, and view components that fit the block.
They outline the definition for Go to, find, highlight and rename attribute
codes and include aliases together with every reference to them (including
those inside `record_schema` and status transition conditions), and link the
blueprint preview with the definition both ways: a field or view leads to its
declaration, and the cursor's attribute or view is shown in the preview.

The editors read JSON Schemas generated from the Rust parser types:
`contracts/blueprint-definition-v1.schema.json` and
`contracts/reusable-attribute-definition-v1.schema.json`. Field doc comments
become descriptions, and `x-attricat-*` extensions carry editor hints such as
the kind of value a field references. The schema describes structure and
allowed values only; cross-field rules, such as file policy keys requiring
`value_type: file`, remain enforced by the API. A test checks that the schema
accepts every definition in the repository that the parser accepts.

After changing a definition type, regenerate both contracts with
`just contracts`. `cargo test` fails while a committed contract is stale.
