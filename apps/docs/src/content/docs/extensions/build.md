---
title: Build an extension
description: The parts of an Attricat extension, how they are packaged, and how to test one against a local workspace.
---

An extension is a `.tar.zst` archive with a `manifest.json` at its root and the files the manifest declares. It can contain:

- **Server components**: WebAssembly components that handle catalog events, answer commands from the extension's UI, and run long operations such as imports and exports. See [Server runtime](/extensions/server/).
- **Client UI** at fixed places in the web app, or as full pages, in one of three tiers described below.
- **Declarations** the host acts on without running your code: attribute types, table cells, configuration schemas, and event contracts.

## Three ways to draw UI

| | Declarative | Native UI | Frame UI |
| --- | --- | --- | --- |
| Who draws | Attricat | Attricat, from UI kit components your code composes | Your code, with its own HTML |
| Where your code runs | Nowhere in the browser | An isolated Worker | A sandboxed frame |
| Manifest | `cell`, `action`, field notes | A `client_native` artifact | A `client_frame` artifact |
| Typical uses | Table cells, menu actions, badges beside fields | Record tabs, panels, dialogs, settings, attribute editors | Full pages and tools the UI kit cannot build |

Prefer them in that order. **Declarative** contributions only describe what to show, so they cost nothing per record and always look like Attricat. **Native UI** lets your logic drive the screen while Attricat draws it with its own components: it follows the theme, the language and keyboard behavior of the rest of the app, and keeps its state when the context or theme changes. See [Native UI](/extensions/native/). **Frame UI** gives you a whole page to draw as you like, for workbenches and editors the UI kit cannot express. See [Client contributions](/extensions/client/).

A reference extension, `attricat-extension-example`, shows all of these working together: it computes numeric attributes from formulas, with a workbench page, a record action, a table cell, and a server event handler.

## Where to start

| You want to | Read |
| --- | --- |
| Show something on a record or in the Explorer | [A minimal manifest](#a-minimal-manifest), then [Native UI](/extensions/native/) or [Client contributions](/extensions/client/) |
| Give blueprint fields a custom input | [Attribute editors](/extensions/native/#attribute-editors) |
| React to catalog changes or back your UI with server logic | [Build a server component](#build-a-server-component), then [Server runtime](/extensions/server/) |
| Import or export files, or process a selection of records | [Operations](/extensions/operations/) |
| Keep a marketplace or storefront in sync | [Export connectors](/extensions/exports/) |

## The security model

Extension code never runs in Attricat's own process or page.

- Server components run in a WebAssembly sandbox with no file system, environment, sockets, or system clock. Everything they do goes through host calls, and each call is checked against the permissions the administrator granted.
- Native UI runs in a dedicated Worker per extension, inside a hidden sandboxed frame with an opaque origin and no network access. Attricat checks every element it is asked to draw against the UI kit and draws it with its own components; no extension markup reaches Attricat's page.
- Frame UI runs in `<iframe sandbox="allow-scripts">` frames with an opaque origin and a Content Security Policy that blocks network access.
- Both talk to Attricat through a message channel that exposes only the operations they were granted, and each channel identifies its extension.
- Attricat changes made by an extension go through the same validation, audit log, and event stream as a person's edits.

Design with that in mind: your extension asks for capabilities; an administrator decides which to grant.

## A minimal manifest

```json
{
  "manifest_version": 1,
  "name": "Inventory panel",
  "version": "1.0.0",
  "description": "Shows warehouse stock on product pages.",
  "icons": { "48": "assets/icon-48.svg" },
  "attricat": {
    "id": "acme.inventory",
    "host_api": ">=1.0.0, <2.0.0"
  },
  "permissions": ["attricat.read", "client.record_tab"],
  "artifacts": [
    { "id": "panel", "kind": "client_frame", "path": "dist/panel.js" }
  ],
  "ui": [
    {
      "id": "summary",
      "version": 1,
      "kind": "tab",
      "artifact": "panel",
      "outlet": "record_tab",
      "title": "Stock"
    }
  ]
}
```

```js
// dist/panel.js
export const mount = async (root, attricat) => {
  const form = await attricat.request(`/api/v1/records/${attricat.context.record_id}`);
  root.textContent = `${form.blueprint.blueprint.name} v${form.record.blueprint_version}`;
  return () => root.replaceChildren();
};
```

The full manifest format is in the [manifest reference](/extensions/manifest/).

## Versions

Three version numbers are independent:

| Field | Meaning |
| --- | --- |
| `manifest_version` | The manifest format. Currently `1`. |
| `version` | Your release, as SemVer. |
| `attricat.host_api` | The SemVer range of Attricat host APIs your code works with, such as `>=1.0.0, <2.0.0`. |

Attricat never installs a release whose `host_api` range does not include the running host API.

## Build a server component

A server artifact is a WebAssembly component that imports only the `attricat:host` interfaces. Any language that produces components works; the examples here use Rust and [`wit-bindgen`](https://github.com/bytecodealliance/wit-bindgen).

1. Copy `crates/extension-runtime/wit-host/attricat-extension.wit` from the Attricat repository into your crate's `wit/` directory.
2. Generate bindings for a [world](/extensions/server/#host-api-version) and implement its exports:

   ```rust
   // Cargo.toml: crate-type = ["cdylib"]; dependencies wit-bindgen, serde_json
   wit_bindgen::generate!({ path: "wit", world: "handler-extension" });

   use attricat::host::api::{self, Event};
   use exports::attricat::host::handler::{CommandRequest, CommandResponse, Guest};

   struct Extension;

   impl Guest for Extension {
       fn handle_event(event: Event) -> Result<(), String> {
           if event.event_type != "record.updated.v1" {
               return Ok(());
           }
           // Needs logging.write. Ignore a failed log: an error here would quarantine the extension.
           let _ = api::log("info", &format!("record changed: {}", event.aggregate_id));
           Ok(())
       }

       fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
           match request.handler.as_str() {
               "ping" => Ok(CommandResponse { payload: r#"{"ok":true}"#.into() }),
               other => Err(format!("unknown command {other}")),
           }
       }
   }

   export!(Extension);
   ```

3. Build for `wasm32-unknown-unknown` and wrap the module as a component:

   ```sh
   cargo build --release --target wasm32-unknown-unknown
   wasm-tools component new target/wasm32-unknown-unknown/release/acme_inventory.wasm \
     -o dist/server.wasm
   ```

Don't build for `wasm32-wasip1` or `wasm32-wasip2`: those targets import WASI interfaces, which Attricat never provides, so the component fails to instantiate. To check, run `wasm-tools component wit dist/server.wasm`. Every `import` it lists should be an `attricat:host/…` interface.

## Package

Build your artifacts, then create a zstd-compressed tar archive whose root contains `manifest.json`, every declared artifact path, and your icons:

```text
manifest.json
assets/icon-48.svg
dist/server.wasm
dist/panel.js
```

```sh
tar -cf - manifest.json assets dist | zstd -19 -o dist/acme.inventory-1.0.0.tar.zst
```

Packages are limited to 32 MiB compressed. Paths must be relative, without `..`, links, or devices.

## Test locally

1. Run a local Attricat workspace.
2. Upload the archive under **Manage → Extensions → Upload archive**, or with `acli extension sideload --file dist/acme.inventory-1.0.0.tar.zst`.
3. Grant every permission you need and enable the extension.
4. Exercise each contribution: open the pages where your UI appears, trigger the events your handlers listen to, run your commands and operations.

Every upload is a new release, so grants and enablement must be repeated after each one.

Check that your code handles:

- **Duplicate and out-of-order events.** Delivery is at least once. Use the event ID as an idempotency key.
- **Its own events.** A write from your event handler produces a new event, which your handler receives too. With a 1.1 `event-handler`, skip deliveries whose `source-name` is `extension:<your-extension-id>`. A 1.0 `handler` cannot see the source, so make its writes converge: read the current value and write only when it differs. Otherwise the handler triggers itself forever.
- **Errors that quarantine.** With the 1.0 `handler` export, any error returned from `handle-event` quarantines the installation. Return `Ok` for bad data that a retry cannot fix, and record the problem in storage or a log. With host API 1.1, report it as a [typed outcome](/extensions/server/#typed-outcomes) instead.
- **Missing permissions.** Optional permissions may not be granted; degrade gracefully.
- **Light and dark mode.** Read `attricat.theme` in client components.

### Debug

- `api.log(level, message)` with `logging.write` writes to the Attricat server log, tagged with your extension ID. Levels are `trace`, `debug`, `info`, `warn`, and `error`. Messages are limited to 16 KiB.
- Every host call returns `Err(message)` when it is refused, such as `missing capability 'storage.extension'`. Pass these messages through to your logs; they name the missing grant.
- A quarantined installation shows its state on its page under **Manage → Extensions**. Fix the cause, uploading a corrected release if your code was at fault, and enable it again.
- For operations, `acli extension operation get <run-id>` shows a run's status, progress, and error.
- In a client frame, the browser developer tools show your console output and errors. Select the extension's frame in the console's context menu.

## Publish

To distribute through a registry, publish the archive as an asset of a GitHub Release (not a draft or prerelease) in your extension's repository, and list the repository in a registry's `registry.json`:

```json
{
  "registry_version": 1,
  "extensions": [{
    "id": "acme.inventory",
    "name": "Inventory panel",
    "description": "Shows warehouse stock on product pages.",
    "icon": "icon.svg",
    "repository": "acme/attricat-inventory"
  }]
}
```

Workspace administrators add your registry with `acli extension registry add --source acme/attricat-extensions`. The official registry is `attricat/attricat-extensions`.
