# Component Authoring

View components connect declarative blueprint views to catalog-web renderers.
They are platform components: blueprint definitions select a component by ID and
version, but never execute arbitrary frontend code.

## Files

Every component has a TypeScript definition that owns its ID, version,
applicability metadata, and any renderer or editor it implements. The central
`apps/catalog-web/src/features/views/components/registry.ts` imports each
definition and exposes the runtime lookup map.

Layout, relationship and table components have one module each in
`views/components`. The string field controls (color, email, URL, phone,
Markdown) share `apps/catalog-web/src/features/views/controls`:

| File | Contents |
| --- | --- |
| `color.ts`, `email.ts`, `phone.ts`, `url.ts` | Pure value policies (parsing, link targets) and form validators. |
| `TextControl.tsx` | `TextControlEditor` (single-line input) and `LinkedTextValue` (text that links when its policy allows). |
| `editors.tsx`, `values.tsx` | One editor and one renderer per control. |
| `definitions.ts` | The registered display/edit definitions, built with `stringDisplay` and `stringEdit`. |

A new string control usually needs a policy function, a few lines in
`editors.tsx` and `values.tsx`, and two `definitions.ts` entries.

The Rust blueprint compiler uses `contracts/view-components.json` to validate
blueprint references before they reach the client. The TypeScript registry is
the frontend implementation source; the JSON file is its shared validation
contract.

Persisted value constraints do not belong to view components. Blueprints own
attribute and entity JSON Schemas, which Rust enforces before values are stored.
See [JSON Schema Validation](json-schema-validation.md) for that contract.

## Add A Component

1. Create a component module under
   `apps/catalog-web/src/features/views/components` (or add a string control
   to `views/controls`, as above).
2. Export a definition that `satisfies ViewComponentDefinition`.
3. Add that definition to `viewComponents` in `registry.ts` (string controls
   are registered through `stringControlComponents`).
4. Add equivalent metadata to `contracts/view-components.json` in the same
   order.
5. Add the component reference to a blueprint view where it is applicable.

For example, a scalar field display component can delegate to a React value
renderer:

```tsx
import { AttributeValue } from "./values/AttributeValue";
import type { ViewComponentDefinition } from "./componentTypes";

export const priceDisplayComponent = {
  id: "catalog.price_display",
  version: 1,
  capabilities: ["display"],
  placements: ["field"],
  value_types: ["number"],
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
```

Use lowercase dotted IDs whose dot-delimited segments are lowercase,
underscore-separated words. A component ID and version uniquely identify the
implementation, so increment the version when making an incompatible change.

## Metadata

`capabilities` limits a component to display or edit views. `placements`
selects the supported view block type: `field`, `relationship_list`,
`incoming_relationship_list`, `table`, or `stack`. `value_types` limits the attribute types for data placements.

The compiler enforces capabilities, placements, and value types against the
component reference. When `props` is an object, it also rejects keys outside `allowed_props`.
Non-object JSON props are currently accepted but receive no key validation; use
an object for forward-compatible props. Do not add props
to `allowed_props` until the applicable renderer consumes them: component props
are currently validated but are not passed to frontend renderers.

`value_types` describes renderer applicability, not data integrity. A renderer
must tolerate valid values for its declared attribute type; it cannot make a
stored value valid or invalid.

## Renderers

`valueRenderer` is used for `field` and `relationship_list` display nodes, for
table column `renderer` references in Explorer, and in the blueprint preview
sandbox. It receives the attribute definition and resolved value; a table cell
may receive an array when a column projects several values:

```ts
type ValueRenderer = ComponentType<{
  attribute: Attribute;
  value: unknown;
}>;
```

A built-in table renderer that needs specialized search data should have a
focused Explorer cell component instead; for example, `catalog.table_image@1`
renders the hydrated metadata for a direct image-only single-file attribute.

`headingRenderer` is reserved for a stack component that renders the entity
page heading. It receives attributes, resolved values, the entity ID, and the
detail view.

## Editors

`EntityForm` owns form state. An edit component may supply a `valueEditor`
that receives `ValueEditorProps` (attribute, string value, disabled, required,
error, helper text and `onChange`); the entity form and the blueprint sandbox
use it when the component supports the attribute's value type
(`resolveValueEditor`). Without one, the form's built-in input is used, as for
`catalog.field_edit`.

Two optional fields add form behavior:

- `validateValue(value)` returns a message for an invalid string. The form runs
  it on Save after the required check; editors can show the same message while
  typing. It is a web-form check only, never a persisted constraint.
- `preservesWhitespace: true` saves the edited string verbatim instead of
  trimming it (used by Markdown).

Statuses are not components: `x-attricat-status` in the attribute's
`value_schema` selects the status editor and display for that attribute
everywhere. See [Status attributes](status-control.md).

## Verify

`registry.test.ts` compares registry metadata with
`contracts/view-components.json`. Run it after changing either side:

```sh
pnpm --dir apps/catalog-web test -- src/features/views/components/registry.test.ts
cargo test -p catalog-blueprint
```

Run `pnpm --dir apps/catalog-web typecheck` as well when adding or
changing renderer types.
