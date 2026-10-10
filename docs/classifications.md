# Tags, Labels, And Classifications

Use **records and relationships** for tags, labels, categories, and other
controlled classifications. Do not model them as free-text strings, JSON arrays,
or a new scalar `tags` type.

A classification is data with a stable identity, can be reused by more than one
record type, and often needs its own name, translation, hierarchy, ownership,
or lifecycle. A target record provides all of those capabilities while a
relationship records which classifications apply to a source record.

## Model a classification type

Create a record blueprint for each classification domain whose semantics differ,
such as `category`, `color`, `material`, `certification`, or `label`. Give it a
human-readable scalar attribute and configure that attribute as its dropdown
option display. Use a stable code or external identifier when integrations need
one; record UUIDs remain the relationship identity.

```toml
format_version = 1
code = "label"
name = "Label"
kind = "record"

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]
```

Then point a relationship attribute from the classified record to that blueprint:

```toml
# On product
[[attributes]]
code = "labels"
value_type = "relationship"
target_blueprint = "label"
```

The relationship is the assignment. A product can have multiple labels, and a
label can be assigned to multiple products. Catalog validates that every target
is a `label` record and retains current and historical assignments in the same
way as other relationships.

Use a domain-specific relationship name (`categories`, `certifications`, or
`materials`) where possible. Reserve a generic `labels` or `tags` relationship
for genuinely cross-cutting annotations rather than using it as a catch-all for
unrelated taxonomies.

## Contexts and multilingual names

Both sides are contextual:

- The source-to-classification relationship set can differ by context. For
  example, a product may have a market-specific set of labels.
- The classification record's `name` can have contextual scalar overrides. For
  example, write translated names in language contexts and use normal
  `context_fallback` to resolve a display name.

Relationship dropdown labels are rendered from the target blueprint's
`views.dropdown_option` fields and respect those fields' contextual fallback.
This keeps a classification's stable record identity separate from its
localized presentation.

Contexts are not inherently languages. Only use a context for translations when
that workspace's context model says it represents a locale; otherwise document
which context dimension owns the localized value and its fallback policy.

## Hierarchies and facets

When a classification is hierarchical, add a self-targeting relationship on its
blueprint. For example, `category.parent` can target `category`. Products then
point to `category` through `product.categories`. This model supports the
relationship tree facet documented in [Relationship tree facets](search-facets.md).

Keep distinct classification domains in distinct blueprints unless they truly
share both vocabulary and lifecycle. A color is not a category merely because
both can be selected on a product.

## Cardinality caveat

Use `cardinality = "one"` when a source may select one classification while the
classification remains reusable by many sources. Set `target_cardinality =
"one"` as well only for genuinely exclusive pairing. Both directions default to
`"many"`.

For example, a product's brand can be single-select and shared:

```toml
[[attributes]]
code = "brand"
value_type = "relationship"
target_blueprint = "brand"
cardinality = "one"
```

## When not to add a new type

Use an ordinary scalar attribute for an intrinsic value that is not a shared
controlled concept, such as a SKU, title, or free-form note. Consider a future
blueprint-local option/enum type only when choices are intentionally local,
small, and do not need identity, reuse, contextual names, hierarchy, or other
metadata. Do not use `value_json` for classifications today: it is reserved and
has no supported application read/write path.
