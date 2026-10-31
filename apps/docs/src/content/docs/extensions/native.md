---
title: Native UI
description: Build tabs, panels, dialogs, settings and attribute editors from Attricat UI kit components, with your logic in an isolated Worker.
---

Native UI is the middle tier of [the three ways to draw UI](/extensions/build/#three-ways-to-draw-ui). Your code runs in an isolated Worker and composes components from the **Attricat UI kit**. Attricat checks what you render and draws it with its own components, so it looks and behaves like the rest of the app: it follows the theme and language, uses the same keyboard behavior and focus handling, and keeps working when Attricat's design changes.

Use native UI for record tabs, panels, action dialogs, attribute settings and attribute editors. Use [Frame UI](/extensions/client/) for full pages and tools the UI kit cannot build, such as a canvas or a custom editor.

## Get started

Scaffold an extension with the CLI:

```sh
acli extension new inventory --id acme.inventory --name "Inventory"
cd inventory
pnpm install
pnpm test
pnpm package
```

It creates a manifest with one `client_native` artifact, a blueprint panel and an attribute editor, a Vite configuration, a component for each contribution, and a test. Side-load the archive from `dist/` through **Manage → Extensions → Upload archive**, grant its permissions and enable it.

### The manifest

Declare one `client_native` artifact and point contributions at it:

```json
"artifacts": [
  { "id": "ui", "kind": "client_native", "path": "dist/ui.js", "ui_kit": "^1.0" }
],
"ui": [
  { "id": "stock", "version": 1, "kind": "tab", "artifact": "ui", "outlet": "record_tab", "title": "Stock" }
]
```

`ui_kit` is the range of UI kit versions your bundle works with. Everything else about a contribution — its kind, outlet, title, translations, `when` and the capability its outlet needs — is the same as for Frame UI. Native UI is available at `record_tab`, `blueprint_attribute_configuration`, `blueprint_detail_panel`, `blueprint_panel`, `data_health_card`, `export_mapping`, `action_dialog` and `attribute_editor`. All native contributions of a release share one Worker. See the [manifest reference](/extensions/manifest/#artifacts).

### Components

Register one Preact component per native contribution, under the contribution's `id`:

```tsx
import { register, useAttricat, useOutletContext } from '@attricat/extension';
import { Button, Stack, Table, Text } from '@attricat/ui-kit';
import { useEffect, useState } from 'preact/hooks';

type RecordContext = { record_id: string };
type Row = { id: string; cells: { warehouse: string; quantity: number } };

const Stock = () => {
  const { record_id } = useOutletContext<RecordContext>();
  const attricat = useAttricat();
  const [rows, setRows] = useState<Row[]>([]);
  const load = () =>
    attricat.command({ command_id: 'stock', payload: { record_id } }).then((answer) => setRows(answer.rows));
  useEffect(() => void load(), [record_id]);
  return (
    <Stack gap="sm">
      <Text tone="muted">Stock by warehouse</Text>
      <Table
        columns={[
          { key: 'warehouse', label: 'Warehouse', sortable: true },
          { key: 'quantity', label: 'Quantity', align: 'end', sortable: true },
        ]}
        label="Stock"
        rows={rows}
      />
      <Button icon="refresh" onPress={load}>Refresh</Button>
    </Stack>
  );
};

register('stock', Stock);
```

The hooks give each mounted contribution what it needs:

| Hook | Returns |
| --- | --- |
| `useOutletContext<T>()` | The outlet's context, for example the record or blueprint revision. It updates in place when the context changes. |
| `useLocale()` | The viewer's language, such as `en` or `pl`. Localize your own text with it. |
| `useTheme()` | `{ color_mode: 'light' \| 'dark' }`. You rarely need it: Attricat already draws your components in the current mode. |
| `useConfiguration()` | The installation's configuration, when the extension has `configuration.read`. |
| `useCapabilities()` | The capabilities granted to the extension. |
| `useAttricat()` | The same calls as the [Frame UI `attricat` API](/extensions/client/#the-attricat-api): `request`, `command`, `storage`, `operations`, `navigate`, `notify`, `refresh` and `dialog.close`, each checked against your grants. |
| `useEditor()` | In an attribute editor: the field's value, attribute, props, required state, errors, and `propose(value)`. |

When the context, language or color mode changes, Attricat sends the new values to your mounted component instead of starting it again, so its state survives.

### Build, test and develop

The SDK's Vite preset builds one self-contained Worker bundle, enforces the 1 MiB size budget and records the UI kit version it was built against beside the bundle:

```ts
import { defineConfig } from 'vite';
import { attricatExtension } from '@attricat/extension/vite';

export default defineConfig(attricatExtension({ entry: 'src/main.tsx' }));
```

`attricat-pack` packs `manifest.json`, the bundle and its build record into the archive Attricat installs.

Test a contribution with Vitest and the SDK's harness, which runs your component the way Attricat would and lets you check what it draws and which calls it makes:

```ts
import { renderContribution } from '@attricat/extension/testing';
import { Stock } from './Stock';

const view = await renderContribution(Stock, {
  context: { record_id: '…' },
  broker: { command: async () => ({ rows: [] }) },
});
```

While you develop against a local Attricat started with developer tools, `acli extension dev acme.inventory --build "pnpm build" --watch src` rebuilds on every change and swaps your Worker in the running server, without uploading a new release or granting permissions again. Open pages pick up the new build within 15 seconds.

## The UI kit

The [UI kit reference](/extensions/ui-kit/) lists every component with its props, events and limits. In short:

- **Layout:** `Stack`, `Grid`, `Divider`.
- **Text:** `Text`, `Heading`, `Badge`, `Link`.
- **Actions:** `Button`, `IconButton`.
- **Inputs:** `TextField`, `NumberField`, `Select`, `Checkbox`, `Switch`.
- **Data:** `Table`, `EmptyState`.

Styling is not free-form. Spacing, size, tone and emphasis are fixed choices that Attricat maps to its design, status tones are `info`, `success`, `warning` and `error`, and icons are Attricat's [host icons](/extensions/ui-kit/#host-icons). Inputs need a `label`, and icon buttons need one too: Attricat uses it as the accessible name. Large content is data, not children: pass a table's `rows`, a select's `options` and a text field's `suggestions`, and Attricat sorts, pages and lists them.

## Attribute editors

An attribute editor replaces the default input of a blueprint field. Declare it in the manifest (see [Attribute editors](/extensions/manifest/#attribute-editors)), and a blueprint names it on a field of its `detail` view.

Attricat keeps the field's label, help text, errors and saving. Your editor only proposes values:

```tsx
import { register, useEditor } from '@attricat/extension';
import { TextField } from '@attricat/ui-kit';

const FormulaEditor = () => {
  const editor = useEditor();
  return (
    <TextField
      hideLabel
      label={editor.attribute.name}
      monospace
      onChange={({ value }) => editor.propose(value)}
      value={typeof editor.value === 'string' ? editor.value : ''}
    />
  );
};

register('formula', FormulaEditor);
```

Attricat checks each proposal against the attribute's type and constraints and saves it like any other field, with the person's own permissions: when they leave the field or press Enter. Escape restores the saved value. While your extension is disabled, lacks the grant, fails or is still starting, the field shows its default control, so a blueprint never depends on your extension being available. Editors are used only on the record page; Explorer tables keep using [table cells](/extensions/manifest/#table-cells).

## Limits

- **No DOM, no CSS.** Your components render UI kit elements only. There is no `document`, no HTML element, no stylesheet, SVG or canvas; anything else you render is dropped.
- **Events are asynchronous.** An event reaches your Worker after the person acted, and what you render in response is drawn a moment later. Inputs are owned by Attricat and send their changes once typing pauses, and again when the input loses focus.
- **Draw only in your box.** Native UI cannot open overlays, popovers or windows. Only Attricat opens dialogs, through an action that opens your `dialog` contribution.
- **No network.** The Worker cannot reach the network or load code. Reach your server through `command` and Attricat through `request`.
- **Budgets.** The bundle may be at most 1 MiB. Attricat limits how many elements you draw, how often you change them and how large each message is, and stops a Worker that stops responding. A stopped Worker shows Attricat's error message in each of its contributions; other extensions keep running.

## Security model

Each extension's native code runs in its own dedicated Worker, inside a hidden frame that Attricat creates with an opaque origin and a Content Security Policy that denies network access. The Worker has no access to Attricat's page, cookies, storage or credentials, and never shares a realm with another extension. It talks to Attricat only through its own message channel, and that channel identifies the extension: every call is checked against that extension's grants and the contribution's context, never against identifiers in the message. Attricat validates every element, property and event against the UI kit before drawing anything.
