---
title: Blueprint TOML reference
description: Every key accepted in a blueprint definition, with types, defaults, and validation rules.
---

This page lists every key the blueprint compiler accepts. For a walkthrough that builds a blueprint step by step, read [Author a blueprint](/builders/blueprints/).

The compiler is strict. An unknown key, a value of the wrong type, or a reference to something that does not exist rejects the whole definition with `422 invalid_blueprint_definition` and a message naming the problem.

## Codes

Blueprint codes, attribute codes, include aliases, relationship targets, rule codes, connector job codes, and role codes in `[publication]` must be **codes**: non-empty strings of ASCII letters, digits, `-`, and `_`. `product`, `seo-fields`, and `stock_on_hand` are valid. `product type` and `prodükt` are not.

## Translated labels

The blueprint `name` and `description`, attribute `name` and `description`, tab and accordion section `label`, table `columns[].label`, and `incoming_relationship_list` `label` can contain lexicon references: `{{Product}}`, or `{{Order|purchase}}` with a context. Text outside braces is literal, and `\{{` writes a literal `{{`. Malformed references are rejected when the blueprint is saved. See [Translate labels](/builders/translations/).

## Top level

```toml
format_version = 1
code = "product"
name = "Product"
kind = "record"
record_schema = '''{ "type": "object", "required": ["title"] }'''
```

Record blueprints use `kind = "record"`.

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `format_version` | integer | Yes | Must be `1`. |
| `code` | code | Yes | Identifier of the blueprint family. It cannot change between revisions. |
| `name` | string | Yes | Display name. Can change between revisions. |
| `description` | string | No | What the blueprint's records are, up to 500 characters, such as "Product groupings, such as Basic tools". The agent reads it to match people's words to records; the web app does not show it yet. |
| `kind` | `"record"` or `"mixin"` | Yes | An `record` blueprint can have records. A `mixin` only supplies attributes to other blueprints through `[[includes]]`. |
| `attributes` | array of tables | Yes | At least one attribute. See [Attributes](#attributes). |
| `includes` | array of tables | No | Mixins this blueprint pulls attributes from. See [Includes](#includes). |
| `views` | table | For `record`: yes | Layouts for the web app. Record blueprints must define `views.dropdown_option`. See [Views](#views). |
| `record_schema` | string (JSON) | No | JSON Schema for the whole record, optionally with [`x-attricat-checks`](#record-checks). Record blueprints only. See [Validation](/builders/validation/). |
| `publication` | table | No | Publication reapproval policy. See [Publication](#publication). |
| `rules` | array of tables | No | Data-quality rules owned by this blueprint. See [Rules](/builders/rules/). |
| `unique_keys` | array of tables | No | Business keys whose values must be unique. Record blueprints only. See [Unique keys](#unique-keys). |
| `connector_jobs` | array of tables | No | Scheduled or manual import and export jobs run by a connector extension. Record blueprints only. See [Connector jobs](#connector-jobs). |
| `extensions` | table | No | Free-form data for extensions, namespaced as `[extensions.<extension-id>]`. The core compiler ignores it; extensions read it from the stored definition. |

## Attributes

Each `[[attributes]]` table declares exactly one of `value_type`, `from`, or `extension_type`.

```toml
[[attributes]]
code = "title"
value_type = "string"
```

### Common keys

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Unique within the blueprint, including attributes pulled in from mixins. |
| `name` | string | Humanized `code` | Human-readable label shown in forms, filters, previews, and as the default table column heading. Must not be blank. Not allowed with `from`; a selected attribute keeps the mixin attribute's name. |
| `description` | string | | What the attribute holds, up to 500 characters. Read by the agent; the web app does not show it yet. Not allowed with `from`. |
| `value_type` | string | | One of the [value types](#value-types). |
| `from` | string | | `"<include-alias>.<attribute-code>"`. Materializes an attribute from an included mixin. `code` must equal the mixin attribute's code. |
| `extension_type` | string | | `"<extension-id>:<type-id>@<semver-range>"`. Uses an attribute type declared by an enabled extension. See [Extension attribute types](#extension-attribute-types). |
| `context_fallback` | `"default"` or `"none"` | `"default"` | What a context with no value of its own shows. `default` walks up the context tree to the nearest ancestor with a value. `none` shows nothing. |
| `context_editable` | `"all"` or `"default"` | `"all"` | Where values can be written. `default` restricts writes to the default context: other contexts show the field read-only and the API rejects writes there. |
| `readonly` | boolean | `false` | Shows the field in the web app but prevents editing it there. The API, CLI, agents, workflows, and extensions can still write it. Use it for values owned by an integration. |
| `tags` | array of strings | `[]` | Free-form metadata. Must be unique and non-empty. Some tags hide the attribute in the web app; see [Visibility tags](#visibility-tags). |
| `default_value` | matches the type | Unset | Value stored in the default context when a record is created without one. Scalar types only. |
| `value_schema` | string (JSON) | Unset | JSON Schema for one value. Scalar types only. A `string` attribute's schema can make it a [status](#statuses) or a [user or team assignment](#user-or-team-assignments). See [Validation](/builders/validation/). |

### Value types

| `value_type` | Stored value | CLI/TOML example | Notes |
| --- | --- | --- | --- |
| `string` | Text | `value = "Blue shirt"` | Clearing a string in a non-default context removes the override instead of storing `""`. Can also be a [status](#statuses) or a [user or team assignment](#user-or-team-assignments). |
| `number` | Decimal | `value = 19.99` | |
| `integer` | 64-bit integer | `value = 12` | |
| `boolean` | `true` or `false` | `value = true` | |
| `date` | Calendar date | `value = 2026-03-01` | |
| `datetime` | Timestamp with offset | `value = 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Wall-clock time and IANA time zone | `value = { time = "09:30:00", time_zone = "Europe/Warsaw" }` | Both parts are required. |
| `json` | Any JSON value | | Cannot be sorted or used in Explorer filters. Prefer typed attributes or relationships. |
| `relationship` | Links to other records | | See [Relationship keys](#relationship-keys). |
| `file` | Uploaded files | | See [File keys](#file-keys). |

### Statuses

A `string` attribute whose `value_schema` has an `enum` and an `x-attricat-status` annotation is a status. See [Statuses](/builders/validation/#statuses) for how they behave.

```toml
[[attributes]]
code = "status"
value_type = "string"
value_schema = '''{
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
        "roles": ["reviewer"], "separate_from": ["submit"] },
      { "from": "released", "to": "draft", "code": "correct", "permission": "records.publish" }
    ]
  }
}'''
```

| Key | Value | Description |
| --- | --- | --- |
| `version` | `1` | Required. |
| `options` | array of 1 to 100 tables | Required. One option per `enum` code, in display order. |
| `transitions` | array of up to 10,000 tables | Allowed changes. Omit it to allow any change; with it, only the listed changes are allowed, and an empty array allows none. |

Each option:

| Key | Value | Description |
| --- | --- | --- |
| `code` | code, up to 128 characters | Required. Must match exactly one `enum` value. |
| `label` | string, 1 to 200 characters | Required. Can contain [lexicon references](/builders/translations/#status-labels). |
| `tone` | `default`, `success`, `warning`, `error`, or `info` | Color of the status chip. |
| `lock` | `"all"`, or 1 to 500 attribute codes | Makes the listed attributes read-only while the record has this status. `"all"` covers every attribute, relationship, and file except the status itself. A reusable attribute is written `namespace:code`. A record with a locking status cannot be deleted. Requires `transitions`. |
| `approval` | table with `covers` and `void_to` | Records an approval when the record enters this status. `covers` is `"all"` or 1 to 500 attribute codes; `void_to` is another option the record moves to when covered content changes. |
| `retention_days` | integer, 1 to 36,600 | Places a retention hold on the locked attributes' files when the record enters this status. Requires `lock`. |

Each transition:

| Key | Value | Description |
| --- | --- | --- |
| `from` | option code or `null` | Required. `null` means no value, so an edge from `null` allows setting the first value, including defaults. |
| `to` | option code or `null` | Required. An edge to `null` allows clearing the value. |
| `code` | code, up to 128 characters | Names the transition in `separate_from` and in history. |
| `permission` | permission code | The person saving must hold this [permission](/reference/permissions/) for the record. |
| `roles` | 1 to 20 role codes | The person saving must hold at least one of these roles for the workspace, the blueprint, or the record. |
| `separate_from` | 1 to 20 transition codes | The person saving must not be the one who most recently made one of these transitions on this record and context. |
| `conditions` | up to 16 checks | Requirements on the data. See [Transition conditions](#transition-conditions). |

Each `from`/`to` pair may appear only once. `permission`, `roles`, and `separate_from` always refer to the person saving; they cannot refer to a [user or team assignment](#user-or-team-assignments) on the record. Refusals return `403 status_transition_forbidden` or `403 status_separation_of_duties`, and changes to locked content return `409 record_locked`. See [Control a record's lifecycle](/builders/validation/#control-a-records-lifecycle).

### User or team assignments

A `string` attribute whose `value_schema` has an `x-attricat-principal` annotation stores a reference to a workspace user or team:

```toml
[[attributes]]
code = "assignee"
value_type = "string"
value_schema = '''{"type": "string", "x-attricat-principal": {"version": 1, "kinds": ["user", "team"]}}'''
```

| Key | Value | Description |
| --- | --- | --- |
| `version` | `1` | Required. |
| `kinds` | `["user"]`, `["team"]`, or both | Required. What the attribute accepts. |

- Values are `user:<id>` or `team:<id>`, with the lowercase ID from `acli directory` or `GET /directory`. A missing value means unassigned.
- The schema must have `"type": "string"` and no `enum`, `const`, `pattern`, or `format`. The attribute cannot have a `default_value` or also be a status. Reusable attributes accept the same annotation.
- A new or changed value must be an active workspace member or a team that has not been deleted, of an accepted kind. Otherwise saving returns `422 attribute_value_schema_mismatch`. Unchanged values are not checked again.
- Search filters match the stored value exactly. The value `@me` with operator `eq` matches the caller and every team the caller belongs to.
- [Predicates](#predicates) treat the value as text: `required` checks that a record is assigned, and `compare` with `eq` or `one_of` can match a specific `user:<id>` or `team:<id>`. No predicate and no [transition requirement](#statuses) can refer to the person saving, so a rule such as "only the assignee may close this" cannot be declared.

### Relationship keys

```toml
[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `target_blueprint` | code | Any record blueprint | Restricts targets to records of this blueprint family. |
| `target_blueprints` | array of codes | Any record blueprint | Restricts targets to records of any of these blueprint families. Cannot be combined with `target_blueprint`; a one-item list is the same as `target_blueprint`. |
| `cardinality` | `"one"`, `"many"`, or `"one_to_one"` | `"many"` | How many targets one record may link to in one context. `one_to_one` is shorthand for `cardinality = "one"` plus `target_cardinality = "one"` and cannot be combined with `target_cardinality`. |
| `target_cardinality` | `"one"` or `"many"` | `"many"` | How many records may link to the same target through this attribute in one context. |
| `acyclic` | boolean | `false` | Rejects links that would form a cycle through this attribute. Requires `context_editable = "default"`, and the targets must include the blueprint itself. See [Hierarchies](#hierarchies). |
| `tree` | boolean | `false` | An acyclic hierarchy in which every record has at most one target (its parent). Implies `acyclic = true` and defaults `cardinality` to `"one"`; `cardinality = "many"` is rejected. |

`cardinality = "one"` gives a single-select field whose options can be shared, such as a brand. Add `target_cardinality = "one"` only for an exclusive pairing, where each target can be claimed once. A write that breaks either limit returns `409 relationship_cardinality_conflict`.

A write that links a record of a blueprint not allowed by `target_blueprint` or `target_blueprints` returns `422 relationship_target_type_mismatch`. The record picker in the web app offers only the allowed blueprints; when there are several, the picker has a **Target blueprint** selector. An `incoming_relationship_list` on any of the allowed target blueprints can list the field.

#### Hierarchies

```toml
[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "location"
tree = true
context_editable = "default"
```

`acyclic` and `tree` protect self-referencing structures such as location or asset hierarchies, and predecessor chains such as revision → previous revision.

- A write that would close a cycle returns `409 relationship_cycle`. `error.details.path` lists the record IDs along the cycle, starting and ending with the record being written. Linking a record to itself is a cycle of length one.
- In a `tree`, giving a record a second target returns `409 relationship_cardinality_conflict`.
- Checks see links of every revision of the blueprint and run inside the write's transaction. Two concurrent writes cannot each add half of a cycle.
- The hierarchy applies to the whole blueprint family as declared by its latest published revision, including records still pinned to older revisions.
- Publishing a revision that adds `acyclic` or `tree` checks the existing links first. If they contain cycles, or a tree has records with more than one target, publication fails with `409 relationship_hierarchy_violations`; `error.details` lists up to 20 cycles and records with extra targets. Fix the links and publish again.

Relationships cannot have `value_schema` or `default_value`. Constrain them with `record_schema` instead.

### File keys

```toml
[[attributes]]
code = "gallery"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "png", "webp"]
max_bytes = 10485760
image_only = true
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `cardinality` | `"one"` or `"many"` | `"one"` | Single file or a list of files. |
| `ordered` | boolean | `true` for `many` | Whether the order of a `many` list is meaningful. Must be `false` or omitted for `one`. |
| `allowed_mime_groups` | array of strings | Any | Accepted MIME groups, such as `image`. |
| `allowed_extensions` | array of strings | Any | Accepted extensions, letters and digits only (`jpg`, not `*.jpg`). A leading `.` is ignored. |
| `max_bytes` | positive integer | Server limit | Per-file size limit. Cannot exceed the server's `FILE_UPLOAD_MAX_BYTES`. |
| `purposes` | array of codes | `[]` | Labels describing what the files are for, such as `product_image`. |
| `image_only` | boolean | `false` | Accept images only. Required for the `catalog.table_image` renderer. |

These keys are rejected on any attribute that is not `value_type = "file"`. File attributes cannot have `value_schema`, `default_value`, or `target_blueprint`.

Uploaded images get two generated variants, `thumbnail` and `display`, both WebP. Other files are stored as uploaded.

### Visibility tags

Some tags change where the web app shows an attribute when it builds a layout automatically. A view that names the attribute explicitly still shows it. These tags are not access control: the API, CLI, agents, and extensions still return the values.

| Tag | Hides the attribute from |
| --- | --- |
| `hidden` | Every surface below |
| `hidden:form` | Editors added outside the layout, such as **Other attributes**, and the automatically generated create form |
| `hidden:detail` | Automatically generated record detail views |
| `hidden:explorer` | Explorer filter and facet choices |
| `hidden:metadata` | The Attributes table on the blueprint page |

### Extension attribute types

An enabled extension can declare attribute types, for example a money type with a currency setting:

```toml
[[attributes]]
code = "price"
extension_type = "com.acme.commerce:money@^1"
extension_configuration = '{"currency":"USD"}'
```

When the blueprint is saved, Attricat finds the enabled extension `com.acme.commerce`, picks the highest declared `money` version matching `^1`, validates `extension_configuration` against the type's configuration schema, and records the resolved type on the revision. The value is stored with the type's underlying primitive (`string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`, or `json`) and checked against the type's value schema. Values stay readable if the extension is later disabled.

`extension_type` cannot be combined with `value_schema`, relationship keys, or file keys.

## Includes

Mixins share attribute definitions between blueprints. An include pins one exact published mixin revision. Each attribute you want must then be selected with `from`.

```toml
[[includes]]
alias = "seo"
code = "seo"
version = 2

[[attributes]]
code = "meta_title"
from = "seo.meta_title"
```

| Key | Type | Description |
| --- | --- | --- |
| `alias` | code | Local name used in `from`. Unique within the blueprint. |
| `code` | code | Code of the mixin blueprint. |
| `version` | positive integer | Exact published revision of the mixin. |

A selected attribute keeps all of the mixin's settings. To pick up changes in the mixin, publish a new mixin revision and point `version` at it in a new revision of the consuming blueprint.

## Views

`views` is a table keyed by view name. The web app uses these names:

| View | Purpose | Allowed `type` |
| --- | --- | --- |
| `dropdown_option` | Label for the record in relationship pickers, filter pills, and search results. Required on record blueprints. | `dropdown_option` |
| `detail` | Record page and create form. Fields the user may change are edited in place. | A layout block |
| `edit` | Deprecated and ignored by the web app. Still accepted, and edit components in it are still validated. | A layout block |
| `table` | Explorer columns. | `table` |
| `extension_layout` | Order and visibility of extension contributions on this blueprint's record pages. | `extension_layout` |

When `detail` or `table` is missing, the web app lists attributes in declaration order.

### `dropdown_option`

```toml
[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "sku"]
separator = " / "
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `fields` | array of attribute codes | Required | One or more unique attributes joined into the label. |
| `separator` | string | `" · "` | Text placed between fields. |

Labels are resolved per context and follow each attribute's `context_fallback`.

### `table`

```toml
[views.table]
type = "table"

[[views.table.columns]]
field = "title"
label = "Product"

[[views.table.columns]]
field = "category.name"
label = "Category"

[[views.table.columns]]
field = "main_photo"
label = "Image"
renderer = { id = "catalog.table_image", version = 1 }
```

| Key | Type | Description |
| --- | --- | --- |
| `columns` | array of tables | Column definitions. Must be non-empty with unique `field` values. |
| `columns[].field` | string | A local scalar attribute, a local file attribute with a compatible renderer, or a path of up to three relationship hops ending in a scalar attribute, such as `family.product_type.name`. |
| `columns[].label` | string | Column header. Defaults to the attribute name. |
| `columns[].renderer` | component reference | `catalog.table_image@1`, a string display component (`catalog.color_display@1`, `catalog.email_display@1`, `catalog.url_display@1`, or `catalog.phone_display@1`), or an extension cell renderer. |
| `fields` | array of attribute codes | Older shorthand for local scalar columns. Cannot be combined with `columns`. |
| `component` | component reference | Optional table component. |

A column can be sorted in the Explorer only when it is scalar and every relationship hop in its path has `cardinality = "one"`.

### Layout blocks

`detail` and `edit` start with a container block and nest other blocks inside it. Blocks are TOML tables with a `type` key. You can write them as nested arrays of tables or as inline tables; both produce the same result.

| `type` | Keys | Description |
| --- | --- | --- |
| `stack` | `children` | Children stacked vertically. |
| `grid` | `children` | Children in a responsive grid. |
| `section` | `children` | A grouped region. |
| `tabs` | `tabs` = array of `{ label, children }` | One tab per entry. |
| `accordion` | `sections` = array of `{ label, children }` | Collapsible sections. |
| `field` | `field` | One scalar or file attribute. |
| `relationship_list` | `field` | One relationship attribute and its targets. |
| `incoming_relationship_list` | `label`, `relationships`, `page_size` | A button that opens a paged list of records linking to this one. `relationships` is an array of `{ source_blueprint, field }`. `page_size` is capped by the server's `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE`. |
| `heading` | `text` | Static heading. |
| `text` | `text` | Static paragraph. |
| `divider` | | Horizontal rule. |

Every block accepts an optional `component` reference. Every `field` and `relationship_list` must name an attribute the blueprint has, including attributes selected from mixins.

### Component references

```toml
component = { id = "catalog.url_display", version = 1 }
renderer = { id = "example.currency", version = 1, props = { currency = "USD" } }
```

| Key | Type | Description |
| --- | --- | --- |
| `id` | string | Registered component ID. |
| `version` | integer | Component version. |
| `props` | table | Component options. Only props the component declares are accepted. |

Built-in components:

| ID | Version | Used on | Value types | Props |
| --- | --- | --- | --- | --- |
| `catalog.field_display` | 1 | `field` (detail) | Scalars | |
| `catalog.field_edit` | 1 | `field` (edit) | Scalars | |
| `catalog.relationship_list_display` | 1 | `relationship_list` (detail) | `relationship` | |
| `catalog.relationship_list_edit` | 1 | `relationship_list` (edit) | `relationship` | |
| `catalog.relationship_hierarchy` | 1 | `relationship_list` (detail) | `relationship` | `parent_field` |
| `catalog.incoming_relationship_list_display` | 1 | `incoming_relationship_list` | | |
| `catalog.record_heading` | 1 | `stack` (detail) | | |
| `catalog.table_display` | 1 | `table` | Scalars | |
| `catalog.table_edit` | 1 | `table` | Scalars | |
| `catalog.table_image` | 1 | table column `renderer` | `file` with `cardinality = "one"` and `image_only = true` | |
| `catalog.color_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `catalog.color_edit` | 1 | `field` (edit) | `string` | |
| `catalog.email_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `catalog.email_edit` | 1 | `field` (edit) | `string` | |
| `catalog.url_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `catalog.url_edit` | 1 | `field` (edit) | `string` | |
| `catalog.phone_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `catalog.phone_edit` | 1 | `field` (edit) | `string` | |
| `catalog.markdown_display` | 1 | `field` (detail) | `string` | |
| `catalog.markdown_edit` | 1 | `field` (edit) | `string` | |

Components marked *(edit)* are accepted only in the deprecated `edit` view. Where a `detail` field is editable, the web app uses the display component's paired edit component, or the standard editor for the value type. See [Field controls](/builders/views/#field-controls) for how these behave.

An extension cell renderer's ID and version must match a renderer declared by an enabled extension for the column's value type.

### `extension_layout`

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.record_preview_panel]
order = ["acme.inventory:summary"]
hidden = ["acme.legacy:panel"]
```

| Key | Description |
| --- | --- |
| `version` | Must be `1`. |
| `outlets` | Keyed by `record_preview_panel`, `record_attribute_decoration`, or `record_action`. |
| `outlets.<outlet>.order` | Contribution keys (`<extension-id>:<contribution-id>`) in display order. |
| `outlets.<outlet>.hidden` | Contribution keys to hide. |

An outlet you list here replaces the workspace default for that outlet on this blueprint's records. Outlets you leave out keep the workspace layout. Keys for extensions that are not installed are kept, so a layout survives an extension being disabled and re-enabled.

## Publication

```toml
[publication]
retain_on_edit_roles = ["editor"]
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | array of role codes | `[]` | Workspace roles whose edits keep a record's existing channel publications. Everyone else's edits withdraw them. The roles must exist when the revision is published. |

This does not grant any permission. See [Publishing](/guides/publishing/).

## Rules

`[[rules]]` tables use the rule syntax described in [Rules](/builders/rules/), without `format_version`. Rule codes must be unique within the blueprint, and their [predicates](#predicates) are type-checked against the blueprint's attributes.

```toml
[[rules]]
code = "released-documents-approved"
name = "Released documents have an approver"
severity = "error"
triggers = [{ type = "event", event_type = "record.updated.v1" }]
predicate = { type = "required", attribute_code = "approved_by" }

[rules.enforcement]
on_save = false

[[rules.enforcement.transitions]]
attribute_code = "status"
from = "review"
to = "released"
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enforcement.on_save` | boolean | `false` | Reject every write that leaves the record violating the rule. |
| `enforcement.transitions` | array of tables | `[]` | Up to 16 guarded status changes, each with `attribute_code`, optional `from`, and `to`. Without `from`, every change into `to` is guarded. |

An `enforcement` table needs `on_save = true` or at least one transition, `severity` `error` or `critical`, and a predicate without `stale`, `unique`, or `acyclic`. Each transition's `attribute_code` must be a status attribute, and `from` and `to` must be codes of its `enum`. Violations return `422 rule_violation`. See [Enforce a rule](/builders/rules/#enforce-a-rule).

## Predicates

Rules, record checks, transition conditions, and publication channel checks share one predicate language. A predicate is a table (in TOML) or object (in JSON) tagged by `type`. It **holds** when the data is acceptable. Unknown keys are rejected.

| `type` | Keys | Holds when |
| --- | --- | --- |
| `required` | `attribute_code` | The attribute has a value. A relationship needs at least one target. |
| `stale` | `attribute_code`, `max_age_seconds` (1 to 31536000) | The value changed within `max_age_seconds`. Reporting rules only. |
| `has_tag` | `tag` | The record has the system tag. |
| `missing_tag` | `tag` | The record does not have the system tag. |
| `compare` | `attribute_code`, `op`, and exactly one of `other_attribute_code`, `subject_attribute_code`, or `value` | The comparison is true. |
| `one_of` | `attribute_code`, `values` (1 to 100) | The value is one of `values`. Not for relationships or files. |
| `relative_date` | `attribute_code`, `op` (`lt`, `lte`, `gt`, `gte`), `offset_days` (-36500 to 36500, default `0`) | The date or datetime compares true with the current time plus `offset_days`. |
| `unique` | `attribute_codes` (1 to 4 string, number, integer, boolean, date, or datetime attributes) | No other live record of the blueprint family, on any revision, has the same values in the same context. Reporting rules only. |
| `linked` | `relationship_code`, `quantifier` (`all`, `any`, `none`; default `all`), `predicate` | `all`: every linked record satisfies `predicate` (true with no links). `any`: at least one does. `none`: none does. |
| `referenced_by` | `blueprint_code`, `relationship_code`, optional `predicate`, `min` and/or `max` (0 to 1000) | The number of `blueprint_code` records whose `relationship_code` targets this record, and that satisfy `predicate`, is between `min` and `max`. `max = 0` means "none". |
| `acyclic` | `relationship_code` | Following the relationship never returns to the record. Reporting rules only. |
| `all_of` | `predicates` (1 to 16) | Every nested predicate holds. |
| `any_of` | `predicates` (1 to 16) | At least one nested predicate holds. |

`compare` operators:

| `op` | Applies to |
| --- | --- |
| `eq`, `ne` | Any type except files. Relationships are equal when they have the same set of targets. |
| `lt`, `lte`, `gt`, `gte` | `number`, `integer`, `date`, `datetime`. |
| `disjoint` | Two relationships with no target in common. |

- Both sides must have compatible types: a number with a number or integer, a date with a date. A literal `value` must match the attribute's type; write dates and datetimes as strings, such as `"2026-01-31"`. A relationship can only be compared with another relationship.
- A `compare` with a missing operand holds, and so do `one_of` and `relative_date` on an empty attribute. Combine them with `required` when a value must exist.
- Inside `linked` and `referenced_by`, `attribute_code` and `other_attribute_code` refer to the other record, and `subject_attribute_code` refers to the record being checked. `subject_attribute_code` is rejected elsewhere.
- `linked` and `referenced_by` follow one hop: their nested predicate cannot use `linked`, `referenced_by`, `unique`, `acyclic`, or `stale`.
- A predicate nests at most 4 levels deep and has at most 32 parts.
- At run time, `linked` fails above 200 linked records per relationship, `referenced_by` above 1,000 referring records, and `acyclic` when it cannot finish within 1,000 records.

`stale`, `unique`, and `acyclic` are reporting-only because they cannot be checked within a single save. They are rejected in enforcing rules, record checks, and transition conditions.

### Record checks

`x-attricat-checks` is an array inside `record_schema`:

```toml
record_schema = '''
{
  "type": "object",
  "x-attricat-checks": [
    { "code": "valid-range", "message": "Valid until must not be before valid from",
      "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } }
  ]
}
'''
```

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `code` | code | Yes | Unique within the array. |
| `message` | string | No | 1 to 500 characters. Replaces the generated message. |
| `predicate` | object | Yes | A [predicate](#predicates), except `stale`, `unique`, and `acyclic`. |

At most 32 checks. Failures return `422 record_check_failed`. See [Validation](/builders/validation/#compare-attributes-with-checks).

### Transition conditions

An edge in an `x-attricat-status` annotation's `transitions` can carry `conditions`, an array of up to 16 checks with the same keys as [record checks](#record-checks):

```json
{ "from": "review", "to": "released", "conditions": [
  { "code": "approver-set", "message": "Set an approver before release",
    "predicate": { "type": "required", "attribute_code": "approved_by" } }
] }
```

Each `from`/`to` pair may appear only once. Unmet conditions return `422 transition_conditions_unmet`. See [Conditions on transitions](/builders/validation/#conditions-on-transitions).

## Unique keys

A unique key declares a business identifier that two records of the blueprint family cannot share, such as a part number, a document number, or a combination such as manufacturer and part number.

```toml
[[unique_keys]]
code = "manufacturer_part"
attributes = ["manufacturer", "part_number"]

[[unique_keys]]
code = "slug"
attributes = ["slug"]
scope = "context"
case_sensitive = true
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Unique within the blueprint. Reported in errors. |
| `attributes` | array of attribute codes | Required | One to eight attributes whose combined values must be unique. Each must be a scalar other than `json`, or a relationship with `cardinality = "one"`. |
| `scope` | `"workspace"` or `"context"` | `"workspace"` | `workspace` compares values in the default context. `context` compares the values each context shows, including inherited ones, separately in every context. |
| `case_sensitive` | boolean | `false` | Compare text exactly instead of case-insensitively. |

How values are compared:

- Text is trimmed and every run of whitespace becomes one space. Unless `case_sensitive = true`, text is also compared in lowercase, so `ABC-1  Rev` and ` abc-1 rev` are the same key.
- Numbers compare by value (`1.50` equals `1.5`), date-times by instant, and relationships by the linked record.
- A record that has no value, or only blank text, for any of the key's attributes is not checked against that key. Make the attributes required in `record_schema` if every record must have the key.
- The key covers the whole blueprint family as declared by its latest published revision, including records pinned to older revisions. Attributes are matched by code.

A write that would give a second record the same key value returns `409 unique_key_conflict`. `error.details` names the `key`, the `context` code, the normalized `values`, and the `conflicting_record_id` that already holds them. The check runs in the database inside the write's transaction, so when two people save the same value at the same moment, exactly one save succeeds.

Publishing a revision that adds or changes unique keys checks existing records first. If some already share a value, publication fails with `409 unique_key_duplicates`, and `error.details.duplicates` lists up to 20 groups with the key, context, values, and record IDs (`error.details.total` counts all groups). Change or delete the duplicates and publish again.

## Connector jobs

Connector jobs run an operation of an installed connector extension, such as a CSV import or export, against this blueprint.

```toml
[[connector_jobs]]
code = "csv_export"
direction = "export"
extension_id = "attricat-connector-csv"
operation_id = "export"
interval_seconds = 3600
input = { profile = { version = 1, columns = [{ header = "ID", attribute = "external_id", kind = "string" }] } }
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Stable identifier. Jobs are matched by code across revisions. |
| `direction` | `"import"` or `"export"` | Required | |
| `extension_id` | string | Required | Connector extension ID. |
| `operation_id` | string | Required | Operation declared by the extension. |
| `input` | table | Required | Operation input, validated against the operation's schema. |
| `context` | context code | | Required for imports, rejected for exports. Imported values are written to this context. |
| `input_file_id` | UUID | | Imports only. A ready workspace file to import. |
| `interval_seconds` | integer | Manual only | Run every N seconds, 60 to 2592000. |
| `enabled` | boolean | `true` | Set `false` to pause the job. |

Jobs are checked against the enabled extension when the revision is published; an invalid job blocks publication. A job removed in a later revision is disabled and its run history is kept. An export job runs once per enabled publication channel and exports only records published to that channel. See [Operations and connectors](/extensions/operations/).

## Limits and errors

A few compile-time rules that are easy to miss:

- `format_version` other than `1` is rejected.
- A record blueprint without `views.dropdown_option` is rejected.
- `record_schema` on a mixin is rejected.
- `record_schema` may only name attributes the blueprint has in its top-level `required`, `properties`, `dependentRequired`, and `dependentSchemas`.
- Predicates in `x-attricat-checks`, transition `conditions`, and `[[rules]]` must name attributes the blueprint has, with types that suit the predicate. An ordering comparison on a string, or a comparison of a date with a number, is rejected.
- `target_blueprint`, `target_blueprints`, `acyclic`, and `tree` on a non-relationship attribute are rejected.
- `unique_keys` on a mixin, a key naming an unknown, `json`, file, or many-target relationship attribute, or a key listing an attribute twice is rejected.
- `from` must be `alias.code` where `code` matches the attribute's own code.
