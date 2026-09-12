# Workflows (foundation)

Workflows are workspace-scoped, versioned TOML definitions, independent of blueprint TOML. Every saved revision retains its raw TOML, SHA-256 hash, and compiled declarative plan. Revisions start as drafts; publishing makes an immutable revision eligible for enablement. A workflow has one enabled revision or is disabled.

Definitions require `format_version = 1`, a stable `code`, `name`, at least one exact core-event trigger, and ordered actions. Trigger `envelope` and `facts` filters contain scalar exact matches. v1 accepts only `entity.created.v1`, `entity.updated.v1`, `entity.deleted.v1`, `attribute_value.appended.v1`, and `blueprint.published.v1`.

Actions are deliberately declarative: `system_tags_add`, `system_tags_remove`, `system_metadata_merge`, `system_metadata_delete`, and `attribute_write`. Attribute writes accept exactly one scalar `fixed` value or a narrowly named `event_field`. Unknown TOML fields, scripts, SQL, HTTP, templates, loops, arrays/objects in filters, and unrecognized events/actions are rejected.

```toml
format_version = 1
code = "tag-new-products"
name = "Tag new products"
[[triggers]]
event_type = "entity.created.v1"
[triggers.facts]
blueprint = "product"
[[actions]]
type = "system_tags_add"
tags = ["new"]
```

The lifecycle records the current domain-event sequence as an internal activation high-water boundary when enabled. It is intentionally not exposed through the management API. **Event dispatch, action execution/workers, and UI are later slices; this foundation never executes a workflow.**
