---
title: Blueprint YAML reference
description: Every key accepted in a blueprint definition, with types, defaults, and validation rules.
---

This page lists every key the blueprint compiler accepts. For a walkthrough that builds a blueprint step by step, read [Author a blueprint](/builders/blueprints/).

The compiler is strict. An unknown key, a value of the wrong type, or a reference to something that does not exist rejects the whole definition with `422 invalid_blueprint_definition` and a message naming the problem.

## YAML syntax

YAML is indentation-sensitive: indent with spaces, never tabs, and keep the items of a list at the same depth. Quote a string that contains `: ` or ` #`, or starts with `{`, `[`, `*`, `&`, `!`, `|`, `>`, `'`, `"`, `%`, or `@`. That includes every label that starts with a lexicon reference: write `name: '{{Product}}'`, not `name: {{Product}}`. Only `true` and `false` are booleans; `yes`, `no`, `on`, and `off` are strings.

## Codes

Blueprint codes, attribute codes, include aliases, relationship targets, connector job codes, and role codes in `publication` must be **codes**: non-empty strings of ASCII letters, digits, `-`, and `_`. `product`, `seo-fields`, and `stock_on_hand` are valid. `product type` and `prodükt` are not.

## Translated labels

The blueprint `name` and `description`, attribute `name` and `description`, tab and accordion section `label`, table `columns[].label`, and `incoming_relationship_list` `label` can contain lexicon references: `{{Product}}`, or `{{Order|purchase}}` with a context. Text outside braces is literal, and `\{{` writes a literal `{{`. Malformed references are rejected when the blueprint is saved. See [Translate labels](/builders/translations/).

## Top level

```yaml
format_version: 1
code: product
name: Product
kind: record
record_schema: '{ "type": "object", "required": ["title"] }'
```

Record blueprints use `kind: record`.

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `format_version` | integer | Yes | Must be `1`. |
| `code` | code | Yes | Identifier of the blueprint family. It cannot change between revisions. |
| `name` | string | Yes | Display name. Can change between revisions. |
| `description` | string | No | What the blueprint's records are, up to 500 characters, such as "Product groupings, such as Basic tools". The agent reads it to match people's words to records; the web app does not show it yet. |
| `kind` | `"record"` or `"mixin"` | Yes | A `record` blueprint can have records. A `mixin` only supplies attributes to other blueprints through `includes`. |
| `attributes` | list of mappings | Yes | At least one attribute. See [Attributes](#attributes). |
| `includes` | list of mappings | No | Mixins this blueprint pulls attributes from. See [Includes](#includes). |
| `views` | mapping | For `record`: yes | Layouts for the web app. Record blueprints must define `views.dropdown_option`. See [Views](#views). |
| `record_schema` | string (JSON) | No | JSON Schema for the whole record, optionally with [`x-attricat-checks`](#record-checks). Record blueprints only. See [Validation](/builders/validation/). |
| `publication` | mapping | No | Publication reapproval policy. See [Publication](#publication). |
| `unique_keys` | list of mappings | No | Business keys whose values must be unique. Record blueprints only. See [Unique keys](#unique-keys). |
| `connector_jobs` | list of mappings | No | Scheduled or manual import and export jobs run by a connector extension. Record blueprints only. See [Connector jobs](#connector-jobs). |
| `movements` | list of mappings | No | How creating a record changes a balance on another record, such as a goods issue lowering a stock level. Up to 4. Record blueprints only. See [Balances and movements](#balances-and-movements). |
| `extensions` | mapping | No | Free-form data for extensions, namespaced under `extensions.<extension-id>`. The core compiler ignores it; extensions read it from the stored definition. |

## Attributes

Each entry in `attributes` declares exactly one of `value_type`, `from`, or `extension_type`.

```yaml
attributes:
  - code: title
    value_type: string
```

### Common keys

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Unique within the blueprint, including attributes pulled in from mixins. |
| `name` | string | Humanized `code` | Human-readable label shown in forms, filters, previews, and as the default table column heading. Must not be blank. Not allowed with `from`; a selected attribute keeps the mixin attribute's name. |
| `description` | string | | What the attribute holds, up to 500 characters. Record forms show it below the field, and the agent reads it. Not allowed with `from`. |
| `value_type` | string | | One of the [value types](#value-types). |
| `from` | string | | `"<include-alias>.<attribute-code>"`. Materializes an attribute from an included mixin. `code` must equal the mixin attribute's code. |
| `extension_type` | string | | `"<extension-id>:<type-id>@<semver-range>"`. Uses an attribute type declared by an enabled extension. See [Extension attribute types](#extension-attribute-types). |
| `context_fallback` | `"default"` or `"none"` | `"default"` | What a context with no value of its own shows. `default` walks up the context tree to the nearest ancestor with a value. `none` shows nothing. |
| `context_editable` | `"all"` or `"default"` | `"all"` | Where values can be written. `default` restricts writes to the default context: other contexts show the field read-only and the API rejects writes there. |
| `readonly` | boolean | `false` | Shows the field in the web app but prevents editing it there. The API, CLI, agents, workflows, and extensions can still write it. Use it for values owned by an integration. |
| `tags` | array of strings | `[]` | Free-form metadata. Must be unique and non-empty. Some tags hide the attribute in the web app; see [Visibility tags](#visibility-tags). |
| `default_value` | matches the type | Unset | Value stored in the default context when a record is created without one. Scalar types only. |
| `default_from` | string | Unset | A relationship path, such as `item.sale_price_net`, whose value is copied into the empty attribute when the path's first relationship gets a target. Scalar attributes of record blueprints only; not with `default_value`. See [Values from related records](#values-from-related-records). |
| `value_schema` | string (JSON) | Unset | JSON Schema for one value. Scalar types only. A `string` attribute's schema can make it a [status](#statuses) or a [user or team assignment](#user-or-team-assignments). See [Validation](/builders/validation/). |
| `generated` | mapping | Unset | Lets the server number new records, such as invoice or order numbers. `string` and `integer` attributes of record blueprints only. See [Generated values](#generated-values). |
| `balance` | mapping | Unset | A running total that only [movements](#balances-and-movements) change, such as a stock level. `number` and `integer` attributes of record blueprints only. |

### Values from related records

`default_from` copies a value from the record a relationship links to, such as an order line's price from the chosen product:

```yaml
attributes:
  - code: item
    value_type: relationship
    target_blueprints: [product, service]
    cardinality: one
  - code: unit_price_net
    value_type: number
    default_from: item.sale_price_net
```

- The path has one to three relationships and an attribute, like a table column. Every relationship needs `cardinality: one`. The last attribute must have the same value type; an `integer` can fill a `number`.
- The value is copied when a record is created with the relationship, or when a later save links it to another record, and only while the attribute is empty in that context and the save does not set or clear it. Imports, workflows, extensions and solution-pack sample records follow the same rule.
- It is a snapshot: a later change to the product's price does not change existing lines, and a value you enter always wins.
- The value is read in the relationship's context, with the source attribute's context fallback, and only from records the person saving can read. When there is nothing to copy, such as a service without a price, the attribute stays empty and the save still succeeds.
- The record history marks a copied value as filled in from the related record.
- In the web app, choosing a record fills these fields at once, in the record form and in a new row of a table of linked records such as an invoice's lines, and names the record the value came from. A new line also gets the values it copies from the invoice as soon as the row opens. A field you have typed in is never changed.

`default_from` cannot be combined with `default_value`, `generated`, `balance`, a status or an assignment.

### Value types

| `value_type` | Stored value | CLI/YAML example | Notes |
| --- | --- | --- | --- |
| `string` | Text | `value: Blue shirt` | Clearing a string in a non-default context removes the override instead of storing `""`. Can also be a [status](#statuses) or a [user or team assignment](#user-or-team-assignments). |
| `number` | Decimal | `value: 19.99` | |
| `integer` | 64-bit integer | `value: 12` | |
| `boolean` | `true` or `false` | `value: true` | |
| `date` | Calendar date | `value: 2026-03-01` | |
| `datetime` | Timestamp with offset | `value: 2026-03-01T09:30:00Z` | RFC 3339. |
| `time` | Wall-clock time and IANA time zone | `value: {time: '09:30:00', time_zone: Europe/Warsaw}` | Both parts are required. |
| `json` | Any JSON value | | Cannot be sorted or used in Explorer filters. Prefer typed attributes or relationships. |
| `relationship` | Links to other records | | See [Relationship keys](#relationship-keys). |
| `file` | Uploaded files | | See [File keys](#file-keys). |

### Pattern hints

An optional `x-attricat-pattern-hint` string describes the schema's `pattern` in words for record forms. It requires a `pattern` and is 1 to 200 characters. See [What forms show](/builders/validation/#what-forms-show).

### Choice lists

A `string` attribute whose `value_schema` has an `enum` is edited by choosing one value: radio buttons for up to five values, a searchable list for more. An optional `x-attricat-enum-labels` object maps enum values to display labels; every key must be an enum value, and labels are 1 to 200 characters. It cannot be combined with `x-attricat-status`. See [Choice lists](/builders/validation/#choice-lists).

### Statuses

A `string` attribute whose `value_schema` has an `enum` and an `x-attricat-status` annotation is a status. See [Statuses](/builders/validation/#statuses) for how they behave.

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
              "roles": ["reviewer"], "separate_from": ["submit"] },
            { "from": "released", "to": "draft", "code": "correct", "permission": "records.publish" }
          ]
        }
      }
```

| Key | Value | Description |
| --- | --- | --- |
| `version` | `1` | Required. |
| `options` | list of 1 to 100 mappings | Required. One option per `enum` code, in display order. |
| `transitions` | list of up to 10,000 mappings | Allowed changes. Omit it to allow any change; with it, only the listed changes are allowed, and an empty array allows none. |

Each option:

| Key | Value | Description |
| --- | --- | --- |
| `code` | code, up to 128 characters | Required. Must match exactly one `enum` value. |
| `label` | string, 1 to 200 characters | Required. Can contain [lexicon references](/builders/translations/#status-and-choice-labels). |
| `tone` | `default`, `success`, `warning`, `error`, or `info` | Color of the status chip. |
| `lock` | `"all"`, or 1 to 500 attribute codes | Makes the listed attributes read-only while the record has this status. `"all"` covers every attribute, relationship, and file except the status itself. A reusable attribute is written `namespace:code`. A record with a locking status cannot be deleted. Owners and admins can always unlock it to another status. Requires `transitions`. |
| `approval` | mapping with `covers` and `void_to` | Records an approval when the record enters this status. `covers` is `"all"` or 1 to 500 attribute codes; `void_to` is another option the record moves to when covered content changes. |
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

```yaml
attributes:
  - code: assignee
    value_type: string
    value_schema: '{"type": "string", "x-attricat-principal": {"version": 1, "kinds": ["user", "team"]}}'
```

| Key | Value | Description |
| --- | --- | --- |
| `version` | `1` | Required. |
| `kinds` | `["user"]`, `["team"]`, or both | Required. What the attribute accepts. |

- Values are `user:<id>` or `team:<id>`, with the lowercase ID from `acli directory list` or `GET /directory`. A missing value means unassigned.
- The schema must have `"type": "string"` and no `enum`, `const`, `pattern`, or `format`. The attribute cannot have a `default_value` or also be a status. Reusable attributes accept the same annotation.
- A new or changed value must be an active workspace member or a team that has not been deleted, of an accepted kind. Otherwise saving returns `422 attribute_value_schema_mismatch`. Unchanged values are not checked again.
- Search filters match the stored value exactly. The value `@me` with operator `eq` matches the caller and every team the caller belongs to.
- [Predicates](#predicates) treat the value as text: `required` checks that a record is assigned, and `compare` with `eq` or `one_of` can match a specific `user:<id>` or `team:<id>`. No predicate and no [transition requirement](#statuses) can refer to the person saving, so a rule such as "only the assignee may close this" cannot be declared.

### Relationship keys

```yaml
attributes:
  - code: brand
    value_type: relationship
    target_blueprint: brand
    cardinality: one
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `target_blueprint` | code | Any record blueprint | Restricts targets to records of this blueprint family. |
| `target_blueprints` | array of codes | Any record blueprint | Restricts targets to records of any of these blueprint families. Cannot be combined with `target_blueprint`; a one-item list is the same as `target_blueprint`. |
| `cardinality` | `"one"`, `"many"`, or `"one_to_one"` | `"many"` | How many targets one record may link to in one context. `one_to_one` is shorthand for `cardinality: one` plus `target_cardinality: one` and cannot be combined with `target_cardinality`. |
| `target_cardinality` | `"one"` or `"many"` | `"many"` | How many records may link to the same target through this attribute in one context. |
| `acyclic` | boolean | `false` | Rejects links that would form a cycle through this attribute. Requires `context_editable: default`, and the targets must include the blueprint itself. See [Hierarchies](#hierarchies). |
| `tree` | boolean | `false` | An acyclic hierarchy in which every record has at most one target (its parent). Implies `acyclic: true` and defaults `cardinality` to `"one"`; `cardinality: many` is rejected. |
| `target_scope` | mapping | Unset | `{attribute, equals}`: the record picker offers only targets whose `attribute` links to the same record as this record's `equals`. See [Target scope](#target-scope). |

`cardinality: one` gives a single-select field whose options can be shared, such as a brand. Add `target_cardinality: one` only for an exclusive pairing, where each target can be claimed once. A write that breaks either limit returns `409 relationship_cardinality_conflict`.

A write that links a record of a blueprint not allowed by `target_blueprint` or `target_blueprints` returns `422 relationship_target_type_mismatch`. The record picker in the web app offers only the allowed blueprints; when there are several, the picker has a **Target blueprint** selector. An `incoming_relationship_list` on any of the allowed target blueprints can list the field.

#### Hierarchies

```yaml
attributes:
  - code: parent
    value_type: relationship
    target_blueprint: location
    tree: true
    context_editable: default
```

`acyclic` and `tree` protect self-referencing structures such as location or asset hierarchies, and predecessor chains such as revision → previous revision.

- A write that would close a cycle returns `409 relationship_cycle`. `error.details.path` lists the record IDs along the cycle, starting and ending with the record being written. Linking a record to itself is a cycle of length one.
- In a `tree`, giving a record a second target returns `409 relationship_cardinality_conflict`.
- Checks see links of every revision of the blueprint and run inside the write's transaction. Two concurrent writes cannot each add half of a cycle.
- The hierarchy applies to the whole blueprint family as declared by its latest published revision, including records still pinned to older revisions.
- Publishing a revision that adds `acyclic` or `tree` checks the existing links first. If they contain cycles, or a tree has records with more than one target, publication fails with `409 relationship_hierarchy_violations`; `error.details` lists up to 20 cycles and records with extra targets. Fix the links and publish again.

#### Target scope

```yaml
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    target_scope: {attribute: product, equals: product}
```

With `target_scope`, the record picker of `conversion` lists only the unit conversions whose `product` is the product selected on the same form. Until a product is selected, the picker lists every conversion.

- The attribute must be a relationship with exactly one target blueprint. `equals` must be another relationship of this blueprint with `cardinality: one`, and `attribute` a relationship of the target blueprint with `cardinality: one`. Publication checks `attribute` when the target blueprint has a published revision.
- The scope only filters the picker. To refuse a mismatching link on save, add a [record check](#record-checks) such as `{"type": "linked", "relationship_code": "conversion", "predicate": {"type": "compare", "attribute_code": "product", "op": "eq", "subject_attribute_code": "product"}}`.

Relationships cannot have `value_schema` or `default_value`. Constrain them with `record_schema` instead.

### File keys

```yaml
attributes:
  - code: gallery
    value_type: file
    cardinality: many
    allowed_mime_groups: [image]
    allowed_extensions: [jpg, png, webp]
    max_bytes: 10485760
    image_only: true
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `cardinality` | `"one"` or `"many"` | `"one"` | Single file or a list of files. |
| `ordered` | boolean | `true` for `many` | Whether the order of a `many` list is meaningful. Must be `false` or omitted for `one`. |
| `allowed_mime_groups` | array of strings | Any | Accepted MIME groups, such as `image`. CSV and other text files are stored as `text/plain`, so they belong to the `text` group; use `allowed_extensions` to accept only `csv`. |
| `allowed_extensions` | array of strings | Any | Accepted extensions, letters and digits only (`jpg`, not `*.jpg`). A leading `.` is ignored. |
| `max_bytes` | positive integer | Server limit | Per-file size limit. The server's `FILE_UPLOAD_MAX_BYTES` still applies when this is higher. |
| `purposes` | array of codes | `[]` | Labels describing what the files are for, such as `product_image`. |
| `image_only` | boolean | `false` | Accept images only. Required for the `attricat.table_image` renderer. |

These keys are rejected on any attribute that is not `value_type: file`. File attributes cannot have `value_schema`, `default_value`, or `target_blueprint`.

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

### Search tags

`search:fuzzy` on a text attribute makes Explorer search also find values similar to a term, such as another form of the same word or a value with a letter missing or added: `tarcze` finds "Tarcza tnąca". It applies to plain and qualified terms of four letters or more, not to terms ending in `*`. Untagged attributes still match only values that contain the term. See [Search syntax](/guides/search-syntax/#similar-words).

```yaml
attributes:
  - code: name
    value_type: string
    tags: ['search:fuzzy']
```

### Generated values

`generated` makes the server give every new record the next number from a counter, such as an invoice, order, or document number:

```yaml
attributes:
  - code: invoice_number
    value_type: string
    generated:
      format: 'INV-{yyyy}-{n:05}'
      start: 1
      reset: yearly
      time_zone: Europe/Warsaw
```

The first invoice created in 2026 gets `INV-2026-00001`, the next one `INV-2026-00002`, and the first one created in 2027 `INV-2027-00001`.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `format` | string, up to 100 characters | Required for `string` | How the value is built from the [format tokens](#format-tokens). Not allowed on `integer`. |
| `start` | integer, 0 to 999,999,999,999,999,999 | `1` | The first number a new counter gives out. |
| `reset` | `"never"`, `"yearly"`, `"monthly"`, or `"daily"` | `"never"` | Starts counting again from `start` in every new period. Not allowed on `integer`. |
| `time_zone` | IANA time zone | `"UTC"` | Zone in which the date tokens and the reset period are read from the record's creation time. Not allowed on `integer`. |

Only `string` and `integer` attributes can be generated. A `string` needs a `format`; an `integer` stores the number itself and takes only `start`.

`generated` is allowed only on a local attribute of a record blueprint. It is rejected on a mixin, on an attribute selected with `from`, on an `extension_type`, together with `default_value`, and on a [status](#statuses) or a [user or team assignment](#user-or-team-assignments). A generated attribute is always read-only and default-context only (`context_editable: default`), whatever `readonly` and `context_editable` say.

#### Format tokens

| Token | Writes |
| --- | --- |
| `{n}` | The counter, without padding. Required exactly once. |
| `{n:0W}` | The counter, zero-padded to `W` digits, where `W` is 1 to 18. Wider numbers are not truncated: after `999`, `{n:03}` gives `1000`. |
| `{yyyy}` | The four-digit year. |
| `{yy}` | The two-digit year. |
| `{mm}` | The two-digit month. |
| `{dd}` | The two-digit day. |
| `{{` and `}}` | A literal `{` and `}`. |

Any other `{…}` placeholder is rejected. A `reset` needs tokens that identify its period, so that values from different periods never look the same: `yearly` needs `{yyyy}` or `{yy}`, `monthly` also needs `{mm}`, and `daily` also needs `{dd}`. Otherwise the blueprint is rejected.

#### How numbers are given out

- The server assigns the value in the transaction that creates the record. It appears in the create response, in the `record.created.v1` event, and in the audit log, and a `required` on the attribute in `record_schema` is satisfied.
- On the web app's create form, the field stays empty and says the value is assigned when the record is saved. It is never marked as required there, even when `record_schema` requires it, because there is nothing to fill in.
- Numbering is gapless. A create that fails, for example on validation or a unique key conflict, gives its number back. Deleted records keep their numbers, and numbers are never reused.
- There is one counter per blueprint family and attribute code, and with a `reset`, one per period. New revisions of the blueprint continue the same counter. Changing `start` later does not affect a counter that already exists, and changing `format` affects only new values. A new revision cannot change `reset` once an earlier revision generated the attribute, because numbering would restart in the new period and repeat numbers already given out. Such a revision is rejected with `409 generated_reset_changed`, and `error.details` has the `attribute` and its `previous` and `requested` reset.
- Duplicating a record gives the copy a new number. A record batch numbers each record it creates, in order. Sample records of a solution pack use up numbers, and removing the sample data does not give them back.
- No client can write a generated attribute. Creates, updates, clearing the value, restores from history, record batches, workflows, agents, and extensions that try are rejected with `422 generated_attribute_readonly`; `error.details.attribute` names the attribute.
- When an existing record migrates to a revision that adds a generated attribute, it gets a number during the migration. Records are numbered in the order they are migrated, not the order they were created, and the date tokens and reset period use each record's original creation date. A value the record already had in the previous revision is kept.

#### Counters

`GET /blueprints/{id}/sequences` lists the blueprint's counters and needs `blueprints.read`. Each entry has the `attribute_code`, the `period` (empty for a counter that never resets), `next_value` (the number the next record will receive), and `updated_at` (`null` while the counter has not been used).

To continue numbering after moving from another system, raise a counter with `POST /blueprints/{id}/sequences/{attribute_code}/advance`:

```json
{ "next_value": 10000, "period": "2026" }
```

- `period` is optional and defaults to the current period. It is `2026` for `yearly`, `2026-10` for `monthly`, `2026-10-27` for `daily`, and empty for a counter that never resets.
- The call needs `blueprints.publish` and is recorded in the audit log.
- A counter can only be raised. A lower `next_value` fails with `409 generated_sequence_not_raised`, and `error.details` has the `current` and `requested` values. The same value changes nothing.

From the CLI, use `acli blueprint sequences <blueprint-id>` and `acli blueprint advance-sequence <blueprint-id> <attribute-code> <next-value> [--period <period>]`.

### Extension attribute types

An enabled extension can declare attribute types, for example a money type with a currency setting:

```yaml
attributes:
  - code: price
    extension_type: 'com.acme.commerce:money@^1'
    extension_configuration: '{"currency":"USD"}'
```

When the blueprint is saved, Attricat finds the enabled extension `com.acme.commerce`, picks the highest declared `money` version matching `^1`, validates `extension_configuration` against the type's configuration schema, and records the resolved type on the revision. The value is stored with the type's underlying primitive (`string`, `number`, `integer`, `boolean`, `date`, `datetime`, `time`, or `json`) and checked against the type's value schema. Values stay readable if the extension is later disabled.

`extension_type` cannot be combined with `value_schema`, relationship keys, or file keys.

## Includes

Mixins share attribute definitions between blueprints. An include pins one exact published mixin revision. Each attribute you want must then be selected with `from`.

```yaml
includes:
  - alias: seo
    code: seo
    version: 2

attributes:
  - code: meta_title
    from: seo.meta_title
```

| Key | Type | Description |
| --- | --- | --- |
| `alias` | code | Local name used in `from`. Unique within the blueprint. |
| `code` | code | Code of the mixin blueprint. |
| `version` | positive integer | Exact published revision of the mixin. |

A selected attribute keeps all of the mixin's settings. To pick up changes in the mixin, publish a new mixin revision and point `version` at it in a new revision of the consuming blueprint.

## Views

`views` is a mapping keyed by view name. The web app uses these names:

| View | Purpose | Allowed `type` |
| --- | --- | --- |
| `dropdown_option` | Label for the record in relationship pickers, filter pills, and search results. Required on record blueprints. | `dropdown_option` |
| `page_title` | Browser tab title of the record page. Optional on record blueprints. | `page_title` |
| `detail` | Record page and create form. Fields the user may change are edited in place. | A layout block |
| `edit` | Deprecated and ignored by the web app. Still accepted, and edit components in it are still validated. | A layout block |
| `table` | Explorer columns. | `table` |
| `explorer` | Explorer defaults, such as the field the **Browse by** panel lists. | `explorer` |

When `detail` or `table` is missing, the web app lists attributes in declaration order.

### `dropdown_option`

```yaml
views:
  dropdown_option:
    type: dropdown_option
    fields: [name, sku]
    separator: ' / '
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `fields` | array of attribute codes | Required | One or more unique attributes joined into the label. |
| `separator` | string | `" · "` | Text placed between fields. |

Labels are resolved per context and follow each attribute's `context_fallback`.

### `page_title`

```yaml
views:
  page_title:
    type: page_title
    fields: [name]
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `fields` | array of attribute codes | Required | One or more unique scalar attributes joined into the title. Unset values are skipped. |
| `separator` | string | `" · "` | Text placed between fields. |

Only a record blueprint can define it, and only under the name `page_title`. The title uses the values shown in the selected context. Without this view, or when all its fields are empty, the record page uses the `dropdown_option` label, then a short record ID.

### `table`

```yaml
views:
  table:
    type: table
    columns:
      - {field: title, label: Product}
      - {field: category.name, label: Category}
      - field: main_photo
        label: Image
        renderer: {id: attricat.table_image, version: 1}
```

| Key | Type | Description |
| --- | --- | --- |
| `columns` | list of mappings | Column definitions. Must be non-empty with unique `field` values. |
| `columns[].field` | string | A local scalar attribute, a local relationship (the linked records' labels, up to five, of any target blueprint), a local file attribute with a compatible renderer, or a path of up to three relationship hops ending in a scalar attribute, such as `family.product_type.name`. A hop may link to several blueprints; the next attribute must then have one value type on every blueprint that declares it. |
| `columns[].label` | string | Column header. Defaults to the attribute name. |
| `columns[].renderer` | component reference | `attricat.table_image@1`, a string display component (`attricat.color_display@1`, `attricat.email_display@1`, `attricat.url_display@1`, or `attricat.phone_display@1`), or an extension's table cell. |
| `component` | component reference | Optional table component. |

A column can be sorted in the Explorer only when it is scalar and every relationship hop in its path has `cardinality: one`.

### `explorer`

```yaml
views:
  explorer:
    type: explorer
    browse_by: categories
```

| Key | Type | Description |
| --- | --- | --- |
| `browse_by` | attribute code, relationship path, or `false` | Relationship attribute the **Browse by** panel lists until a person picks another. A relationship to several blueprints lists their records together, without a tree. A dotted path such as `model.brand` follows up to three relationships, each with a single target blueprint and `cardinality: one`, and lists the last one's targets. `false` keeps the panel closed until a person opens it. |

Only a record blueprint can define it, and only under the name `explorer`. Without `browse_by`, the panel opens for the first relationship whose target has a hierarchy. Relationships after the first on a path are checked against the published target blueprints when the blueprint is saved and published.

### Layout blocks

`detail` and `edit` start with a container block and nest other blocks inside it. Blocks are YAML mappings with a `type` key. You can write them in block style, one key per line, or in flow style as `{type: field, field: title}`; both produce the same result.

| `type` | Keys | Description |
| --- | --- | --- |
| `stack` | `children` | Children stacked vertically. |
| `grid` | `children` | Children in a responsive grid. |
| `section` | `children` | A grouped region. |
| `tabs` | `tabs`: list of `{label, children}` | One tab per entry. |
| `accordion` | `sections`: list of `{label, children}` | Collapsible sections. |
| `field` | `field`; optional `editor` | One scalar or file attribute. In `detail`, `editor` names an extension's attribute editor by `extension_id`, `id`, `version` and `props`; see [Attribute editors](/extensions/manifest/#attribute-editors). |
| `relationship_list` | `field` | One relationship attribute and its targets. |
| `incoming_relationship_list` | `label`, `relationships`, `page_size`, `display`, `editable`, `columns` | A link that opens the Explorer in a new tab, filtered to records linking to this one; one link per relationship. `relationships` is a list of `{source_blueprint, field}`. `page_size` is capped by the server's `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE`. With `display: table` (default `links`), the records of exactly one relationship are listed in a table on the page instead, up to `page_size` of them, with the source attributes `columns` names (every attribute but the link back when omitted). `editable: true` makes the table's cells editable and adds **Add** and **Delete** to it; see [Order lines](#order-lines). Allowed only in `detail`. |
| `heading` | `text` | Static heading. |
| `text` | `text` | Static paragraph. |
| `divider` | | Horizontal rule. |

Every block except `explorer` accepts an optional `component` reference. Every `field` and `relationship_list` must name an attribute the blueprint has, including attributes selected from mixins.

### Component references

```yaml
component:
  id: attricat.url_display
  version: 1

renderer:
  id: example.currency
  version: 1
  props:
    currency: USD
```

| Key | Type | Description |
| --- | --- | --- |
| `id` | string | Registered component ID. |
| `version` | integer | Component version. |
| `props` | mapping | Component options. Only props the component declares are accepted. |

Built-in components:

| ID | Version | Used on | Value types | Props |
| --- | --- | --- | --- | --- |
| `attricat.field_display` | 1 | `field` (detail) | Scalars | |
| `attricat.relationship_list_display` | 1 | `relationship_list` (detail) | `relationship` | |
| `attricat.relationship_hierarchy` | 1 | `relationship_list` (detail) | `relationship` | `parent_field` |
| `attricat.incoming_relationship_list_display` | 1 | `incoming_relationship_list` | | |
| `attricat.record_heading` | 1 | `stack` (detail) | | |
| `attricat.table_display` | 1 | `table` | Scalars | |
| `attricat.table_image` | 1 | table column `renderer` | `file` with `cardinality: one` and `image_only: true` | |
| `attricat.color_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `attricat.email_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `attricat.url_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `attricat.phone_display` | 1 | `field` (detail), table column `renderer` | `string` | |
| `attricat.markdown_display` | 1 | `field` (detail) | `string` | |

Where a `detail` field is editable, the web app uses the input that goes with its display component, or the standard editor for the value type. See [Field controls](/builders/views/#field-controls) for how these behave.

An extension cell's ID and version must match a [table cell](/extensions/manifest/#table-cells) declared by an enabled extension for the column's value type, and its `props` must match the cell's declared prop types.

### `extensions`

`detail` and `table` may list the extension contributions that appear on this blueprint's records:

```yaml
views:
  detail:
    type: stack
    extensions: [acme.stock/recalculate, acme.pricing/customer_price]
  table:
    type: table
    extensions: [acme.stock/recalculate-selected]
```

| Key | Description |
| --- | --- |
| `views.detail.extensions` | Record actions, header actions, record tabs and form assists, as `<extension-id>/<contribution-id>`, in display order. Up to 32. |
| `views.table.extensions` | Explorer row, selection and toolbar actions, in the same form. Up to 32. |

Only contributions that declare the view appear in it. A `detail` stack without `children` and a `table` without `columns` keep the default layout and columns. Entries for extensions that are not installed or enabled are kept and ignored. Saving checks an enabled form assist's watched and proposed fields against the blueprint; an assist enabled later whose fields do not fit never runs. A record form runs at most the first 8 form assists listed. Record blueprints only.

## Publication

```yaml
publication:
  retain_on_edit_roles: [editor]
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `retain_on_edit_roles` | array of role codes | `[]` | Workspace roles whose edits keep a record's existing channel publications. Everyone else's edits withdraw them. The roles must exist when the revision is published. |

This does not grant any permission. See [Publishing](/guides/publishing/).

## Rules

Blueprints do not contain data quality rules. A rule is created separately and applies to the blueprint's records; see [Rules](/builders/rules/). A definition with a `rules` list is rejected.

## Predicates

Rules, record checks, transition conditions, and publication channel checks share one predicate language. A predicate is a mapping (in YAML) or object (in JSON) tagged by `type`. It **holds** when the data is acceptable. Unknown keys are rejected.

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
| `referenced_by` | `blueprint_code`, `relationship_code`, optional `predicate`, `min` and/or `max` (0 to 1000) | The number of `blueprint_code` records whose `relationship_code` targets this record, and that satisfy `predicate`, is between `min` and `max`. `max: 0` means "none". |
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

```yaml
record_schema: |
  {
    "type": "object",
    "x-attricat-checks": [
      { "code": "valid-range", "message": "Valid until must not be before valid from",
        "predicate": { "type": "compare", "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from" } }
    ]
  }
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

```yaml
unique_keys:
  - code: manufacturer_part
    attributes: [manufacturer, part_number]

  - code: slug
    attributes: [slug]
    scope: context
    case_sensitive: true
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Unique within the blueprint. Reported in errors. |
| `attributes` | array of attribute codes | Required | One to eight attributes whose combined values must be unique. Each must be a scalar other than `json`, or a relationship with `cardinality: one`. |
| `scope` | `"workspace"` or `"context"` | `"workspace"` | `workspace` compares values in the default context. `context` compares the values each context shows, including inherited ones, separately in every context. |
| `case_sensitive` | boolean | `false` | Compare text exactly instead of case-insensitively. |

How values are compared:

- Text is trimmed and every run of whitespace becomes one space. Unless `case_sensitive: true`, text is also compared in lowercase, so `ABC-1  Rev` and ` abc-1 rev` are the same key.
- Numbers compare by value (`1.50` equals `1.5`), date-times by instant, and relationships by the linked record.
- A record that has no value, or only blank text, for any of the key's attributes is not checked against that key. Make the attributes required in `record_schema` if every record must have the key.
- The key covers the whole blueprint family as declared by its latest published revision, including records pinned to older revisions. Attributes are matched by code.

A write that would give a second record the same key value returns `409 unique_key_conflict`. `error.details` names the `key`, the `context` code, the normalized `values`, and the `conflicting_record_id` that already holds them. The check runs in the database inside the write's transaction, so when two people save the same value at the same moment, exactly one save succeeds.

Publishing a revision that adds or changes unique keys checks existing records first. If some already share a value, publication fails with `409 unique_key_duplicates`, and `error.details.duplicates` lists up to 20 groups with the key, context, values, and record IDs (`error.details.total` counts all groups). Change or delete the duplicates and publish again.

## Balances and movements

Some records keep a running total that other records change, such as a stock level that goods receipts raise and goods issues lower. A **balance** attribute holds the total. A **movement** on another blueprint says how creating one of its records changes the total. The server applies the movement in the same transaction that creates the record, so the total always matches its documents, and two people saving at the same moment cannot take it below its minimum.

```yaml
# stock_item: one record per product
unique_keys:
  - code: product
    attributes: [product]
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0}
```

```yaml
# goods_issue: lowers the stock level of its product
attributes:
  - code: product
    value_type: relationship
    target_blueprint: product
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: unit_conversion
    cardinality: one
    context_editable: default
    target_scope: {attribute: product, equals: product}
  - code: stock_item
    value_type: relationship
    target_blueprint: stock_item
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: goods_issue
    cardinality: one
    context_editable: default
movements:
  - code: stock
    direction: out
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: stock_item
      balance: quantity
      key: product
      values: {product: product}
    reversal: {relationship: reverses}
```

A goods receipt is the same with `direction: in` and `create_missing: true`, so the first receipt of a product creates its stock item.

### Balance attributes

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `balance.min` | number | No minimum | The lowest value a movement may leave, such as `0` for stock that cannot go negative. On an `integer` balance it must be a whole number. |
| `balance.reserved` | attribute code | None | Another balance of the same record that holds part of this one, such as reserved stock. `min` then applies to this balance minus the reserved one. See [Reservations](#reservations). |

- `balance` is allowed only on a local `number` or `integer` attribute of a record blueprint. It is rejected on a mixin, with `from`, with `generated` or `default_value`, and on a status or a user or team assignment.
- A balance is always read-only and default-context only (`context_editable: default`). A record without a value counts as zero.
- Only movements change a balance. Creates, updates, clearing the value, restores from history, record batches, workflows, agents, and extensions that try are rejected with `422 balance_attribute_readonly`.
- A record whose balance is not zero cannot be deleted: `409 balance_record_not_empty`, with the `attribute` and its `balance`. Solution-pack sample records are exempt.
- The attributes of the unique key a movement finds the record by, such as a stock item's product, cannot change once they have a value: `409 movement_target_key_immutable`. Movements already applied to the record by that key.

### Movement keys

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `code` | code | Yes | Unique within the blueprint. |
| `direction` | `"in"` or `"out"` | Yes | `in` adds the moved amount to the balance; `out` subtracts it. |
| `quantity` | attribute code | Yes | `number` or `integer` attribute holding the quantity. It must be greater than zero. |
| `factor.relationship` | attribute code | No | Relationship with `cardinality: one` and one target blueprint, to the record holding a conversion factor. |
| `factor.attribute` | code | With `factor` | `number` or `integer` attribute of the linked record. The moved amount is `quantity × factor`; without a link it is the quantity. |
| `result` | attribute code | For `reversal` | `number` attribute in which the server stores the moved amount, in the balance's unit. `integer` only for an integer quantity without a factor. |
| `target.relationship` | attribute code | Yes | Relationship with `cardinality: one` and `target_blueprint: <target.blueprint>`, in which the server stores the target. |
| `target.blueprint` | code | Yes | Record blueprint of the target. |
| `target.balance` | code | Yes | Balance attribute of the target blueprint. |
| `target.key` | code | Yes | Unique key of the target blueprint, with `scope: workspace`, that finds the target. |
| `target.values` | mapping | Yes | Every attribute of the key, mapped to the attribute of this record holding its value, such as `{product: product}`. The types must match. |
| `target.create_missing` | boolean | `false` | Create the target with the key values when no record has them. Otherwise the movement fails. |
| `reversal.relationship` | attribute code | No | Relationship with `cardinality: one` to this same blueprint. A record that sets it reverses the linked record. See [Reversals](#reversals). |
| `apply_on` | list | Apply on create | Up to 8 status transitions that apply the movement instead of the create: `{attribute, to, from?}`, optionally with `relationship`. See [Status-triggered movements](#status-triggered-movements). |
| `requires` | list of codes | None | Movements of this blueprint, declared earlier, that must have applied to the record first; otherwise this one is skipped. |

The attributes a movement reads (the quantity, the factor relationship, the key values, the reversal relationship and a trigger's relationship) must be `context_editable: default`. Publication checks the target and factor blueprints at their latest published revisions, so publish the stock item and unit conversion blueprints first.

### How a movement applies

When a movement applies (when its record is created, unless it has `apply_on`):

1. The quantity must be greater than zero, or the save returns `422 movement_quantity_invalid`. A linked factor must be greater than zero, or it returns `409 movement_factor_invalid`.
2. The server finds the target through `target.key` and the record's key values. With `create_missing: true` a missing target is created; otherwise the save returns `409 movement_target_missing` with the `blueprint` and `key`. Concurrent first receipts of one product create one target.
3. The target's balance changes by `quantity × factor`. If it would go below `min`, the save returns `409 balance_below_minimum`, and `error.details` has the `attribute`, the target `record_id`, the `available` balance, the `requested` amount, and the `minimum`. Nothing is saved: neither the record nor the change.
4. The server stores the target in `target.relationship` and the amount in `result`. Clients cannot write them: `422 movement_attribute_readonly`.

The target's change is an ordinary update. Its status locks apply, it is audited as `attricat.records.movement.balance_update` with the movement record in the metadata, and it emits `record.updated.v1`.

A movement applies once. After it applies, the attributes it read or set cannot change, and a save that tries returns `409 movement_immutable`. The record's other attributes stay editable, and so do the inputs of movements that have not applied yet, such as those of a draft order. A record with an applied movement cannot be deleted (`409 movement_record_undeletable`); reverse it instead. The record form response lists the record's `applied_movements`. Duplicating a movement record creates a new movement.

### Status-triggered movements

With `apply_on`, a movement applies at a status transition instead of when its record is created, such as reserving material when a production order is approved and consuming it when the order is completed.

```yaml
# production_order: one material, reserved on approval
movements:
  - code: reserve
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: approved}]
  - code: release
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: completed}, {attribute: status, to: cancelled}]
    requires: [reserve]
  - code: consume
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: quantity,
             key: product, values: {product: material}}
    apply_on: [{attribute: status, to: completed}]
    requires: [reserve]
```

- `attribute` is a status attribute and `to` (and the optional `from`) its option codes. A transition into `to` from `from`, or from any option, applies the movement.
- Each movement applies once per record, at the first matching transition. Re-entering the status does nothing.
- With `requires`, a movement applies only after the movements it names. Cancelling a draft order releases nothing, because nothing was reserved.
- Movements that find the same record may share a target relationship, such as `stock_item` above.
- The movements apply in the transaction of the save that changes the status, in declaration order, whichever way the status changes: a form, a record batch, a workflow, an agent or an extension. If one fails, for example on `409 balance_below_minimum`, the save fails and the status stays as it was.

#### Order lines

A trigger with `relationship` applies a movement of a line record when the record it links to makes the transition. A production order with several materials keeps its status, while each material line reserves and consumes its own material:

```yaml
# production_order_line
movements:
  - code: reserve
    # ...
    apply_on: [{relationship: production_order, attribute: status, to: approved}]
```

- The relationship has `cardinality: one`, one target blueprint and `context_editable: default`. Publication checks that the status and options exist on that blueprint.
- One transition applies the movements of at most 200 lines; more returns `422 movement_lines_exceeded`. If any line fails, the whole transition fails.
- Once an order's lines of one blueprint have been applied, those lines are settled: adding one to the order, moving one to another order or deleting one returns `409 movement_lines_locked`, with the `order_id`. Lines of another blueprint settle when their own movements apply, so an approved order still takes output lines until its completion. The order's record form lists them in `settled_lines` as `{blueprint, relationship}`.

To work on the lines from the order's page, list them in an editable table:

```yaml
# production_order: views.detail
- type: incoming_relationship_list
  label: Materials
  display: table
  editable: true
  columns: [item, quantity, conversion]
  page_size: 50
  relationships:
    - {source_blueprint: production_order_line, field: production_order}
```

The table's cells edit each line in place, and every changed cell saves when you leave it, as fields do on a record's page. Cells a line doesn't let you change show its value. **Add** opens a new row at the bottom of the table: fill in its cells and press Enter or its save button, and the line is created with the order already set while the row empties for the next one. Escape or its discard button closes it. Required values you didn't fill in are marked in their cells. **Open** shows a line's whole record beside the order, and **Delete** removes one. Once the lines are settled, the table only shows and opens them.

### Reservations

A balance with `reserved` keeps part of itself aside. `min` applies to the balance minus the reserved balance, after every change to either:

```yaml
# stock_item
attributes:
  - code: quantity
    value_type: number
    balance: {min: 0, reserved: reserved}
  - code: reserved
    value_type: number
    balance: {min: 0}
```

- An issue cannot take reserved stock, and a reservation cannot hold more than is free.
- In `409 balance_below_minimum`, `available` is then the free amount, and `details.reserved` holds the reserved amount.
- `reserved` names another balance of the same blueprint, which has no `reserved` of its own.

### Reversals

A record that sets `reversal.relationship` reverses the record it links to. It moves back every movement that applied to the original, latest first, on the original's targets. Movements that have not applied are skipped; a record with nothing applied cannot be reversed (`409 movement_not_applied`). A reversal applies when it is created, and its own status never applies its movements again.

- The reversal takes the original's quantity, factor link and key values. Values submitted for them must match, or the save returns `422 movement_reversal_mismatch`; numbers compare by value.
- A record can be reversed once. A second reversal returns `409 movement_already_reversed`, with the `record_id` and the reversal that already exists in `reversed_by`. A reversal cannot be reversed: `409 movement_reversal_of_reversal`.
- The reversal is subject to the balance's minimum. Reversing a receipt whose goods were already issued returns `409 balance_below_minimum`; reverse the issues first.

## Connector jobs

Connector jobs run an operation of an installed connector extension, such as a CSV import or export, against this blueprint.

```yaml
connector_jobs:
  - code: csv_export
    direction: export
    extension_id: attricat-connector-csv
    operation_id: export
    interval_seconds: 3600
    input:
      profile:
        version: 1
        columns:
          - {header: ID, attribute: external_id, kind: string}
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `code` | code | Required | Stable identifier. Jobs are matched by code across revisions. |
| `direction` | `"import"` or `"export"` | Required | |
| `extension_id` | string | Required | Connector extension ID. |
| `operation_id` | string | Required | Operation declared by the extension. |
| `input` | mapping | Required | Operation input, validated against the operation's schema. |
| `context` | context code | | Required for imports: imported values are written to this context. For exports, the publication channel to export (it must be one); without it the job runs once per enabled channel. A job of an extension that keeps export mappings must set it when several channels are enabled. |
| `input_file_id` | UUID | | Imports only. A ready workspace file to import, such as a value of a file attribute. |
| `interval_seconds` | integer | Manual only | Run every N seconds, 60 to 2592000. |
| `enabled` | boolean | `true` | Set `false` to pause the job. |
| `connection` | string | Default destination | Exports only. The extension [connection](/builders/extensions/#connections) the job pushes to: 1 to 128 ASCII letters, digits, `.`, `_`, or `-`. |
| `trigger` | mapping | | Exports only. Also run the job when records change: `{events: [...], debounce_seconds: N}`. |

`trigger.events` lists one or more of `record.created.v1`, `record.updated.v1`, `record.deleted.v1`, `record.migrated.v1`, `record.published.v1`, `record.unpublished.v1`, `attribute_value.changed.v1`, `attribute_value.restored.v1`, and `relationship.changed.v1`. `trigger.debounce_seconds` is 1 to 3600: a run starts once the newest matching event of the blueprint's records is that old, or once the oldest pending change is ten times that old, and covers only the changed records. An event of a record that the blueprint's records link to through a relationship counts as a change of the linking records.

```yaml
connector_jobs:
  - code: shopify
    direction: export
    extension_id: acme.shopify
    operation_id: export
    connection: shop-eu
    trigger:
      events: [record.updated.v1, record.published.v1]
      debounce_seconds: 30
    input: {}
```

Jobs are checked against the enabled extension when the revision is published; an invalid job blocks publication. A job removed in a later revision is disabled and its run history is kept. An export job runs once per enabled publication channel and exports only records published to that channel. See [Operations and connectors](/extensions/operations/).

## Limits and errors

A few compile-time rules that are easy to miss:

- `format_version` other than `1` is rejected.
- A record blueprint without `views.dropdown_option` is rejected.
- `record_schema` on a mixin is rejected.
- `record_schema` may only name attributes the blueprint has in its top-level `required`, `properties`, `dependentRequired`, and `dependentSchemas`.
- Predicates in `x-attricat-checks` and transition `conditions` must name attributes the blueprint has, with types that suit the predicate. An ordering comparison on a string, or a comparison of a date with a number, is rejected.
- `target_blueprint`, `target_blueprints`, `acyclic`, and `tree` on a non-relationship attribute are rejected.
- `unique_keys` on a mixin, a key naming an unknown, `json`, file, or many-target relationship attribute, or a key listing an attribute twice is rejected.
- `from` must be `alias.code` where `code` matches the attribute's own code.
- `balance` on a mixin, a `from` selection, a type other than `number` or `integer`, or together with `generated` or `default_value` is rejected.
- A movement naming an unknown attribute, using an attribute in two roles, reading an attribute that is not `context_editable: default`, or declaring `reversal` without `result` is rejected. Publication rejects a movement whose target blueprint is not published or lacks the balance or key, and a factor that is not a number.
- `generated` on a mixin, a `from` selection, an `extension_type`, a status, or a user or team assignment, on a type other than `string` or `integer`, or together with `default_value` is rejected. So is a `format` without exactly one `{n}`, with an unknown placeholder, or longer than 100 characters, and a `reset` whose period the `format` does not identify.
