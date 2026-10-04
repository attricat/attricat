---
title: Blueprint TOML reference
description: Every key accepted in a blueprint definition, with types, defaults, and validation rules.
---

This page lists every key the blueprint compiler accepts. For a walkthrough that builds a blueprint step by step, read [Author a blueprint](/builders/blueprints/).

The compiler is strict. An unknown key, a value of the wrong type, or a reference to something that does not exist rejects the whole definition with `422 invalid_blueprint_definition` and a message naming the problem.

## Codes

Blueprint codes, attribute codes, include aliases, relationship targets, rule codes, connector job codes, and role codes in `[publication]` must be **codes**: non-empty strings of ASCII letters, digits, `-`, and `_`. `product`, `seo-fields`, and `stock_on_hand` are valid. `product type` and `prodükt` are not.

## Translated labels

The blueprint `name`, attribute `name`, tab and accordion section `label`, table `columns[].label`, and `incoming_relationship_list` `label` can contain lexicon references: `{{Product}}`, or `{{Order|purchase}}` with a context. Text outside braces is literal, and `\{{` writes a literal `{{`. Malformed references are rejected when the blueprint is saved. See [Translate labels](/builders/translations/).

## Top level

```toml
format_version = 1
code = "product"
name = "Product"
kind = "entity"
entity_schema = '''{ "type": "object", "required": ["title"] }'''
```

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `format_version` | integer | Yes | Must be `1`. |
| `code` | code | Yes | Identifier of the blueprint family. It cannot change between revisions. |
| `name` | string | Yes | Display name. Can change between revisions. |
| `kind` | `"entity"` or `"mixin"` | Yes | An `entity` blueprint can have entities. A `mixin` only supplies attributes to other blueprints through `[[includes]]`. |
| `attributes` | array of tables | Yes | At least one attribute. See [Attributes](#attributes). |
| `includes` | array of tables | No | Mixins this blueprint pulls attributes from. See [Includes](#includes). |
| `views` | table | Entity: yes | Layouts for the web app. Entity blueprints must define `views.dropdown_option`. See [Views](#views). |
| `entity_schema` | string (JSON) | No | JSON Schema for the whole entity. Entity blueprints only. See [Validation](/builders/validation/). |
| `publication` | table | No | Publication reapproval policy. See [Publication](#publication). |
| `rules` | array of tables | No | Data-quality rules owned by this blueprint. See [Rules](/builders/rules/). |
| `unique_keys` | array of tables | No | Business keys whose values must be unique. Entity blueprints only. See [Unique keys](#unique-keys). |
| `connector_jobs` | array of tables | No | Scheduled or manual import and export jobs run by a connector extension. Entity blueprints only. See [Connector jobs](#connector-jobs). |
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
| `value_type` | string | | One of the [value types](#value-types). |
| `from` | string | | `"<include-alias>.<attribute-code>"`. Materializes an attribute from an included mixin. `code` must equal the mixin attribute's code. |
| `extension_type` | string | | `"<extension-id>:<type-id>@<semver-range>"`. Uses an attribute type declared by an enabled extension. See [Extension attribute types](#extension-attribute-types). |
| `context_fallback` | `"default"` or `"none"` | `"default"` | What a context with no value of its own shows. `default` walks up the context tree to the nearest ancestor with a value. `none` shows nothing. |
| `context_editable` | `"all"` or `"default"` | `"all"` | Where values can be written. `default` restricts writes to the default context: other contexts show the field read-only and the API rejects writes there. |
| `readonly` | boolean | `false` | Shows the field in the web app but prevents editing it there. The API, CLI, agents, workflows, and extensions can still write it. Use it for values owned by an integration. |
| `tags` | array of strings | `[]` | Free-form metadata. Must be unique and non-empty. Some tags hide the attribute in the web app; see [Visibility tags](#visibility-tags). |
| `default_value` | matches the type | Unset | Value stored in the default context when an entity is created without one. Scalar types only. |
| `value_schema` | string (JSON) | Unset | JSON Schema for one value. Scalar types only. See [Validation](/builders/validation/). |

### Value types

| `value_type` | Stored value | CLI/TOML example | Notes |
| --- | --- | --- | --- |
| `string` | Text | `value = "Blue shirt"` | Clearing a string in a non-default context removes the override instead of storing `""`. Can also be a [status](/builders/validation/#statuses) or a [user or team assignment](#user-or-team-assignments). |
| `number` | Decimal | `value = 19.99` | |
| `integer` | 64-bit integer | `value = 12` | |
| `boolean` | `true` or `false` | `value = true` | |
| `date` | Calendar date | `value = 2026-03-01` | |
| `datetime` | Timestamp with offset | `value = 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Wall-clock time and IANA time zone | `value = { time = "09:30:00", time_zone = "Europe/Warsaw" }` | Both parts are required. |
| `json` | Any JSON value | | Cannot be sorted or used in Explorer filters. Prefer typed attributes or relationships. |
| `relationship` | Links to other entities | | See [Relationship keys](#relationship-keys). |
| `file` | Uploaded files | | See [File keys](#file-keys). |

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
| `target_blueprint` | code | Any entity blueprint | Restricts targets to entities of this blueprint family. |
| `target_blueprints` | array of codes | Any entity blueprint | Restricts targets to entities of any of these blueprint families. Cannot be combined with `target_blueprint`; a one-item list is the same as `target_blueprint`. |
| `cardinality` | `"one"`, `"many"`, or `"one_to_one"` | `"many"` | How many targets one entity may link to in one context. `one_to_one` is shorthand for `cardinality = "one"` plus `target_cardinality = "one"` and cannot be combined with `target_cardinality`. |
| `target_cardinality` | `"one"` or `"many"` | `"many"` | How many entities may link to the same target through this attribute in one context. |
| `acyclic` | boolean | `false` | Rejects links that would form a cycle through this attribute. Requires `context_editable = "default"`, and the targets must include the blueprint itself. See [Hierarchies](#hierarchies). |
| `tree` | boolean | `false` | An acyclic hierarchy in which every entity has at most one target (its parent). Implies `acyclic = true` and defaults `cardinality` to `"one"`; `cardinality = "many"` is rejected. |

`cardinality = "one"` gives a single-select field whose options can be shared, such as a brand. Add `target_cardinality = "one"` only for an exclusive pairing, where each target can be claimed once. A write that breaks either limit returns `409 relationship_cardinality_conflict`.

A write that links an entity of a blueprint not allowed by `target_blueprint` or `target_blueprints` returns `422 relationship_target_type_mismatch`. The entity picker in the web app offers only the allowed blueprints; when there are several, the picker has a **Target blueprint** selector. An `incoming_relationship_list` on any of the allowed target blueprints can list the field.

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

- A write that would close a cycle returns `409 relationship_cycle`. `error.details.path` lists the entity IDs along the cycle, starting and ending with the entity being written. Linking an entity to itself is a cycle of length one.
- In a `tree`, giving an entity a second target returns `409 relationship_cardinality_conflict`.
- Checks see links of every revision of the blueprint and run inside the write's transaction. Two concurrent writes cannot each add half of a cycle.
- The hierarchy applies to the whole blueprint family as declared by its latest published revision, including entities still pinned to older revisions.
- Publishing a revision that adds `acyclic` or `tree` checks the existing links first. If they contain cycles, or a tree has entities with more than one target, publication fails with `409 relationship_hierarchy_violations`; `error.details` lists up to 20 cycles and entities with extra targets. Fix the links and publish again.

Relationships cannot have `value_schema` or `default_value`. Constrain them with `entity_schema` instead.

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
| `hidden:form` | Automatically generated create and edit forms |
| `hidden:detail` | Automatically generated entity detail views |
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
| `dropdown_option` | Label for the entity in relationship pickers, filter pills, and search results. Required on entity blueprints. | `dropdown_option` |
| `detail` | Read-only entity page. | A layout block |
| `edit` | Create and edit form. | A layout block |
| `table` | Explorer columns. | `table` |
| `extension_layout` | Order and visibility of extension contributions on this blueprint's entity pages. | `extension_layout` |

When `detail`, `edit`, or `table` is missing, the web app lists attributes in declaration order.

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
| `incoming_relationship_list` | `label`, `relationships`, `page_size` | A button that opens a paged list of entities linking to this one. `relationships` is an array of `{ source_blueprint, field }`. `page_size` is capped by the server's `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE`. |
| `heading` | `text` | Static heading. |
| `text` | `text` | Static paragraph. |
| `divider` | | Horizontal rule. |

Every block accepts an optional `component` reference. Every `field` and `relationship_list` must name an attribute the blueprint has, including attributes selected from mixins.

### Component references

```toml
component = { id = "catalog.field_edit", version = 1 }
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
| `catalog.entity_heading` | 1 | `stack` (detail) | | |
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

See [Field controls](/builders/views/#field-controls) for how these behave.

An extension cell renderer's ID and version must match a renderer declared by an enabled extension for the column's value type.

### `extension_layout`

```toml
[views.extension_layout]
type = "extension_layout"
version = 1

[views.extension_layout.outlets.entity_preview_panel]
order = ["acme.inventory:summary"]
hidden = ["acme.legacy:panel"]
```

| Key | Description |
| --- | --- |
| `version` | Must be `1`. |
| `outlets` | Keyed by `entity_preview_panel`, `entity_attribute_decoration`, or `entity_action`. |
| `outlets.<outlet>.order` | Contribution keys (`<extension-id>:<contribution-id>`) in display order. |
| `outlets.<outlet>.hidden` | Contribution keys to hide. |

An outlet you list here replaces the workspace default for that outlet on this blueprint's entities. Outlets you leave out keep the workspace layout. Keys for extensions that are not installed are kept, so a layout survives an extension being disabled and re-enabled.

## Publication

```toml
[publication]
retain_on_edit_roles = ["editor"]
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | array of role codes | `[]` | Workspace roles whose edits keep an entity's existing channel publications. Everyone else's edits withdraw them. The roles must exist when the revision is published. |

This does not grant any permission. See [Publishing](/guides/publishing/).

## Rules

`[[rules]]` tables use the rule syntax described in [Rules](/builders/rules/), without `format_version`. Rule codes must be unique within the blueprint, and `required` and `stale` predicates must name an attribute of the blueprint.

## Unique keys

A unique key declares a business identifier that two entities of the blueprint family cannot share, such as a part number, a document number, or a combination such as manufacturer and part number.

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
- Numbers compare by value (`1.50` equals `1.5`), date-times by instant, and relationships by the linked entity.
- An entity that has no value, or only blank text, for any of the key's attributes is not checked against that key. Make the attributes required in `entity_schema` if every entity must have the key.
- The key covers the whole blueprint family as declared by its latest published revision, including entities pinned to older revisions. Attributes are matched by code.

A write that would give a second entity the same key value returns `409 unique_key_conflict`. `error.details` names the `key`, the `context` code, the normalized `values`, and the `conflicting_entity_id` that already holds them. The check runs in the database inside the write's transaction, so when two people save the same value at the same moment, exactly one save succeeds.

Publishing a revision that adds or changes unique keys checks existing entities first. If some already share a value, publication fails with `409 unique_key_duplicates`, and `error.details.duplicates` lists up to 20 groups with the key, context, values, and entity IDs (`error.details.total` counts all groups). Change or delete the duplicates and publish again.

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

Jobs are checked against the enabled extension when the revision is published; an invalid job blocks publication. A job removed in a later revision is disabled and its run history is kept. An export job runs once per enabled publication channel and exports only entities published to that channel. See [Operations and connectors](/extensions/operations/).

## Limits and errors

A few compile-time rules that are easy to miss:

- `format_version` other than `1` is rejected.
- An entity blueprint without `views.dropdown_option` is rejected.
- `entity_schema` on a mixin is rejected.
- `entity_schema` may only name attributes the blueprint has in its top-level `required`, `properties`, `dependentRequired`, and `dependentSchemas`.
- `target_blueprint`, `target_blueprints`, `acyclic`, and `tree` on a non-relationship attribute are rejected.
- `unique_keys` on a mixin, a key naming an unknown, `json`, file, or many-target relationship attribute, or a key listing an attribute twice is rejected.
- `from` must be `alias.code` where `code` matches the attribute's own code.
